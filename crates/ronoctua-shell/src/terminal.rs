//! A terminal in an `xdg_toplevel` of its own — §7.5 and §7.10.
//!
//! # Where each piece lives
//!
//! ```text
//!   keyboard (Wayland)  ->  Pty::write_bytes  ->  process
//!                                                 |
//!   screen (wgpu)  <-  Grid  <-  Terminal  <-  Pty::pump
//!                       ^            ^
//!               VT thread ------------+  (section 7.10: `!Send` engine)
//! ```
//!
//! There are THREE threads and no lock: the compositor's (this one), the VT
//! engine's and the one that reads the PTY. Communication is by channel, and
//! the only meeting point is calloop's `ping`, which only wakes the loop — it
//! carries no data.
//!
//! # Why the window is an `xdg_toplevel`
//!
//! The same reason as the native buffer: invariant 4 says the shell never
//! reimplements window layout. A terminal is a window, and whoever positions
//! it is niri.
//!
//! # Why the drawing goes through the protocol
//!
//! The grid becomes a [`ronoctua_protocol::Command::Grid`] before becoming a
//! pixel, and not a direct path from `Grid` to `Span`. The WIT's `grid`
//! node exists precisely for this (§7.10, "from day one"): the day a wasip2
//! buffer asks for the `terminal` capability, it emits the same command and
//! falls into the same drawer. A shortcut here would be a second render path
//! to keep in sync.

use std::sync::Arc;

use anyhow::{Context as _, Result};
use ronoctua_term::pty::{Pty, Stream};
use ronoctua_term::terminal::Terminal;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::EventLoop,
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
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
    protocol::{wl_keyboard, wl_output, wl_seat, wl_surface},
};

/// The terminal window's `app_id`.
///
/// Distinct from the native buffer's on purpose: it is by it that the modeline
/// and the test know which window is which, and an equal `app_id` would make
/// the two look like the same thing in niri's IPC.
pub const APP_ID: &str = "br.ronoctua.terminal";

/// The MONITOR window's `app_id` — the same terminal, another identity.
///
/// # Why a second one, and not the same
///
/// `Ctrl+Alt+Esc`, `LEADER+Esc` and `LEADER+S-Esc` open a FLOATING window, and
/// what floats it is a niri `window-rule` matched on this string. The shell
/// does not place windows (invariant 4): it says WHAT the window is, and the
/// compositor decides where. With a single `app_id` the rule would float every
/// terminal, including `LEADER w o`.
pub const APP_ID_MONITOR: &str = "br.ronoctua.monitor";

/// The RESCUE console's `app_id` — the third identity of the same terminal.
///
/// A third one and not the monitor's, for the reason the second exists: the
/// rule that floats it asks for a bigger window. What `Ctrl+Alt+Del` opens is
/// meant to be read while something is wrong, and 70% of the screen is a list
/// with the log cut off.
pub const APP_ID_RESCUE: &str = "br.ronoctua.rescue";

/// The `window-rule` that makes the monitor window float, in niri's KDL.
///
/// # Why it is generated from here, and not written in the two configs
///
/// The `app_id` above and the rule's regex have to be the same string, and it
/// would exist in THREE places — this file, `nix/modules/niri.nix` and
/// `nix/dev.nix`. A string that crosses a border needs a check (`AGENTS.md`),
/// and generating it is the check that cannot go stale: `export-niri-window-rules`
/// prints this, and both configs take it in — the same arrangement as the
/// chords in `nix/niri-binds.nix`.
///
/// `r#"…"#` and not plain quotes: `\.` is not a valid escape in KDL, and niri's
/// own configuration writes its regexes in raw strings for that reason.
///
/// The proportions are the compositor's geometry, like `gaps` and the border's
/// width in the same file — invariant 9's tokens are the design system's, and
/// no component of it draws this window.
#[must_use]
pub fn floating_rules() -> String {
    // The rescue console is WIDER, and that is the only difference: it is read
    // while something is wrong, and a list with the log cut off is a diagnosis
    // one has to scroll for.
    [(APP_ID_MONITOR, "0.7"), (APP_ID_RESCUE, "0.9")]
        .into_iter()
        .map(|(app_id, size)| {
            let regex = app_id.replace('.', r"\.");
            format!(
                "window-rule {{\n\
                 \x20   match app-id=r#\"^{regex}$\"#\n\
                 \x20   open-floating true\n\
                 \x20   open-focused true\n\
                 \x20   default-column-width {{ proportion {size}; }}\n\
                 \x20   default-window-height {{ proportion {size}; }}\n\
                 }}\n"
            )
        })
        .collect()
}

