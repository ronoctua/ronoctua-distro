//! The camera app (`LEADER m c`): the program, live, in a floating window.
//!
//! What the keys and the buttons DO is `camera.rs`; this is the window — a
//! [`crate::door`] content, so its plumbing (hidpi, one state per cursor, a
//! keyboard that repeats, the three watches) is every door's — plus the two
//! things only this door has: a picture that changes thirty times a second,
//! and controls drawn on it.
//!
//! # The frames are the clock
//!
//! The picture comes from the studio's preview ring (`ronoctua_studio::preview`)
//! on a thread of its own, and every frame that arrives is what asks for the
//! next paint — no timer. The newest frame wins: the thread keeps ONE slot, so
//! a window that was behind never plays a backlog. That is also what hides the
//! controls on time: the paint that follows a frame asks `View::showing`, and
//! three seconds after the last sign of a person the answer is no.
//!
//! # Nothing here opens a camera
//!
//! The studio does (`ronoctua-studio`, the one process that owns the video).
//! Closing this window stops the preview, and with nothing else consuming the
//! program the studio closes the camera and its light goes out.

use std::sync::{Arc, Mutex};

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, RIGHT_BUTTON, Ui};
use anyhow::{Context as _, Result};
use ronoctua_studio::{
    scene::Rect,
    wire::{Ask, State},
};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

use crate::camera::{self, Errand, MenuKey, Press};

/// Where a pointer that has left the window is kept: nowhere a cell is.
const OUTSIDE: (f64, f64) = (-1.0, -1.0);

/// The key the program's texture is uploaded under — ONE key for every frame,
/// so a film fills one slot of the renderer's ceiling and never evicts
/// anything (the miniplayer's rule).
const PROGRAM: &str = "camera:program";

/// What the preview thread hands over: the newest frame, and nothing queued.
type Slot = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

