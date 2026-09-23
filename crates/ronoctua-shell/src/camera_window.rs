//! The camera app (`LEADER m c`): the program, live, in a floating window.
//!
//! What the keys and the buttons DO is `camera.rs`; this is a window's
//! plumbing — the same shape every door in the shell has (`pointers_window.rs`
//! is the model: hidpi, one state per cursor, a keyboard that repeats) — plus
//! the two things only this door has: a picture that changes thirty times a
//! second, and controls drawn on it.
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

use anyhow::{Context as _, Result};
use ronoctua_studio::{
    scene::Rect,
    wire::{Ask, State},
};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::{self, EventLoop, channel},
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface as _,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

use crate::camera::{self, Errand, MenuKey, Press};

/// `BTN_LEFT`.
const LEFT_BUTTON: u32 = 0x110;
const RIGHT_BUTTON: u32 = 0x111;
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
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<CameraWindow> = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;
    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(crate::language::texts());
    window.set_title(texts.t(ronoctua_i18n::key::CAMERA_TITLE));
    window.set_app_id(camera::APP_ID);
    window.commit();

    let mut cycle: EventLoop<CameraWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME AND THE LANGUAGE, watched like every door watches them.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut CameraWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.owed = true;
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;
    let (language_sender, language_changed) = channel::channel::<()>();
    let _language_watch = crate::language::watch(move || language_sender.send(()).is_ok());
    handle
        .insert_source(language_changed, |event, (), state: &mut CameraWindow| {
            if let channel::Event::Msg(()) = event {
                state.texts = std::rc::Rc::new(crate::language::texts());
                state.owed = true;
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the language channel: {e}"))?;
    // AND THE SIZE OF THE TEXT: the controls over the picture are text, and a
    // size chosen anywhere moves them without the window being reopened.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut CameraWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE STUDIO'S STATE, on its own thread (`client::watch` starts the daemon
    // when nobody is home — the nested loop has no unit).
    let (state_sender, states) = channel::channel::<Option<State>>();
    let _watch = ronoctua_studio::client::watch(move |s| {
        let _ = state_sender.send(s);
    });
    handle
        .insert_source(states, |event, (), w: &mut CameraWindow| {
            if let channel::Event::Msg(s) = event {
                w.state_arrived(s);
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
    handle
        .insert_source(frames, |event, (), w: &mut CameraWindow| {
            if let channel::Event::Msg(()) = event {
                w.frame_arrived();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the preview channel: {e}"))?;

    let mut state = CameraWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboards: crate::seats::Seats::default(),
        key_here: Modifiers::default(),
        pointers: crate::seats::Seats::default(),
        here: PerSeat::default(),
        window,
        width: 0,
        height: 0,
        asked: (0, 0),
        scale: 1,
        scale_told: 1,
        columns: 80,
        lines: 24,
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        images: None,
        texts,
        hinted: crate::chrome::Hinted::default(),
        view: camera::View::default(),
        studio: None,
        slot,
        frame: None,
        uploaded: false,
        message: None,
        buttons: Vec::new(),
        menu_box: None,
        menu_under: None,
        button_under: None,
        mixer_rows: Vec::new(),
        last_place: std::time::Instant::now(),
        told_photo: None,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;
    let signal = cycle.get_signal();
    let run = cycle.run(None, &mut state, |w| {
        w.settle();
        if w.quit {
            signal.stop();
        }
    });
    alive.store(false, std::sync::atomic::Ordering::SeqCst);
    run.context("event loop")?;
    Ok(())
}

/// What this window keeps for ONE pointer.
#[derive(Debug, Default)]
struct PerSeat {
    /// Where the pointer is, in logical pixels.
    at: (f64, f64),
    /// A layer being dragged: which, its rect when the drag began, and where
    /// the drag began in program fractions.
    dragging: Option<(usize, Rect, (f32, f32))>,
    /// Wheel lines owed.
    lines_owed: f64,
    /// Dragging the header's drawer.
    hint_drag: bool,
}

struct CameraWindow {
    conn: Connection,
    handle: calloop::LoopHandle<'static, Self>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    key_here: Modifiers,
    pointers: crate::seats::Seats<PerSeat>,
    here: PerSeat,
    window: Window,
    width: u32,
    height: u32,
    asked: (u32, u32),
    scale: i32,
    scale_told: i32,
    columns: u16,
    lines: u16,
    quit: bool,
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    images: Option<ronoctua_render::image::Images>,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    hinted: crate::chrome::Hinted,
    view: camera::View,
    /// The studio's last word; `None` while it is not answering.
    studio: Option<State>,
    slot: Slot,
    /// The newest frame, as the renderer takes it.
    frame: Option<ronoctua_render::image::Rgba>,
    /// Whether `frame` is already on the GPU.
    uploaded: bool,
    message: Option<String>,
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
    /// The sound drawer's rows as last drawn: `(line, which)`.
    mixer_rows: Vec<(u16, ronoctua_studio::wire::Sound)>,
    last_place: std::time::Instant,
    /// The photo last said on screen, so a new one is said once.
    told_photo: Option<String>,
}

impl CameraWindow {
    fn state_arrived(&mut self, s: Option<State>) {
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
                self.message = Some(
                    self.texts
                        .t_with(ronoctua_i18n::key::STUDIO_PHOTO_SAVED, &[("file", &name)]),
                );
                self.view.sign();
            }
            self.told_photo.clone_from(&s.last_photo);
        }
        self.studio = s;
        self.owed = true;
    }

    fn frame_arrived(&mut self) {
        if let Some((w, h, px)) = self.slot.lock().ok().and_then(|mut s| s.take()) {
            self.frame = ronoctua_render::image::Rgba::new(w, h, px);
            self.uploaded = false;
            self.owed = true;
        }
    }

    fn program(&self) -> (u32, u32) {
        self.frame
            .as_ref()
            .map(|f| (f.width, f.height))
            .or_else(|| self.studio.as_ref().map(|s| (s.width, s.height)))
            .unwrap_or((1280, 720))
    }

    fn picture(&self) -> ronoctua_render::image::Area {
        #[allow(clippy::cast_precision_loss)]
        camera::picture_in(
            self.width as f32,
            self.height as f32,
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

    /// The size of the text changed under this window: the shaper hears it,
    /// and the grid is counted again, or letters and cells disagree.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(ronoctua_tokens::text::size());
        }
        self.recount();
        self.owed = true;
    }

    fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        let usable = (self.width as f32) - 2.0 * cell * f32::from(crate::chrome::FIELDS_LEAD);
        self.columns = tokens::metrics::columns(usable.max(0.0), size);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = (self.height as f32 / line).floor().max(0.0) as u16;
        self.lines = lines;
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast()),
            std::ptr::NonNull::new(self.window.wl_surface().id().as_ptr().cast()),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        let first = crate::hidpi::physical(self.asked, self.scale);
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` —
        // the handles do not outlive what produced them.
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                first.0,
                first.1,
            )
        };
        let Ok(mut g) = gpu else {
            tracing::error!("no usable GPU; the window will not draw");
            return;
        };
        let (w, h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
        self.width = w;
        self.height = h;
        self.recount();
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            self.scale,
            tokens::text::size(),
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "assembling the text failed"),
        }
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "assembling the quads failed"),
        }
        match ronoctua_render::image::Images::new(&g.device, g.config.format) {
            Ok(i) => self.images = Some(i),
            Err(e) => tracing::error!(error = %e, "assembling the images failed"),
        }
        self.gpu = Some(g);
    }

    fn typed(&mut self, event: KeyEvent) {
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
                self.message = None;
                let lines = self.menu_box.map_or(1, |b| b.lines);
                let errand = self.view.menu_key(key, lines, self.studio.as_ref());
                self.apply(errand);
                return;
            }
        }
        // `C-` AND A LETTER is written the way the table writes it; the
        // letter comes from the keysym, because with Ctrl held the text the
        // keyboard gives is a control character.
        let chord = (self.key_here.ctrl && !self.key_here.alt)
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
            self.pressed(press);
        }
    }

    fn pressed(&mut self, press: Press) {
        self.message = None;
        let errand = self.view.key(press, self.studio.as_ref());
        self.apply(errand);
    }

    fn apply(&mut self, errand: Errand) {
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                self.quit = true;
                return;
            }
            Errand::Run(id) => {
                if let Err(e) = crate::leader::send_it(&format!("command {id}")) {
                    tracing::warn!(id, error = %e, "the footer did not take the command");
                }
            }
            Errand::Tell(ask) => self.tell(&ask),
        }
        self.owed = true;
    }

    fn tell(&mut self, ask: &Ask) {
        match ronoctua_studio::client::ask(ask) {
            Ok(ronoctua_studio::wire::Say::Refused(t)) => {
                self.message = Some(camera::trouble_sentence(&t, &self.texts))
            }
            Ok(_) => {}
            Err(_) => self.message = Some(self.texts.t(ronoctua_i18n::key::STUDIO_NOT_RUNNING)),
        }
    }

    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
            if self.hinted.settled() {
                self.paint();
            }
            // ASKED AGAIN AFTER THE PAINT: the buttons and the menu exist
            // only once drawn, and the pointer that brought the controls back
            // arrived while there were none — the header would go on teaching
            // the keys under a hand already resting on a button.
            if self.lit_under(self.here.at) {
                self.paint();
            }
        }
    }

    /// The line the bottom row of buttons is on.
    fn buttons_line(&self) -> u16 {
        self.lines.saturating_sub(1)
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::text::size();
        let line = Self::line_height();
        let margin = crate::chrome::FIELDS_LEAD;
        let showing = self.view.showing();
        let area = self.picture();
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let mut under: Vec<ronoctua_render::quad::Quad> = vec![ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            color: tokens::color::SURFACE_BACKDROP,
        }];
        let mut over: Vec<ronoctua_render::quad::Quad> = Vec::new();
        let span =
            |text: String, column: u16, line: u16, color: tokens::Color| ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text,
                column,
                line,
                color,
                edge: false,
                foot: false,
            };

        // NO PICTURE YET: say why, in the middle, rather than a black window.
        if self.frame.is_none() {
            let why = self.studio.as_ref().map_or_else(
                || self.texts.t(ronoctua_i18n::key::STUDIO_NOT_RUNNING),
                |s| {
                    s.troubles
                        .first()
                        .map_or_else(String::new, |t| camera::trouble_sentence(t, &self.texts))
                },
            );
            for (i, l) in crate::chrome::note_lines(&why, self.columns)
                .into_iter()
                .enumerate()
            {
                spans.push(span(
                    l,
                    margin,
                    self.lines / 2 + u16::try_from(i).unwrap_or(0),
                    tokens::color::TEXT_SECONDARY,
                ));
            }
        }

        self.buttons.clear();
        self.mixer_rows.clear();
        if showing {
            // THE TOP BAND: the hint line, on the transport's translucent strip.
            over.push(ronoctua_render::quad::Quad {
                x: 0.0,
                y: 0.0,
                width: w,
                height: line * f32::from(1 + self.hinted.rows()),
                color: tokens::color::SURFACE_TRANSPORT,
            });
            // A BUTTON UNDER THE POINTER TAKES THE LINE: a row of glyphs says
            // nothing on its own, so the line that teaches the keys says what
            // THIS one does, its key, and — on a button that opens a menu —
            // what each mouse button does to it.
            let (pieces, hinting) = match (&self.message, self.button_under) {
                (Some(t), _) => (vec![crate::chrome::Piece::said(t.clone())], false),
                (None, Some(id)) => (camera::about_button(id, &self.texts), false),
                (None, None) => (camera::hint(&self.texts), true),
            };
            spans.extend(
                self.hinted
                    .head(&pieces, hinting, margin, self.columns, &self.texts),
            );

            // THE STATUS LINE, above the buttons: what is rolling, and what is wrong.
            let status_line = self.buttons_line().saturating_sub(1);
            let mut status: Vec<(String, tokens::Color)> = Vec::new();
            if let Some(s) = &self.studio {
                if let Some(r) = &s.recording {
                    status.push((
                        format!(
                            "{} {}",
                            tokens::icon::STUDIO_RECORD,
                            self.texts.t_with(
                                ronoctua_i18n::key::STUDIO_RECORDING_FOR,
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
                            self.texts.t_with(
                                ronoctua_i18n::key::STUDIO_STREAMING_FOR,
                                &[("time", &camera::elapsed(r.since))]
                            )
                        ),
                        tokens::color::STATE_ERROR,
                    ));
                }
                if let Some(n) = s.countdown {
                    status.push((
                        self.texts.t_with(
                            ronoctua_i18n::key::STUDIO_COUNTDOWN,
                            &[("seconds", &n.to_string())],
                        ),
                        tokens::color::STATE_ACCENT,
                    ));
                }
                if self.frame.is_some()
                    && let Some(t) = s.troubles.first()
                {
                    status.push((
                        camera::trouble_sentence(t, &self.texts),
                        tokens::color::STATE_WARNING,
                    ));
                }
            }
            if !status.is_empty() {
                over.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: line * f32::from(status_line),
                    width: w,
                    height: line,
                    color: tokens::color::SURFACE_TRANSPORT,
                });
                let mut at = margin;
                for (text, colour) in status {
                    let cells = ronoctua_protocol::cells(&text);
                    spans.push(span(text, at, status_line, colour));
                    at += cells + 3;
                }
            }

            // THE BOTTOM ROW: every button of the table, glyph first.
            let bl = self.buttons_line();
            over.push(ronoctua_render::quad::Quad {
                x: 0.0,
                y: line * f32::from(bl),
                width: w,
                height: h - line * f32::from(bl),
                color: tokens::color::SURFACE_TRANSPORT,
            });
            let mut at = margin;
            for b in camera::buttons(&self.view, self.studio.as_ref(), &self.texts) {
                let colour = if b.on {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_PRIMARY
                };
                let start = at;
                spans.push(span(
                    b.glyph.to_string(),
                    at,
                    bl,
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
                    spans.push(span(words, at, bl, tokens::color::TEXT_SECONDARY));
                    at += cells + 1;
                }
                if at + 2 > self.columns + margin {
                    break;
                }
                self.buttons.push((start, at, b.id));
                at += 2;
            }

            // THE OPEN MENU, over the picture and above the status line, under
            // the button that opened it. `menu_box` is where it is drawn AND
            // where it is hit.
            self.menu_box = None;
            if let Some(open) = self.view.menu {
                let rows = camera::choices(open.menu, self.studio.as_ref(), &self.texts);
                let labels: Vec<String> = rows.iter().map(|c| c.label.clone()).collect();
                let anchor = self
                    .buttons
                    .iter()
                    .find(|(_, _, id)| *id == open.menu.button())
                    .map_or(margin, |(a, _, _)| *a);
                let first = 1 + self.hinted.rows();
                self.menu_box = camera::menu_box(
                    &labels,
                    anchor,
                    first,
                    status_line,
                    margin,
                    margin + self.columns,
                );
                if let Some(b) = self.menu_box {
                    let cell = Self::cell_width();
                    over.push(ronoctua_render::quad::Quad {
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
                            over.push(ronoctua_render::quad::Quad {
                                x: cell * f32::from(col),
                                y: line * f32::from(l),
                                width: cell * f32::from(b.width),
                                height: line,
                                color,
                            });
                        }
                        // AIR, the mark, a space, the label, air
                        // (`camera::MENU_FRAME`).
                        if c.current {
                            spans.push(span(
                                tokens::icon::SUCCESS.to_string(),
                                col + 1,
                                l,
                                tokens::color::STATE_ACCENT,
                            ));
                        }
                        spans.push(span(
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
            }

            // THE SOUND DRAWER: the two sounds, their level and their volume.
            if self.view.mixer
                && let Some(s) = &self.studio
            {
                let rows = camera::sounds(s);
                let top = status_line.saturating_sub(u16::try_from(rows.len()).unwrap_or(2) + 1);
                over.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: line * f32::from(top),
                    width: w,
                    height: line * f32::from(u16::try_from(rows.len()).unwrap_or(2)),
                    color: tokens::color::SURFACE_TRANSPORT,
                });
                for (i, (which, level, gain, muted)) in rows.into_iter().enumerate() {
                    let l = top + u16::try_from(i).unwrap_or(0);
                    let name = self.texts.t(match which {
                        ronoctua_studio::wire::Sound::Microphone => {
                            ronoctua_i18n::key::STUDIO_SOUND_MICROPHONE
                        }
                        ronoctua_studio::wire::Sound::Desktop => {
                            ronoctua_i18n::key::STUDIO_SOUND_DESKTOP
                        }
                    });
                    spans.push(span(
                        name,
                        margin,
                        l,
                        if muted {
                            tokens::color::TEXT_DIM
                        } else {
                            tokens::color::TEXT_PRIMARY
                        },
                    ));
                    // THE LEVEL IS `chrome::meter`, the sound view's own
                    // bar, at the column every value in this system starts at.
                    let bar_at = crate::chrome::value_column(margin, self.columns);
                    let bar = crate::chrome::meter(level, crate::audio::BAR_CELLS);
                    let bar_cells = crate::audio::BAR_CELLS;
                    spans.push(span(
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
                        self.texts.t(ronoctua_i18n::key::STUDIO_MUTED)
                    } else {
                        self.texts.t_with(
                            ronoctua_i18n::key::STUDIO_LEVEL,
                            &[("percent", &gain.to_string())],
                        )
                    };
                    spans.push(span(
                        value,
                        bar_at + bar_cells + 1,
                        l,
                        tokens::color::TEXT_SECONDARY,
                    ));
                    self.mixer_rows.push((l, which));
                }
            }

            // THE LAYERS A HAND CAN MOVE, outlined, so the drag is findable.
            if let Some(s) = &self.studio {
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
                    // THE ACCENT, and not a rule's tone: the outline says
                    // "this can be taken", which is what the accent says
                    // everywhere else — and `RULE_STRONG` is a scrollbar's.
                    let c = tokens::color::STATE_ACCENT;
                    over.push(ronoctua_render::quad::Quad {
                        x,
                        y,
                        width: lw,
                        height: t,
                        color: c,
                    });
                    over.push(ronoctua_render::quad::Quad {
                        x,
                        y: y + lh - t,
                        width: lw,
                        height: t,
                        color: c,
                    });
                    over.push(ronoctua_render::quad::Quad {
                        x,
                        y,
                        width: t,
                        height: lh,
                        color: c,
                    });
                    over.push(ronoctua_render::quad::Quad {
                        x: x + lw - t,
                        y,
                        width: t,
                        height: lh,
                        color: c,
                    });
                }
            }
            over.extend(self.hinted.bar(crate::chrome::Frame::of(w, h, 0)));
        }

        let under_count = u32::try_from(under.len()).unwrap_or(0);
        under.extend(over);
        let surface = self.window.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        if let Some(im) = self.images.as_mut() {
            im.clear();
            let mut pictures = Vec::new();
            if let Some(f) = &self.frame {
                if !self.uploaded || im.missing(PROGRAM) {
                    im.upload_it(&gpu.device, &gpu.queue, PROGRAM, f);
                    self.uploaded = true;
                }
                pictures.push((PROGRAM.to_owned(), area));
            }
            im.prepare(&gpu.queue, self.width, self.height, &pictures);
        }
        if let Err(e) = text.prepare(
            &gpu.device,
            &gpu.queue,
            gpu.config.width,
            gpu.config.height,
            ronoctua_render::Atlas::Pinned,
            &spans,
        ) {
            tracing::error!(error = %e, "preparing the text failed");
            return;
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(&gpu.device, &gpu.queue, self.width, self.height, &under);
        }
        // UNDER THE PICTURE the backdrop; OVER it the bands and outlines; the
        // text last. There is no depth test: whoever draws later is on top.
        let quads_ref = self.quads.as_ref();
        let images_ref = self.images.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw_range(pass, 0, under_count);
                }
                if let Some(im) = images_ref {
                    im.draw(pass);
                }
                if let Some(q) = quads_ref {
                    q.draw_range(pass, under_count, u32::MAX);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            },
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
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

    /// What is under the pointer — a button of the bottom row, a row of the
    /// open menu — kept; `true` when that changed and the window is owed a
    /// frame.
    fn lit_under(&mut self, at: (f64, f64)) -> bool {
        if at == OUTSIDE {
            let changed = self.button_under.is_some() || self.menu_under.is_some();
            self.button_under = None;
            self.menu_under = None;
            return changed;
        }
        let line = Self::line_at(at.1);
        let button = (self.view.showing() && line == self.buttons_line())
            .then(|| self.button_at(at.0))
            .flatten();
        let row = self
            .menu_box
            .and_then(|b| b.at(Self::column_at(at.0), line));
        let changed = button != self.button_under || row != self.menu_under;
        self.button_under = button;
        self.menu_under = row;
        changed
    }

    fn line_at(y: f64) -> u16 {
        let line = f64::from(Self::line_height());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let l = (y / line.max(1.0)) as u16;
        l
    }
}

impl WindowHandler for CameraWindow {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        cfg: WindowConfigure,
        _: u32,
    ) {
        self.asked = (
            cfg.new_size.0.map_or(960, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(600, std::num::NonZeroU32::get),
        );
        self.width = self.asked.0;
        self.height = self.asked.1;
        let (asked, scale) = (self.asked, self.scale);
        if let Some(g) = self.gpu.as_mut() {
            let (w, h) = crate::hidpi::resized(g, asked, scale);
            self.width = w;
            self.height = h;
        }
        self.recount();
        self.owed = true;
    }
}

impl SeatHandler for CameraWindow {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard {
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, kb, event| {
                    state.keyboards.switch_to(kb, &mut state.key_here);
                    state.typed(event);
                }),
            ) {
                Ok(k) => {
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.keyboards.took(&seat, k, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => {
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.pointers.took(&seat, p, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no pointer; keyboard only"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard {
            self.keyboards.lost(&seat, &mut self.key_here);
        }
        if cap == Capability::Pointer {
            self.pointers.lost(&seat, &mut self.here);
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for CameraWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        self.hinted.let_everything_go();
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        if self.hinted.took_a_key(event.keysym, true) {
            self.owed = true;
            return;
        }
        self.typed(event);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, kb, serial, event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        self.hinted.took_a_key(event.keysym, false);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        // KEPT, and it was not: `C-e` opens a menu, and a window that threw
        // the modifiers away read it as `e`.
        self.key_here = modifiers;
    }
}

impl PointerHandler for CameraWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.pointers.switch_to(pointer, &mut self.here);
        self.hinted.swap_hand(&mut self.here.hint_drag);
        self.pointer_frame_here(events);
        self.hinted.swap_hand(&mut self.here.hint_drag);
    }
}

impl CameraWindow {
    fn pointer_frame_here(&mut self, events: &[PointerEvent]) {
        #[allow(clippy::cast_precision_loss)]
        let head = crate::chrome::Frame::of(self.width as f32, self.height as f32, 0);
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    // A SIGN OF A PERSON: the controls come back.
                    let was = self.view.showing();
                    self.view.sign();
                    self.here.at = e.position;
                    if self.hinted.dragged(head, e.position.1) {
                        self.owed = true;
                        continue;
                    }
                    let now = self
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &self.texts));
                    if self.hinted.say(now) || !was {
                        self.owed = true;
                    }
                    if self.lit_under(e.position) {
                        self.owed = true;
                    }
                    // A LAYER FOLLOWS THE HAND, sent at most every 30 ms: a drag
                    // is dozens of motions a second and the studio rewrites a
                    // scene for each one it takes.
                    #[allow(clippy::cast_possible_truncation)]
                    if let Some((layer, start, from)) = self.here.dragging
                        && let Some((u, v)) = camera::on_program(
                            self.picture(),
                            e.position.0 as f32,
                            e.position.1 as f32,
                        )
                        && self.last_place.elapsed() > std::time::Duration::from_millis(30)
                    {
                        self.last_place = std::time::Instant::now();
                        let rect = start.moved(u - from.0, v - from.1);
                        self.tell(&Ask::Place { layer, rect });
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.here.at = OUTSIDE;
                    if self.button_under.take().is_some() | self.menu_under.take().is_some() {
                        self.owed = true;
                    }
                    self.hinted.let_go();
                    if self.hinted.say(None) {
                        self.owed = true;
                    }
                    self.here.dragging = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.hinted.let_go();
                    self.here.dragging = None;
                }
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    self.view.sign();
                    let line = Self::line_at(e.position.1);
                    if self.view.showing()
                        && line == self.buttons_line()
                        && let Some(id) = self.button_at(e.position.0)
                        && let Some(press) = camera::press_of(id)
                    {
                        self.pressed(press);
                    } else if self.view.menu.take().is_some() {
                        self.owed = true;
                    }
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.view.sign();
                    if self.hinted.pressed(head, e.position.0, e.position.1) {
                        self.owed = true;
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
                        self.apply(errand);
                        continue;
                    }
                    if self.view.showing() && line == self.buttons_line() {
                        // THE LEFT BUTTON OPENS the menu of a control that has
                        // several values; the right one turns it a step.
                        if let Some(id) = self.button_at(e.position.0) {
                            if let Some(menu) = camera::Menu::of_button(id) {
                                self.view.toggle_menu(menu, self.studio.as_ref());
                                self.owed = true;
                            } else if let Some(press) = camera::press_of(id) {
                                self.pressed(press);
                            }
                        }
                        continue;
                    }
                    // A CLICK ANYWHERE ELSE SHUTS AN OPEN MENU, and does only
                    // that: the click that closes it must not also move a
                    // layer or mute a sound the person was not aiming at.
                    if self.view.menu.take().is_some() {
                        self.owed = true;
                        continue;
                    }
                    if let Some((_, which)) =
                        self.mixer_rows.iter().find(|(l, _)| *l == line).copied()
                    {
                        let muted = self.studio.as_ref().is_some_and(|s| {
                            s.muted[usize::from(which == ronoctua_studio::wire::Sound::Desktop)]
                        });
                        self.tell(&Ask::Sound {
                            which,
                            gain: None,
                            muted: Some(!muted),
                        });
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if let Some(s) = &self.studio
                        && let Some((u, v)) = camera::on_program(
                            self.picture(),
                            e.position.0 as f32,
                            e.position.1 as f32,
                        )
                        && let Some((layer, rect)) = camera::movable_at(s, u, v)
                    {
                        self.here.dragging = Some((layer, rect, (u, v)));
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::surface::wheel_arrived(
                        &mut self.here.lines_owed,
                        &vertical,
                        &horizontal,
                    );
                    let by = crate::surface::wheel_lines(&mut self.here.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    self.view.sign();
                    // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER, and not
                    // the picture under it: the gesture belongs to what is
                    // under the pointer.
                    if self.hinted.turned(head, e.position.1, by) {
                        self.owed = true;
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
                        self.tell(&Ask::Sound {
                            which,
                            gain: Some(next),
                            muted: None,
                        });
                        continue;
                    }
                    // THE WHEEL OVER A SMALL LAYER RESIZES IT, about its centre.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                    if let Some(s) = &self.studio
                        && let Some((u, v)) = camera::on_program(
                            self.picture(),
                            e.position.0 as f32,
                            e.position.1 as f32,
                        )
                        && let Some((layer, rect)) = camera::movable_at(s, u, v)
                    {
                        let rect = rect.scaled(1.1f32.powi(-(by as i32)));
                        self.tell(&Ask::Place { layer, rect });
                    }
                }
                _ => {}
            }
        }
    }
}

impl CompositorHandler for CameraWindow {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if let Some((w, h)) = crate::hidpi::followed(
            &mut self.scale,
            factor,
            surface,
            self.gpu.as_mut(),
            self.text.as_mut(),
            self.asked,
        ) {
            self.width = w;
            self.height = h;
            self.recount();
            self.owed = true;
        }
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for CameraWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for CameraWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(CameraWindow);
smithay_client_toolkit::delegate_dispatch2!(CameraWindow);
