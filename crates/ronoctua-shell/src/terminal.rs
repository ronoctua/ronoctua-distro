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
    let conn = Connection::connect_to_env()
        .context("nao ha compositor Wayland acessivel (WAYLAND_DISPLAY esta definida?)")?;
    let (globals, fila) = registry_queue_init(&conn).context("registry do Wayland")?;
    let qh: QueueHandle<TerminalWindow> = fila.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("sem wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("o compositor nao expoe xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the compositor (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    window.set_title("ronoctua: terminal");
    window.set_app_id(APP_ID);
    window.commit();

    // 80x24 until the `configure` says the actual size. The following `SIGWINCH`
    // corrects the program; being born with a size is better than being born
    // without, because a 0x0 pty makes the shell print wrong once.
    let (columns, lines) = (80u16, 24u16);
    let program = program.map_or_else(ronoctua_term::pty::default_program, ToOwned::to_owned);
    let pty = Arc::new(
        Pty::open(&program, args, columns, lines).with_context(|| format!("abrir {program}"))?,
    );
    let vt = Arc::new(Terminal::fresh(columns, lines, engine).context("subir a thread do motor")?);
    tracing::info!(engine = vt.name(), %program, "terminal aberto");

    let mut cycle: EventLoop<TerminalWindow> = EventLoop::try_new().context("calloop")?;
    // The `ping` wakes the loop when the PTY has news. It carries no data: the
    // bytes have already gone to the engine through its own queue. Sending the
    // bytes through here would make them go through two queues, and the second
    // one could reorder them against what the keyboard wrote.
    let (ping, face) = smithay_client_toolkit::reexports::calloop::ping::make_ping()
        .context("criar o ping do pty")?;
    cycle
        .handle()
        .insert_source(face, |(), (), j: &mut TerminalWindow| j.paint())
        .map_err(|e| anyhow::anyhow!("pendurar o ping: {e}"))?;

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
                    tracing::warn!(error = %e, "a leitura do pty terminou com erro");
                }
                // End of the program: it wakes the loop one last time so it
                // notices and closes the window.
                ping.ping();
            })
            .context("subir a thread do pty")?;
    }

    let mut state = TerminalWindow {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        mods: Modifiers::default(),
        window,
        width: 0,
        height: 0,
        columns,
        lines,
        quit: false,
        gpu: None,
        text: None,
        pty,
        vt,
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

/// The terminal's window.
struct TerminalWindow {
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    mods: Modifiers,
    window: Window,
    width: u32,
    height: u32,
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
            tracing::info!("a thread do motor saiu; fechando o terminal");
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
        let spans = crate::buffer::spans_of(&list, &texts, self.columns);
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
        if let Err(e) = gpu.frame(|pass| text.draw(ronoctua_render::Atlas::Pinned, pass)) {
            tracing::error!(error = %e, "desenhar o frame falhou");
            return;
        }
        tracing::info!(spans = spans.len(), "terminal desenhado no xdg_toplevel");
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "flush falhou");
        }
    }

    /// Recomputes columns and rows from the size in pixels.
    ///
    /// **Integer arithmetic, without measuring text**: the cell is exactly
    /// 0.600 x the font size at normal width, including in the Nerd Fonts.
    /// Measuring here would be trading one calculation for a shaping call on
    /// every `configure`.
    fn recount(&mut self) {
        // The BUFFER's size, the same one the drawer uses. With the footer's, the
        // arithmetic would give more columns than fit, and the text would go out
        // past the right edge with no error at all.
        let cell_width_px = tokens::font::SIZE_LARGE * tokens::font::CELL_WIDTH_EM;
        let altura_celula = tokens::font::SIZE_LARGE * tokens::font::LINE_HEIGHT;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let columns = ((self.width as f32 / cell_width_px) as u32).clamp(1, u32::from(u16::MAX));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = ((self.height as f32 / altura_celula) as u32).clamp(1, u32::from(u16::MAX));
        let (columns, lines) = (columns as u16, lines as u16);
        if (columns, lines) == (self.columns, self.lines) {
            return;
        }
        self.columns = columns;
        self.lines = lines;
        // BOTH sides. Only the engine would leave the program printing at the old
        // width; only the pty would leave the grid at the old size, and the
        // difference would show up as cut text with no error at all.
        if let Err(e) = self.vt.resize(columns, lines) {
            tracing::warn!(error = %e, "redimensionar o motor falhou");
        }
        if let Err(e) = self.pty.resize(columns, lines) {
            tracing::warn!(error = %e, "redimensionar o pty falhou");
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
            tracing::error!("ponteiros do Wayland nulos");
            return;
        };
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what they came from.
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                self.width,
                self.height,
            )
        };
        match gpu {
            Ok(g) => match ronoctua_render::Text::new(
                &g.device,
                &g.queue,
                g.config.format,
                // SIZE_LARGE and not SIZE_BASE: the footer is peripheral
                // reference, the buffer is what is read. Having both at the same
                // body made the content look like a footnote.
                tokens::font::SIZE_LARGE,
            ) {
                Ok(t) => {
                    self.text = Some(t);
                    self.gpu = Some(g);
                    tracing::info!("contexto grafico do terminal pronto");
                }
                Err(e) => tracing::error!(error = %e, "desenhador falhou"),
            },
            Err(e) => tracing::error!(error = %e, "sem contexto grafico"),
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
        self.width = cfg.new_size.0.map_or(640, std::num::NonZeroU32::get);
        self.height = cfg.new_size.1.map_or(400, std::num::NonZeroU32::get);
        if let Some(g) = self.gpu.as_mut() {
            g.resize(self.width, self.height);
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
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "sem teclado; nao da para digitar"),
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
        tracing::debug!("o terminal ganhou o foco de teclado");
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
        let Some(bytes) = key_bytes(event.keysym, event.utf8.as_deref(), self.mods.ctrl) else {
            return;
        };
        if let Err(e) = self.pty.write_bytes(&bytes) {
            tracing::warn!(error = %e, "escrever no pty falhou; fechando");
            self.quit = true;
        }
        // It does NOT redraw here. The echo comes from the program through the pty,
        // and drawing now would show the screen from BEFORE the key — a late frame
        // that only shows up when one types fast.
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
    use smithay_client_toolkit::seat::keyboard::Keysym;

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
            otherwise => panic!("nao virou grade: {otherwise:?}"),
        }
    }
}