/// Opens the camera app.
///
/// # Errors
///
/// No Wayland compositor, or no `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // THE STUDIO'S STATE, on its own thread (`client::watch` starts the
        // daemon when nobody is home — the nested loop has no unit).
        let (state_sender, states) = channel::channel::<Option<State>>();
        let watch = ronoctua_studio::client::watch(move |s| {
            let _ = state_sender.send(s);
        });
        w.handle
            .insert_source(states, |event, (), door: &mut Door<CameraWindow>| {
                if let channel::Event::Msg(s) = event {
                    door.content.state_arrived(&mut door.ui, s);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the studio channel: {e}"))?;

        // THE PICTURE: one slot, and a ping per frame.
        let slot: Slot = Arc::default();
        let (frame_sender, frames) = channel::channel::<()>();
        let feeding = slot.clone();
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let still = alive.clone();
        std::thread::Builder::new()
            .name("camera-preview".into())
            .spawn(move || {
                let mut buf = Vec::new();
                while still.load(std::sync::atomic::Ordering::SeqCst) {
                    let Ok(mut feed) = ronoctua_studio::client::preview() else {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        continue;
                    };
                    loop {
                        match feed.next(&mut buf) {
                            Ok(true) => {
                                if let Ok(mut s) = feeding.lock() {
                                    *s = Some((feed.width, feed.height, buf.clone()));
                                }
                                if frame_sender.send(()).is_err() {
                                    return;
                                }
                            }
                            Ok(false) => {}
                            Err(_) => break,
                        }
                        if !still.load(std::sync::atomic::Ordering::SeqCst) {
                            return;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            })
            .context("the preview thread")?;
        w.handle
            .insert_source(frames, |event, (), door: &mut Door<CameraWindow>| {
                if let channel::Event::Msg(()) = event {
                    door.content.frame_arrived(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the preview channel: {e}"))?;

        Ok(CameraWindow {
            watch,
            alive,
            view: camera::View::default(),
            studio: None,
            slot,
            frame: None,
            uploaded: false,
            buttons: Vec::new(),
            menu_box: None,
            menu_under: None,
            button_under: None,
            pointer_at: OUTSIDE,
            mixer_rows: Vec::new(),
            last_place: std::time::Instant::now(),
            told_photo: None,
        })
    })
}

/// What this window keeps for ONE pointer, past what every door keeps.
#[derive(Debug, Default)]
struct PerSeat {
    /// A layer being dragged: which, its rect when the drag began, and where
    /// the drag began in program fractions.
    layer: Option<(usize, Rect, (f32, f32))>,
}

struct CameraWindow {
    /// The studio's watch: the flag that stops its thread.
    watch: Arc<std::sync::atomic::AtomicBool>,
    /// Whether the preview thread is to go on reading — let go of when the
    /// window is, so the studio stops feeding a preview nobody sees.
    alive: Arc<std::sync::atomic::AtomicBool>,
    view: camera::View,
    /// The studio's last word; `None` while it is not answering.
    studio: Option<State>,
    slot: Slot,
    /// The newest frame, as the renderer takes it.
    frame: Option<ronoctua_render::image::Rgba>,
    /// Whether `frame` is already on the GPU.
    uploaded: bool,
    /// The bottom row as last DRAWN: `(first column, last column, id)` — what
    /// a click is compared against, so the control is picked where it was
    /// painted.
    buttons: Vec<(u16, u16, &'static str)>,
    /// Where the open menu was DRAWN — what a click and the pointer ask.
    menu_box: Option<camera::MenuBox>,
    /// The menu row under the pointer.
    menu_under: Option<usize>,
    /// The button under the pointer: the header says what it does.
    button_under: Option<&'static str>,
    /// Where the pointer that spoke last is — `OUTSIDE` once it has left.
    ///
    /// ASKED AGAIN INSIDE THE PAINT, once the buttons and the menu of THIS
    /// frame exist: the pointer that brought the controls back arrived while
    /// there were none, and without asking again the header would go on
    /// teaching the keys under a hand already resting on a button.
    pointer_at: (f64, f64),
    /// The sound drawer's rows as last drawn: `(line, which)`.
    mixer_rows: Vec<(u16, ronoctua_studio::wire::Sound)>,
    last_place: std::time::Instant,
    /// The photo last said on screen, so a new one is said once.
    told_photo: Option<String>,
}

impl Drop for CameraWindow {
    fn drop(&mut self) {
        self.alive.store(false, std::sync::atomic::Ordering::SeqCst);
        self.watch.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl CameraWindow {
    fn state_arrived(&mut self, ui: &mut Ui, s: Option<State>) {
        if let Some(s) = &s
            && s.last_photo.is_some()
            && s.last_photo != self.told_photo
        {
            if self.told_photo.is_some()
                || self
                    .studio
                    .as_ref()
                    .is_some_and(|old| old.last_photo != s.last_photo)
            {
                let name = s
                    .last_photo
                    .as_deref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ui.say(
                    ui.texts
                        .t_with(crate::key::STUDIO_PHOTO_SAVED, &[("file", &name)]),
                );
                self.view.sign();
            }
            self.told_photo.clone_from(&s.last_photo);
        }
        self.studio = s;
        ui.ask_paint();
    }

    fn frame_arrived(&mut self, ui: &mut Ui) {
        if let Some((w, h, px)) = self.slot.lock().ok().and_then(|mut s| s.take()) {
            self.frame = ronoctua_render::image::Rgba::new(w, h, px);
            self.uploaded = false;
            ui.ask_paint();
        }
    }

    fn program(&self) -> (u32, u32) {
        self.frame
            .as_ref()
            .map(|f| (f.width, f.height))
            .or_else(|| self.studio.as_ref().map(|s| (s.width, s.height)))
            .unwrap_or((1280, 720))
    }

    fn picture(&self, ui: &Ui) -> ronoctua_render::image::Area {
        #[allow(clippy::cast_precision_loss)]
        crate::chrome::picture_in(
            ui.width as f32,
            ui.height as f32,
            self.program(),
            self.view.fill,
        )
    }

    fn line_height() -> f32 {
        tokens::metrics::line_height(tokens::text::size())
    }

    fn cell_width() -> f32 {
        tokens::metrics::cell_width(tokens::text::size())
    }

    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let errand = self.view.key(press, self.studio.as_ref());
        self.apply(ui, errand);
    }

    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Run(id) => {
                if let Err(e) = crate::ipc::send_it(&format!("command {id}")) {
                    tracing::warn!(id, error = %e, "the footer did not take the command");
                }
            }
            Errand::Tell(ask) => Self::tell(ui, &ask),
        }
        ui.ask_paint();
    }

    fn tell(ui: &mut Ui, ask: &Ask) {
        match ronoctua_studio::client::ask(ask) {
            Ok(ronoctua_studio::wire::Say::Refused(t)) => {
                ui.say(camera::trouble_sentence(&t, &ui.texts));
            }
            Ok(_) => {}
            Err(_) => ui.say(ui.texts.t(crate::key::STUDIO_NOT_RUNNING)),
        }
    }

    /// The line the bottom row of buttons is on.
    const fn buttons_line(ui: &Ui) -> u16 {
        ui.lines.saturating_sub(1)
    }

    /// Which button a click at `x` on the bottom row is.
    fn button_at(&self, x: f64) -> Option<&'static str> {
        let cell = f64::from(Self::cell_width());
        if cell <= 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / cell) as u16;
        // THE SAME COLUMNS THE ROW WAS DRAWN IN. This took `FIELDS_LEAD` off
        // a second time — the margin is already inside `start` — so every
        // button answered that many cells to its right, and a glyph with no
        // words beside it answered only in the empty cells after it.
        self.buttons
            .iter()
            .find(|(a, b, _)| column >= *a && column < *b)
            .map(|(_, _, id)| *id)
    }

    fn column_at(x: f64) -> u16 {
        let cell = f64::from(Self::cell_width()).max(1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let c = (x / cell) as u16;
        c
    }

    fn line_at(y: f64) -> u16 {
        let line = f64::from(Self::line_height());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let l = (y / line.max(1.0)) as u16;
        l
    }

    /// The button of the bottom row under `at`, if the controls are showing.
    fn button_under_at(&self, ui: &Ui, at: (f64, f64)) -> Option<&'static str> {
        if at == OUTSIDE {
            return None;
        }
        (self.view.showing() && Self::line_at(at.1) == Self::buttons_line(ui))
            .then(|| self.button_at(at.0))
            .flatten()
    }

    /// The row of the open menu under `at`.
    fn menu_under_at(&self, at: (f64, f64)) -> Option<usize> {
        if at == OUTSIDE {
            return None;
        }
        self.menu_box
            .and_then(|b| b.at(Self::column_at(at.0), Self::line_at(at.1)))
    }

    /// What is under the pointer — a button of the bottom row, a row of the
    /// open menu — kept; `true` when that changed and the window is owed a
    /// frame.
    fn lit_under(&mut self, ui: &Ui, at: (f64, f64)) -> bool {
        self.pointer_at = at;
        let button = self.button_under_at(ui, at);
        let row = self.menu_under_at(at);
        let changed = button != self.button_under || row != self.menu_under;
        self.button_under = button;
        self.menu_under = row;
        changed
    }

    /// The bottom row: every button of the table, glyph first — laid out
    /// into `self.buttons` and returned as text.
    ///
    /// LAID OUT BEFORE THE HEADER IS DRAWN, because the header says what the
    /// button under the pointer does, and which button that is depends on
    /// where the buttons of THIS frame are.
    fn bottom_row(&mut self, ui: &Ui, line: u16) -> Vec<ronoctua_render::Span> {
        let size = tokens::text::size();
        let margin = ui.margin;
        let mut spans = Vec::new();
        self.buttons.clear();
        let mut at = margin;
        for b in camera::buttons(&self.view, self.studio.as_ref(), &ui.texts) {
            let colour = if b.on {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            let start = at;
            spans.push(span(
                size,
                b.glyph.to_string(),
                at,
                line,
                if b.id == "camera-record" && b.on {
                    tokens::color::STATE_ERROR
                } else {
                    colour
                },
            ));
            at += 2;
            if let Some(words) = b.words {
                let words = ronoctua_render::layout::elide_end(&words, 18);
                let cells = ronoctua_protocol::cells(&words);
                spans.push(span(size, words, at, line, tokens::color::TEXT_SECONDARY));
                at += cells + 1;
            }
            if at + 2 > ui.columns + margin {
                break;
            }
            self.buttons.push((start, at, b.id));
            at += 2;
        }
        spans
    }

    /// What the studio is doing, for the status line: what is rolling, and
    /// what is wrong.
    fn status(&self, texts: &ronoctua_i18n::Texts) -> Vec<(String, tokens::Color)> {
        let mut status: Vec<(String, tokens::Color)> = Vec::new();
        let Some(s) = &self.studio else {
            return status;
        };
        if let Some(r) = &s.recording {
            status.push((
                format!(
                    "{} {}",
                    tokens::icon::STUDIO_RECORD,
                    texts.t_with(
                        crate::key::STUDIO_RECORDING_FOR,
                        &[("time", &camera::elapsed(r.since))]
                    )
                ),
                tokens::color::STATE_ERROR,
            ));
        }
        if let Some(r) = &s.streaming {
            status.push((
                format!(
                    "{} {}",
                    tokens::icon::STUDIO_STREAM,
                    texts.t_with(
                        crate::key::STUDIO_STREAMING_FOR,
                        &[("time", &camera::elapsed(r.since))]
                    )
                ),
                tokens::color::STATE_ERROR,
            ));
        }
        if let Some(n) = s.countdown {
            status.push((
                texts.t_with(crate::key::STUDIO_COUNTDOWN, &[("seconds", &n.to_string())]),
                tokens::color::STATE_ACCENT,
            ));
        }
        if self.frame.is_some()
            && let Some(t) = s.troubles.first()
        {
            status.push((
                camera::trouble_sentence(t, texts),
                tokens::color::STATE_WARNING,
            ));
        }
        status
    }

    /// The open menu, over the picture and above the status line, under the
    /// button that opened it. `menu_box` is where it is drawn AND where it is
    /// hit.
    fn draw_menu(&mut self, ui: &Ui, canvas: &mut Canvas<'_>, status_line: u16) {
        let size = tokens::text::size();
        let line = Self::line_height();
        let margin = ui.margin;
        self.menu_box = None;
        let Some(open) = self.view.menu else {
            return;
        };
        let rows = camera::choices(open.menu, self.studio.as_ref(), &ui.texts);
        let labels: Vec<String> = rows.iter().map(|c| c.label.clone()).collect();
        let anchor = self
            .buttons
            .iter()
            .find(|(_, _, id)| *id == open.menu.button())
            .map_or(margin, |(a, _, _)| *a);
        let first = 1 + ui.hinted.rows();
        self.menu_box = camera::menu_box(
            &labels,
            anchor,
            first,
            status_line,
            margin,
            margin + ui.columns,
        );
        // AND THE ROW UNDER THE POINTER, against the box of THIS frame.
        self.menu_under = self.menu_under_at(self.pointer_at);
        let Some(b) = self.menu_box else {
            return;
        };
        let cell = Self::cell_width();
        canvas.over.push(ronoctua_render::quad::Quad {
            x: cell * f32::from(b.left),
            y: line * f32::from(b.top),
            width: cell * f32::from(b.columns * b.width),
            height: line * f32::from(b.lines),
            color: tokens::color::SURFACE_TRANSPORT,
        });
        for (i, c) in rows.iter().enumerate() {
            let Some((col, l)) = b.place(i) else { break };
            let here = i == open.cursor;
            let band = if here {
                Some(tokens::color::SURFACE_SELECTION)
            } else if self.menu_under == Some(i) {
                Some(tokens::color::SURFACE_HOVER)
            } else {
                None
            };
            if let Some(color) = band {
                canvas.over.push(ronoctua_render::quad::Quad {
                    x: cell * f32::from(col),
                    y: line * f32::from(l),
                    width: cell * f32::from(b.width),
                    height: line,
                    color,
                });
            }
            // AIR, the mark, a space, the label, air (`camera::MENU_FRAME`).
            if c.current {
                canvas.spans.push(span(
                    size,
                    tokens::icon::SUCCESS.to_string(),
                    col + 1,
                    l,
                    tokens::color::STATE_ACCENT,
                ));
            }
            canvas.spans.push(span(
                size,
                ronoctua_render::layout::elide_end(&c.label, b.label_cells()),
                col + 3,
                l,
                if here {
                    tokens::color::TEXT_HERE_ACTS
                } else if c.current {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_PRIMARY
                },
            ));
        }
    }

    /// The sound drawer: the two sounds, their level and their volume.
    fn draw_mixer(&mut self, ui: &Ui, canvas: &mut Canvas<'_>, status_line: u16) {
        let size = tokens::text::size();
        let line = Self::line_height();
        let margin = ui.margin;
        #[allow(clippy::cast_precision_loss)]
        let w = ui.width as f32;
        if !self.view.mixer {
            return;
        }
        let Some(s) = &self.studio else {
            return;
        };
        let rows = camera::sounds(s);
        let top = status_line.saturating_sub(u16::try_from(rows.len()).unwrap_or(2) + 1);
        canvas.over.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: line * f32::from(top),
            width: w,
            height: line * f32::from(u16::try_from(rows.len()).unwrap_or(2)),
            color: tokens::color::SURFACE_TRANSPORT,
        });
        for (i, (which, level, gain, muted)) in rows.into_iter().enumerate() {
            let l = top + u16::try_from(i).unwrap_or(0);
            let name = ui.texts.t(match which {
                ronoctua_studio::wire::Sound::Microphone => crate::key::STUDIO_SOUND_MICROPHONE,
                ronoctua_studio::wire::Sound::Desktop => crate::key::STUDIO_SOUND_DESKTOP,
            });
            canvas.spans.push(span(
                size,
                name,
                margin,
                l,
                if muted {
                    tokens::color::TEXT_DIM
                } else {
                    tokens::color::TEXT_PRIMARY
                },
            ));
            // THE LEVEL IS `chrome::meter`, the sound view's own bar, at the
            // column every value in this system starts at.
            let bar_at = crate::chrome::value_column(margin, ui.columns);
            let bar = crate::chrome::meter(level, crate::sizes::BAR_CELLS);
            let bar_cells = crate::sizes::BAR_CELLS;
            canvas.spans.push(span(
                size,
                bar,
                bar_at,
                l,
                if muted {
                    tokens::color::TEXT_DIM
                } else {
                    tokens::color::STATE_ACCENT
                },
            ));
            let value = if muted {
                ui.texts.t(crate::key::STUDIO_MUTED)
            } else {
                ui.texts
                    .t_with(crate::key::STUDIO_LEVEL, &[("percent", &gain.to_string())])
            };
            canvas.spans.push(span(
                size,
                value,
                bar_at + bar_cells + 1,
                l,
                tokens::color::TEXT_SECONDARY,
            ));
            self.mixer_rows.push((l, which));
        }
    }

    /// The layers a hand can move, outlined, so the drag is findable.
    fn draw_outlines(&self, canvas: &mut Canvas<'_>, area: ronoctua_render::image::Area) {
        let Some(s) = &self.studio else {
            return;
        };
        for l in s
            .layers
            .iter()
            .filter(|l| l.visible && (l.rect.w < 0.999 || l.rect.h < 0.999))
        {
            let x = area.x + l.rect.x * area.width;
            let y = area.y + l.rect.y * area.height;
            let lw = l.rect.w * area.width;
            let lh = l.rect.h * area.height;
            let t = f32::from(tokens::space::RULE_THICKNESS);
            // THE ACCENT, and not a rule's tone: the outline says "this can be
            // taken", which is what the accent says everywhere else — and
            // `RULE_STRONG` is a scrollbar's.
            let c = tokens::color::STATE_ACCENT;
            for (qx, qy, qw, qh) in [
                (x, y, lw, t),
                (x, y + lh - t, lw, t),
                (x, y, t, lh),
                (x + lw - t, y, t, lh),
            ] {
                canvas.over.push(ronoctua_render::quad::Quad {
                    x: qx,
                    y: qy,
                    width: qw,
                    height: qh,
                    color: c,
                });
            }
        }
    }

    /// One pointer's frame, with the header's drawer hand already swapped in.
    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<PerSeat>, events: &[PointerEvent]) {
        let head = ui.frame(0);
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    // A SIGN OF A PERSON: the controls come back.
                    let was = self.view.showing();
                    self.view.sign();
                    if ui.hinted.dragged(head, e.position.1) {
                        ui.ask_paint();
                        continue;
                    }
                    let texts = ui.texts.clone();
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                    if ui.hinted.say(now) || !was {
                        ui.ask_paint();
                    }
                    if self.lit_under(ui, e.position) {
                        ui.ask_paint();
                    }
                    // A LAYER FOLLOWS THE HAND, sent at most every 30 ms: a drag
                    // is dozens of motions a second and the studio rewrites a
                    // scene for each one it takes.
                    #[allow(clippy::cast_possible_truncation)]
                    if let Some((layer, start, from)) = hand.extra.layer
                        && let Some((u, v)) = camera::on_program(
                            self.picture(ui),
                            e.position.0 as f32,
                            e.position.1 as f32,
                        )
                        && self.last_place.elapsed() > std::time::Duration::from_millis(30)
                    {
                        self.last_place = std::time::Instant::now();
                        let rect = start.moved(u - from.0, v - from.1);
                        Self::tell(ui, &Ask::Place { layer, rect });
                    }
                }
                PointerEventKind::Leave { .. } => {
                    if self.lit_under(ui, OUTSIDE) {
                        ui.ask_paint();
                    }
                    ui.hinted.let_go();
                    if ui.hinted.say(None) {
                        ui.ask_paint();
                    }
                    hand.extra.layer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.extra.layer = None;
                }
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    self.view.sign();
                    let line = Self::line_at(e.position.1);
                    if self.view.showing()
                        && line == Self::buttons_line(ui)
                        && let Some(id) = self.button_at(e.position.0)
                        && let Some(press) = camera::press_of(id)
                    {
                        self.pressed(ui, press);
                    } else if self.view.menu.take().is_some() {
                        ui.ask_paint();
                    }
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.view.sign();
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        ui.ask_paint();
                        continue;
                    }
                    let line = Self::line_at(e.position.1);
                    // A ROW OF THE OPEN MENU: the first click selects, the
                    // second takes it.
                    if let Some(i) = self
                        .menu_box
                        .and_then(|b| b.at(Self::column_at(e.position.0), line))
                    {
                        let errand = self.view.menu_clicked(i, self.studio.as_ref());
                        self.apply(ui, errand);
                        continue;
                    }
                    if self.view.showing() && line == Self::buttons_line(ui) {
                        // THE LEFT BUTTON OPENS the menu of a control that has
                        // several values; the right one turns it a step.
                        if let Some(id) = self.button_at(e.position.0) {
                            if let Some(menu) = camera::Menu::of_button(id) {
                                self.view.toggle_menu(menu, self.studio.as_ref());
                                ui.ask_paint();
                            } else if let Some(press) = camera::press_of(id) {
                                self.pressed(ui, press);
                            }
                        }
                        continue;
                    }
                    // A CLICK ANYWHERE ELSE SHUTS AN OPEN MENU, and does only
                    // that: the click that closes it must not also move a
                    // layer or mute a sound the person was not aiming at.
                    if self.view.menu.take().is_some() {
                        ui.ask_paint();
                        continue;
                    }
                    if let Some((_, which)) =
                        self.mixer_rows.iter().find(|(l, _)| *l == line).copied()
                    {
                        let muted = self.studio.as_ref().is_some_and(|s| {
                            s.muted[usize::from(which == ronoctua_studio::wire::Sound::Desktop)]
                        });
                        Self::tell(
                            ui,
                            &Ask::Sound {
                                which,
                                gain: None,
                                muted: Some(!muted),
                            },
                        );
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if let Some(s) = &self.studio
                        && let Some((u, v)) = camera::on_program(
                            self.picture(ui),
                            e.position.0 as f32,
                            e.position.1 as f32,
                        )
                        && let Some((layer, rect)) = camera::movable_at(s, u, v)
                    {
                        hand.extra.layer = Some((layer, rect, (u, v)));
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    self.view.sign();
                    // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER, and not
                    // the picture under it: the gesture belongs to what is
                    // under the pointer.
                    if ui.hinted.turned(head, e.position.1, by) {
                        ui.ask_paint();
                        continue;
                    }
                    let line = Self::line_at(e.position.1);
                    if let Some((_, which)) =
                        self.mixer_rows.iter().find(|(l, _)| *l == line).copied()
                    {
                        let gain = self.studio.as_ref().map_or(100, |s| {
                            s.gains[usize::from(which == ronoctua_studio::wire::Sound::Desktop)]
                        });
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let next = (i64::from(gain) - by * 10).clamp(0, 200) as u8;
                        Self::tell(
                            ui,
                            &Ask::Sound {
                                which,
                                gain: Some(next),
                                muted: None,
                            },
                        );
                        continue;
                    }
                    // THE WHEEL OVER A SMALL LAYER RESIZES IT, about its centre.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                    if let Some(s) = &self.studio
                        && let Some((u, v)) = camera::on_program(
                            self.picture(ui),
                            e.position.0 as f32,
                            e.position.1 as f32,
                        )
                        && let Some((layer, rect)) = camera::movable_at(s, u, v)
                    {
                        let rect = rect.scaled(1.1f32.powi(-(by as i32)));
                        Self::tell(ui, &Ask::Place { layer, rect });
                    }
                }
                _ => {}
            }
        }
    }
}

