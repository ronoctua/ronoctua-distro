//! The file manager's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
//! invariant 14) and it is right that it does: it is a footer. Twelve rows are
//! enough to look something up and act on it, and not enough for the job people
//! open a file manager to do.
//!
//! So this exists — and everything it knows lives in [`crate::files::View`],
//! which the footer draws from too. What this module holds is a window, a GPU
//! and the events of both; what it decides is nothing. A second implementation
//! of "what does `d` do here" is how two doors come to disagree, and it is the
//! shape the rescue console already refused once.
//!
//! # Why it is not a wasm buffer
//!
//! Because a wasip2 component has no filesystem — invariant 11, and it is
//! structural: the `world` does not import one. A file manager as a buffer
//! would need a capability of its own, three new `input` cases for hover, the
//! secondary button and the wheel, and a whole directory crossing the boundary
//! by value on every change. `ronoctua-shell terminal` is already a native
//! window of exactly this shape, and that is the precedent this follows.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::{EventLoop, channel},
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

/// The window's `app_id`.
///
/// Its own, like the terminal's and the monitor's: it is by this that the
/// modeline and the VM test tell one window from another, and a shared id would
/// make two different things look like one in niri's IPC.
pub const APP_ID: &str = "br.ronoctua.files";

/// `BTN_LEFT` and `BTN_RIGHT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;
const RIGHT_BUTTON: u32 = 0x111;

/// The first line of the listing. Above it: the path, and the message line.
///
/// The same arrangement as the footer — two lines glued, content below — and
/// repeated here deliberately: a person should not have to learn a second
/// shape for the same thing.
const FIRST_OF_LIST: u16 = 2;

/// Opens the manager in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(start: Option<&std::path::Path>) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("nao ha compositor Wayland acessivel (WAYLAND_DISPLAY esta definida?)")?;
    let (globals, fila) = registry_queue_init(&conn).context("registry do Wayland")?;
    let qh: QueueHandle<FilesWindow> = fila.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("sem wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("o compositor nao expoe xdg_shell")?;

    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let preferred = std::env::var("RONOCTUA_FILES_START").ok();
    let where_ = start.map_or_else(
        || ronoctua_files::listing::start_at(home.as_deref(), preferred.as_deref()),
        std::path::Path::to_path_buf,
    );

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    // `*name*` is Emacs's convention for a buffer that is not a file, the same
    // one the modeline already uses for `*scratch*`.
    window.set_title("*arquivos*");
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<FilesWindow> = EventLoop::try_new().context("calloop")?;
    let alca = cycle.handle();
    let (sender, reports) = channel::channel::<crate::files::Report>();
    // THE QUEUE, AS REPORTS. This door has no modeline and no panel: what it
    // needs from the transfer queue is "the thing you asked for is over, read
    // the directory again". Without it, `Errand::Done` would refresh the
    // listing at the instant the work was HANDED OVER — showing the directory
    // as it was before the copy, and never again.
    crate::transfers::listen_as_reports(sender.clone());
    alca.insert_source(reports, |event, (), state: &mut FilesWindow| {
        if let channel::Event::Msg(r) = event {
            state.reported(r);
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal de arquivos: {e}"))?;

    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    // The catalogue is swept HERE and not inherited: this is a process of its
    // own, and `open with` needs to know who declares a type. 128 entries in
    // 68 ms, measured — once, at startup, like the launcher's.
    let catalogue = std::sync::Arc::new(ronoctua_apps::scan(&language()));
    let data = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from);
    let view = crate::files::View::at(
        &where_,
        ronoctua_files::who::Roster::read_from(&ronoctua_files::who::Files::default()),
        ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        ronoctua_files::trash::Bin::of_home(data.as_deref(), home.as_deref()),
        catalogue.clone(),
        texts.clone(),
    );

    let mut state = FilesWindow {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointer: None,
        mods: Modifiers::default(),
        window,
        width: 0,
        height: 0,
        columns: 80,
        lines: 24,
        quit: false,
        gpu: None,
        text: None,
        quads: None,
        view,
        catalogue,
        texts,
        sender,
        row_under_pointer: None,
        cell_under_pointer: None,
        offset: 0,
        message: None,
    };

    WaylandSource::new(conn, fila)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("nao consegui pendurar o Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            if e.quit {
                signal.stop();
            }
        })
        .context("loop de eventos")?;
    Ok(())
}

