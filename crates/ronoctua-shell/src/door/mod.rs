//! EVERY WINDOW'S PLUMBING, written once.
//!
//! # Why there is one
//!
//! Because there were nineteen. Every door of this shell — the power, network
//! and Bluetooth windows, the file manager's, the manual's, the buffer host —
//! carried its own copy of the same thousand lines: the Wayland registry, the
//! seats and every pointer and keyboard on them, the scale of the screen it is
//! on, the GPU, the three watches (theme, language, size of the text), the
//! repeat of a held key, the header that teaches the keys, the list under it
//! and its bar, and the arithmetic of the wheel. The copies drifted the way
//! copies do: the language watch was missing from three of them, the bar's
//! column answered the row beside it in all of them, and touch reached none —
//! adding it meant adding it nineteen times, and forgetting one.
//!
//! The owner asked for a base that gives every window, and every window
//! still to come, the same predictable behaviour by construction. That is
//! [`Door`]: it owns the plumbing, and a window is a [`Content`] — what it
//! draws, and what its keys and its rows do.
//!
//! # Why nothing is locked in
//!
//! Because the owner asked for that too: a window may decide not to follow a
//! standard. Every behaviour of [`Content`] past the three a window must write
//! has a DEFAULT that is the standard, and a window that wants otherwise
//! overrides that one method — the file manager keeps its own pointer, the
//! camera its own wheel. What it cannot do is forget one: a door that
//! implements no pointer still answers the pointer, a door that says nothing
//! about touch still answers a finger.

pub mod handlers;
pub mod keys;
pub mod list;
pub mod touch;

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::calloop::{self, EventLoop, channel},
    reexports::calloop_wayland_source::WaylandSource,
    registry::RegistryState,
    seat::{
        SeatState,
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::PointerEvent,
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{LayerShell, LayerSurface},
        xdg::{
            XdgShell,
            window::{Window, WindowDecorations},
        },
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::{GlobalList, registry_queue_init},
    protocol::{wl_keyboard, wl_surface, wl_touch},
};

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
pub const LEFT_BUTTON: u32 = 0x110;

/// `BTN_RIGHT`.
pub const RIGHT_BUTTON: u32 = 0x111;

/// The first line of a list, under the header.
///
/// Everything asks [`Ui::first_of_list`] instead: the header's drawer and
/// the sentence about the shortcut under the pointer push it down.
pub const FIRST_OF_LIST: u16 = 1;

/// What every window keeps that is not Wayland's — the state a [`Content`]
/// reads and writes.
///
/// It is its own struct, and not fields of [`Door`], so that a content's
/// methods can take it by `&mut` while the door still holds the plumbing:
/// the two borrows are of different things.
pub struct Ui {
    /// The words, in the language chosen — replaced when the choice moves.
    pub texts: std::rc::Rc<ronoctua_i18n::Texts>,
    /// The window's size in LOGICAL pixels — what the BUFFER can show. See
    /// [`crate::hidpi`] for why this is not always what was asked.
    pub width: u32,
    /// See [`Ui::width`].
    pub height: u32,
    /// How many physical pixels one logical pixel is on the screen the window
    /// is on.
    pub scale: i32,
    /// Columns of text, net of both margins.
    pub columns: u16,
    /// Lines of text the window holds.
    pub lines: u16,
    /// The margin inside the window, in cells.
    pub margin: u16,
    /// What the last order said, on line 0 instead of the keys.
    pub message: Option<String>,
    /// The window is to close when the queue empties.
    pub quit: bool,
    /// Whether a frame is owed. A frame is drawn when the event queue empties
    /// — never straight from an event, which would draw once per event of a
    /// burst.
    owed: bool,
    /// The first visible row of the list. A window has no page: it scrolls.
    pub offset: usize,
    /// The header: the line that teaches the keys, and its drawer.
    pub hinted: crate::chrome::Hinted,
    /// Where the list's BAR was drawn on the last frame, if one was.
    ///
    /// The pointer asks this and nothing else about whether it is on the bar,
    /// which is what gives the bar its own column while it is drawn — and
    /// only then. See [`crate::chrome::Room`].
    pub room: crate::chrome::Room,
    /// The connection, for a flush and for a window that needs to hand it on.
    pub conn: Connection,
    /// The surface this window draws to.
    pub surface: wl_surface::WlSurface,
    /// WHOSE HAND the pointer event being handled is — its cursor's colour
    /// number (`crate::seats`), for a window that lights one row per hand.
    pub pointer_seat: u16,
    /// Whether more than one cursor is on this window — a hover is drawn in
    /// its cursor's colour only then (`crate::seats::band_colour`).
    pub many_pointers: bool,
    /// Whose KEYBOARD the key being handled came from.
    pub keyboard_seat: u16,
    /// THE MODIFIERS OF THE KEYBOARD ON THE POINTER'S OWN SEAT — what a click
    /// reads (`Ctrl`+click marks, `Shift`+click takes a run). Per seat, and
    /// not the last keyboard that spoke: in Independent mode each cursor has
    /// its own keyboard, and a Ctrl held on one is not held on the other.
    pub pointer_mods: Modifiers,
    /// The seat of the pointer whose events are being handled — what a drag
    /// is started on, since the press's serial is that seat's.
    pub pointer_on: Option<smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat>,
    /// Whether the key being handled is a REPEAT of a held one.
    pub repeating: bool,
    /// The window's own size changed (`Content::own_size`), and the door is to
    /// lay it out again without waiting for a configure that may not come.
    size_owed: bool,
}