/// One run of text at the system's one size.
const fn span(
    size: f32,
    text: String,
    column: u16,
    line: u16,
    color: tokens::Color,
) -> ronoctua_render::Span {
    ronoctua_render::Span {
        size: ronoctua_render::Size::Px(size),
        text,
        column,
        line,
        color,
        edge: false,
        foot: false,
    }
}

impl Content for CameraWindow {
    type Hand = PerSeat;

    fn app_id(&self) -> &'static str {
        camera::APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::CAMERA_TITLE)
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        // AN OPEN MENU TAKES THE ARROWS, `RET` AND `Esc` — and nothing else,
        // so the ring keys and the other menus' keys still work under it.
        if self.view.menu.is_some() {
            let key = match event.keysym {
                Keysym::Up => Some(MenuKey::Up),
                Keysym::Down => Some(MenuKey::Down),
                Keysym::Left => Some(MenuKey::Left),
                Keysym::Right => Some(MenuKey::Right),
                Keysym::Return | Keysym::KP_Enter => Some(MenuKey::Choose),
                Keysym::Escape => Some(MenuKey::Close),
                _ => None,
            };
            if let Some(key) = key {
                ui.message = None;
                let lines = self.menu_box.map_or(1, |b| b.lines);
                let errand = self.view.menu_key(key, lines, self.studio.as_ref());
                self.apply(ui, errand);
                return;
            }
        }
        // `C-` AND A LETTER is written the way the table writes it; the
        // letter comes from the keysym, because with Ctrl held the text the
        // keyboard gives is a control character. The modifiers are KEPT for
        // this: `C-e` opens a menu, and a window that threw them away read it
        // as `e`.
        let chord = (mods.ctrl && !mods.alt)
            .then(|| event.keysym.key_char())
            .flatten()
            .filter(char::is_ascii_alphabetic)
            .map(|c| format!("C-{}", c.to_ascii_lowercase()));
        let press = match event.keysym {
            Keysym::Escape => Some(Press::Escape),
            Keysym::space => camera::pressed("SPC"),
            _ if chord.is_some() => chord.as_deref().and_then(camera::pressed),
            _ => event.utf8.as_deref().and_then(camera::pressed),
        };
        if let Some(press) = press {
            self.pressed(ui, press);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let size = tokens::text::size();
        let line = Self::line_height();
        let margin = ui.margin;
        let showing = self.view.showing();
        let area = self.picture(ui);
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);

        // UNDER THE PICTURE the page's own ground; OVER it the bands and
        // outlines; the text last — the canvas's order, and there is no depth
        // test.
        //
        // THE PAGE'S GROUND, `surface.chrome`, and not the desk's colour it
        // was: the sentence below ("no picture yet") is drawn on it, in an
        // ink solved for the page this door wears (`Content::page`, the
        // windows' answer) — on `surface.backdrop` that ink stood on a ground
        // no gate measured, and the window ignored the person's answer about
        // what windows stand on. On a desk's page the chrome IS that desk.
        canvas.under.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            color: tokens::color::SURFACE_CHROME,
        });

        // THE PROGRAM, one texture rewritten in place for every frame — see
        // `PROGRAM`. Written here and not through `Canvas::upload`, which
        // only fills a key that is missing: this one is present and stale.
        if let Some(f) = &self.frame {
            if !self.uploaded || canvas.images.missing(PROGRAM) {
                canvas
                    .images
                    .upload_it(canvas.device, canvas.queue, PROGRAM, f);
                self.uploaded = true;
            }
            canvas.pictures.push((PROGRAM.to_owned(), area));
        }

        // NO PICTURE YET: say why, in the middle, rather than a black window.
        if self.frame.is_none() {
            let why = self.studio.as_ref().map_or_else(
                || ui.texts.t(crate::key::STUDIO_NOT_RUNNING),
                |s| {
                    s.troubles
                        .first()
                        .map_or_else(String::new, |t| camera::trouble_sentence(t, &ui.texts))
                },
            );
            for (i, l) in crate::chrome::note_lines(&why, ui.columns)
                .into_iter()
                .enumerate()
            {
                canvas.spans.push(span(
                    size,
                    l,
                    margin,
                    ui.lines / 2 + u16::try_from(i).unwrap_or(0),
                    tokens::color::TEXT_SECONDARY,
                ));
            }
        }

        self.mixer_rows.clear();
        if !showing {
            self.buttons.clear();
            return;
        }
        let bl = Self::buttons_line(ui);
        let bottom = self.bottom_row(ui, bl);
        // THE BUTTON UNDER THE POINTER, against the buttons of THIS frame.
        self.button_under = self.button_under_at(ui, self.pointer_at);

        // THE TOP BAND: the hint line, on the transport's translucent strip.
        canvas.over.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: line * f32::from(1 + ui.hinted.rows()),
            color: tokens::color::SURFACE_TRANSPORT,
        });
        // A BUTTON UNDER THE POINTER TAKES THE LINE: a row of glyphs says
        // nothing on its own, so the line that teaches the keys says what THIS
        // one does, its key, and — on a button that opens a menu — what each
        // mouse button does to it.
        let texts = ui.texts.clone();
        let question = crate::door::list::the_question(ui);
        let (pieces, hinting) = match (question, &ui.message, self.button_under) {
            // A QUESTION BEING TYPED OUTRANKS THE REST: the keyboard is its.
            (Some(q), _, _) => (q, true),
            (None, Some(t), _) => (vec![crate::chrome::Piece::said(t.clone())], false),
            (None, None, Some(id)) => (camera::about_button(id, &texts), false),
            (None, None, None) => (camera::hint(&texts), true),
        };
        let columns = ui.columns;
        canvas
            .spans
            .extend(ui.hinted.head(&pieces, hinting, margin, columns, &texts));

        // THE STATUS LINE, above the buttons: what is rolling, and what is wrong.
        let status_line = bl.saturating_sub(1);
        let status = self.status(&texts);
        if !status.is_empty() {
            canvas.over.push(ronoctua_render::quad::Quad {
                x: 0.0,
                y: line * f32::from(status_line),
                width: w,
                height: line,
                color: tokens::color::SURFACE_TRANSPORT,
            });
            let mut at = margin;
            for (text, colour) in status {
                let cells = ronoctua_protocol::cells(&text);
                canvas.spans.push(span(size, text, at, status_line, colour));
                at += cells + 3;
            }
        }

        // THE BOTTOM ROW, on its own strip to the window's edge.
        canvas.over.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: line * f32::from(bl),
            width: w,
            height: h - line * f32::from(bl),
            color: tokens::color::SURFACE_TRANSPORT,
        });
        canvas.spans.extend(bottom);

        self.draw_menu(ui, canvas, status_line);
        self.draw_mixer(ui, canvas, status_line);
        self.draw_outlines(canvas, area);
        canvas.over.extend(ui.hinted.bar(ui.frame(0)));
    }

    /// THE CAMERA'S OWN POINTER: buttons, menus, the sound drawer, layers a
    /// hand drags and a wheel that resizes them — none of it a list.
    /// NO SWEEP OF TEXT HERE: a held button is this window's own gesture
    /// (the picture and its menus answer the pointer), and a sweep would take its motion away.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<PerSeat>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }
}