/// The session's language, for the catalogue's localised names.
fn language() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| l.split('.').next().map(ToOwned::to_owned))
        .unwrap_or_else(|| "en_GB".to_owned())
}

/// The window.
struct FilesWindow {
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    mods: Modifiers,
    window: Window,
    width: u32,
    height: u32,
    columns: u16,
    lines: u16,
    quit: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    view: crate::files::View,
    catalogue: crate::apps::Catalogue,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    sender: channel::Sender<crate::files::Report>,
    row_under_pointer: Option<usize>,
    cell_under_pointer: Option<u16>,
    /// The first visible row. A window has no page: it scrolls, like the
    /// locator and the manual.
    offset: usize,
    message: Option<String>,
}

impl FilesWindow {
    /// How many rows of listing fit.
    ///
    /// One less than the region, because the header takes a line and it is
    /// drawn from the same arithmetic the hit test uses — a room that counted
    /// the header would put the last row under the window's edge.
    const fn room(&self) -> usize {
        (self.lines.saturating_sub(FIRST_OF_LIST + 1)) as usize
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
    ///
    /// `SCROLL_MARGIN` two, the same as the footer's — it is vi's `scrolloff`,
    /// and the reason is that a cursor pinned to the last visible line gives no
    /// warning of what is coming.
    fn follow(&mut self) {
        const MARGIN: usize = 2;
        let room = self.room();
        if room == 0 {
            return;
        }
        let cursor = self.view.cursor();
        if cursor < self.offset + MARGIN {
            self.offset = cursor.saturating_sub(MARGIN);
        } else if cursor + MARGIN >= self.offset + room {
            self.offset = cursor + MARGIN + 1 - room;
        }
        let last = self.view.amount().saturating_sub(room);
        self.offset = self.offset.min(last);
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, step: crate::files::Step) {
        use crate::files::{Doing, Errand, Step};
        match step {
            Step::Nothing => return,
            Step::Changed => {}
            Step::Closed => {
                self.quit = true;
                return;
            }
            Step::Say(e) => {
                self.message = Some(self.texts.t_with(e.key, &e.borrowed()));
            }
            Step::Do(doing) => {
                // THE FOUR THAT SPAWN are the door's business; everything else
                // is the same code the footer runs.
                match doing {
                    Doing::Open(path) => {
                        self.open_the_file(&path);
                        self.paint();
                        return;
                    }
                    Doing::OpenWith(path, which) => {
                        if let Some(entry) = self.catalogue.iter().find(|e| e.id == which).cloned()
                        {
                            let (program, args) = crate::apps::command_for(&entry, &path);
                            self.spawn(&program, &args, entry.working_directory.as_deref());
                        }
                        self.paint();
                        return;
                    }
                    Doing::Terminal(dir) => {
                        if let Ok(me) = std::env::current_exe() {
                            self.spawn(
                                &me.to_string_lossy(),
                                &["terminal".to_owned()],
                                Some(&dir.to_string_lossy()),
                            );
                        }
                        self.paint();
                        return;
                    }
                    Doing::RootTerminal(dir) => {
                        self.root_terminal(&dir);
                        self.paint();
                        return;
                    }
                    Doing::Copy(text) => {
                        let ok = crate::buffer::copy_to_clipboard(&text).is_ok();
                        self.message = Some(self.texts.t(if ok {
                            ronoctua_i18n::key::FILES_COPIED
                        } else {
                            ronoctua_i18n::key::FILES_NOT_COPIED
                        }));
                        self.paint();
                        return;
                    }
                    other => {
                        let clip = self.view.clip().clone();
                        if matches!(other, Doing::Paste(_)) {
                            self.view.clip_used();
                        }
                        match crate::files::carry_out(other, &self.sender, &clip) {
                            Errand::Done => self.view.reread(),
                            Errand::Say(e) => {
                                self.message = Some(self.texts.t_with(e.key, &e.borrowed()));
                            }
                        }
                    }
                }
            }
        }
        self.follow();
        self.paint();
    }

    /// Opens a file with whatever handles its type.
    fn open_the_file(&mut self, path: &std::path::Path) {
        let of_type = ronoctua_files::mime::of_path(path, ronoctua_files::Kind::File, true);
        let Some(entry) = crate::apps::opens(&self.catalogue, of_type).cloned() else {
            self.message = Some(self.texts.t_with(
                ronoctua_i18n::key::FILES_NOTHING_OPENS,
                &[("type", of_type)],
            ));
            return;
        };
        let (program, args) = crate::apps::command_for(&entry, path);
        self.spawn(&program, &args, entry.working_directory.as_deref());
    }

    /// A root shell in this directory, through polkit.
    fn root_terminal(&mut self, dir: &std::path::Path) {
        let (Ok(me), Some(pkexec), Some(shell)) = (
            std::env::current_exe(),
            option_env!("RONOCTUA_PKEXEC"),
            option_env!("RONOCTUA_RESCUE_SHELL"),
        ) else {
            self.message = Some(self.texts.t(ronoctua_i18n::key::FILES_NO_PKEXEC));
            return;
        };
        self.spawn(
            &me.to_string_lossy(),
            &[
                "terminal".to_owned(),
                pkexec.to_owned(),
                "--keep-cwd".to_owned(),
                shell.to_owned(),
            ],
            Some(&dir.to_string_lossy()),
        );
    }

    /// Starts a program, in a scope of its own.
    ///
    /// Through `foreign::launch` and a NAMED thread: the manual is explicit
    /// that `systemd-run --scope` "is synchronous, and will return only when
    /// the command finishes". Waiting here would freeze this window for the
    /// whole life of whatever it opened — the defect the app launcher already
    /// paid for once.
    fn spawn(&mut self, program: &str, args: &[String], cwd: Option<&str>) {
        let Ok(scope) = crate::foreign::scope_name() else {
            tracing::error!("sem nome de scope; nada aberto");
            return;
        };
        let cmd = crate::foreign::scope_command(&scope, cwd, program, args);
        // `direct` is what runs when there is no `systemd-run` — the nested
        // loop's case. It is the same command without the wrapper.
        let mut direct = vec![program.to_owned()];
        direct.extend(args.iter().cloned());
        if let Err(e) = std::thread::Builder::new()
            .name("arquivos-abrir".to_owned())
            .spawn(move || {
                let _ = crate::foreign::launch(&scope, &direct, &cmd);
            })
        {
            tracing::error!(error = %e, "nao consegui subir a thread de abrir");
        }
    }

    /// What a thread came back with.
    fn reported(&mut self, report: crate::files::Report) {
        match report {
            crate::files::Report::Done => self.view.reread(),
            crate::files::Report::Measured(name, bytes) => self.view.measured(&name, bytes),
            crate::files::Report::Failed(why) => self.message = Some(why),
        }
        self.paint();
    }

    /// Columns and rows from the size in pixels.
    ///
    /// Integer arithmetic, without measuring text: the cell is exactly 0.600 ×
    /// the font size, including in the Nerd Fonts.
    fn recount(&mut self) {
        let cell = tokens::font::SIZE_LARGE * tokens::font::CELL_WIDTH_EM;
        let line = tokens::metrics::line_height(tokens::font::SIZE_LARGE);
        #[allow(clippy::cast_precision_loss)]
        let width = self.width as f32 - 2.0 * cell * f32::from(CONTENT_MARGIN);
        #[allow(clippy::cast_precision_loss)]
        let height = self.height as f32;
        self.columns = if cell > 0.0 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = (width / cell).floor().max(1.0) as u16;
            n
        } else {
            1
        };
        self.lines = if line > 0.0 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = (height / line).floor().max(1.0) as u16;
            n
        } else {
            1
        };
        self.follow();
    }

    /// Which row and cell a pointer position lands on.
    fn at(&self, x: f64, y: f64) -> (Option<usize>, Option<u16>) {
        let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_LARGE));
        let cell_px = f64::from(tokens::font::SIZE_LARGE * tokens::font::CELL_WIDTH_EM);
        if line_px <= 0.0 || cell_px <= 0.0 || x < 0.0 || y < 0.0 {
            return (None, None);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_px) as u16;
        let margin = f64::from(CONTENT_MARGIN) * cell_px;
        let cell = u16::try_from(((x - margin).max(0.0) / cell_px) as u64).unwrap_or(0);
        // The header's own line, which is a row of the table and not of the
        // listing: `usize::MAX` is how the click handler tells them apart,
        // exactly as in the footer.
        if line == FIRST_OF_LIST {
            return (Some(usize::MAX), Some(cell));
        }
        if line < FIRST_OF_LIST {
            return (None, Some(cell));
        }
        let row = self.offset + usize::from(line - FIRST_OF_LIST - 1);
        (Some(row), Some(cell))
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast::<std::ffi::c_void>()),
            std::ptr::NonNull::new(
                self.window
                    .wl_surface()
                    .id()
                    .as_ptr()
                    .cast::<std::ffi::c_void>(),
            ),
        ) else {
            tracing::error!("ponteiros do Wayland nulos");
            return;
        };
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what produced them.
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                self.width,
                self.height,
            )
        };
        let Ok(g) = gpu else {
            tracing::error!("sem GPU utilizavel; a janela nao vai desenhar");
            return;
        };
        // SIZE_LARGE and not SIZE_BASE, for the reason the buffer host gives:
        // the footer is peripheral reference and this is what one reads.
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            tokens::font::SIZE_LARGE,
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "montar o texto falhou"),
        }
        // The quad pipeline is what draws the focused row's background. Its
        // failure costs the highlight, not the window.
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "pipeline de quad falhou"),
        }
        self.gpu = Some(g);
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::font::SIZE_LARGE;
        let margin = CONTENT_MARGIN;
        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let mut quads: Vec<ronoctua_render::quad::Quad> = Vec::new();

        // LINE 0: where one is, as clickable pieces.
        for f in self.view.crumbs() {
            spans.push(ronoctua_render::Span {
                size: Some(size),
                text: f.text.clone(),
                column: margin + f.at,
                line: 0,
                color: f.color,
            });
        }

        // LINE 1: what is typed, what the pointer explains, or what went wrong.
        //
        // The pointer's explanation WINS, which is the same precedence the
        // footer's message line uses: pointing at something is a question, and
        // a question asked now beats a message from before.
        let explained = self
            .row_under_pointer
            .zip(self.cell_under_pointer)
            .and_then(|(row, cell)| {
                self.view
                    .explanation(row, cell, self.columns, &self.texts)
                    .map(|e| self.texts.t_with(e.key, &e.borrowed()))
            });
        // THE QUERY IS NOT HERE. It goes at the end of the path, on the line
        // above — see `files::View::crumbs`. This line is for what the pointer
        // is resting on and what the last action said, and while the query
        // shared it the two took turns erasing each other.
        // THE MESSAGE COMES FIRST, for the reason the footer's own precedence
        // now gives: it is what the manager has to SAY about an action just
        // asked for, and behind the pointer's description it was unreachable in
        // the one situation it exists for — the hand on the mouse, the pointer
        // resting on the row that was just clicked.
        let second = self
            .message
            .clone()
            .or(explained)
            // WITH NOTHING TYPED AND NOTHING TO SAY, the hint. Typing is not a
            // key and so has no line in the manual; without this the one
            // gesture the list is built around is the one thing nothing on
            // screen mentions.
            .or_else(|| (!self.view.filtering()).then(|| crate::files::hint(&self.texts)))
            .unwrap_or_default();
        if !second.is_empty() {
            spans.push(ronoctua_render::Span {
                size: Some(size),
                text: ronoctua_render::layout::elide_end(&second, self.columns),
                column: margin,
                line: 1,
                color: tokens::color::TEXT_SECONDARY,
            });
        }

        // THE HEADER, on the line above the listing. It names the columns and
        // says which one is sorting; clicking it sorts, as it does in the
        // footer. Not drawn over the menu, the picker or the detail sheet:
        // none of those has columns.
        let line_px = tokens::metrics::line_height(size);
        if !self.view.menu_open() && !self.view.showing_detail() {
            for f in &self.view.header(self.columns, &self.texts).fields {
                spans.push(ronoctua_render::Span {
                    size: Some(size),
                    text: f.text.clone(),
                    column: margin + f.at,
                    line: FIRST_OF_LIST,
                    color: f.color,
                });
            }
        }

        // THE LISTING — only the rows that fit, and not the whole directory.
        //
        // The same reason the footer door has: building a row costs a mode's
        // nine cells, a size, a date, an owner and a group, and this window
        // draws `room()` of them. See `files::View::rows_from` for the numbers.
        let rows = self
            .view
            .rows_from(self.offset, self.room(), self.columns, &self.texts);
        for (n, row) in rows.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let y = FIRST_OF_LIST + 1 + n as u16;
            // THE POINTER GETS THE BACKGROUND, and the focus gets the text's
            // colour — the same split the footer uses, and for the reason its
            // own note gives: marking both leaves "which one is selected?"
            // unanswered the moment the pointer rests on another row.
            //
            // A flat quad, no border and no radius, which is why it does not
            // violate invariant 15.
            if self.row_under_pointer == Some(self.offset + n) {
                quads.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(y) * line_px,
                    #[allow(clippy::cast_precision_loss)]
                    width: self.width as f32,
                    height: line_px,
                    color: tokens::color::SURFACE_SELECTION,
                });
            }
            for f in &row.fields {
                // THE FIELD UNDER THE POINTER LIGHTS UP, so the sentence on
                // the line above has a visible subject. A colour and not a
                // box — invariant 15.
                let under = self.row_under_pointer == Some(self.offset + n)
                    && self.cell_under_pointer.is_some_and(|c| {
                        c >= f.at && c < f.at + ronoctua_render::layout::width(&f.text)
                    });
                spans.push(ronoctua_render::Span {
                    size: Some(size),
                    text: f.text.clone(),
                    column: margin + f.at,
                    line: y,
                    color: if under {
                        tokens::color::STATE_ACCENT
                    } else {
                        f.color
                    },
                });
            }
        }

        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        if let Err(e) = text.prepare(
            &gpu.device,
            &gpu.queue,
            gpu.config.width,
            gpu.config.height,
            ronoctua_render::Atlas::Pinned,
            &spans,
        ) {
            tracing::error!(error = %e, "preparar texto falhou");
            return;
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(
                &gpu.device,
                &gpu.queue,
                gpu.config.width,
                gpu.config.height,
                &quads,
            );
        }
        let quads_ref = self.quads.as_ref();
        if let Err(e) = gpu.frame(|pass| {
            if let Some(q) = quads_ref {
                q.draw(pass);
            }
            text.draw(ronoctua_render::Atlas::Pinned, pass)
        }) {
            tracing::error!(error = %e, "desenhar o frame falhou");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "flush falhou");
        }
    }
}

