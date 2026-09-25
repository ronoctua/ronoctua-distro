//! A manual in a window of its own — the second door onto
//! [`crate::manual`], opened from the list `LEADER h` shows (`C-w` there, or
//! the "open in a window" line of an open manual).
//!
//! It DECIDES nothing: which lines a manual has is `manual.rs`, and both
//! doors draw the same lines. It is `history_window.rs` with the view swapped
//! for a manual, deliberately: eleven doors that drifted apart would be
//! eleven answers to how a wheel scrolls.
//!
//! # Read-only
//!
//! A manual is read: the arrows, the page keys, the wheel and the bar move it,
//! `Esc` closes it, and nothing is typed.

use anyhow::{Context as _, Result};
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

use crate::manual::{Line, Tone, Which};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.manual";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list: line 0 is the header.
const FIRST_OF_LIST: u16 = 1;

/// The margin inside the window, in cells — see `chrome::FIELDS_LEAD`.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Opens manual `which` in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(which: Which) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<ManualWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(crate::language::texts());
    window.set_title(format!("*{}*", texts.t(which.name())));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<ManualWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window — and its drawings, whose ink is the theme's — without it being
    // reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut ManualWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // THE LANGUAGE'S FILE, watched for the theme's reason and by its means.
    let (language_sender, language_changed) = channel::channel::<()>();
    let _language_watch = crate::language::watch(move || language_sender.send(()).is_ok());
    handle
        .insert_source(language_changed, |event, (), state: &mut ManualWindow| {
            if let channel::Event::Msg(()) = event {
                state.texts = std::rc::Rc::new(crate::language::texts());
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the language channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, for the same reason and by the same means.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut ManualWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    let mut state = ManualWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
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
        // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE — see `crate::hidpi`.
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
        art_sizes: std::collections::HashMap::new(),
        which,
        texts,
        hinted: crate::chrome::Hinted::default(),
        offset: 0,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            e.settle();
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The window.
struct ManualWindow {
    conn: Connection,
    /// The loop, for a keyboard that REPEATS — see `history_window`'s field
    /// of the same name.
    handle: calloop::LoopHandle<'static, Self>,
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    /// The modifiers of the keyboard whose events are being handled.
    key_here: Modifiers,
    /// Every pointer this window was offered — one per cursor.
    pointers: crate::seats::Seats<PerSeat>,
    /// The state of the pointer whose events are being handled now.
    here: PerSeat,
    window: Window,
    /// The window's size in LOGICAL pixels — see `history_window`.
    width: u32,
    /// See [`ManualWindow::width`].
    height: u32,
    /// The logical size the COMPOSITOR configured, before any clamp — see
    /// [`crate::hidpi`].
    asked: (u32, u32),
    /// Physical pixels per logical one, on this window's screen.
    scale: i32,
    /// What the SURFACE has been told, which lags `scale` by one frame.
    scale_told: i32,
    columns: u16,
    lines: u16,
    quit: bool,
    /// Whether a frame is owed.
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    /// The list's scrollbar, and every pointer's hover.
    quads: Option<ronoctua_render::quad::Quads>,
    /// The gestures' drawings.
    images: Option<ronoctua_render::image::Images>,
    /// The size each drawing went up at, by its key: it is centred in its
    /// box by that size.
    art_sizes: std::collections::HashMap<String, (u32, u32)>,
    which: Which,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    /// The header line, and its drawer.
    hinted: crate::chrome::Hinted,
    /// The first visible line. A window has no page: it scrolls.
    offset: usize,
}

/// What this window keeps for ONE pointer — see `crate::seats`.
#[derive(Debug, Default)]
struct PerSeat {
    /// Is the pointer dragging the bar?
    dragging: bool,
    /// How many LINES the wheel has asked for and not yet been paid — see
    /// `surface::wheel_arrived`.
    lines_owed: f64,
    row_under_pointer: Option<usize>,
    /// Whether this pointer is dragging the header's drawer bar.
    hint_drag: bool,
}

impl ManualWindow {
    /// The manual's lines, in the language of now, WRAPPED to this window —
    /// the text column starts two past the key column, and the bar takes the
    /// last cell.
    fn manual(&self) -> Vec<Line> {
        crate::manual::laid_out(
            self.which.lines(&self.texts, &crate::leader_chord()),
            self.columns,
            3,
            1,
        )
    }

    /// How many lines fit under the header.
    fn room(&self) -> usize {
        self.lines.saturating_sub(self.first_of_list()) as usize
    }

    /// Keeps the offset inside the manual, and nothing else — a paint may not
    /// move it (see `history_window::settle_offset`).
    fn settle_offset(&mut self, rows: usize) {
        self.offset = self.offset.min(rows.saturating_sub(self.room()));
    }

    /// Moves by `by` lines, inside the manual.
    fn scroll_by(&mut self, by: i64) {
        let last = self.manual().len().saturating_sub(self.room());
        self.offset = usize::try_from(
            i64::try_from(self.offset)
                .unwrap_or(0)
                .saturating_add(by)
                .max(0),
        )
        .unwrap_or(0)
        .min(last);
        self.ask_paint();
    }

    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(ronoctua_tokens::text::size());
        }
        self.recount();
        self.ask_paint();
    }

    /// Columns and rows from the size in pixels.
    fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        let usable = (self.width as f32) - 2.0 * cell * f32::from(CONTENT_MARGIN);
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
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what produced them.
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
        let (shown_w, shown_h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
        self.width = shown_w;
        self.height = shown_h;
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

    /// One key, from a press or from a REPEAT — see `history_window::typed`.
    fn typed(&mut self, event: KeyEvent) {
        let page = i64::try_from(self.room().max(1)).unwrap_or(1);
        match event.keysym {
            Keysym::Escape => self.quit = true,
            Keysym::Down => self.scroll_by(1),
            Keysym::Up => self.scroll_by(-1),
            Keysym::Page_Down | Keysym::space => self.scroll_by(page),
            Keysym::Page_Up => self.scroll_by(-page),
            Keysym::Home => self.scroll_by(i64::MIN / 2),
            Keysym::End => self.scroll_by(i64::MAX / 2),
            _ => {}
        }
    }

    /// Asks for a frame. It is drawn when the event queue empties.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Draws the owed frame, if one is owed.
    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
            if self.hinted.settled() {
                self.paint();
            }
        }
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line_height = tokens::metrics::line_height(size);
        let margin = CONTENT_MARGIN;
        let manual = self.manual();
        self.settle_offset(manual.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        // LINE 0: `MANUAL:` in the cool grey the system keeps for what is
        // said ABOUT something (`Register::Explain`), then the manual's name in
        // the answer's tone — two colours, the owner's ask — and the header
        // every window of this system has, drawer and all (`chrome::Hinted`).
        let pieces = vec![
            crate::chrome::Piece::key_in(
                format!("{} ", self.texts.t(ronoctua_i18n::key::MANUAL_WINDOW_LEAD)),
                crate::chrome::Register::Explain,
            ),
            crate::chrome::Piece::said(self.texts.t(self.which.name())),
        ];
        spans.extend(
            self.hinted
                .head(&pieces, false, margin, self.columns, &self.texts),
        );

        // THE KEY COLUMN is as wide as the widest key, or the drawings' column
        // when the manual has drawings — the footer's arithmetic, by the same
        // numbers.
        let key_cells = crate::manual::key_column(&manual);
        let text_at = margin + key_cells + 2;
        let first = self.first_of_list();
        let room = self.room();
        let mut pictures: Vec<(&'static str, ronoctua_render::image::Area)> = Vec::new();
        let visible: Vec<&Line> = manual.iter().skip(self.offset).take(room).collect();
        for (i, l) in visible.iter().enumerate() {
            let y = first + u16::try_from(i).unwrap_or(0);
            let color = match l.tone {
                Tone::Heading => tokens::color::TEXT_HEADING,
                Tone::Plain | Tone::Pose => tokens::color::TEXT_SECONDARY,
                // WHAT A GESTURE DOES, the loudest — the footer's rule.
                Tone::Function => tokens::color::TEXT_PRIMARY,
                Tone::Detail => tokens::color::TEXT_DIM,
                // AIR: nothing is written on it.
                Tone::Air => continue,
            };
            if !l.key.is_empty() {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: l.key.clone(),
                    column: margin,
                    line: y,
                    color: tokens::color::TEXT_KEY,
                    edge: false,
                    foot: false,
                });
            }
            // A HEADING TAKES THE WHOLE LINE from the margin; everything else
            // starts after the key column.
            let (column, room_cells) = if l.title() {
                (margin, self.columns)
            } else {
                (text_at, self.columns.saturating_sub(key_cells + 2))
            };
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: ronoctua_render::layout::elide_end(&l.description, room_cells),
                column,
                line: y,
                color,
                edge: false,
                foot: false,
            });
            // THE DRAWING, when every line of its gesture is on screen — see
            // the footer's reason in `Footer::paint`.
            if let Some(g) = l.picture
                && i + crate::manual::GESTURE_LINES <= visible.len()
            {
                let (bw, bh) = crate::manual::gesture_art_box();
                pictures.push((
                    g,
                    ronoctua_render::image::Area {
                        x: f32::from(margin) * cell,
                        y: f32::from(y) * line_height,
                        width: bw,
                        height: bh,
                    },
                ));
            }
        }

        // THE BAR, from the same `Scroll` every other bar in this system uses,
        // and every cursor's hover in its own colour.
        let scroll = crate::chrome::list_scroll(manual.len(), room, self.offset);
        #[allow(clippy::cast_precision_loss)]
        let bands = crate::chrome::hover_bands(
            self.pointers
                .lit(&self.here)
                .map(|(color, s)| (color, s.row_under_pointer)),
            self.offset,
            room,
            manual.len(),
            first,
            self.width as f32,
        );
        #[allow(clippy::cast_precision_loss)]
        let mut bar: Vec<ronoctua_render::quad::Quad> =
            crate::chrome::list_bar(scroll, first, self.width as f32, self.height as f32)
                .into_iter()
                .collect();
        bar.splice(0..0, bands);
        #[allow(clippy::cast_precision_loss)]
        bar.extend(self.hinted.bar(crate::chrome::Frame::of(
            self.width as f32,
            self.height as f32,
            0,
        )));

        let surface = self.window.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        let (width, height) = (self.width, self.height);
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
            q.prepare(&gpu.device, &gpu.queue, width, height, bar.as_slice());
        }
        // THE DRAWINGS go up once per key (size and theme are in it), and are
        // centred in their box by their own size.
        if let Some(im) = self.images.as_mut() {
            let mut batch = Vec::with_capacity(pictures.len());
            for (g, area) in pictures {
                let key = crate::manual::gesture_art_key(g);
                if im.missing(&key)
                    && let Some(rgba) = crate::manual::gesture_art(g)
                {
                    im.upload_it(&gpu.device, &gpu.queue, &key, &rgba);
                    self.art_sizes
                        .insert(key.clone(), (rgba.width, rgba.height));
                }
                if let Some(&(w, h)) = self.art_sizes.get(&key) {
                    #[allow(clippy::cast_precision_loss)]
                    let (w, h) = (w as f32, h as f32);
                    batch.push((
                        key,
                        ronoctua_render::image::Area {
                            x: area.x + (area.width - w).max(0.0) / 2.0,
                            y: area.y + (area.height - h).max(0.0) / 2.0,
                            width: w.min(area.width),
                            height: h.min(area.height),
                        },
                    ));
                }
            }
            im.prepare(&gpu.queue, width, height, &batch);
        }
        // THE RECTANGLES, THEN THE DRAWINGS, THEN THE TEXT: there is no depth
        // test, and whoever draws later is on top.
        let quads_ref = self.quads.as_ref();
        let images_ref = self.images.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw(pass);
                }
                if let Some(im) = images_ref {
                    im.draw(pass);
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

    /// The first line of the list, pushed down by the header's drawer.
    fn first_of_list(&self) -> u16 {
        FIRST_OF_LIST + self.hinted.rows()
    }

    /// Which manual line is at height `y`.
    fn row_at(&self, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line <= 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (y / line) as usize;
        which
            .checked_sub(usize::from(self.first_of_list()))
            .map(|i| i + self.offset)
    }
}