/// It builds the engine INSIDE the VT thread — see `ronoctua_term::terminal`.
fn engine(columns: u16, lines: u16) -> Box<dyn ronoctua_term::Engine> {
    Box::new(ronoctua_term::alacritty::Alacritty::fresh(columns, lines))
}

/// Opens a terminal in a window of its own and runs until it closes.
///
/// An empty `program` uses the `$SHELL`.
///
/// # Errors
///
/// There is no compositor, the pty does not open, or the program does not exist.
pub fn host_it(program: Option<&str>, args: &[String]) -> Result<()> {
    host_as(APP_ID, "ronoctua: terminal", program, args)
}

/// The same terminal, under the monitor's `app_id` — so niri floats it.
///
/// # Errors
///
/// The same as [`host_it`].
pub fn host_monitor(program: &str, args: &[String]) -> Result<()> {
    host_as(APP_ID_MONITOR, "ronoctua: monitor", Some(program), args)
}

/// The same terminal again, under the rescue console's `app_id`.
///
/// # Errors
///
/// The same as [`host_it`].
pub fn host_rescue(program: &str, args: &[String]) -> Result<()> {
    host_as(APP_ID_RESCUE, "ronoctua: rescue", Some(program), args)
}

/// The body of both: everything is the same but the window's identity.
fn host_as(app_id: &str, title: &str, program: Option<&str>, args: &[String]) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<TerminalWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the compositor (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    window.set_title(title);
    window.set_app_id(app_id);
    window.commit();

    // 80x24 until the `configure` says the actual size. The following `SIGWINCH`
    // corrects the program; being born with a size is better than being born
    // without, because a 0x0 pty makes the shell print wrong once.
    let (columns, lines) = (80u16, 24u16);
    let program = program.map_or_else(ronoctua_term::pty::default_program, ToOwned::to_owned);
    let pty = Arc::new(
        Pty::open(&program, args, columns, lines).with_context(|| format!("open {program}"))?,
    );
    let vt =
        Arc::new(Terminal::fresh(columns, lines, engine).context("start the engine's thread")?);
    tracing::info!(engine = vt.name(), %program, "terminal opened");

    let mut cycle: EventLoop<TerminalWindow> = EventLoop::try_new().context("calloop")?;
    // The `ping` wakes the loop when the PTY has news. It carries no data: the
    // bytes have already gone to the engine through its own queue. Sending the
    // bytes through here would make them go through two queues, and the second
    // one could reorder them against what the keyboard wrote.
    let (ping, face) = smithay_client_toolkit::reexports::calloop::ping::make_ping()
        .context("create the pty's ping")?;
    cycle
        .handle()
        .insert_source(face, |(), (), j: &mut TerminalWindow| j.paint())
        .map_err(|e| anyhow::anyhow!("hang the ping: {e}"))?;

    // THE SIZE OF THE TEXT, watched. A terminal is the door somebody stares
    // at longest, and one that kept yesterday's size while the footer moved
    // is the exact mismatch this system decided against when it went from two
    // sizes to one.
    let (size_sender, size_changed) =
        smithay_client_toolkit::reexports::calloop::channel::channel::<()>();
    // HELD BY THE LOOP'S OWN STATE, not by this function's frame. Every
    // other door keeps its `Notice` in its struct; a local survives here only
    // because `cycle.run` is called inline below, and the day that moves the
    // watch is dropped at the end of this function with nothing saying so.
    let size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    cycle
        .handle()
        .insert_source(size_changed, |event, (), j: &mut TerminalWindow| {
            if let smithay_client_toolkit::reexports::calloop::channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                j.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("hang the text size channel: {e}"))?;

    // AND THE THEME, for the same reason and by the same means.
    //
    // This was the ONE door of the ten without it: choosing a theme in the
    // footer recoloured the whole system except the terminal, which sat there
    // in the palette it had opened with. It was found while giving this door
    // the size watch above — the two are the same six lines, and only one of
    // them was here.
    let (theme_sender, theme_changed) =
        smithay_client_toolkit::reexports::calloop::channel::channel::<()>();
    let theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    cycle
        .handle()
        .insert_source(theme_changed, |event, (), j: &mut TerminalWindow| {
            if let smithay_client_toolkit::reexports::calloop::channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                // `paint` AND NOT `ask_paint`: this door has no frame-callback
                // throttle of its own — the module's header says the PTY's
                // ping is what wakes it — so there is nothing to ask.
                j.paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("hang the theme channel: {e}"))?;

    // The PTY's thread. It touches neither Wayland nor wgpu — it only feeds the
    // engine and rings the ping.
    {
        let pty = Arc::clone(&pty);
        let vt = Arc::clone(&vt);
        std::thread::Builder::new()
            .name("ronoctua-pty".into())
            .spawn(move || {
                let r = pty.pump(|b| {
                    if vt.feed(b).is_err() {
                        // The engine's thread died: carrying on would read bytes
                        // to throw away.
                        return Stream::Para;
                    }
                    ping.ping();
                    Stream::Follow
                });
                if let Err(e) = r {
                    tracing::warn!(error = %e, "reading the pty ended with an error");
                }
                // End of the program: it wakes the loop one last time so it
                // notices and closes the window.
                ping.ping();
            })
            .context("start the pty's thread")?;
    }

    let mut state = TerminalWindow {
        _size_watch: size_watch,
        _theme_watch: theme_watch,
        conn: conn.clone(),
        handle: cycle.handle(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        mods: Modifiers::default(),
        window,
        width: 0,
        height: 0,
        asked: (0, 0),
        // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE — see `crate::hidpi`.
        scale: 1,
        scale_told: 1,
        columns,
        lines,
        quit: false,
        gpu: None,
        text: None,
        pty,
        vt,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The terminal's window.
/// How many cells fit in a window of `width` x `height` pixels.
///
/// # Why it is a free function and not a method
///
/// Because it is the one thing here that can be WRONG in a way nothing shows,
/// and a method on a struct that needs a Wayland connection cannot be tested.
/// Free, it is arithmetic over two numbers, and the test runs in microseconds.
///
/// **Integer arithmetic, without measuring text**: the cell is exactly 0.600 x
/// the font size at normal width, including in the Nerd Fonts. Measuring here
/// would be trading one calculation for a shaping call on every `configure`.
///
/// It is the BUFFER's cell, the same one the drawer uses. With the footer's,
/// the arithmetic would give more columns than fit and the text would go out
/// past the right edge with no error at all.
///
/// # And the margin comes off, TWICE — one on each side
///
/// The paragraph above says the text would otherwise leak past the right edge
/// with no error at all, and that is exactly what WAS happening:
/// `paint::spans_of` shifts every run by `CONTENT_INSET`, and this counted
/// columns across the whole window. The last two columns of every terminal
/// were written outside it, and the compositor does not clip.
///
/// `Host::context_of` in `buffer.rs` already subtracted it, and `render-png`
/// adds it back to work out a window's size from a cell count. Three places,
/// one arithmetic; this was the one that disagreed.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn cells_that_fit(width: u32, height: u32) -> (u16, u16) {
    let cell_width_px = tokens::text::size() * tokens::font::CELL_WIDTH_EM;
    let cell_height_px = tokens::text::size() * tokens::font::LINE_HEIGHT;
    let columns = ((width as f32 / cell_width_px) as u32)
        .saturating_sub(u32::from(tokens::space::CONTENT_INSET) * 2)
        .clamp(1, u32::from(u16::MAX)) as u16;
    let lines = ((height as f32 / cell_height_px) as u32)
        .saturating_sub(u32::from(tokens::space::CONTENT_INSETY) * 2)
        .clamp(1, u32::from(u16::MAX)) as u16;
    (columns, lines)
}

struct TerminalWindow {
    conn: Connection,
    /// `display.json`, watched — the size of the text.
    ///
    /// HELD HERE and not in `host_as`'s frame, which is where it started:
    /// every other door keeps its `Notice` in its state, and a local survives
    /// only while `cycle.run` happens to be called inline. Splitting that
    /// function would drop the watch at its end, and nothing anywhere would
    /// say so — the terminal would simply stop following the size.
    _size_watch: Option<ronoctua_files::notice::Notice>,
    /// The theme's watch, kept alive for the window's life — see `_size_watch`.
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    /// The loop, for the one thing that needs it: a keyboard that REPEATS.
    /// Holding backspace in a terminal is meant to erase several characters,
    /// and SCTK only repeats for a keyboard made with
    /// `get_keyboard_with_repeat` — without it, the `repeat_key` below was
    /// dead code and the comment beside it said the opposite.
    handle: smithay_client_toolkit::reexports::calloop::LoopHandle<'static, Self>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    mods: Modifiers,
    window: Window,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See the field above.
    height: u32,
    /// The logical size the COMPOSITOR configured, before any clamp.
    ///
    /// `width`/`height` are what the BUFFER can show — which is what the
    /// columns, the bars and the hit tests are laid out against; this is what
    /// the GPU is asked for. They are the same pair on every machine anybody
    /// has, and they come apart exactly at the GPU's texture ceiling. Keeping
    /// only one of them is a ratchet: a window clamped once at scale 3 would
    /// ask for the size it was cut down to and could never grow back. See
    /// [`crate::hidpi`].
    asked: (u32, u32),
    /// How many physical pixels one logical pixel is on the screen this
    /// window is on — see [`crate::hidpi`], where the rule and its
    /// measurement live.
    scale: i32,
    /// What the SURFACE has been told, which lags `scale` by one frame.
    ///
    /// `set_buffer_scale` has to travel in the commit that carries a buffer of
    /// the matching size, and the only such commit is the present inside
    /// `Gpu::frame` — see [`crate::hidpi::told_before_present`].
    scale_told: i32,
    columns: u16,
    lines: u16,
    quit: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    pty: Arc<Pty>,
    vt: Arc<Terminal>,
}

/// The grid as a display list.
///
/// A single `Command::Grid`, and not a `Text` per line: the `grid` node
/// exists in the WIT exactly so that terminal content crosses the protocol in
/// one piece. See the top of the module.
fn grid_list(g: &ronoctua_term::Grid) -> ronoctua_protocol::DisplayList {
    let mut d = ronoctua_protocol::DisplayList::fresh_one();
    d.push(ronoctua_protocol::Command::Grid {
        line: 0,
        column: 0,
        // `visible_rows()` and not `canonical_f()`: the second one puts the cursor's
        // position on an extra line, for the differential diff. Drawing that would
        // show `cursor: 3,0` as if it were the program's output.
        lines: g.visible_rows(),
        color: ronoctua_protocol::Color::name("text-primary"),
    });
    d
}

impl TerminalWindow {
    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        let Ok(grid) = self.vt.grid() else {
            tracing::info!("the engine's thread left; closing the terminal");
            self.quit = true;
            return;
        };
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let list = grid_list(&grid);
        // Node i18n: EVERY terminal text is the program's data, never an interface
        // label. Translating the output of `ls` would be absurd, and that is why
        // `Command::Grid` only carries `String`.
        let texts = ronoctua_i18n::Texts::from_env();
        // The window's COLUMNS: the host guarantees that nothing leaks past the
        // edge, and a terminal line wider than the window is the common case.
        let spans = crate::paint::spans_of(&list, &texts, self.columns);
        // THE SURFACE AND THE TWO SCALE NUMBERS, taken before the borrows:
        // `set_buffer_scale` has to travel in the commit `Gpu::frame` makes.
        // See `crate::hidpi::told_before_present`.
        let surface = self.window.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
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
            tracing::error!(error = %e, "preparing the text failed");
            return;
        }
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| text.draw(ronoctua_render::Atlas::Pinned, pass),
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        tracing::info!(spans = spans.len(), "terminal drawn on the xdg_toplevel");
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }

    /// The size of the text changed under this door.
    ///
    /// The shaper keeps a size of its own and has to hear it, or the grid
    /// moves and the letters do not — and a terminal's grid IS its content,
    /// so the mismatch is not cosmetic here: the engine is resized in CELLS,
    /// and a cell that stopped matching its glyph is a wrong number of
    /// columns handed to whatever is running.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(ronoctua_tokens::text::size());
        }
        self.recount();
        self.paint();
    }

    /// Recomputes columns and rows from the size in pixels.
    ///
    /// **Integer arithmetic, without measuring text**: the cell is exactly
    /// 0.600 x the font size at normal width, including in the Nerd Fonts.
    /// Measuring here would be trading one calculation for a shaping call on
    /// every `configure`.
    fn recount(&mut self) {
        // A WINDOW WITH NO SIZE HAS NO GRID. `cells_that_fit` floors at one
        // cell rather than declining to answer, so at 0x0 this told the engine
        // AND the pty that the terminal was one column by one row — a
        // `SIGWINCH` of 1x1 to a shell that has just started, which is exactly
        // what `host_as`'s 80x24 guess exists to prevent.
        //
        // It is reachable before the first `configure`: the scale can arrive
        // first, and the size watch on `display.json` is installed before it
        // too. Both call this.
        if self.width == 0 || self.height == 0 {
            return;
        }
        let (columns, lines) = cells_that_fit(self.width, self.height);
        if (columns, lines) == (self.columns, self.lines) {
            return;
        }
        self.columns = columns;
        self.lines = lines;
        // BOTH sides. Only the engine would leave the program printing at the old
        // width; only the pty would leave the grid at the old size, and the
        // difference would show up as cut text with no error at all.
        if let Err(e) = self.vt.resize(columns, lines) {
            tracing::warn!(error = %e, "resizing the engine failed");
        }
        if let Err(e) = self.pty.resize(columns, lines) {
            tracing::warn!(error = %e, "resizing the pty failed");
        }
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
            tracing::error!("null Wayland pointers");
            return;
        };
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what they came from.
        // ONE CALL, not the same one twice: two calls to a pure
        // function are two places the arguments can be edited apart.
        let first = crate::hidpi::physical(self.asked, self.scale);
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                first.0,
                first.1,
            )
        };
        match gpu {
            Ok(mut g) => {
                // AND THE FIRST BUFFER IS DIVISIBLE TOO. `Gpu::new` clamps at
                // the GPU's ceiling like `resize` does, and the first frame
                // sends the scale — see `crate::hidpi::resized`.
                let (shown_w, shown_h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
                self.width = shown_w;
                self.height = shown_h;
                // AND THE COLUMNS ARE COUNTED AGAIN. `configure` counts them before the
                // GPU exists, against the size the compositor gave; if the buffer came
                // back smaller than that, the count that is on file is for a picture
                // wider than the one this door draws into.
                self.recount();
                match ronoctua_render::Text::new(
                    &g.device,
                    &g.queue,
                    g.config.format,
                    self.scale,
                    // THE SYSTEM'S ONE SIZE — see `ronoctua_tokens::text::size`. It used to be
                    // `SIZE_LARGE`, until the owner asked for the footer's measure
                    // everywhere.
                    tokens::text::size(),
                ) {
                    Ok(t) => {
                        self.text = Some(t);
                        self.gpu = Some(g);
                        tracing::info!("the terminal's graphics context is ready");
                    }
                    Err(e) => tracing::error!(error = %e, "the renderer failed"),
                }
            }
            Err(e) => tracing::error!(error = %e, "no graphics context"),
        }
    }
}