/// Where the listing starts, in cells from the window's edge.
///
/// THE ROW ALREADY BRINGS ITS OWN FIRST CELL: `files::layout_for` starts every
/// row at the gutter — the mark's cell, and the air the icon needs. Adding the
/// buffer's whole margin on top of that indented this door by three cells and
/// the footer by one, for the same table.
///
/// What this window owes is the DIFFERENCE. And it is written once, because the
/// number is used three times — drawing, measuring how many columns fit, and
/// deciding which cell the pointer is over. Two of the three disagreeing is a
/// tooltip about the neighbouring field.
const CONTENT_MARGIN: u16 =
    tokens::space::BUFFER_MARGINX.saturating_sub(tokens::space::FILES_GUTTER);

/// A `KeyEvent` as the manager's keymap sees it.
///
/// The same mapping `Footer::translate` does, minus the leader's inheritance:
/// that exists because the footer receives the keyboard with the leader still
/// held down, and this window is given focus like any other.
fn key_of(event: &KeyEvent, mods: Modifiers) -> Option<ronoctua_keymap::Key> {
    use ronoctua_keymap::{Key, Sym};
    let sym = match event.keysym {
        Keysym::Escape => Sym::Esc,
        Keysym::Return | Keysym::KP_Enter => Sym::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab => Sym::Tab,
        Keysym::BackSpace => Sym::Backspace,
        Keysym::space => Sym::Space,
        Keysym::Left => Sym::Left,
        Keysym::Right => Sym::Right,
        Keysym::Up => Sym::Up,
        Keysym::Down => Sym::Down,
        otherwise => Sym::Char(otherwise.key_char()?),
    };
    Some(Key {
        mods: ronoctua_keymap::Mods {
            ctrl: mods.ctrl,
            alt: mods.alt,
            // SHIFT DOES NOT ENTER, for the reason `Footer::discount` gives:
            // xkb has already applied it when producing the character, so
            // counting both would turn `M-S` into `M-S-S` and never match. The
            // two doors have to answer the same key the same way, and this one
            // did not — it was invisible only while every key in the table was
            // a bare letter and the notation dropped the modifiers anyway.
            shift: false,
            logo: mods.logo,
        },
        sym,
    })
}