impl WindowHandler for ManualWindow {
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
        // WHAT THE COMPOSITOR SAID IS KEPT SEPARATELY, because what the
        // buffer can show is what comes back below — see `asked`. It is
        // taken from the CONFIGURE and from nothing else: built out of
        // the door's own fields it would be the size `resized` last
        // answered, which can only ever shrink.
        self.asked = (
            cfg.new_size.0.map_or(900, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(600, std::num::NonZeroU32::get),
        );
        self.width = self.asked.0;
        self.height = self.asked.1;
        let asked = self.asked;
        let scale = self.scale;
        if let Some(g) = self.gpu.as_mut() {
            // AND THE WINDOW KEEPS WHAT THE BUFFER CAN SHOW. Below the GPU's
            // ceiling that is exactly what the compositor configured; above
            // it, laying out against the bigger number would put the columns
            // and the bars in a space the buffer does not have.
            let (shown_w, shown_h) = crate::hidpi::resized(g, asked, scale);
            self.width = shown_w;
            self.height = shown_h;
        }
        self.recount();
        self.ask_paint();
    }
}

impl SeatHandler for ManualWindow {
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
            // A KEYBOARD THAT REPEATS — see the `handle` field. The rate and
            // the delay are the COMPOSITOR's, arriving as `RepeatInfo`;
            // nothing here invents either.
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
                    // EVERY KEYBOARD, and not only the first: one per seat, and in
                    // Independent mode one per cursor.
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.keyboards.took(&seat, k, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => {
                    // EVERY POINTER, and not only the first: one per cursor.
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

impl KeyboardHandler for ManualWindow {
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
        // A MODIFIER HELD WHEN THE FOCUS WENT IS A MODIFIER NOBODY WILL SEE
        // COME UP — the release goes to whoever has the keyboard now.
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
        // THE HEADER'S OWN KEYS FIRST — the right `Ctrl` and its pair, and the
        // `Esc` that shuts an open drawer before it closes the window. See
        // `chrome::Hinted::took_a_key`.
        if self.hinted.took_a_key(event.keysym, true) {
            self.ask_paint();
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
        // THE MODIFIER IS LET GO HERE, and it has to be: xkb merges both
        // Ctrls into one bit, so the modifier event cannot say which of them
        // came up.
        self.hinted.took_a_key(event.keysym, false);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
    }
}

impl PointerHandler for ManualWindow {
    /// EVERY CURSOR HAS ITS OWN STATE: the pointer that spoke is made `here`
    /// first, so a drag, a hover or a wheel's leftover never crosses from
    /// one cursor to another. See `crate::seats`.
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.pointers.switch_to(pointer, &mut self.here);
        self.hinted.swap_hand(&mut self.here.hint_drag);
        self.pointer_frame_here(conn, qh, events);
        self.hinted.swap_hand(&mut self.here.hint_drag);
    }
}

impl ManualWindow {
    /// One pointer's frame, with its state already `here`: the header's
    /// controls, the bar, the wheel, and the hover. A manual has nothing to
    /// click — it is read.
    fn pointer_frame_here(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        events: &[PointerEvent],
    ) {
        let before = self.here.row_under_pointer;
        let mut turned = false;
        let rows = self.manual().len();
        let scroll = crate::chrome::list_scroll(rows, self.room(), self.offset);
        #[allow(clippy::cast_precision_loss)]
        let (bar_w, bar_h) = (self.width as f32, self.height as f32);
        let head = crate::chrome::Frame::of(bar_w, bar_h, 0);
        let mut header = false;
        let mut wheel_at: Option<f64> = None;
        let mut dragged: Option<usize> = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if self.hinted.dragged(head, e.position.1) {
                        header = true;
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if self.here.dragging {
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            self.first_of_list(),
                            bar_w,
                            bar_h,
                            e.position.1 as f32,
                        ));
                    } else {
                        self.here.row_under_pointer = self.row_at(e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.hinted.let_go();
                    self.here.dragging = false;
                    self.here.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.hinted.let_go();
                    self.here.dragging = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if self.hinted.pressed(head, e.position.0, e.position.1) {
                        header = true;
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if crate::chrome::on_the_list_bar(
                        scroll,
                        self.first_of_list(),
                        bar_w,
                        bar_h,
                        e.position.0 as f32,
                        e.position.1 as f32,
                    ) {
                        self.here.dragging = true;
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            self.first_of_list(),
                            bar_w,
                            bar_h,
                            e.position.1 as f32,
                        ));
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
                    wheel_at = Some(e.position.1);
                    turned = true;
                }
                _ => {}
            }
        }
        if header {
            self.ask_paint();
            return;
        }
        if let Some(offset) = dragged {
            self.offset = offset;
            self.ask_paint();
            return;
        }
        if turned {
            let by = crate::surface::wheel_lines(&mut self.here.lines_owed);
            if by != 0 && wheel_at.is_some_and(|y| self.hinted.turned(head, y, by)) {
                self.ask_paint();
                return;
            }
            if by != 0 {
                self.scroll_by(by);
            }
            return;
        }
        if before != self.here.row_under_pointer {
            self.ask_paint();
        }
    }
}

impl CompositorHandler for ManualWindow {
    /// The window moved to a screen of another scale — or was told its first.
    ///
    /// It was EMPTY here and in eight other doors, which is what made every
    /// one of them blurry on a HiDPI screen. The rule and the measurement are
    /// in [`crate::hidpi`].
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        // THE SURFACE IS NEEDED to ask its VERSION, not to tell it anything:
        // it is told its scale inside the frame, which is the only commit that
        // carries a matching buffer — see `crate::hidpi::told_before_present`.
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
            // AND THE DOOR KEEPS WHAT THE BUFFER CAN SHOW. Below the
            // GPU's ceiling this is what the compositor configured;
            // above it, going on with the old number lays the picture
            // out in a space the buffer does not have.
            self.width = w;
            self.height = h;
            self.recount();
            self.ask_paint();
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

impl OutputHandler for ManualWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for ManualWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(ManualWindow);
smithay_client_toolkit::delegate_dispatch2!(ManualWindow);
