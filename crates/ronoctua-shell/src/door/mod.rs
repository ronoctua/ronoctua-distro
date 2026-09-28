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

/// `BTN_SIDE`, the mouse's Back button. Over a window it is `Esc`, as it is
/// over the footer: the pointer's way back, beside the `ESC` a header teaches.
pub const BACK_BUTTON: u32 = 0x113;

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
    /// Whether the compositor has CONFIGURED the surface this door draws to —
    /// the toolkit acks a configure before the door hears it — which is what
    /// lets a buffer be committed to it. See [`Ui::may_draw`].
    ///
    /// # Why having a size is not the answer
    ///
    /// It was: a door drew once it had a width, and a width came only from a
    /// configure. Then a window OWNED its size (`Content::own_size`, the
    /// player's corner): that size is laid out on the first turn, before the
    /// compositor has said anything, and the first frame went out on it — a
    /// buffer committed to an `xdg_surface` never configured, which is a
    /// protocol error (`unconfigured_buffer`). The compositor closed the
    /// connection and `ronoctua-shell player pip` died on its first frame.
    configured: bool,
    /// The window's own size changed (`Content::own_size`), and the door is to
    /// lay it out again without waiting for a configure that may not come.
    size_owed: bool,
    /// THE QUESTION BEING TYPED, on line 0 — see [`Ui::ask`].
    ///
    /// While it is here every key is its own (`Door::typed`), as every key is
    /// the footer's minibuffer's while it asks: a question is answered or
    /// let go, and a letter meant for it never reaches the view under it.
    pub asking: Option<crate::line::Line>,
    /// How many questions this window has asked — the number of the one on
    /// the line, so a paste read for an older one is not typed into this one.
    questions: u64,
    /// The clipboard's text is wanted, and for whom — read off the loop on
    /// the next turn (`Door::settle`).
    paste_owed: Option<PasteFor>,
    /// Orders for the footer, sent off the loop on the next turn — see
    /// [`Ui::tell_the_footer`].
    orders: Vec<String>,
    /// Programs to start, off the loop on the next turn — see [`Ui::launch`].
    launches: Vec<Launching>,
}

/// A program a window starts — see [`Ui::launch`] and [`Ui::launch_entry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launching {
    /// A program and its words, in a directory of its own choosing.
    Program {
        /// The program, by path.
        program: String,
        /// Its words.
        args: Vec<String>,
        /// Where it starts.
        cwd: Option<String>,
    },
    /// An entry of the launcher, by its desktop id.
    Entry(String),
}

/// Who a paste read off the loop is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasteFor {
    /// The question on line 0 — the one asked that many questions in.
    Line(u64),
    /// The window itself: [`Content::pasted`].
    Content,
}