impl WindowHandler for FilesWindow {
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
        self.width = cfg.new_size.0.map_or(900, std::num::NonZeroU32::get);
        self.height = cfg.new_size.1.map_or(600, std::num::NonZeroU32::get);
        if let Some(g) = self.gpu.as_mut() {
            g.resize(self.width, self.height);
        }
        self.recount();
        self.paint();
    }
}

impl SeatHandler for FilesWindow {
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
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "sem teclado; nao da para digitar"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                Err(e) => tracing::warn!(error = %e, "sem ponteiro; so teclado"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if cap == Capability::Pointer
            && let Some(p) = self.pointer.take()
        {
            p.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for FilesWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        // The modifiers whole, for the reason the footer already records: a
        // state that only updates with focus cannot keep holding without it,
        // and a stuck `alt` makes every following key come out as `M-…`.
        self.mods = Modifiers::default();
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let Some(t) = key_of(&event, self.mods) else {
            return;
        };
        // A key clears the last message: it answered something that is no
        // longer what is being done.
        self.message = None;
        let step = self.view.key(t);
        self.apply(step);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // Holding `j` walks the list, which is what a listing wants — unlike
        // the footer, where repeating a leader key is always a mistake.
        self.press_key(conn, qh, kb, serial, event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        mods: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.mods = mods;
    }
}

impl PointerHandler for FilesWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = (self.row_under_pointer, self.cell_under_pointer);
        let mut left = None;
        let mut right = None;
        let mut wheel = 0.0f64;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    let (row, cell) = self.at(e.position.0, e.position.1);
                    self.row_under_pointer = row;
                    self.cell_under_pointer = cell;
                }
                PointerEventKind::Leave { .. } => {
                    self.row_under_pointer = None;
                    self.cell_under_pointer = None;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    let (row, cell) = self.at(e.position.0, e.position.1);
                    self.row_under_pointer = row;
                    self.cell_under_pointer = cell;
                    left = row;
                }
                // THE RIGHT BUTTON OPENS THE MENU. It draws in this window's own
                // region, has no surface of its own and does not float — see the
                // note in `surface.rs`, where the same decision is written out.
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    let (row, cell) = self.at(e.position.0, e.position.1);
                    self.row_under_pointer = row;
                    self.cell_under_pointer = cell;
                    right = row;
                }
                PointerEventKind::Axis { vertical, .. } => {
                    wheel += vertical.absolute;
                }
                _ => {}
            }
        }
        if let Some(row) = right {
            self.open_menu(row);
            return;
        }
        if let Some(row) = left {
            self.click(row);
            return;
        }
        if wheel.abs() >= 1.0 {
            let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_LARGE));
            #[allow(clippy::cast_possible_truncation)]
            let by = (wheel / line_px.max(1.0)) as i64 * 3;
            let last = self.view.amount().saturating_sub(self.room());
            self.offset = usize::try_from(
                i64::try_from(self.offset)
                    .unwrap_or(0)
                    .saturating_add(by)
                    .max(0),
            )
            .unwrap_or(0)
            .min(last);
            self.paint();
            return;
        }
        // Only redraws if something CHANGED: a `Motion` arrives on every pixel
        // of movement, and painting per pixel would spend the GPU drawing the
        // same thing.
        if before != (self.row_under_pointer, self.cell_under_pointer) {
            self.paint();
        }
    }
}