/// The bytes a key sends to the program.
///
/// # Why there is no keymap here
///
/// What the terminal receives has already gone through the compositor and is
/// not a leader chord: it is text and the few control keys the VT
/// understands. Mapping this through `ronoctua-keymap` would give the
/// terminal a second keymap, and §7.4 wants only one — the shell's, which
/// does not even reach here.
#[must_use]
pub fn key_bytes(sym: Keysym, utf8: Option<&str>, ctrl: bool) -> Option<Vec<u8>> {
    // `\r` and not `\n`: a terminal sends a CARRIAGE RETURN on Enter, and the
    // line driver converts it. Sending `\n` makes the shell accept the line and
    // not echo the break — the cursor stays where it was, with no error at all.
    let especial = match sym {
        Keysym::Return | Keysym::KP_Enter => Some(b"\r".to_vec()),
        Keysym::BackSpace => Some(vec![0x7f]),
        Keysym::Tab => Some(b"\t".to_vec()),
        Keysym::Escape => Some(vec![0x1b]),
        // CSI: the sequences every full-screen program expects.
        Keysym::Up => Some(b"\x1b[A".to_vec()),
        Keysym::Down => Some(b"\x1b[B".to_vec()),
        Keysym::Right => Some(b"\x1b[C".to_vec()),
        Keysym::Left => Some(b"\x1b[D".to_vec()),
        Keysym::Home => Some(b"\x1b[H".to_vec()),
        Keysym::End => Some(b"\x1b[F".to_vec()),
        Keysym::Delete => Some(b"\x1b[3~".to_vec()),
        _ => None,
    };
    if especial.is_some() {
        return especial;
    }

    let text = utf8?;
    let mut c = text.chars().next()?;
    if !ctrl {
        return Some(text.as_bytes().to_vec());
    }
    // `Ctrl-A` is byte 1. Without this, `Ctrl-C` would become the letter `c` and
    // there would be no way of interrupting anything.
    c = c.to_ascii_lowercase();
    if c.is_ascii_lowercase() {
        return Some(vec![c as u8 - b'a' + 1]);
    }
    Some(text.as_bytes().to_vec())
}