impl Ui {
    /// Asks for a frame. It is drawn when the event queue empties.
    pub const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// The window's own size changed (`Content::own_size`): lay it out again.
    pub const fn retake_the_size(&mut self) {
        self.size_owed = true;
        self.owed = true;
    }

    /// The first line of the list, pushed down by the header's drawer and the
    /// sentence about the shortcut under the pointer.
    #[must_use]
    pub fn first_of_list(&self) -> u16 {
        FIRST_OF_LIST + self.hinted.rows()
    }

    /// How many rows of the list fit under the header.
    #[must_use]
    pub fn room_for_rows(&self) -> usize {
        self.lines.saturating_sub(self.first_of_list()) as usize
    }

    /// Keeps the offset inside a list of `rows`, and nothing else.
    ///
    /// # Why this is not `follow`
    ///
    /// Because a paint may not move the list. Following the cursor from a
    /// paint undid, on the same frame, every offset the pointer had just set:
    /// a drag of the bar and a turn of the wheel both wrote an offset and then
    /// watched the paint put it back under the cursor. What a paint may do is
    /// keep the offset inside a list that changed size under it.
    pub fn settle_offset(&mut self, rows: usize) {
        let room = self.room_for_rows();
        if room == 0 {
            return;
        }
        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the
    /// edge — called on a KEY or a CLICK, never from a paint.
    pub fn follow(&mut self, cursor: usize, rows: usize) {
        const MARGIN: usize = 2;
        let room = self.room_for_rows();
        if room == 0 {
            return;
        }
        if cursor < self.offset + MARGIN {
            self.offset = cursor.saturating_sub(MARGIN);
        } else if cursor + MARGIN >= self.offset + room {
            self.offset = cursor + MARGIN + 1 - room;
        }
        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    /// Which drawn row of the list is at `(x, y)`.
    ///
    /// `None` above the list — and ON THE BAR'S COLUMN while a bar is drawn:
    /// the bar has its own cells, and a pointer on them is on the bar and not
    /// on the row beside it. Without a bar the row answers across the whole
    /// width, which is what the owner asked for in both halves.
    #[must_use]
    pub fn row_at(&self, x: f64, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line <= 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        if self.room.on_the_bar(x as f32) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (y / line) as usize;
        which
            .checked_sub(usize::from(self.first_of_list()))
            .map(|i| i + self.offset)
    }

    /// The window's own pixels, as a [`crate::chrome::Frame`] whose first
    /// line is `first`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn frame(&self, first: u16) -> crate::chrome::Frame {
        crate::chrome::Frame::of(self.width as f32, self.height as f32, first)
    }

    /// Columns and rows from the size in pixels.
    pub fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        let usable = (self.width as f32) - 2.0 * cell * f32::from(self.margin);
        self.columns = tokens::metrics::columns(usable.max(0.0), size);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = (self.height as f32 / line).floor().max(0.0) as u16;
        self.lines = lines;
    }
}