impl FilesWindow {
    /// A left click, on whatever cell it landed on.
    fn click(&mut self, row: usize) {
        // A CLICK CLEARS THE LAST MESSAGE, like a key does. This window has no
        // timer, so the message stays until something else is being done — and
        // with it drawn ABOVE the pointer's description (so a refusal cannot be
        // covered), "something else" has to include the mouse. Otherwise a
        // single error would own the line until a key was pressed.
        self.message = None;
        let cell = self.cell_under_pointer.unwrap_or(0);
        let found = if row == usize::MAX {
            self.view
                .header(self.columns, &self.texts)
                .field_at(cell)
                .map(|f| f.hit.clone())
        } else {
            self.view
                .rows_from(row, 1, self.columns, &self.texts)
                .first()
                .and_then(|r| r.field_at(cell))
                .map(|f| f.hit.clone())
        };
        let Some(hit) = found else {
            return;
        };
        // THROUGH THE SAME CODE THE KEY GOES THROUGH — the one thing that keeps
        // the mouse from reaching somewhere the keyboard cannot.
        let step = self.view.click(row, &hit);
        self.apply(step);
    }

    /// A right click: focus that row, then open the action menu.
    fn open_menu(&mut self, row: usize) {
        self.message = None;
        // The header has no menu: right-clicking a column name is not a
        // gesture about any file.
        if row == usize::MAX {
            return;
        }
        // FOCUS, AND ONLY FOCUS. See `files::View::focus_row`. With nothing to
        // focus — an empty directory — the menu opens anyway, about it.
        if matches!(self.view.focus_row(row), crate::files::Step::Nothing) && !self.view.is_empty()
        {
            return;
        }
        // THE MENU'S OWN KEY, read from the table. See `files::key_that_runs`.
        let Some(t) = crate::files::key_that_runs("files-menu") else {
            return;
        };
        let menu = self.view.key(t);
        self.apply(menu);
    }
}

impl CompositorHandler for FilesWindow {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
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

impl OutputHandler for FilesWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for FilesWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(FilesWindow);
smithay_client_toolkit::delegate_dispatch2!(FilesWindow);