impl WindowHandler for TerminalWindow {
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
            cfg.new_size.0.map_or(640, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(400, std::num::NonZeroU32::get),
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
        self.paint();
    }
}

impl SeatHandler for TerminalWindow {
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
            // A KEYBOARD THAT REPEATS: holding a key here is meant to work,
            // and SCTK only repeats for one made this way.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, _, event| state.typed(event)),
            ) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
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
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl TerminalWindow {
    /// One key, from a press or from a REPEAT.
    ///
    /// SCTK delivers a repeat through the callback given to
    /// `get_keyboard_with_repeat`, which has no `Connection` and no
    /// `QueueHandle` to hand on — so the work cannot live in `press_key`, and
    /// while it did, `repeat_key` was dead code and holding a key did it
    /// once. The comment beside `repeat_key` said the opposite for a year.
    fn typed(&mut self, event: KeyEvent) {
        let Some(bytes) = key_bytes(event.keysym, event.utf8.as_deref(), self.mods.ctrl) else {
            return;
        };
        if let Err(e) = self.pty.write_bytes(&bytes) {
            tracing::warn!(error = %e, "writing to the pty failed; closing");
            self.quit = true;
        }
        // It does NOT redraw here. The echo comes from the program through the pty,
        // and drawing now would show the screen from BEFORE the key — a late frame
        // that only shows up when one types fast.
    }
}