/// What one pointer keeps while another one is `here` — see `crate::seats`.
///
/// The half every door needs is here; what a window keeps of its own for one
/// pointer is `extra`.
#[derive(Debug, Default)]
pub struct Hand<H> {
    /// Is this pointer dragging the list's bar?
    ///
    /// Held, because a drag is three events: the press that grabs it, every
    /// motion until the release, and the release.
    pub dragging: bool,
    /// How many LINES the wheel and the touchpad have asked for and not yet
    /// been paid — see `surface::wheel_arrived`.
    pub lines_owed: f64,
    /// The row of the list under this pointer.
    pub row_under_pointer: Option<usize>,
    /// Whether a button of this pointer is down — a drag in progress, which a
    /// bar's column does not interrupt.
    pub held: bool,
    /// Whether this pointer is dragging the header's drawer bar.
    pub hint_drag: bool,
    /// What the window keeps of its own for this pointer.
    pub extra: H,
}

/// One frame, as a window describes it.
///
/// The ORDER of drawing is the canvas's and not the window's: rectangles,
/// then pictures, then the rectangles that go OVER pictures, then the text.
/// There is no depth test, and whoever draws later is on top — so the order is
/// a decision, and it is made once here.
pub struct Canvas<'a> {
    /// The text.
    pub spans: Vec<ronoctua_render::Span>,
    /// The rectangles under everything else: bands, grounds, bars.
    pub under: Vec<ronoctua_render::quad::Quad>,
    /// The rectangles over the pictures and under the text: a transport band
    /// laid over a film, a selection swept over thumbnails.
    pub over: Vec<ronoctua_render::quad::Quad>,
    /// The pictures, by key, in pixels of the window.
    pub pictures: Vec<(String, ronoctua_render::image::Area)>,
    /// Every pointer's row, in its cursor's colour — for the hover bands.
    pub lit: &'a [(tokens::Color, Option<usize>)],
    /// The GPU, for a window that uploads a picture.
    pub gpu: &'a ronoctua_render::gpu::Gpu,
    /// The pictures already on the GPU.
    pub images: &'a mut ronoctua_render::image::Images,
    /// Every bar this frame draws, by its track — see `crate::bars`.
    pub bars: &'a mut crate::bars::Drawn,
}

impl Canvas<'_> {
    /// A bar is drawn on this frame, with this track: its column is the
    /// bar's, for the pointer and for every band (`crate::bars`).
    pub fn bar(&mut self, track: Option<crate::bars::Track>) {
        if let Some(t) = track {
            self.bars.record(t);
        }
    }

    /// Puts a picture on the GPU under `key`, unless it is already there.
    ///
    /// The picture is made only when it is missing: `make` is not called on
    /// a frame that already has it.
    pub fn upload(
        &mut self,
        key: &str,
        make: impl FnOnce() -> Option<ronoctua_render::image::Rgba>,
    ) -> bool {
        if self.images.missing(key) {
            let Some(rgba) = make() else {
                return false;
            };
            self.images
                .upload_it(&self.gpu.device, &self.gpu.queue, key, &rgba);
        }
        true
    }
}

/// How a door appears on the screen.
pub enum Shell {
    /// A window of its own, which the compositor places (invariant 4) — and
    /// the global that made it, kept for as long as the window lives.
    Window {
        /// The window.
        window: Window,
        /// The global that made it — never read, and kept for as long as the
        /// window lives.
        _global: XdgShell,
    },
    /// A layer-shell surface — the right column (invariant 14) — and the
    /// global, SHARED so a column raised again on another screen is made by
    /// the same one (`Door::layer_shell`).
    Layer(LayerSurface, std::rc::Rc<LayerShell>),
}

impl Shell {
    /// The surface under it.
    #[must_use]
    pub fn wl_surface(&self) -> &wl_surface::WlSurface {
        match self {
            Self::Window { window, .. } => window.wl_surface(),
            Self::Layer(l, _) => l.wl_surface(),
        }
    }
}

/// What a door is handed while it is being built: the loop to hang its own
/// channels on, the words, and the globals.
pub struct Wiring<'a, C: Content> {
    /// The event loop's handle, for the window's own sources.
    pub handle: calloop::LoopHandle<'static, Door<C>>,
    /// The words.
    pub texts: std::rc::Rc<ronoctua_i18n::Texts>,
    /// The globals, for a window that binds a protocol of its own.
    pub globals: &'a GlobalList,
    /// The queue.
    pub qh: &'a QueueHandle<Door<C>>,
}