/// What a key did while the line asked — what the window is told.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Routed {
    /// The line kept it: typed, erased, a paste asked for, or nothing.
    Kept,
    /// Answered, under the id that asked.
    Answered(&'static str, String),
    /// Let go.
    Cancelled(&'static str),
}

/// What a launch's verdict says on a window's line: only a failure is said,
/// in the footer's own sentence (`echo-no-opened`); the rest is the log's.
fn launch_said(verdict: &crate::foreign::Launch, texts: &ronoctua_i18n::Texts) -> Option<String> {
    use crate::foreign::Launch;
    match verdict {
        Launch::WithScope { scope, program } => {
            tracing::info!(scope, program, "opened from a window");
            None
        }
        Launch::Orphan { program, reason } => {
            tracing::warn!(program, reason, "opened WITHOUT a cgroup of its own");
            None
        }
        Launch::Failed { program, error } => {
            tracing::error!(error, program, "could not open");
            Some(texts.t_with(
                ronoctua_i18n::key::ECHO_NO_OPENED,
                &[("program", program.as_str())],
            ))
        }
    }
}

/// What a thread of the door hands back to its loop.
enum Back {
    /// The clipboard's text, read.
    Pasted {
        /// Who asked for it.
        to: PasteFor,
        /// The text, or why there is none.
        text: Result<String>,
    },
    /// The footer did not take an order: nothing listens on its socket.
    Unheard,
    /// How a program this window started went.
    Launched(crate::foreign::Launch),
    /// The launcher has no entry by the id this window asked to open.
    NotInLauncher,
}

impl Ui {
    /// Asks for a frame. It is drawn when the event queue empties.
    pub const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Whether a frame may be drawn — a buffer committed — now: the surface
    /// was configured, and there is a size to draw at. A window that knows
    /// its size before the compositor has spoken still waits for it.
    const fn may_draw(&self) -> bool {
        self.configured && self.width != 0
    }

    /// ASKS, on the header's line: a name, an address, a path, a password.
    ///
    /// The window types what the footer types (the owner's rule: the two
    /// doors do the same things). The line takes line 0 in place of the
    /// message and the shortcuts, with `ESC` and `RET` on it as clicks; every
    /// key is the line's until it is answered — [`Content::answered`], with
    /// the text — or let go ([`Content::unanswered`]). `C-v` pastes into it,
    /// read off the loop. A SECRET is masked on the screen and in the log,
    /// and its answer reaches `answered` and nothing else: it never leaves
    /// this process by the door's hand.
    ///
    /// A question already on the line is replaced, unanswered.
    pub fn ask(&mut self, line: crate::line::Line) {
        self.questions = self.questions.wrapping_add(1);
        self.asking = Some(line);
        self.message = None;
        self.ask_paint();
    }

    /// Starts `program` with `args` in a scope of its own, OFF THE LOOP — the
    /// one launch path (`foreign::start_in_scope`) — and a program that could
    /// not be started is a sentence on this window's line (`echo-no-opened`),
    /// as it is on the footer's. Every window that opens something opens it
    /// here, so none of them is left starting a bare child and only logging
    /// what went wrong.
    pub fn launch(&mut self, program: &str, args: &[String], cwd: Option<&str>) {
        self.launches.push(Launching::Program {
            program: program.to_owned(),
            args: args.to_vec(),
            cwd: cwd.map(ToOwned::to_owned),
        });
    }

    /// Starts the launcher's entry `desktop_id`, as the footer starts it —
    /// the catalogue read off the loop (`apps::launch_entry`) — and says on
    /// THIS window's line when there is no such entry yet
    /// (`software-not-in-launcher`) or it could not be started.
    pub fn launch_entry(&mut self, desktop_id: &str) {
        self.launches.push(Launching::Entry(desktop_id.to_owned()));
    }

    /// The clipboard's text is wanted by the window: it is read OFF THE LOOP
    /// (`crate::pasting`), with a deadline, and handed to
    /// [`Content::pasted`] when it arrives. A window never waits for the
    /// program that copied.
    pub const fn paste(&mut self) {
        self.paste_owed = Some(PasteFor::Content);
    }

    /// A key, while a question is on the line. Every key is the line's: it
    /// is typed, answered, let go, or asks for the clipboard — and the answer
    /// leaves the line before the window hears it.
    fn line_key(&mut self, key: ronoctua_keymap::Key) -> Routed {
        use crate::line::Step;
        let Some(line) = self.asking.as_mut() else {
            return Routed::Kept;
        };
        let asked_for = line.id;
        match line.key(key) {
            Step::Nothing => Routed::Kept,
            Step::Changed => {
                self.ask_paint();
                Routed::Kept
            }
            Step::WantsPaste => {
                self.paste_owed = Some(PasteFor::Line(self.questions));
                Routed::Kept
            }
            Step::Answer(id, text) => {
                self.asking = None;
                self.ask_paint();
                Routed::Answered(id, text)
            }
            Step::Cancel => {
                self.asking = None;
                self.ask_paint();
                Routed::Cancelled(asked_for)
            }
        }
    }

    /// A keysym, pressed (`down`) or let go, that belongs to the header when
    /// the window lets the header have its keys now (`its`,
    /// [`Content::header_keys`]).
    ///
    /// THE RIGHT `Ctrl` IS FOLLOWED EITHER WAY, and only taking a key is
    /// gated. A window may give the header its keys back while that `Ctrl`
    /// is down or up — the column coming back from its bar — and followed
    /// only while the header had them, a right `Ctrl` let go in the bar was
    /// still held after it: a plain `Down` walked the drawer instead of the
    /// list, until that `Ctrl` went down and up again. While the header is
    /// away the key is still not taken: the window reads it as it always
    /// did.
    fn header_took(&mut self, its: bool, sym: Keysym, down: bool) -> bool {
        if its {
            return self.hinted.took_a_key(sym, down);
        }
        if sym == Keysym::Control_R {
            self.hinted.right_ctrl_is(down);
        }
        false
    }

    /// A paste read off the loop, typed into the line when it was read FOR
    /// the line. `Some(false)` when it is the window's (`Content::pasted`);
    /// `Some(true)` when the line took it; `None` when it was for a question
    /// that is gone — answered or replaced since — which a late clipboard
    /// must not type into.
    fn pasted_into_the_line(&mut self, to: PasteFor, text: &str) -> Option<bool> {
        match to {
            PasteFor::Content => Some(false),
            PasteFor::Line(n) if n == self.questions => {
                let line = self.asking.as_mut()?;
                if line.paste(text) == crate::line::Step::Changed {
                    self.ask_paint();
                }
                Some(true)
            }
            PasteFor::Line(_) => None,
        }
    }

    /// Whether the clipboard is wanted on the next turn — for a test of a
    /// window's paste.
    #[cfg(test)]
    #[must_use]
    pub const fn wants_the_clipboard(&self) -> bool {
        self.paste_owed.is_some()
    }

    /// The orders waiting for the footer — for a test of a window's.
    #[cfg(test)]
    #[must_use]
    pub fn orders(&self) -> &[String] {
        &self.orders
    }

    /// The programs waiting to be started — for a test of a window's.
    #[cfg(test)]
    #[must_use]
    pub fn launches(&self) -> &[Launching] {
        &self.launches
    }

    /// A `Ui` with no compositor behind it: a connection over a socket pair
    /// nobody serves, and an inert surface — what a test hands a window.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests(texts: ronoctua_i18n::Texts) -> Self {
        let (ours, _theirs) = std::os::unix::net::UnixStream::pair().expect("a socket pair");
        let conn = Connection::from_socket(ours).expect("a connection over the pair");
        let surface = wl_surface::WlSurface::inert(conn.backend().downgrade());
        Self {
            texts: std::rc::Rc::new(texts),
            width: 900,
            height: 600,
            scale: 1,
            columns: 80,
            lines: 24,
            margin: 1,
            message: None,
            quit: false,
            owed: false,
            offset: 0,
            hinted: crate::chrome::Hinted::default(),
            room: crate::chrome::Room::default(),
            conn,
            surface,
            pointer_seat: 0,
            many_pointers: false,
            keyboard_seat: 0,
            pointer_mods: Modifiers::default(),
            pointer_on: None,
            repeating: false,
            // A TEST'S WINDOW IS ONE ON THE SCREEN, sized and configured.
            configured: true,
            size_owed: false,
            asking: None,
            questions: 0,
            paste_owed: None,
            orders: Vec::new(),
            launches: Vec::new(),
        }
    }

    /// Hands command `id` — with its argument — to the footer, the way
    /// `ronoctua-shell command` does, OFF THE LOOP: the order goes down the
    /// control socket from a thread, and a footer that is not there is a
    /// sentence on this window's line (`door-no-footer`), never silence.
    ///
    /// A SECRET GOES ONLY WHERE IT IS FOR. A password for this window's own
    /// daemon never takes this path: the footer's line is read by another
    /// process, and such a secret stays in the one that typed it. The one
    /// secret it carries is a pairing's PIN or passkey, because the agent
    /// that answers bluetoothd IS the footer's — and on this path it is in
    /// no argv and in no log (the id is logged, never the argument).
    ///
    /// ONE LINE IS ONE ORDER: an id or an argument carrying a line break would
    /// be read as two, so it is not sent.
    pub fn tell_the_footer(&mut self, id: &str, argument: Option<&str>) {
        if id
            .chars()
            .chain(argument.unwrap_or_default().chars())
            .any(char::is_control)
        {
            tracing::warn!(id, "an order for the footer that is not one line; not sent");
            return;
        }
        let order = match argument {
            Some(a) => format!("command {id} {a}"),
            None => format!("command {id}"),
        };
        self.orders.push(order);
    }

    /// The window's own size changed (`Content::own_size`): lay it out again.
    pub const fn retake_the_size(&mut self) {
        self.size_owed = true;
        self.owed = true;
    }

    /// The first line of the list, pushed down by the header's drawer and the
    /// sentence about the shortcut under the pointer — and by the line the
    /// header is on, which is 1 in a column (`Content::header_line`).
    #[must_use]
    pub fn first_of_list(&self) -> u16 {
        FIRST_OF_LIST + self.hinted.top() + self.hinted.rows()
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
    /// A held button that may become — or has become — a sweep of text
    /// (`crate::text_select`).
    pub selecting: crate::text_select::Selecting,
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

    /// Puts a picture on the GPU under `key`, whether or not one is there —
    /// for a picture that CHANGES under the same key (a magnifier following
    /// the pointer). The texture is reused while the size stays the same.
    pub fn replace(&mut self, key: &str, rgba: &ronoctua_render::image::Rgba) {
        self.images
            .upload_it(&self.gpu.device, &self.gpu.queue, key, rgba);
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
    /// NOTHING ON SCREEN: a surface with no role, and the layer shell to give
    /// it one again — a column whose workspace no screen shows (`crate::column`).
    ///
    /// # Why a surface at all
    ///
    /// Because the door is built around one: the scale, the GPU and every
    /// handler read `ui.surface`, and a door with an `Option` there would be
    /// every door checking it. A surface with no role is never configured, so
    /// nothing is drawn to it (`Ui::may_draw`), no pointer or key reaches it,
    /// and the process and everything it keeps stay alive behind it.
    Parked(wl_surface::WlSurface, std::rc::Rc<LayerShell>),
}

impl Shell {
    /// The surface under it.
    #[must_use]
    pub fn wl_surface(&self) -> &wl_surface::WlSurface {
        match self {
            Self::Window { window, .. } => window.wl_surface(),
            Self::Layer(l, _) => l.wl_surface(),
            Self::Parked(surface, _) => surface,
        }
    }
}

/// What a door does when the compositor closes its layer surface
/// (`Content::layer_closed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// The door closes with it — every layer surface but a column.
    Quit,
    /// The door stays, off the screen (`Shell::Parked`): a screen that went
    /// away took the surface, not what the door was showing.
    Park,
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
    ///
    /// Read already as every window reads a key (`keys::as_read`): Caps Lock
    /// undone and `Delete` without its U+007F. [`keys::key_of`] turns it into
    /// the keymap's notation.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers);

    /// What was typed on the line [`Ui::ask`] opened, under the id that
    /// asked. The window does with it what the footer does with the same
    /// answer — the same call on the same view.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let _ = (ui, id, text);
    }

    /// The question `id` was let go — `Esc`, `C-g`, or a click on `ESC`.
    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        let _ = (ui, id);
    }

    /// The clipboard's text, read for [`Ui::paste`]. Empty when nothing — or
    /// nothing that is text — was copied.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let _ = (ui, text);
    }

    /// Whether this window TYPES TEXT where Caps Lock means capitals — a
    /// terminal, a buffer's editor. Every other window reads its keys with
    /// the Lock undone, as the footer does (`keys::as_read`); a chord is read
    /// without it in every window.
    fn types_capitals(&self) -> bool {
        false
    }

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

    /// The PAGE the window wears (`crate::theme::Page`): the palette solved
    /// on the chrome, or on the desk the person put OUR windows on
    /// (`LEADER s t` — `crate::theme::windows_page`). On a desk's page the
    /// chrome IS the desk and every band and ink was solved from it, so the
    /// window clears with `surface.chrome` and paints as it always did — the
    /// bands follow the ground. Every buffer it hosts wears it too. A right
    /// column answers the FOOTER's page: it is the footer's neighbour,
    /// chrome drawn in a strip (invariant 14), not a window.
    ///
    /// Asked at every frame and at every change of the theme's file, never
    /// kept: a preview and a commit reach the process the same way
    /// (`crate::theme::wear`), and the theme's watch repaints the door on
    /// either.
    fn page(&self) -> crate::theme::Page {
        crate::theme::windows_page()
    }

    /// The line the header is drawn on — `0`, in every window. A column
    /// answers `1`: its line 0 is its own control line (`crate::column`), and
    /// the header, the drawer and the list move down with it
    /// (`chrome::Hinted::at`, `Ui::first_of_list`). Read once, when the door
    /// is built.
    fn header_line(&self) -> u16 {
        0
    }

    /// The compositor closed the door's layer surface — a screen went away.
    /// Every layer surface closes with it; a column answers
    /// [`Closed::Park`] and stays, off the screen.
    fn layer_closed(&mut self, ui: &mut Ui) -> Closed {
        let _ = ui;
        Closed::Quit
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

    /// Where a sweep of text that starts on `(line, column)` of the surface
    /// may go — or `None` where a held button there is the window's own
    /// gesture (a drag of files, a turn of a shape, a film's timeline).
    ///
    /// The default is every window's list: the whole width under the header.
    /// A window of two texts side by side answers the half the cell is in.
    fn text_area(&self, ui: &Ui, line: u16, column: u16) -> Option<crate::text_select::Area> {
        let _ = column;
        let top = ui.first_of_list();
        (line >= top).then(|| crate::text_select::Area {
            left: 0,
            right: ui.columns + 2 * ui.margin,
            top,
            bottom: ui.lines,
        })
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

    /// The date's shape, the clock, the decimal mark, the units or the zone
    /// changed (`locale.json`), and `locale::prefs::current` already says so
    /// — every date this window draws from now on is written the new way. A
    /// window that keeps a copy of the file re-reads it here.
    fn locale_changed(&mut self, ui: &mut Ui) {
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
    /// The arrow each pointer is asked for on entering. See `crate::cursor_shape`.
    shapes: crate::cursor_shape::Shapes,
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
    /// The text the last frame drew, in cells — what a sweep copies
    /// (`crate::text_select`).
    painted: Vec<crate::text_select::Painted>,
    /// The text swept with the pointer, which `Ctrl+C` copies.
    selection: Option<crate::text_select::Selection>,
    _watches: Vec<ronoctua_files::notice::Notice>,
    /// The queue, for a window that makes Wayland objects of its own.
    pub qh: QueueHandle<Self>,
    /// What the door's own threads hand back — a paste, a footer that did
    /// not answer.
    back: channel::Sender<Back>,
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
    let header_line = content.header_line();

    let watches = hang_the_watches(&handle)?;

    // THE DOOR'S OWN THREADS answer here: a paste, an order the footer did
    // not take.
    let (back, came_back) = channel::channel::<Back>();
    handle
        .insert_source(came_back, |event, (), door: &mut Door<C>| {
            if let channel::Event::Msg(back) = event {
                door.came_back(back);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the door's channel: {e}"))?;

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
            hinted: crate::chrome::Hinted::at(header_line),
            room: crate::chrome::Room::default(),
            conn: conn.clone(),
            surface: shell.wl_surface().clone(),
            pointer_seat: 0,
            many_pointers: false,
            keyboard_seat: 0,
            pointer_mods: Modifiers::default(),
            pointer_on: None,
            repeating: false,
            // NOTHING SAID YET: the surface was just made.
            configured: false,
            size_owed: false,
            asking: None,
            questions: 0,
            paste_owed: None,
            orders: Vec::new(),
            launches: Vec::new(),
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
        shapes: crate::cursor_shape::Shapes::bind(&globals, &qh),
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
        painted: Vec::new(),
        selection: None,
        _watches: watches,
        qh: qh.clone(),
        back,
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

/// The theme, the language, the size of the text and the shape of a date,
/// watched — so a choice made anywhere reaches this window without it being
/// reopened.
///
/// # Why every door has all four
///
/// Because a door that adopted a choice only at its next start is the
/// incomplete preview the owner read off the screen when the theme did
/// exactly that. The language watch was missing from three doors; here it
/// cannot be. And the date's shape was read once, when the process started:
/// a clock turned to twelve hours in the footer left every open window
/// writing twenty-four — the language view's own window included, which then
/// wrote its stale copy back over the footer's choice.
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
                // THE PAGE AT ONCE, not at the next frame: a picture asked of
                // a child process before it (a waveform, a shape) is painted
                // with the page's values and keyed by them
                // (`ronoctua_tokens::theme::palette`).
                tokens::theme::set_ground(door.content.page());
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

    let (locale_sender, locale_changed) = channel::channel::<()>();
    kept.extend(crate::locale::prefs::watch(move || {
        locale_sender.send(()).is_ok()
    }));
    handle
        .insert_source(locale_changed, |event, (), door: &mut Door<C>| {
            if let channel::Event::Msg(()) = event {
                crate::locale::prefs::adopt();
                door.content.locale_changed(&mut door.ui);
                door.ui.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the locale channel: {e}"))?;
    Ok(kept)
}

impl<C: Content> Door<C> {
    /// The layer shell this door's column was made by, to make another.
    #[must_use]
    pub fn layer_shell(&self) -> Option<std::rc::Rc<LayerShell>> {
        match &self.shell {
            Shell::Layer(_, shell) | Shell::Parked(_, shell) => Some(shell.clone()),
            Shell::Window { .. } => None,
        }
    }

    /// The screen whose connector is `connector` (`eDP-1`), as this door's
    /// connection knows it — `None` until the compositor has described it,
    /// and then `Content::outputs_changed` is the moment to ask again.
    #[must_use]
    pub fn output_named(
        &self,
        connector: &str,
    ) -> Option<wayland_client::protocol::wl_output::WlOutput> {
        self.outputs.outputs().find(|o| {
            self.outputs
                .info(o)
                .and_then(|i| i.name)
                .is_some_and(|n| n == connector)
        })
    }

    /// Takes the door OFF the screen and keeps it: the surface's role is
    /// destroyed and a surface with none takes its place (`Shell::Parked`).
    /// Only a layer surface parks — a window has nowhere to be put away.
    pub fn park(&mut self) {
        if matches!(self.shell, Shell::Parked(..)) {
            return;
        }
        let Some(layer_shell) = self.layer_shell() else {
            return;
        };
        let surface = self._compositor.create_surface(&self.qh);
        self.reopen(Shell::Parked(surface, layer_shell));
    }

    /// The compositor state, for a window that makes a surface of its own.
    #[must_use]
    pub const fn compositor(&self) -> &CompositorState {
        &self._compositor
    }

    /// Puts the door on ANOTHER surface — a column shown on the screen of its
    /// workspace, moved to another, or parked (`crate::column`).
    ///
    /// A new surface is a new texture: the GPU, the text and the pictures are
    /// let go of and built again against it on the next frame, and the scale
    /// starts from one until the new screen says otherwise.
    ///
    /// The shell it replaces is dropped here, and its surface with it — a
    /// layer surface destroys its own. A PARKED surface is a bare one, which
    /// nothing else would ever destroy, unless it is the very surface the new
    /// shell gives a role to.
    ///
    /// WHAT THE POINTER AND THE KEYBOARD HELD goes with the old surface: a
    /// destroyed surface may never be told its pointer left, and a button, a
    /// drag or a modifier held across it would still be held on the new one.
    pub fn reopen(&mut self, shell: Shell) {
        self.here = Hand::default();
        self.mods = Modifiers::default();
        self.selection = None;
        self.fingers.cancel();
        self.ui.hinted.let_everything_go();
        let _ = self.ui.hinted.light_at(None);
        let _ = self.ui.hinted.say(None);
        self.images = None;
        self.quads = None;
        self.text = None;
        self.gpu = None;
        self.ui.surface = shell.wl_surface().clone();
        let old = std::mem::replace(&mut self.shell, shell);
        if let Shell::Parked(bare, _) = old
            && &bare != self.shell.wl_surface()
        {
            bare.destroy();
        }
        self.ui.scale = 1;
        self.scale_told = 1;
        self.ui.width = 0;
        self.ui.height = 0;
        // AND THE NEW SURFACE WAITS FOR ITS OWN FIRST CONFIGURE — the old
        // one's says nothing about it (`Ui::may_draw`).
        self.ui.configured = false;
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
                // OPAQUE: only the footer's hidden modes are see-through.
                false,
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

    /// Draws the owed frame, if one is owed — and starts what the turn asked
    /// of the door's threads: a paste, an order for the footer.
    fn settle(&mut self) {
        if let Some(to) = self.ui.paste_owed.take() {
            self.read_the_clipboard(to);
        }
        for launching in std::mem::take(&mut self.ui.launches) {
            self.start(launching);
        }
        for order in std::mem::take(&mut self.ui.orders) {
            // THE LAST ORDERS ARE SENT HERE, and not from a thread: the loop
            // stops at the end of this turn and the process with it, and a
            // thread would die before it connected — an order given as the
            // window closes would be lost without a word.
            if self.ui.quit {
                if let Err(e) = crate::leader::send_it(&order) {
                    tracing::warn!(error = %e, "the footer did not take the last order");
                }
            } else {
                self.send_to_the_footer(order);
            }
        }
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
        // NOT BEFORE THE FIRST CONFIGURE, whatever size is already laid out:
        // a frame is a buffer committed, and one committed to a surface the
        // compositor has not configured closes the connection. The configure
        // lays the size out again and asks for this frame (`take_the_size`).
        if !self.ui.may_draw() {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let lit = self.lit();
        self.bars.clear();
        // THE PAGE THIS DOOR WEARS, worn for the frame: every colour below
        // resolves through it, and its chrome is the ground.
        tokens::theme::set_ground(self.content.page());
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
        // WHAT WAS DRAWN, for a sweep to copy — and the sweep itself, over the
        // letters it covers (`crate::text_select`).
        self.painted = crate::text_select::painted_of(&spans);
        if let Some(sel) = &self.selection {
            let f = self.ui.frame(0);
            over.extend(crate::text_select::highlight(
                sel,
                f.cell(),
                f.line(),
                |l| f.y_of(l),
            ));
        }
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
            tokens::color::SURFACE_CHROME.to_linear(),
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
        // EVERY WINDOW READS A KEY THE SAME WAY — see `keys::as_read`.
        let (event, mods) = keys::as_read(event, mods, self.content.types_capitals());
        // A SWEEP OF TEXT IS THE DOOR'S, and so are the two keys that answer
        // it: `Ctrl+C` copies it and `Esc` lets it go. Asked before the
        // window, and only while there is a sweep — every other `Esc` and
        // every other `C-c` is still the window's.
        if let Some(sel) = self.selection {
            if event.keysym == Keysym::Escape {
                self.selection = None;
                self.ui.ask_paint();
                return;
            }
            if mods.ctrl && !mods.alt && matches!(event.keysym, Keysym::c | Keysym::C) {
                let text = sel.extract(&self.painted);
                if !text.is_empty() {
                    self.ui.message = Some(crate::chrome::copied(&text, &self.ui.texts));
                }
                self.ui.ask_paint();
                return;
            }
        }
        // A QUESTION ON THE LINE TAKES EVERY KEY — the footer's
        // `minibuffer_key` intercept, in every window.
        if self.ui.asking.is_some() {
            self.line_key(&event, mods);
            return;
        }
        self.content.typed(&mut self.ui, &event, mods);
        self.follow();
    }

    /// A key while the line asks — [`Ui::line_key`], and what the window
    /// is told of the answer.
    fn line_key(&mut self, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = keys::key_of(event, mods) else {
            return;
        };
        match self.ui.line_key(key) {
            Routed::Kept => {}
            Routed::Answered(id, text) => {
                self.content.answered(&mut self.ui, id, &text);
                self.follow();
            }
            Routed::Cancelled(id) => self.content.unanswered(&mut self.ui, id),
        }
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
    /// of the registry is handed to the footer, which is where commands run —
    /// off the loop, and a footer that is not there says so
    /// ([`Ui::tell_the_footer`]).
    fn act(&mut self, act: crate::chrome::Act) {
        match act {
            crate::chrome::Act::Key(key) => {
                let (event, mods) = keys::event_of(key);
                self.typed(&event, mods);
                // A CLICK IS A PRESS AND A RELEASE. A window that acts on
                // the release — the buffer host's `Esc`, which is a tap only
                // once it comes up, because held it chooses a state — heard
                // the press alone: the Back button did nothing over a buffer
                // window, and left its `Esc` held for the next key.
                self.content.released(&mut self.ui, &event);
            }
            crate::chrome::Act::Run(id) => self.ui.tell_the_footer(id, None),
        }
        self.ui.ask_paint();
    }

    /// THE BACK BUTTON IS `Esc`, by the path the `Esc` key takes: the
    /// header's drawer shuts first, and then the window's own `Esc` — the
    /// line's, when a question is up.
    fn went_back(&mut self) {
        if self.header_took(Keysym::Escape, true) {
            self.ui.ask_paint();
            return;
        }
        if let Ok(esc) = ronoctua_keymap::Key::parse("ESC") {
            self.act(crate::chrome::Act::Key(esc));
        }
    }

    /// What one of the door's threads handed back.
    fn came_back(&mut self, back: Back) {
        match back {
            Back::Pasted { to, text } => {
                let text = match text {
                    Ok(text) => text,
                    Err(e) => {
                        tracing::warn!(error = %e, "the clipboard could not be read");
                        self.ui.message =
                            Some(self.ui.texts.t(ronoctua_i18n::key::DOOR_PASTE_FAILED));
                        self.ui.ask_paint();
                        return;
                    }
                };
                if self.ui.pasted_into_the_line(to, &text) == Some(false) {
                    self.content.pasted(&mut self.ui, &text);
                    self.follow();
                    self.ui.ask_paint();
                }
            }
            Back::Unheard => {
                self.ui.message = Some(self.ui.texts.t(ronoctua_i18n::key::DOOR_NO_FOOTER));
                self.ui.ask_paint();
            }
            Back::Launched(verdict) => {
                if let Some(said) = launch_said(&verdict, &self.ui.texts) {
                    self.ui.message = Some(said);
                    self.ui.ask_paint();
                }
            }
            Back::NotInLauncher => {
                self.ui.message = Some(
                    self.ui
                        .texts
                        .t(ronoctua_i18n::key::SOFTWARE_NOT_IN_LAUNCHER),
                );
                self.ui.ask_paint();
            }
        }
    }

    /// Starts what a window asked to start, on a thread — and the verdict
    /// comes back on the door's channel.
    fn start(&mut self, launching: Launching) {
        let back = self.back.clone();
        match launching {
            Launching::Program { program, args, cwd } => {
                let started = crate::foreign::start_in_scope(
                    "door-launch",
                    &program,
                    &args,
                    cwd.as_deref(),
                    move |verdict| {
                        let _ = back.send(Back::Launched(verdict));
                    },
                );
                if let Err(e) = started {
                    tracing::error!(error = %e, program, "nothing opened");
                    let failed = crate::foreign::Launch::Failed {
                        program,
                        error: e.to_string(),
                    };
                    self.ui.message = launch_said(&failed, &self.ui.texts);
                    self.ui.ask_paint();
                }
            }
            Launching::Entry(desktop_id) => {
                let language = self.ui.texts.language().to_owned();
                crate::apps::launch_entry(&desktop_id, &language, move |opened| {
                    let _ = back.send(match opened {
                        crate::apps::Opened::Started(verdict) => Back::Launched(verdict),
                        crate::apps::Opened::NotInLauncher => Back::NotInLauncher,
                    });
                });
            }
        }
    }

    /// The clipboard's text, read on a thread with a deadline
    /// (`crate::pasting`) and handed back through the door's channel.
    fn read_the_clipboard(&self, to: PasteFor) {
        let back = self.back.clone();
        crate::pasting::text_then(move |text| {
            let _ = back.send(Back::Pasted { to, text });
        });
    }

    /// One order down the footer's control socket, from a thread: a connect
    /// to a footer whose listener is stuck would otherwise stop this loop.
    fn send_to_the_footer(&self, order: String) {
        let back = self.back.clone();
        let born = std::thread::Builder::new()
            .name("door-order".to_owned())
            .spawn(move || {
                if let Err(e) = crate::leader::send_it(&order) {
                    tracing::warn!(error = %e, "the footer did not take the order");
                    let _ = back.send(Back::Unheard);
                }
            });
        if let Err(e) = born {
            tracing::error!(error = %e, "could not start the thread that tells the footer");
        }
    }

    /// One pointer's frame — from a mouse, or from a finger played as one.
    ///
    /// A press on a shortcut of the header is taken HERE, for every window:
    /// the header is the system's, and so is what a click on it does.
    fn pointer_frame_here(&mut self, events: &[PointerEvent]) {
        use smithay_client_toolkit::seat::pointer::PointerEventKind;
        let mut rest: Vec<PointerEvent> = Vec::with_capacity(events.len());
        // WHAT THE HEADER'S CONTROLS WERE ASKED — `None` is the Back button.
        let mut acts: Vec<Option<crate::chrome::Act>> = Vec::new();
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
            // A SWEEP OF TEXT: a press ARMS it (and is still the window's
            // click), a held button a cell away STARTS it, and from then on
            // the motion is the sweep's and not the window's. Never on a bar,
            // which is grabbed where it is drawn, and never where the window
            // says a held button is its own gesture (`Content::text_area`).
            match e.kind {
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if self.selection.take().is_some() {
                        self.ui.ask_paint();
                    }
                    self.here.selecting = crate::text_select::Selecting::Idle;
                    if !self.bars.on(e.position.0, e.position.1)
                        && let Some((line, cell)) =
                            crate::text_select::Selecting::cell_of(e.position.0, e.position.1)
                        && let Some(area) = self.content.text_area(&self.ui, line, cell)
                        && area.holds(line, cell)
                    {
                        self.here.selecting = crate::text_select::Selecting::Pressed {
                            at: (line, cell),
                            area,
                        };
                    }
                }
                PointerEventKind::Motion { .. } if self.here.held => {
                    if let Some((line, cell)) =
                        crate::text_select::Selecting::cell_of(e.position.0, e.position.1)
                        && let Some(sel) = self.here.selecting.moved_to(line, cell)
                    {
                        if self.selection != Some(sel) {
                            self.selection = Some(sel);
                            self.ui.ask_paint();
                        }
                        continue;
                    }
                }
                PointerEventKind::Release { .. } | PointerEventKind::Leave { .. } => {
                    let swept = self.here.selecting.sweeping();
                    self.here.selecting = crate::text_select::Selecting::Idle;
                    // THE RELEASE OF A SWEEP IS NOT A CLICK of the window.
                    if swept && matches!(e.kind, PointerEventKind::Release { .. }) {
                        continue;
                    }
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
                acts.push(Some(act));
                continue;
            }
            // THE BACK BUTTON, over any part of the window — see `went_back`.
            if let PointerEventKind::Press { button, .. } = e.kind
                && button == BACK_BUTTON
            {
                acts.push(None);
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
            match act {
                Some(act) => self.act(act),
                None => self.went_back(),
            }
        }
    }

    /// A keysym that belongs to the header, when the window lets it have them
    /// — see [`Ui::header_took`].
    fn header_took(&mut self, sym: Keysym, down: bool) -> bool {
        let its = self.content.header_keys();
        self.ui.header_took(its, sym, down)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Line;
    use ronoctua_keymap::Key;

    fn ui() -> Ui {
        Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"))
    }

    fn key(notation: &str) -> Key {
        Key::parse(notation).expect(notation)
    }

    /// EVERY KEY IS THE LINE'S while it asks, and the answer leaves the line
    /// before the window hears it — under the id that asked.
    #[test]
    fn a_question_takes_every_key_and_answers_under_its_id() {
        let mut ui = ui();
        ui.message = Some("an earlier order said this".into());
        ui.ask(Line::new("software-web-new", false, Some("exa")));
        assert!(ui.message.is_none(), "the question takes the line");
        for k in ["m", "p", "l", "e", "DEL"] {
            assert_eq!(ui.line_key(key(k)), Routed::Kept, "{k}");
        }
        // A KEY OF THE VIEW UNDER IT is the line's too: `TAB` does nothing.
        assert_eq!(ui.line_key(key("TAB")), Routed::Kept);
        assert_eq!(
            ui.line_key(key("RET")),
            Routed::Answered("software-web-new", "exampl".into())
        );
        assert!(ui.asking.is_none());
        // AND WITH NO QUESTION UP, nothing is the line's.
        assert_eq!(ui.line_key(key("x")), Routed::Kept);
    }

    #[test]
    fn esc_and_c_g_let_the_question_go() {
        for k in ["ESC", "C-g"] {
            let mut ui = ui();
            ui.ask(Line::new("software-icon-path", false, None));
            assert_eq!(
                ui.line_key(key(k)),
                Routed::Cancelled("software-icon-path"),
                "{k}"
            );
            assert!(ui.asking.is_none(), "{k}");
        }
    }

    /// FAILS IF: a window starts a program on its loop, hands an installed
    /// program to the footer, or keeps quiet when a start fails — the footer
    /// says `echo-no-opened` on its line, and so does every window on its own.
    #[test]
    fn a_window_launches_off_the_loop_and_says_what_failed() {
        let mut ui = ui();
        ui.launch("/bin/gimp", &["-n".to_owned()], Some("/srv"));
        ui.launch_entry("org.gimp.GIMP.desktop");
        assert_eq!(
            ui.launches(),
            [
                Launching::Program {
                    program: "/bin/gimp".to_owned(),
                    args: vec!["-n".to_owned()],
                    cwd: Some("/srv".to_owned()),
                },
                Launching::Entry("org.gimp.GIMP.desktop".to_owned()),
            ]
        );
        assert!(ui.orders().is_empty(), "the footer was asked to open it");
        let failed = crate::foreign::Launch::Failed {
            program: "/bin/gimp".to_owned(),
            error: "No such file or directory".to_owned(),
        };
        let said = launch_said(&failed, &ui.texts).expect("a failure is said");
        assert!(said.contains("/bin/gimp"), "{said}");
        let opened = crate::foreign::Launch::WithScope {
            scope: "a-scope".to_owned(),
            program: "/bin/gimp".to_owned(),
        };
        assert_eq!(launch_said(&opened, &ui.texts), None);
    }

    /// `C-v` ASKS THE DOOR for the clipboard, and what comes back is typed
    /// into THAT question — never into one asked since.
    #[test]
    fn a_paste_goes_into_the_question_it_was_read_for() {
        let mut ui = ui();
        ui.ask(Line::new("software-file-path", false, None));
        assert!(!ui.wants_the_clipboard());
        assert_eq!(ui.line_key(key("C-v")), Routed::Kept);
        assert!(ui.wants_the_clipboard());
        let to = ui.paste_owed.take().expect("a paste owed");
        assert_eq!(
            ui.pasted_into_the_line(to, "/tmp/a b.AppImage\n"),
            Some(true)
        );
        assert_eq!(
            ui.asking.as_ref().map(|l| l.text.as_str()),
            Some("/tmp/a b.AppImage")
        );
        // A LATE PASTE, for a question replaced since, is dropped.
        ui.line_key(key("C-y"));
        let late = ui.paste_owed.take().expect("a paste owed");
        ui.ask(Line::new("software-web-name", false, None));
        assert_eq!(ui.pasted_into_the_line(late, "stale"), None);
        assert_eq!(ui.asking.as_ref().map(|l| l.text.as_str()), Some(""));
        // AND THE WINDOW'S OWN PASTE is the window's.
        ui.paste();
        let mine = ui.paste_owed.take().expect("a paste owed");
        assert_eq!(ui.pasted_into_the_line(mine, "gimp"), Some(false));
        assert_eq!(ui.asking.as_ref().map(|l| l.text.as_str()), Some(""));
    }

    /// THE QUESTION ON LINE 0: what is asked, what is typed — masked when it
    /// is a secret — and `ESC` and `RET` as CLICKS that press those keys.
    #[test]
    fn the_question_is_drawn_with_esc_and_ret_as_clicks() {
        let mut ui = ui();
        assert!(list::the_question(&ui).is_none());
        ui.ask(Line::new("network-connect", true, Some("s3nh4")));
        let pieces = list::the_question(&ui).expect("a question on the line");
        let said: String = pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(!said.contains("s3nh4"), "the secret was drawn: {said}");
        assert!(said.contains("•••••"), "{said}");
        assert!(said.contains(&ui.texts.t("ask-network-connect")), "{said}");
        for (notation, id) in crate::line::OF_THE_LINE {
            let k = key(notation);
            assert!(
                pieces
                    .iter()
                    .any(|p| p.id == Some(*id) && p.press == Some(crate::chrome::Act::Key(k))),
                "{notation} is not a click on the line"
            );
        }
    }

    /// THE END OF WHAT IS TYPED STAYS ON THE LINE, however long it gets: it
    /// is where the next letter goes.
    #[test]
    fn a_long_answer_keeps_its_end_on_the_line() {
        let mut ui = ui();
        let long = format!("https://example.org/{}/end", "a".repeat(200));
        ui.ask(Line::new("software-web-new", false, Some(&long)));
        let pieces = list::the_question(&ui).expect("a question");
        let typed = pieces
            .iter()
            .find(|p| p.register == crate::chrome::Register::Said)
            .expect("the typed text");
        assert!(typed.text.ends_with("/end"), "{}", typed.text);
        assert!(
            ronoctua_render::layout::width(&typed.text) <= ui.columns,
            "{}",
            typed.text
        );
    }

    /// FAILS IF: a door may draw — commit a buffer — before the compositor
    /// configured its surface. A window that OWNS its size (the player's
    /// corner, `ronoctua-shell player pip`) lays it out on its first turn,
    /// before any configure; a door that took "it has a size" for "it was
    /// configured" committed that frame, and the compositor closed the
    /// connection: `xdg_surface has never been configured` (player-vm).
    #[test]
    fn nothing_is_drawn_before_the_first_configure() {
        let mut ui = ui();
        // AS `run` BUILDS IT: a surface just made, nothing said about it.
        (ui.configured, ui.width, ui.height) = (false, 0, 0);
        assert!(!ui.may_draw());
        // ITS OWN SIZE, laid out on the first turn as `take_the_size` does.
        (ui.width, ui.height) = (384, 216);
        ui.recount();
        assert!(ui.columns > 0 && ui.lines > 0, "the size was not laid out");
        assert!(!ui.may_draw(), "a frame before the first configure");
        // THE CONFIGURE, acked by the toolkit before the door hears it.
        ui.configured = true;
        assert!(ui.may_draw());
    }

    /// FAILS IF: a right `Ctrl` let go while the header did not have its
    /// keys — the column in its bar — is still held once the header has them
    /// again, so a plain `Down` opens the drawer instead of reaching the
    /// window; or one pressed there is not held when the header comes back.
    #[test]
    fn the_right_ctrl_is_followed_while_the_header_is_away() {
        let mut ui = ui();
        // HELD at a size with a header: the header's key.
        assert!(ui.header_took(true, Keysym::Control_R, true));
        // LET GO IN THE BAR, which has no header: nobody takes it.
        assert!(!ui.header_took(false, Keysym::Control_R, false));
        // BACK TO A SIZE WITH A HEADER: a plain `Down` is the window's.
        assert!(
            !ui.header_took(true, Keysym::Down, true),
            "a plain Down walked the drawer"
        );
        assert!(!ui.hinted.is_open());
        // PRESSED IN THE BAR, where the window still reads it, it is held
        // once the header comes back.
        assert!(!ui.header_took(false, Keysym::Control_R, true));
        assert!(ui.header_took(true, Keysym::Down, true));
        assert!(ui.hinted.is_open(), "the drawer did not open");
    }

    /// AN ORDER FOR THE FOOTER is the control socket's own line — the one
    /// `ronoctua-shell command` writes — sent on the next turn, off the loop.
    #[test]
    fn an_order_for_the_footer_is_the_command_line() {
        let mut ui = ui();
        ui.tell_the_footer("software-launch-latest", Some("org.gimp.GIMP"));
        ui.tell_the_footer("files-window", None);
        // A LINE BREAK would be a second order, read by the footer as one.
        ui.tell_the_footer("software-launch-latest", Some("x\ncommand quit"));
        assert_eq!(
            ui.orders(),
            [
                "command software-launch-latest org.gimp.GIMP".to_owned(),
                "command files-window".to_owned()
            ]
        );
    }
}