impl KeyboardHandler for TerminalWindow {
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
        tracing::debug!("the terminal gained keyboard focus");
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
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
        // In a terminal the repeat IS the right behavior: holding the key deletes
        // several characters, goes down several lines. Unlike the footer, where
        // repeating a leader key is always a mistake.
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

impl CompositorHandler for TerminalWindow {
    /// The window moved to a screen of another scale — or was told its first.
    ///
    /// It was EMPTY here and in eight other doors, which is what made every
    /// one of them blurry on a HiDPI screen. The rule and the measurement are
    /// in [`crate::hidpi`].
    ///
    /// AND IT RECOUNTS, which took two rounds to get right. The number of
    /// columns is LOGICAL and does not move with the scale — while the buffer
    /// holds what the window asked for. Above the GPU's ceiling it does not,
    /// and then the scale is exactly what moves it: the door keeps what the
    /// buffer can show, so the shell would be told a width the terminal no
    /// longer has. `recount` declines on a window with no size, which is what
    /// makes this safe before the first `configure` — where it was not a
    /// no-op at all but a 1x1 `SIGWINCH` to a shell that had just started.
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
            self.paint();
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

impl OutputHandler for TerminalWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for TerminalWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(TerminalWindow);
smithay_client_toolkit::delegate_dispatch2!(TerminalWindow);

#[cfg(test)]
mod tests {
    use super::{grid_list, key_bytes};
    use ronoctua_tokens as tokens;
    use smithay_client_toolkit::seat::keyboard::Keysym;