/// A WINDOW: what it draws, and what its keys and rows do.
///
/// Three methods are the window's to write — its name, its `app_id` and what
/// a key does. Everything else has a default that IS the system's standard,
/// and a window that wants otherwise overrides that one method.
pub trait Content: Sized + 'static {
    /// What this window keeps of its own for ONE pointer.
    type Hand: Default;

    /// The `app_id`.
    fn app_id(&self) -> &'static str;

    /// The title the compositor shows.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String;

    /// One key, from the keyboard, from a REPEAT or from a click on the
    /// header's shortcut for it — the three arrive here the same way.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers);

    /// How the window appears. By default, a window of its own.
    ///
    /// # Errors
    ///
    /// When the compositor offers no shell for it.
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let xdg = XdgShell::bind(wiring.globals, wiring.qh)
            .context("the compositor does not expose xdg_shell")?;
        let window = xdg.create_window(surface, WindowDecorations::RequestServer, wiring.qh);
        window.set_title(self.title(&wiring.texts));
        window.set_app_id(self.app_id());
        window.commit();
        Ok(Shell::Window {
            window,
            _global: xdg,
        })
    }

    /// The size before the compositor has said one — and the fallback for an
    /// axis it leaves to the window.
    fn default_size(&self) -> (u32, u32) {
        (900, 600)
    }

    /// A size the window OWNS, over what the compositor configured.
    ///
    /// A floating window's size is the one its commits carry — the compositor
    /// adopts it — so a window that has to be a given size draws at it (the
    /// player's corner: 384×216 however it was tiled). `None` is every other
    /// window: laid out at what it was asked. When this answer changes, the
    /// window says so with `Ui::retake_the_size`.
    fn own_size(&self) -> Option<(u32, u32)> {
        None
    }

    /// The compositor configured the WINDOW — for a window that reads more of
    /// the configure than its size (the player: whether it is fullscreen).
    fn window_configured(
        &mut self,
        ui: &mut Ui,
        cfg: &smithay_client_toolkit::shell::xdg::window::WindowConfigure,
    ) {
        let _ = (ui, cfg);
    }

    /// The margin inside the window, in cells.
    fn margin(&self) -> u16 {
        crate::chrome::FIELDS_LEAD
    }

    /// The keys the header teaches.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        let _ = ui;
        Vec::new()
    }

    /// The rows of the list, each already placed in cells.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        let _ = ui;
        Vec::new()
    }

    /// Where the keyboard's cursor is in [`Content::rows`], when there is one:
    /// the list follows it after every key and every click.
    fn cursor(&self) -> Option<usize> {
        None
    }

    /// A click on row `row` of the list. The first click selects and the
    /// second acts, which is the window's to decide (invariant 19).
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let _ = (ui, row);
    }

    /// A press of the SECOND button on row `row`.
    fn right_clicked(&mut self, ui: &mut Ui, row: usize) {
        let _ = (ui, row);
    }

    /// Draws one frame. The default is the standard window: the header, the
    /// list under it, the hover of every cursor, and the list's bar.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        list::paint(self, ui, canvas);
    }

    /// One pointer's frame. The default is the standard list: the header's
    /// controls, the bar, the hover, the click and the wheel.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Self::Hand>, events: &[PointerEvent]) {
        list::pointer(self, ui, hand, events);
    }

    /// Whether the header's own keys — the right `Ctrl` walking its drawer,
    /// and `Esc` shutting it — are the header's. A window with a drawer of
    /// its own answers `false`.
    fn header_keys(&self) -> bool {
        true
    }

    /// A key came up.
    fn released(&mut self, ui: &mut Ui, event: &KeyEvent) {
        let _ = (ui, event);
    }

    /// The keyboard came into this window — whatever was held before belongs
    /// to the window it came from.
    fn keyboard_entered(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The keyboard left this window.
    fn keyboard_left(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The window was given a new size.
    fn resized(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// A screen was plugged in or taken away.
    fn outputs_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// A language was chosen, and `ui.texts` already speaks it.
    fn language_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The compositor asked the window to close. `true` to close.
    fn closing(&mut self, ui: &mut Ui) -> bool {
        let _ = ui;
        true
    }

    /// Once per turn of the loop, before the owed frame is drawn.
    fn each_turn(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The compositor's frame callback for this surface.
    fn frame_done(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// A seat offered a capability — for a window that binds something of its
    /// own on it (the file manager's data device).
    fn seat_capability(
        door: &mut Door<Self>,
        qh: &QueueHandle<Door<Self>>,
        seat: &smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat,
    ) {
        let _ = (door, qh, seat);
    }
}

/// A door: the plumbing, and the window in it.
pub struct Door<C: Content> {
    /// What every window keeps.
    pub ui: Ui,
    /// The window.
    pub content: C,
    /// The loop, for a keyboard that REPEATS and a finger held long.
    ///
    /// SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle`.
    pub handle: calloop::LoopHandle<'static, Self>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    /// How the window appears.
    pub shell: Shell,
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    /// The modifiers of the keyboard whose events are being handled.
    pub mods: Modifiers,
    /// Every pointer — one per cursor — with the state of each while another
    /// is `here`. See `crate::seats`.
    pointers: crate::seats::Seats<Hand<C::Hand>>,
    /// The state of the pointer whose events are being handled now.
    pub here: Hand<C::Hand>,
    /// Every touch screen on every seat.
    touches: crate::seats::Seats<(), wl_touch::WlTouch>,
    /// The fingers down, and the hand they play — its own, so a finger and a
    /// mouse on the same window do not share a drag.
    fingers: touch::Fingers<Hand<C::Hand>>,
    /// What the SURFACE has been told its scale is, which lags `ui.scale` by
    /// one frame — see [`crate::hidpi::told_before_present`].
    scale_told: i32,
    /// The logical size the COMPOSITOR configured, before any clamp — see
    /// [`crate::hidpi`].
    asked: (u32, u32),
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    images: Option<ronoctua_render::image::Images>,
    /// Every bar the last frame drew — see `crate::bars`.
    bars: crate::bars::Drawn,
    _watches: Vec<ronoctua_files::notice::Notice>,
    /// The queue, for a window that makes Wayland objects of its own.
    pub qh: QueueHandle<Self>,
}

/// Opens a window and runs it until it closes.
///
/// `make` builds the window, and is handed the loop so it can hang its own
/// channels on it — a daemon's events, a file of its own watched.
///
/// # Errors
///
/// When there is no Wayland compositor, or it offers no shell for the window.
pub fn run<C: Content>(make: impl FnOnce(&Wiring<'_, C>) -> Result<C>) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<Door<C>> = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;

    let mut cycle: EventLoop<Door<C>> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();
    let texts = std::rc::Rc::new(crate::language::texts());

    let wiring = Wiring {
        handle: handle.clone(),
        texts: texts.clone(),
        globals: &globals,
        qh: &qh,
    };
    let content = make(&wiring)?;
    let surface = compositor.create_surface(&qh);
    let shell = content.open(&wiring, surface)?;

    let watches = hang_the_watches(&handle)?;

    let margin = content.margin();
    let mut door = Door {
        ui: Ui {
            texts,
            width: 0,
            height: 0,
            // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE: the surface exists
            // before the compositor has said which output it is on.
            scale: 1,
            columns: 80,
            lines: 24,
            margin,
            message: None,
            quit: false,
            owed: false,
            offset: 0,
            hinted: crate::chrome::Hinted::default(),
            room: crate::chrome::Room::default(),
            conn: conn.clone(),
            surface: shell.wl_surface().clone(),
            pointer_seat: 0,
            many_pointers: false,
            keyboard_seat: 0,
            pointer_mods: Modifiers::default(),
            pointer_on: None,
            repeating: false,
            size_owed: false,
        },
        content,
        handle,
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        shell,
        keyboards: crate::seats::Seats::default(),
        mods: Modifiers::default(),
        pointers: crate::seats::Seats::default(),
        here: Hand::default(),
        touches: crate::seats::Seats::default(),
        fingers: touch::Fingers::default(),
        scale_told: 1,
        asked: (0, 0),
        gpu: None,
        text: None,
        quads: None,
        images: None,
        bars: crate::bars::Drawn::default(),
        _watches: watches,
        qh: qh.clone(),
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut door, |d| {
            d.content.each_turn(&mut d.ui);
            d.settle();
            if d.ui.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The theme, the language and the size of the text, watched — so a choice
/// made anywhere reaches this window without it being reopened.
///
/// # Why every door has all three
///
/// Because a door that adopted a choice only at its next start is the
/// incomplete preview the owner read off the screen when the theme did
/// exactly that. The language watch was missing from three doors; here it
/// cannot be.
fn hang_the_watches<C: Content>(
    handle: &calloop::LoopHandle<'static, Door<C>>,
) -> Result<Vec<ronoctua_files::notice::Notice>> {
    let mut kept = Vec::new();

    let (theme_sender, theme_changed) = channel::channel::<()>();
    kept.extend(crate::theme::watch(move || theme_sender.send(()).is_ok()));
    handle
        .insert_source(theme_changed, |event, (), door: &mut Door<C>| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                door.ui.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    let (language_sender, language_changed) = channel::channel::<()>();
    kept.extend(crate::language::watch(move || {
        language_sender.send(()).is_ok()
    }));
    handle
        .insert_source(language_changed, |event, (), door: &mut Door<C>| {
            if let channel::Event::Msg(()) = event {
                door.adopt_the_language();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the language channel: {e}"))?;

    let (size_sender, size_changed) = channel::channel::<()>();
    kept.extend(crate::toolkits::watch(move || size_sender.send(()).is_ok()));
    handle
        .insert_source(size_changed, |event, (), door: &mut Door<C>| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                door.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;
    Ok(kept)
}

impl<C: Content> Door<C> {
    /// The layer shell this door's column was made by, to make another.
    #[must_use]
    pub fn layer_shell(&self) -> Option<std::rc::Rc<LayerShell>> {
        match &self.shell {
            Shell::Layer(_, shell) => Some(shell.clone()),
            Shell::Window { .. } => None,
        }
    }

    /// The compositor state, for a window that makes a surface of its own.
    #[must_use]
    pub const fn compositor(&self) -> &CompositorState {
        &self._compositor
    }

    /// Puts the door on ANOTHER surface — the player's column following the
    /// screen with the focus.
    ///
    /// A new surface is a new texture: the GPU, the text and the pictures are
    /// let go of and built again against it on the next frame, and the scale
    /// starts from one until the new screen says otherwise.
    pub fn reopen(&mut self, shell: Shell) {
        self.images = None;
        self.quads = None;
        self.text = None;
        self.gpu = None;
        self.ui.surface = shell.wl_surface().clone();
        self.shell = shell;
        self.ui.scale = 1;
        self.scale_told = 1;
        self.ui.width = 0;
        self.ui.height = 0;
        self.ui.ask_paint();
    }

    /// A language was chosen: the words, the renderer, and the window.
    ///
    /// THE RENDERER IS TOLD TOO. It is not a formality: the locale is what
    /// decides, among the four unified Han faces, whether an ideograph comes
    /// out Japanese or Chinese — and only the footer told it, so every window
    /// drew the same character one way while the footer drew it the other.
    fn adopt_the_language(&mut self) {
        let (id, _) = crate::language::resolve();
        self.ui.texts = std::rc::Rc::new(crate::language::texts());
        if let Some(t) = self.text.as_mut() {
            t.set_locale(&id);
        }
        self.content.language_changed(&mut self.ui);
        self.ui.ask_paint();
    }

    /// The size of the text changed under this door.
    ///
    /// THE SHAPER HAS TO HEAR IT TOO: moving the grid and leaving the letters
    /// gives a grid at one size with text at another.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(tokens::text::size());
        }
        self.ui.recount();
        self.content.resized(&mut self.ui);
        self.ui.ask_paint();
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.ui.conn.backend().display_ptr().cast()),
            std::ptr::NonNull::new(self.shell.wl_surface().id().as_ptr().cast()),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        // THE WINDOW'S OWN SIZE when it owns one (`Content::own_size`), and
        // what the compositor configured otherwise.
        let asked = self.content.own_size().unwrap_or(self.asked);
        let first = crate::hidpi::physical(asked, self.ui.scale);
        // SAFETY: the connection and the surface are fields of this door, like
        // the `Gpu` — the handles do not outlive what produced them.
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
        // AND THE FIRST BUFFER IS DIVISIBLE TOO — see `crate::hidpi::resized`.
        let (shown_w, shown_h) = crate::hidpi::resized(&mut g, asked, self.ui.scale);
        self.ui.width = shown_w;
        self.ui.height = shown_h;
        // AND THE COLUMNS ARE COUNTED AGAIN, against what the buffer can show.
        self.ui.recount();
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            self.ui.scale,
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

    /// Draws the owed frame, if one is owed.
    fn settle(&mut self) {
        if std::mem::take(&mut self.ui.size_owed) {
            self.take_the_size();
        }
        if std::mem::take(&mut self.ui.owed) {
            self.paint();
            // AND ONCE MORE IF THE HEADER'S DRAWER JUST OPENED: the list's
            // first line is asked for before the frame and the wrap is only
            // known inside it. It answers once — see `fold::Fold::held`.
            if self.ui.hinted.settled() {
                self.paint();
            }
        }
    }

    /// Every pointer's row, in its cursor's colour — and the finger's.
    fn lit(&self) -> Vec<(tokens::Color, Option<usize>)> {
        let mut lit: Vec<(tokens::Color, Option<usize>)> = self
            .pointers
            .lit(&self.here)
            .map(|(color, s)| (color, s.row_under_pointer))
            .collect();
        if let Some(row) = self.fingers.hand().row_under_pointer {
            lit.push((tokens::color::SURFACE_HOVER, Some(row)));
        }
        lit
    }

    fn paint(&mut self) {
        if self.ui.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let lit = self.lit();
        self.bars.clear();
        let (Some(gpu), Some(text), Some(quads), Some(images)) = (
            self.gpu.as_mut(),
            self.text.as_mut(),
            self.quads.as_mut(),
            self.images.as_mut(),
        ) else {
            return;
        };
        let mut canvas = Canvas {
            spans: Vec::new(),
            under: Vec::new(),
            over: Vec::new(),
            pictures: Vec::new(),
            lit: &lit,
            gpu,
            images,
            bars: &mut self.bars,
        };
        self.content.paint(&mut self.ui, &mut canvas);
        let Canvas {
            spans,
            mut under,
            mut over,
            pictures,
            ..
        } = canvas;
        // THE HEADER'S DRAWER BAR, in every door — the header is the door's.
        self.bars
            .record_opt(self.ui.hinted.drawer_track(self.ui.frame(0)));
        // AND NO BAND RUNS UNDER A BAR, whoever painted it: see `crate::bars`.
        self.bars.clip(&mut under);
        self.bars.clip(&mut over);

        // THE SURFACE AND THE TWO SCALE NUMBERS, taken before the borrows:
        // `set_buffer_scale` has to travel in the commit `Gpu::frame` makes —
        // see `crate::hidpi::told_before_present`.
        let surface = self.shell.wl_surface().clone();
        let scale = self.ui.scale;
        let told = &mut self.scale_told;
        let (width, height) = (self.ui.width, self.ui.height);
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
        // ONE LIST OF RECTANGLES, drawn in two ranges around the pictures.
        //
        // LOGICAL, where the text took the PHYSICAL surface: a `Quad` is a
        // rectangle in the window's own pixels, and the renderer turns it into
        // NDC by dividing — so a logical rectangle over a logical viewport lands
        // exactly where a physical one over a physical viewport would.
        let split = u32::try_from(under.len()).unwrap_or(u32::MAX);
        under.extend(over);
        let total = u32::try_from(under.len()).unwrap_or(u32::MAX);
        quads.prepare(&gpu.device, &gpu.queue, width, height, under.as_slice());
        images.prepare(&gpu.queue, width, height, &pictures);
        let (quads, images) = (&*quads, &*images);
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                quads.draw_range(pass, 0, split);
                images.draw(pass);
                quads.draw_range(pass, split, total);
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            },
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.ui.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }

    /// One key, from a press, a REPEAT or a click on the header — and the
    /// list follows the cursor, on the EVENT and never in the paint.
    fn typed(&mut self, event: &KeyEvent, mods: Modifiers) {
        self.ui.keyboard_seat = self.keyboards.colour_number_here();
        self.content.typed(&mut self.ui, event, mods);
        self.follow();
    }

    /// The list follows the keyboard's cursor.
    fn follow(&mut self) {
        if let Some(cursor) = self.content.cursor() {
            let rows = self.content.rows(&self.ui).len();
            self.ui.follow(cursor, rows);
        }
    }

    /// A click on a shortcut of the header: what the key would have done.
    ///
    /// A key is pressed through the SAME path the keyboard takes, so the click
    /// does exactly what the key does in the state the window is in. A command
    /// of the registry is handed to the footer, which is where commands run.
    fn act(&mut self, act: crate::chrome::Act) {
        match act {
            crate::chrome::Act::Key(key) => {
                let (event, mods) = keys::event_of(key);
                self.typed(&event, mods);
            }
            crate::chrome::Act::Run(id) => {
                if !forward(id) {
                    self.ui.message = Some(self.ui.texts.t(ronoctua_i18n::key::DOOR_NO_FOOTER));
                }
            }
        }
        self.ui.ask_paint();
    }

    /// One pointer's frame — from a mouse, or from a finger played as one.
    ///
    /// A press on a shortcut of the header is taken HERE, for every window:
    /// the header is the system's, and so is what a click on it does.
    fn pointer_frame_here(&mut self, events: &[PointerEvent]) {
        use smithay_client_toolkit::seat::pointer::PointerEventKind;
        let mut rest: Vec<PointerEvent> = Vec::with_capacity(events.len());
        let mut acts: Vec<crate::chrome::Act> = Vec::new();
        for e in events {
            // THE SHORTCUT UNDER THE POINTER LIGHTS, in every door — the header
            // is the door's (`chrome::Hinted::light_at`).
            let lights = match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.ui.hinted.light_at(Some(e.position))
                }
                PointerEventKind::Leave { .. } => self.ui.hinted.light_at(None),
                _ => false,
            };
            if lights {
                self.ui.ask_paint();
            }
            match e.kind {
                PointerEventKind::Press { .. } => self.here.held = true,
                PointerEventKind::Release { .. } | PointerEventKind::Leave { .. } => {
                    self.here.held = false;
                }
                _ => {}
            }
            // THE BAR'S COLUMN IS THE BAR'S — for every door, whatever drew the
            // bar (`crate::bars`). A pointer resting on a track, with no button
            // held, reaches the content as the pointer LEAVING the rows: no
            // content can light a row under a bar, because none is ever told
            // the pointer is on one. A press and the wheel still reach it — a
            // bar is grabbed and turned where it is drawn — and so does every
            // motion of a drag.
            if matches!(
                e.kind,
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. }
            ) && !self.here.held
                && self.bars.on(e.position.0, e.position.1)
            {
                rest.push(PointerEvent {
                    surface: e.surface.clone(),
                    position: e.position,
                    kind: PointerEventKind::Leave { serial: 0 },
                });
                continue;
            }
            if let PointerEventKind::Press { button, .. } = e.kind
                && button == LEFT_BUTTON
                && let Some(act) = self.ui.hinted.press_under(e.position.0, e.position.1)
            {
                acts.push(act);
                continue;
            }
            rest.push(e.clone());
        }
        if !rest.is_empty() {
            self.ui.pointer_seat = self.pointers.colour_number_here();
            self.ui.many_pointers = self.pointers.many();
            let seat = self.pointers.seat_here().cloned();
            self.ui.pointer_mods = seat
                .as_ref()
                .and_then(|seat| self.keyboards.state_on(seat, &self.mods))
                .copied()
                .unwrap_or_default();
            self.ui.pointer_on = seat;
            self.content.pointer(&mut self.ui, &mut self.here, &rest);
        }
        for act in acts {
            self.act(act);
        }
    }

    /// A keysym that belongs to the header, when the window lets it have them.
    fn header_took(&mut self, sym: Keysym, down: bool) -> bool {
        self.content.header_keys() && self.ui.hinted.took_a_key(sym, down)
    }
}

/// Hands command `id` to the footer, where commands run.
///
/// NOT `status()`, WHICH WAITS: this runs inside the loop, and waiting for a
/// child there freezes the window for as long as the child takes. The child
/// writes one line to the footer's socket and exits, and it is let go of.
/// `false` when it could not even be started — which is a sentence on the
/// window, never silence.
pub fn forward(id: &str) -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|me| {
            std::process::Command::new(me)
                .args(["command", id])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()
        })
        .is_some()
}

/// A layer shell, for a door that is a column (`Content::open`).
///
/// # Errors
///
/// When the compositor offers no layer shell.
pub fn layer_shell<C: Content>(wiring: &Wiring<'_, C>) -> Result<std::rc::Rc<LayerShell>> {
    LayerShell::bind(wiring.globals, wiring.qh)
        .map(std::rc::Rc::new)
        .context("the compositor does not expose layer-shell")
}