    #[test]
    fn the_rule_matches_the_app_id_the_window_actually_sets() {
        // THE RULE AND THE WINDOW ARE ONE STRING. Written apart, an `app_id`
        // renamed here would give a rule that matches nothing — and the symptom
        // is the monitor opening TILED, which looks like the shortcut working.
        let rule = super::floating_rules();
        assert!(
            rule.contains(&super::APP_ID_MONITOR.replace('.', r"\.")),
            "the rule does not name the window's app id:\n{rule}"
        );
        // The dots are escaped: `br.ronoctua.monitor` as a regex would also
        // match `brXronoctuaYmonitor`, and — worse — is not what one reads.
        assert!(rule.contains(r"^br\.ronoctua\.monitor$"), "{rule}");
        // A RAW string. `"^br\.ronoctua\.monitor$"` in KDL is an invalid escape,
        // and niri REFUSES the whole config — which is the compositor falling
        // back to its own default, with none of our shortcuts.
        assert!(
            rule.contains("r#\""),
            "the regex has to be a raw string:\n{rule}"
        );
        assert!(rule.contains("open-floating true"), "{rule}");
    }

    #[test]
    fn enter_sends_a_carriage_return() {
        // `\n` would make the shell accept the line and NOT echo the break: the
        // cursor would stay where it was, with no error at all. A terminal sends `\r`.
        assert_eq!(
            key_bytes(Keysym::Return, Some("\r"), false),
            Some(b"\r".to_vec())
        );
    }

    #[test]
    fn ctrl_c_becomes_the_control_byte() {
        // Without this `Ctrl-C` would become the letter `c` and there would be no way
        // of interrupting anything — and the symptom would be "the terminal ignores Ctrl-C".
        assert_eq!(key_bytes(Keysym::c, Some("c"), true), Some(vec![3]));
        assert_eq!(key_bytes(Keysym::d, Some("d"), true), Some(vec![4]));
    }

    #[test]
    fn with_no_ctrl_the_letter_is_the_letter() {
        assert_eq!(key_bytes(Keysym::c, Some("c"), false), Some(b"c".to_vec()));
    }

    #[test]
    fn the_arrows_come_out_as_csi() {
        assert_eq!(key_bytes(Keysym::Up, None, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(
            key_bytes(Keysym::Left, None, false),
            Some(b"\x1b[D".to_vec())
        );
    }

    #[test]
    fn a_key_with_no_text_and_no_case_sends_nothing() {
        // Shift on its own, for instance. Sending no byte is the right thing; sending
        // an empty one would make the `write` wake the program for nothing.
        assert_eq!(key_bytes(Keysym::Shift_L, None, false), None);
    }

    #[test]
    fn the_grid_becomes_a_single_grid_command() {
        // A `Command::Grid`, and not a `Text` per line: it is the node the WIT
        // already has for terminal content. If this becomes N commands, the wasip2
        // buffer with the `terminal` capability is going to need another drawing
        // path.
        let mut g = ronoctua_term::Grid::fresh_one(10, 2);
        g.put_at(0, 0, ronoctua_term::CellBox { c: 'o' });
        g.put_at(1, 0, ronoctua_term::CellBox { c: 'i' });
        let d = grid_list(&g);
        assert_eq!(d.commands.len(), 1);
        match &d.commands[0] {
            ronoctua_protocol::Command::Grid { lines, .. } => {
                // Without the `cursor: r,c` line of the canonical form: that one belongs to
                // the comparator, not to the screen.
                assert_eq!(lines, &vec!["o".to_owned(), "i".to_owned()]);
            }
            otherwise => panic!("it did not become a grid: {otherwise:?}"),
        }
    }

    #[test]
    fn the_terminal_counts_only_the_cells_it_can_draw_in() {
        // THE DEFECT: `paint::spans_of` shifts every run by `CONTENT_INSET`,
        // and this counted columns across the whole window. The last two
        // columns of every terminal were written outside it — and the
        // compositor does not clip, so there was no error anywhere.
        //
        // The assertion is the INVERSE of what `render-png` does to work out a
        // window's size from a cell count, which is what makes the two agree by
        // construction instead of by coincidence.
        let cell_w = tokens::text::size() * tokens::font::CELL_WIDTH_EM;
        let cell_h = tokens::text::size() * tokens::font::LINE_HEIGHT;
        for width in [200_u32, 640, 800, 1280, 1920, 3840] {
            for height in [200_u32, 600, 1080] {
                let (columns, lines) = super::cells_that_fit(width, height);
                let wanted = f32::from(columns + tokens::space::CONTENT_INSET * 2) * cell_w;
                assert!(
                    wanted <= width as f32,
                    "at {width} px the terminal asked for {columns} columns, \
                     which with the margin is {wanted} px"
                );
                let tall = f32::from(lines + tokens::space::CONTENT_INSETY * 2) * cell_h;
                assert!(
                    tall <= height as f32,
                    "at {height} px it asked for {lines} lines"
                );
            }
        }
        // AND NEVER ZERO. A window narrower than the margin is one the
        // compositor may still hand over, and a grid of zero columns is a panic
        // waiting inside the engine.
        let (columns, lines) = super::cells_that_fit(1, 1);
        assert!(columns >= 1 && lines >= 1, "the grid cannot be empty");
    }
}
