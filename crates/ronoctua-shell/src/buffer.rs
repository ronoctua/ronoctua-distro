//! A native buffer hosted in an `xdg_toplevel` of its own — section 7.5.
//!
//! # Why an `xdg_toplevel`, and not a region of our surface
//!
//! Invariant 4 is explicit: **the shell never reimplements window
//! layout**. A native buffer is a WINDOW like any other, and what
//! decides where it goes is niri. Drawing buffers inside a fullscreen
//! surface of ours would throw the compositor away — and section 7.5 forbids that
//! by name.
//!
//! The practical consequence: a native buffer appears in niri's window
//! column, stacks, resizes and closes like everything else. There is nothing special
//! about it from the compositor's point of view, and that is how it should be.
//!
//! # The cycle
//!
//! ```text
//! .wasm component  ->  instance  ->  render(ctx)  ->  DisplayList
//!                                                            |
//!                   tokens and i18n resolved HERE  <---------+
//!                                                            |
//!                                             wgpu on the xdg_toplevel
//! ```
//!
//! The component sees no token, no language and no Wayland (invariant 11).
//! What resolves all three is this file.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::EventLoop,
    reexports::calloop::channel as calloop_channel,
    // The clock, for the buffer that samples the system. It reaches here
    // through the toolkit's re-export, like everything else from calloop —
    // depending on the crate directly would risk two versions of it.
    reexports::calloop::timer,
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

/// How many pixels each scale is worth.
///
/// The translation lives HERE, and not in the buffer: invariant 11 says a buffer does not
/// receive a token value, and it is the same contract colour already fulfils.
///
/// The cell grid is always `Normal`'s — a `Large` run occupies more
/// than one cell, and what spaces it is whatever draws.
const fn scale_in_px(scale: ronoctua_protocol::Scale) -> f32 {
    match scale {
        // `SIZE_SMALL` is the token that already existed for "smaller than the body" — the
        // system's footer uses `SIZE_BASE`, and the buffer's body, `SIZE_LARGE`.
        ronoctua_protocol::Scale::Small => tokens::font::SIZE_SMALL,
        // THE SYSTEM'S SIZE — the same the footer and the minibuffer use.
        ronoctua_protocol::Scale::Base => tokens::font::SIZE_BASE,
        ronoctua_protocol::Scale::Normal => tokens::font::SIZE_LARGE,
        ronoctua_protocol::Scale::Large => tokens::font::SIZE_XLARGE,
        ronoctua_protocol::Scale::Huge => tokens::font::SIZE_XXLARGE,
    }
}

/// Resolves the colour name that came from the buffer.
///
/// # Why the table is no longer here
///
/// It used to be a hand-written `match` of eight names, with `otherwise =>` falling into
/// `TEXT_DIM`. Two things wrong: the list could diverge from the actual tokens
/// with nothing flagging it, and the fallback made a buffer with a wrong name
/// draw nicely — the defect only showed up when someone found the colour odd.
///
/// Now the table is GENERATED from the same `assets/tokens/colors.nix` that generates the
/// WIT enum (section 7.6). On the guest's side a wrong name **does not compile**;
/// on this side, a name that does not resolve is a protocol violation — and the buffer
/// that commits it is refused, not papered over.
#[must_use]
pub fn token_color(name: &str) -> Option<tokens::Color> {
    tokens::color_by_wit_name(name)
}

/// Translates a component's `DisplayList` into runs for the renderer.
///
/// It is here that the two invariants are resolved: the `Text::Key` goes through the
/// i18n, and the `Color` by name becomes a value.
#[must_use]
/// Shifts a run inside the buffer's margin.
///
/// # Why the margin is applied HERE, and not in the component
///
/// The wasip2 buffer does not know where its window is nor how much is left
/// (invariant 11): it counts rows and columns from zero. The margin is a
/// presentation decision, and presentation belongs to the host — the same reason
/// it is the host that resolves colour and translation.
///
/// Without it the first character touches the window's edge and looks like clipping.
fn with_margin(mut t: ronoctua_render::Span) -> ronoctua_render::Span {
    t.column = t.column.saturating_add(tokens::space::BUFFER_MARGINX);
    t.line = t.line.saturating_add(tokens::space::BUFFER_MARGINY);
    t
}

pub fn spans_of(
    list: &ronoctua_protocol::DisplayList,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Vec<ronoctua_render::Span> {
    // `flat_map`, and not `filter_map`: the grid becomes ONE run per ROW, and it is
    // the only command that produces more than one.
    list.commands
        .iter()
        .flat_map(|c| spans_from(c, texts, columns))
        .map(with_margin)
        .collect()
}

/// Cuts a text to the budget the buffer declared, and to the window's edge.
///
/// # Why BOTH limits
///
/// The budget is what the buffer reserved for that run — it stops
/// two texts on the same row from overlapping. The edge is the host's guarantee:
/// **nothing spills out of the window**, whatever the buffer asked for. Either one
/// alone lets a case through: with no budget, two runs collide; with no
/// edge, a careless buffer writes off screen and the compositor does not clip.
fn elide_to_budget(s: &str, max_width: u16, column: u16, columns: u16) -> String {
    let ate_a_borda = columns.saturating_sub(column);
    // `0` = no budget of its own: only the edge applies.
    let ceiling = if max_width == 0 {
        ate_a_borda
    } else {
        max_width.min(ate_a_borda)
    };
    ronoctua_render::layout::elide_end(s, ceiling)
}

/// Turns a display list command into the runs the renderer draws.
///
/// It returns empty when the command produces no text — or when the colour does not
/// resolve, which is a protocol violation: the command is REFUSED, not papered over
/// with a default colour. See `token_color`.
fn spans_from(
    c: &ronoctua_protocol::Command,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Vec<ronoctua_render::Span> {
    use ronoctua_protocol::{Command, Text};

    match c {
        Command::Text {
            line,
            column,
            text,
            color,
            scale,
            max_width,
        } => {
            let Some(color) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "cor invalida num buffer");
                return Vec::new();
            };
            vec![ronoctua_render::Span {
                size: Some(scale_in_px(*scale)),
                text: {
                    let s = match text {
                        Text::Data(s) => s.clone(),
                        Text::Key(k) => texts.t(k),
                        // THE ICON IS RESOLVED HERE, and not in the buffer: the codepoint
                        // is in the PUA and changes between Nerd Fonts versions. A name
                        // that does not exist in the table becomes a VISIBLE `?` — the buffer
                        // named it through a generated enum, so arriving here without resolving
                        // means the table and the WIT have diverged, and that has
                        // to show.
                        Text::Icon(name) => tokens::icon_by_wit_name(name)
                            .map_or_else(|| "?".to_owned(), |c| c.to_string()),
                    };
                    // THE BUDGET IS ENFORCED HERE, and not in the buffer.
                    //
                    // Whoever has the translated string is the host: the buffer knows
                    // how much space there is, and the sentence's length changes with the
                    // language. Without this, the browser's invitation ran over
                    // the counter in a narrow window.
                    elide_to_budget(&s, *max_width, *column, columns)
                },
                column: *column,
                line: *line,
                color,
            }]
        }
        // A rule and a selection are QUADS, not text. Until the renderer has the
        // quad pipeline (the footer only needs glyphs), the rule comes out as
        // a line of `─` — a glyph from the font, coherent with invariant 10.
        Command::Rule { line, color } => token_color(color.as_str())
            .map(|color| ronoctua_render::Span {
                size: None,
                text: "─".repeat(80),
                column: 0,
                line: *line,
                color,
            })
            .into_iter()
            .collect(),
        // THE GRID: one line of text per grid row, drawn with the
        // SAME font and the SAME tokens as everything else. That is why
        // it is a first-class node and not a `raw-surface` (section 7.10).
        Command::Grid {
            line,
            column,
            lines,
            color,
        } => {
            let Some(color) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "cor invalida numa grade");
                return Vec::new();
            };
            lines
                .iter()
                .enumerate()
                .map(|(n, content)| ronoctua_render::Span {
                    size: None,
                    text: content.clone(),
                    column: *column,
                    // `saturating_add`: a grid too tall for the screen
                    // should truncate, never wrap around the u16.
                    line: line.saturating_add(u16::try_from(n).unwrap_or(u16::MAX)),
                    color,
                })
                .collect()
        }
        // THE SELECTION IS NOT TEXT. It goes out through the quad pipeline —
        // see `Host::selections`. It used to return an empty vector here and
        // nothing drew it, in silence.
        Command::Selection { .. } => Vec::new(),
        // THE SVG IS NOT TEXT. It goes out through the image pipeline — see
        // `Host::areas_de_svg`.
        Command::Svg { .. } => Vec::new(),
    }
}

/// A vector drawing request, with the area already in pixels.
struct VectorDrawing {
    /// The SVG's string, which is also the cache key.
    key: String,
    /// Where it fits, already fitted without stretching.
    area: ronoctua_render::image::Area,
}

/// The pixmap of a [`ronoctua_protocol::Command::Svg`], ready for the GPU.
///
/// # Why it is a function, and not three lines inside the loop
///
/// Because it carries **two decisions the screen does not give away**, and both have
/// already been wrong:
///
/// - **`Natural`, and not `Fill`.** The rectangle is the CEILING, and the drawing comes out
///   at the size the document itself declares. Filling, a `width="24"`
///   and a `width="512"` give the same image — Lucide's size adjustment had
///   no visible effect at all, and the preview confidently showed a size
///   that was not the chosen one.
/// - **The `currentColor` resolved** before rasterising, otherwise the drawing comes out
///   black over the buffer's background.
///
/// Inside the loop, neither of the two could be exercised: `vector_ones`
/// needs a Wayland window. Here they are testable, and it is the only way
/// for the wrong choice to break something before the screen does.
fn rasterize_preview(
    svg: &str,
    color: tokens::Color,
    side: u32,
) -> Option<ronoctua_render::image::Rgba> {
    let documento = resolver_current_color(svg, color);
    let r = ronoctua_notify::icon::from_svg(
        documento.as_bytes(),
        side,
        ronoctua_notify::icon::Fit::Natural,
    )?;
    Some(ronoctua_render::image::Rgba {
        width: r.width,
        height: r.height,
        pixels: r.pixels,
    })
}

/// The SVG's `currentColor` resolved to a value.
///
/// # Why the substitution belongs to the host
///
/// Because the colour crosses the boundary as a **token**, never as a value
/// (invariant 11). And because `currentColor` with no context is **black**: over the
/// buffer's background, the drawing would come out invisible — which is indistinguishable from "this
/// primitive is not implemented yet", the worst possible result.
///
/// When the person has typed a colour of their own, there is no `currentColor` in the
/// document and nothing is replaced: there the colour is **content**, and the drawing comes out
/// as the artefact would.
fn resolver_current_color(svg: &str, color: tokens::Color) -> String {
    // `#RRGGBB`, which is what SVG understands. The token's alpha does not travel: an
    // accidentally semitransparent icon would look like a rendering error.
    let [r, g, b, _] = color.to_rgba8();
    let hex = format!("#{r:02X}{g:02X}{b:02X}");
    svg.replace("currentColor", &hex)
}

/// The state of the window hosting a buffer.
struct Host {
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// The pointer, so the click reaches the buffer's items.
    pointer: Option<wl_pointer::WlPointer>,
    mods: Modifiers,
    window: Window,
    width: u32,
    height: u32,
    quit: bool,
    /// The file this buffer was opened with, when there is one.
    ///
    /// It is the HOST that holds it, and not the component: `Effect::Save`
    /// carries only the text, and the buffer never learns a path. That is what
    /// keeps invariant 11 intact while a file editor exists — the same shape as
    /// `Effect::Copy`, where the buffer hands text over and the host decides
    /// where it goes.
    file: Option<std::path::PathBuf>,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    /// The image pipeline, for [`ronoctua_protocol::Command::Svg`].
    ///
    /// The buffer window only knew how to draw text. It is born along with
    /// `text`, in the same `assemble_gpu`, and stays `None` while there is no GPU —
    /// for the same reason as it.
    images: Option<ronoctua_render::image::Images>,
    /// The flat-rectangle pipeline, for `Command::Selection`.
    ///
    /// It had been missing since the primitive existed: the host answered
    /// `Vec::new()` for it, in silence. A buffer that used it drew nothing and
    /// never found out — which is worse than a primitive that does not exist,
    /// because the WIT says it does.
    quads: Option<ronoctua_render::quad::Quads>,
    list: ronoctua_protocol::DisplayList,
    /// The already-rasterised vector drawings, by the SVG's string.
    ///
    /// The key is the whole document, and it is exact: it carries the icon and the
    /// five adjustments. Two keys that do not change the drawing hit the cache, and
    /// one that does pays for a rasterisation — which is what it costs.
    rasterized: std::collections::HashMap<String, ronoctua_render::image::Rgba>,
    /// What the descriptor ASKED FOR. See [`ronoctua_protocol::DisplayList::validate_it`].
    capabilities: Vec<ronoctua_protocol::Capability>,
    /// The buffer's OWN keymap, straight from the descriptor.
    ///
    /// # Why the buffer's host resolves it, and not the footer
    ///
    /// A buffer is a window of its own (invariant 4), and while it has the
    /// focus it is THIS process that holds the keyboard — the footer only gets
    /// it back when the leader is pressed. A key typed in here never reaches the
    /// footer, so a keymap resolved over there would never fire.
    ///
    /// It stayed declared and unreachable until an editor needed to SAVE, which
    /// is when "declares and nothing consumes it" stopped being tolerable: the
    /// buffer would open, edit, and lose the work.
    keymap: Vec<ronoctua_protocol::Binding>,
    /// The sequence typed so far, waiting to complete a binding.
    pending: Vec<char>,
    texts: ronoctua_i18n::Texts,
    /// The LIVE component.
    ///
    /// The host used to call `render` once and stop: no key reached
    /// the buffer, and the glyph browser was an image — there was no way to
    /// search, scroll or choose. The standalone `Runtime::render` on top of that
    /// creates a new store and clears the state on every call (see CLAUDE.md).
    instance: ronoctua_runtime::Instance,
    /// Kept because `Instance` does not return them, and dropping them
    /// would take the component down.
    _runtime: ronoctua_runtime::Runtime,
    /// What a buffer with [`ronoctua_protocol::Capability::System`] is told,
    /// and what it asked for. `None` for every other buffer — which is the
    /// whole of the cost this capability imposes on the ones that never use it.
    system: Option<System>,
    /// The same, for [`ronoctua_protocol::Capability::Transfers`]: a
    /// subscription to the queue's socket, and the ids it may act on.
    queue: Option<Queue>,
}

/// The queue half of the host: one subscription, and the id check.
///
/// It owns no list. The unit `ronoctua-transfers` owns it, this process is
/// merely subscribed, and a copy of forty gibibytes carries on whether this
/// window is open or not.
struct Queue {
    /// The ids of the last rows delivered.
    ///
    /// THE GATE, and it is the same discipline `stop_consumer` uses for an
    /// index: the buffer names an id, and the host checks it against what it
    /// actually sent before passing it on. Without it a component could name
    /// any number and cancel a job that is not even on its screen.
    known: Vec<u64>,
}

impl Queue {
    const fn fresh() -> Self {
        Self { known: Vec::new() }
    }
}

/// The system half of the host: sampling, scanning and the policy.
///
/// It only exists for a buffer that declared the capability. Everything in it
/// costs something — a clock, a thread, a process spawned to read systemd —
/// and none of it should exist for a glyph browser.
struct System {
    /// Where `/proc` and the cgroups are. A field, and not a constant, because
    /// that is what makes the parsers testable against a tree a test wrote.
    places: ronoctua_memory::Places,
    /// The policy, as systemd last reported it.
    ///
    /// Cached on purpose: reading it spawns `systemctl` and `journalctl`, which
    /// is fine for a key press and not for a clock. It is refreshed when the
    /// buffer asks and after every change.
    guard: ronoctua_protocol::Guard,
    /// The last scan's result, kept so a redraw does not re-scan.
    consumers: Vec<ronoctua_protocol::Consumer>,
    /// A scan in flight, and how far it got, in per mille.
    scanning: bool,
    progress: u16,
    /// Which scan produced the list on screen.
    ///
    /// Kept so that closing something re-runs THAT scan: the list is stale the
    /// moment an entry leaves it, and re-scanning the other one would swap the
    /// screen's contents as a side effect of a key that meant "close this".
    last_scan: Option<Scan>,
    /// What tells a running scan to stop.
    ///
    /// Shared with the thread. Dropping it is not enough: a walk of a home
    /// directory would keep consuming I/O long after the screen moved on.
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// How the scan thread answers.
    reports: Option<calloop_channel::Sender<Scanned>>,
    /// This machine's RAM, from the last survey.
    ///
    /// The guard's ceiling comes back from systemd in BYTES, and turning it
    /// into the percentage the screen shows needs the total. Kept here so
    /// reading the policy does not have to read `/proc/meminfo` again.
    ram_total: u64,
}

/// What a scan thread sends back.
enum Scanned {
    /// How far it got.
    Step(u16),
    /// It finished, or it was stopped: what it had found either way.
    Done(Vec<ronoctua_protocol::Consumer>),
}

/// The host's input in the form the buffer understands.
///
/// # Why the buffer does not see a `Key`
///
/// Invariant 11 and §7.9: the component does not capture keys, it receives intent.
/// An arrow becomes `Up`, `Enter` becomes `Confirm` — that way a buffer does not need
/// to know about keysyms, modifiers or keyboard layout.
#[must_use]
fn input_of_key(sym: Keysym, utf8: Option<&str>) -> Option<ronoctua_protocol::Input> {
    use ronoctua_protocol::Input;
    Some(match sym {
        Keysym::Up => Input::Up,
        Keysym::Down => Input::Down,
        Keysym::Left => Input::Left,
        Keysym::Right => Input::Right,
        Keysym::Return | Keysym::KP_Enter => Input::Confirm,
        Keysym::Escape => Input::Cancel,
        Keysym::BackSpace => Input::Delete,
        // `ISO_Left_Tab` is what xkb delivers on `Shift+Tab`. Both go
        // to the same place: without this the second would fall into the `_` and enter
        // the search as a character.
        Keysym::Tab | Keysym::ISO_Left_Tab => Input::Toggle,
        _ => Input::Char(utf8?.chars().next()?),
    })
}

/// Puts a text on the system clipboard.
///
/// # Why a process, and not a `wl_data_device` of our own
///
/// Because whoever **owns** the selection has to stay alive to serve it
/// when someone pastes. The buffer's window dies when the browser is closed
/// — and copying a glyph **in order to** close the browser and paste is exactly the
/// use case. An owner that dies with it would hand over an empty clipboard, which
/// is worse than not copying: it looks as if it worked.
///
/// `wl-copy` leaves a minimal process serving the selection until someone
/// replaces it. It is the same choice as `niri msg action`: using the client that
/// already exists instead of reimplementing the other side's protocol.
///
/// # Errors
///
/// The binary was not put in the build, did not run, or exited with an error.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write as _;

    // A STORE PATH, never a name: the `PATH` of a systemd scope is
    // minimal, and the failure would show up far from the cause.
    let path = option_env!("RONOCTUA_WL_COPY").context(
        "RONOCTUA_WL_COPY nao foi definida no build.\n\
         Ela vem de `nix/rust.nix`; sem ela nao ha como copiar.",
    )?;

    let mut cmd = std::process::Command::new(path);
    // ITS PATH, and not ours. See `RONOCTUA_WL_COPY_PATH` in
    // `nix/rust.nix`: `wl-copy` does `exec cat`, and under a systemd
    // unit PATH is minimal. Without this the copy fails with
    // `exec cat: Node such file or directory` — and failing silently here
    // would be going back to the defect this whole path fixes.
    if let Some(p) = option_env!("RONOCTUA_WL_COPY_PATH") {
        cmd.env("PATH", p);
    }
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        // INHERITED, and not a pipe of ours.
        //
        // Why this matters: `wl-copy` FORKS a child that stays serving
        // the selection, and that child inherits the pipes. `wait_with_output()` reads the
        // pipes to EOF — and the EOF only comes when the selection's server dies,
        // that is, never. The buffer's event loop stayed stuck there, and
        // on screen that appeared as "the buffer stopped responding after
        // copying". By inheriting, `wl-copy`'s error message lands in OUR
        // journal, which is where it is any use.
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .with_context(|| format!("nao consegui executar {path}"))?;

    child
        .stdin
        .take()
        .context("o wl-copy nao abriu stdin")?
        .write_all(text.as_bytes())
        .context("nao consegui escrever no wl-copy")?;

    // WAIT FOR THE PARENT, and only it. `wl-copy` forks: the parent exits as soon as the
    // selection is set, and the child stays serving. Without waiting, an error
    // (a compositor with no `wl_data_device`, for example) would come out as success, and
    // we would be back to the defect this fixes.
    //
    // `wait()`, and NOT `wait_with_output()`: see the `stderr` comment.
    let status = child.wait().context("o wl-copy nao terminou")?;
    if !status.success() {
        anyhow::bail!("o wl-copy saiu com {status}; o motivo esta no log acima");
    }
    Ok(())
}

/// Reads what is on the system clipboard.
///
/// [`copy_to_clipboard`]'s counterpart. What reads is the host, and not the component: the buffer does not
/// touch the clipboard as it does not touch a file (invariant 11).
///
/// # Errors
///
/// The binary was not put in the build, did not run, or exited with an error.
fn paste_text() -> Result<String> {
    let path = option_env!("RONOCTUA_WL_PASTE").context(
        "RONOCTUA_WL_PASTE nao foi definida no build.\n\
         Ela vem de `nix/rust.nix`; sem ela nao ha como colar.",
    )?;

    let mut cmd = std::process::Command::new(path);
    // THE SAME PATH as `wl-copy`'s, and for the same reason: under a systemd
    // unit it is minimal, and these tools spawn by NAME.
    if let Some(p) = option_env!("RONOCTUA_WL_COPY_PATH") {
        cmd.env("PATH", p);
    }
    let output = cmd
        // `--no-newline`: `wl-paste` adds a `\n` that was not in
        // what was copied, and it would become one more character in the search.
        .arg("--no-newline")
        .output()
        .with_context(|| format!("nao consegui executar {path}"))?;

    // AN EMPTY CLIPBOARD IS NOT AN ERROR. `wl-paste` exits 1 when there is no selection, and
    // treating that as a failure would fill the log with errors every time someone
    // pressed `Ctrl+V` without having copied anything.
    if !output.status.success() {
        // THE REASON IN THE LOG, even though it is a normal case. Without it, "empty clipboard"
        // and "wl-paste failed" become the same line — and it was exactly that
        // ambiguity that made `Ctrl+V` look as if it did not exist.
        tracing::debug!(
            status = %output.status,
            error = %String::from_utf8_lossy(&output.stderr).trim(),
            "o wl-paste nao devolveu texto"
        );
        return Ok(String::new());
    }
    // HERE THE OUTPUT IS READ TO THE END, and it can be: `wl-paste` does NOT leave a child
    // serving anything — unlike `wl-copy`, where that froze the loop.
    String::from_utf8(output.stdout).context("o que estava copiado nao e texto UTF-8")
}

/// Opens an `xdg_toplevel` and draws a component in it — section 7.5.
///
/// # Errors
///
/// There is no compositor, the component does not load, or the `render` fails.
pub fn host_it(path: &std::path::Path, file: Option<&std::path::Path>) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("nao ha compositor Wayland acessivel (WAYLAND_DISPLAY esta definida?)")?;
    let (globals, fila) = registry_queue_init(&conn).context("registry do Wayland")?;
    let qh: QueueHandle<Host> = fila.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("sem wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("o compositor nao expoe xdg_shell")?;

    // THE INSTANCE LIVES. Loaded once, it keeps the component's
    // state between keys — search, cursor, scroll.
    let wasm =
        std::fs::read(path).with_context(|| format!("nao consegui ler {}", path.display()))?;
    let runtime = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = runtime.precompile(&wasm)?;
    let component = runtime.load(&cwasm)?;
    let mut instance = runtime.open(&component)?;

    // The descriptor BEFORE any render: it is what declares the keymap and
    // the capabilities, and §7.9 wants the refusal at load, not afterwards.
    let d = instance.init()?;
    if let Err(e) = d.validate() {
        anyhow::bail!("o descritor do buffer nao passa na guarda da secao 7.9: {e}");
    }
    tracing::info!(name = d.name, mode = d.mode, "componente carregado");

    // THE TITLE AND THE `app_id` COME FROM THE DESCRIPTOR.
    //
    // They used to be a fixed `ronoctua: buffer` and `br.ronoctua.buffer`, and the modeline —
    // which reads the title and app id from the compositor — showed `ronoctua: buffer
    // buffer` for any component. It said the same thing twice and did not
    // say WHICH buffer.
    //
    // `*name*` is Emacs's convention for a buffer that is not a file, the same one
    // as the `*scratch*` the modeline already uses in the empty state.
    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR, never to us. Drawing a
    // border or a title bar here would be reimplementing window layout, and
    // invariant 4 forbids it. niri runs with `prefer-no-csd`, so in
    // practice there is no decoration at all — which is what section 3 wants.
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    window.set_title(format!("*{}*", d.name));
    window.set_app_id(format!("br.ronoctua.{}", d.mode));
    window.commit();

    // The initial columns and rows are a guess; the `configure` brings the actual
    // size and the `render` runs again with it.
    let list = instance.render(80, 24, Host::heights())?;
    tracing::info!(
        commands = list.commands.len(),
        height = list.height(),
        "componente renderizou"
    );

    // THE CAPABILITY GATE, enforced HERE.
    //
    // The WIT has always promised that "the host refuses whoever uses without having asked",
    // and until now it was only a promise: `validate_it` checked the keymap's prefix and
    // nothing else. The first `render` happens BEFORE the window exists, so
    // a buffer that draws a gated primitive without having asked for it dies at
    // startup — which is where §7.9 wants the refusal.
    if let Err(e) = list.validate(&d.capabilities) {
        anyhow::bail!("o buffer `{}` usa primitiva sem capacidade: {e}", d.name);
    }

    // THE SYSTEM HALF EXISTS ONLY FOR WHOEVER ASKED FOR IT.
    //
    // Not a runtime check inside the tick, but the absence of the tick: a
    // buffer without the capability has no clock, no thread and no `systemctl`
    // — the cost of this surface is paid only by the buffer that uses it.
    let system = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::System)
        .then(System::fresh);
    // And the same for the queue: no capability, no thread and no socket.
    let queue = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Transfers)
        .then(Queue::fresh);

    let mut state = Host {
        system,
        queue,
        keymap: d.keymap.clone(),
        pending: Vec::new(),
        file: file.map(std::path::Path::to_path_buf),
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
        quit: false,
        gpu: None,
        text: None,
        images: None,
        quads: None,
        list,
        rasterized: std::collections::HashMap::new(),
        capabilities: d.capabilities,
        texts: ronoctua_i18n::Texts::from_env(),
        instance,
        _runtime: runtime,
    };

    let mut cycle: EventLoop<Host> = EventLoop::try_new().context("calloop")?;
    WaylandSource::new(conn, fila)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("nao consegui pendurar o Wayland: {e}"))?;

    // THE KEYMAP GOES OUT, AND COMMANDS COME BACK IN.
    //
    // The socket thread cannot touch the component: the state lives in the
    // `Instance` and it is not `Send`. So it puts the id on a calloop channel
    // and the LOOP delivers it, on the same thread as every other input — which
    // is the arrangement the footer already uses for its control socket.
    let (orders, channel) = calloop_channel::channel::<String>();
    let published = publish_keymap(d.keymap, orders);
    if cycle
        .handle()
        .insert_source(channel, |event, (), state| {
            if let calloop_channel::Event::Msg(id) = event {
                state.deliver(&ronoctua_protocol::Input::Command(id));
            }
        })
        .is_err()
    {
        tracing::warn!("nao consegui pendurar o canal do keymap");
    }

    // THE CLOCK, and the thread that answers a scan — only for the buffer that
    // declared the capability.
    if state.system.is_some() {
        // The scan's channel FIRST: the tick may start one, and a report with
        // nowhere to arrive would be a scan that runs and never shows.
        let (reports, arrivals) = calloop_channel::channel::<Scanned>();
        if cycle
            .handle()
            .insert_source(arrivals, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(m) = event {
                    state.scanned(m);
                }
            })
            .is_ok()
        {
            if let Some(s) = state.system.as_mut() {
                s.reports = Some(reports);
            }
        } else {
            tracing::warn!("nao consegui pendurar o canal da varredura");
        }

        // ONE SECOND, and the survey it carries is only pseudo-file reads —
        // microseconds. What costs (systemd, the journal, a walk of the disk)
        // is deliberately NOT here: the tick is the cheap half by construction,
        // which is what lets it run inside the loop that also draws.
        let tick = timer::Timer::from_duration(std::time::Duration::from_secs(1));
        if cycle
            .handle()
            .insert_source(tick, |_, (), state: &mut Host| {
                state.sample();
                timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
            })
            .is_err()
        {
            tracing::warn!("nao consegui pendurar o relogio da amostragem");
        }
        // The first survey does not wait a second: the screen opens with
        // numbers, and not with a blank that fills in later.
        //
        // SAMPLE FIRST, AND THE ORDER IS NOT STYLE: the guard's ceiling comes
        // back from systemd in bytes and is shown as a fraction of RAM, so
        // reading it before anything knows how much RAM there is prints `0.0%`
        // — a wrong number on the first screen, which is the one screen
        // everybody sees.
        state.sample();
        state.refresh_guard();
        state.sample();
    }

    // THE QUEUE'S SUBSCRIPTION, and it is NOT a clock.
    //
    // A socket round trip has no business inside a one-second timer: the unit
    // already knows when something changed, and `client::watch` blocks on the
    // socket until it does. So it is a thread that blocks and a channel that
    // wakes the loop — the same shape the scan uses, and the same one the
    // footer already uses for this very list.
    if state.queue.is_some() {
        let (send, arrivals) = calloop_channel::channel::<Vec<ronoctua_transfers::Job>>();
        if cycle
            .handle()
            .insert_source(arrivals, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(jobs) = event {
                    state.queue_arrived(&jobs);
                }
            })
            .is_ok()
        {
            // `listen` starts the unit if it is not up, so an empty screen here
            // means an empty queue and never a queue nobody asked.
            if !crate::transfers::listen(send) {
                tracing::warn!("sem fila de transferencias; o buffer abre vazio");
            }
        } else {
            tracing::warn!("nao consegui pendurar o canal da fila");
        }
    }

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            if e.quit {
                signal.stop();
            }
        })
        .context("loop de eventos")?;
    // The socket outlives nothing: a stale one in `XDG_RUNTIME_DIR` would make
    // the footer connect to a buffer that is not there and wait.
    if let Some(path) = published {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// PUBLISHES THIS BUFFER'S KEYMAP, and takes commands back.
///
/// # Why this exists at all
///
/// The buffer's host resolves its own keymap, and it has to: a buffer is a
/// window of its own (invariant 4), so while it has the focus it is THIS process
/// that holds the keyboard. The footer only knows the windows the compositor
/// tells it about — id, title, `app_id` — and none of that is a key binding.
///
/// So `SPC m` in the footer had nothing to list. This socket is the answer, and
/// it carries BOTH directions on purpose: a list of keys with no way to run them
/// would teach shortcuts that do nothing, which is worse than not listing.
///
/// # The protocol, which is the footer's own
///
/// `keymap` answers one line per binding, `<sequence> <id>`. `command <id>` puts
/// the id on the channel and answers `ok`. Anything else answers `?`.
///
/// # It never brings the buffer down
///
/// A buffer that cannot open its socket is a buffer whose keymap the footer
/// cannot list — it is not a buffer that fails to open. The editor still edits.
fn publish_keymap(
    keymap: Vec<ronoctua_protocol::Binding>,
    orders: calloop_channel::Sender<String>,
) -> Option<std::path::PathBuf> {
    use std::io::{BufRead as _, Write as _};

    let path = match crate::leader::buffer_socket_path(std::process::id()) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "sem socket do buffer; o SPC m nao vai lista-lo");
            return None;
        }
    };
    // An orphan socket from a dead process of the same PID would block the bind.
    let _ = std::fs::remove_file(&path);
    let listener = match std::os::unix::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "nao consegui abrir o socket do buffer");
            return None;
        }
    };
    tracing::info!(path = %path.display(), bindings = keymap.len(), "keymap publicado");

    let answer = keymap
        .iter()
        .map(|b| format!("{} {}", b.keys, b.command))
        .collect::<Vec<_>>()
        .join("\n");
    let known: std::collections::HashSet<String> = keymap.into_iter().map(|b| b.command).collect();

    let built = std::thread::Builder::new()
        .name("keymap".into())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(mut stream) = connection else { continue };
                let mut line = String::new();
                if std::io::BufReader::new(match stream.try_clone() {
                    Ok(c) => c,
                    Err(_) => continue,
                })
                .read_line(&mut line)
                .is_err()
                {
                    continue;
                }
                let reply = match line.trim().split_once(' ') {
                    None if line.trim() == "keymap" => answer.clone(),
                    // ONLY A COMMAND THIS BUFFER DECLARED.
                    //
                    // The socket is in `XDG_RUNTIME_DIR` and so is reachable by
                    // anything running as this user. Forwarding an arbitrary id
                    // would make it a way to drive the component from outside,
                    // which is a wider door than the feature needs: the footer
                    // only ever sends back what it was just told.
                    Some(("command", id)) if known.contains(id) => {
                        if orders.send(id.to_owned()).is_err() {
                            // The loop is gone; so is the reason to listen.
                            return;
                        }
                        "ok".into()
                    }
                    _ => {
                        tracing::warn!(
                            order = line.trim(),
                            "ordem desconhecida no socket do buffer"
                        );
                        "?".into()
                    }
                };
                let _ = writeln!(stream, "{reply}");
            }
        });
    if let Err(e) = built {
        tracing::warn!(error = %e, "nao consegui subir a thread do keymap");
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(path)
}

impl Host {
    /// Feeds a character to the buffer's local keymap.
    ///
    /// Returns `Some(Input::Command)` when a sequence COMPLETES, and `None`
    /// when the character does not finish one. A partial sequence is swallowed.
    ///
    /// It is the CALLER that decides when a sequence may start: the first key
    /// comes with `Ctrl`, and only then do the plain letters that follow reach
    /// here. Letting any letter open a sequence is what made the editor eat
    /// every `m`.
    fn local_key(&mut self, c: char) -> Option<ronoctua_protocol::Input> {
        let mut pending = std::mem::take(&mut self.pending);
        let hit = advance_sequence(&self.keymap, &mut pending, c);
        self.pending = pending;
        if let Some(command) = hit {
            tracing::info!(command, "keymap do buffer");
            return Some(ronoctua_protocol::Input::Command(command));
        }
        None
    }

    /// Rasterises the list's vector drawings and says where each one fits.
    ///
    /// # Why here, and not on every frame
    ///
    /// Because `paint` only runs when the component asks (`Effect::Redraw`),
    /// that is **per key**, and the cache keyed by the SVG's string absorbs the keys
    /// that do not change the drawing. One rasterisation per frame would put resvg in the
    /// frame budget just to redraw the same icon.
    ///
    /// What does **not** rasterise is left out, and the area is simply not
    /// drawn: a half-typed colour (`#8ab`) makes the preview disappear
    /// while one types, which is true information. Inventing a drawing
    /// in its place would be worse.
    fn vector_ones(&mut self) -> Vec<VectorDrawing> {
        use ronoctua_protocol::Command;
        // THE SAME cell as the click's and the text's — see [`Self::cell`]. A
        // second calculation here would make the drawing land in one place and the click find it
        // in another, with the error growing with the distance from the origin.
        let (cell, line_height) = Self::cell();
        let mut out = Vec::new();
        for c in &self.list.commands {
            let Command::Svg {
                line,
                column,
                lines,
                columns,
                svg,
                color,
            } = c
            else {
                continue;
            };
            let Some(token) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "cor invalida num svg");
                continue;
            };
            let key = svg.clone();
            if !self.rasterized.contains_key(&key) {
                // THE SIDE IN PIXELS comes from the rectangle of CELLS the buffer
                // asked for — it does not know pixels.
                let side = (f32::from(*columns) * cell)
                    .min(f32::from(*lines) * line_height)
                    .max(1.0) as u32;
                match rasterize_preview(svg, token, side) {
                    Some(r) => {
                        self.rasterized.insert(key.clone(), r);
                    }
                    None => {
                        tracing::debug!(bytes = svg.len(), "o svg do buffer nao rasterizou");
                        continue;
                    }
                }
            }
            let Some(r) = self.rasterized.get(&key) else {
                continue;
            };
            // THE MARGIN COMES IN HERE, as it does in `with_margin` for the text: the
            // component counts from zero and does not know where its window
            // is (invariant 11). Without it the drawing would come out shifted from the
            // text beside it, and only diagonally — the hardest kind to detect.
            let x = f32::from(column.saturating_add(tokens::space::BUFFER_MARGINX)) * cell;
            let y = f32::from(line.saturating_add(tokens::space::BUFFER_MARGINY)) * line_height;
            out.push(VectorDrawing {
                area: ronoctua_render::image::Area {
                    x,
                    y,
                    width: f32::from(*columns) * cell,
                    height: f32::from(*lines) * line_height,
                }
                // CENTRE, and do not fit: `fit_into` would grow to the edge and would
                // undo the rasterisation at the natural size, which is what makes
                // the size adjustment show.
                .centre(r.width, r.height),
                key,
            });
        }
        out
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let (columns, _) = self.context_of();
        let spans = spans_of(&self.list, &self.texts, columns);
        let vector_ones = self.vector_ones();
        let selections = self.selections();
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
        if let Some(im) = self.images.as_mut() {
            let batch: Vec<(String, ronoctua_render::image::Area)> = vector_ones
                .into_iter()
                .map(|v| {
                    if im.missing(&v.key)
                        && let Some(r) = self.rasterized.get(&v.key)
                    {
                        im.upload_it(&gpu.device, &gpu.queue, &v.key, r);
                    }
                    (v.key, v.area)
                })
                .collect();
            im.prepare(&gpu.queue, gpu.config.width, gpu.config.height, &batch);
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(
                &gpu.device,
                &gpu.queue,
                gpu.config.width,
                gpu.config.height,
                &selections,
            );
        }
        let images = self.images.as_ref();
        let quads = self.quads.as_ref();
        if let Err(e) = gpu.frame(|pass| {
            // THE SELECTION FIRST OF ALL: it is a background, and anything drawn
            // after it stays on top. Drawing it last would hide the very text it
            // exists to highlight.
            if let Some(q) = quads {
                q.draw(pass);
            }
            // THE DRAWING BELOW THE TEXT, as in the footer: what is written
            // over it has to stay legible.
            if let Some(im) = images {
                im.draw(pass);
            }
            text.draw(ronoctua_render::Atlas::Pinned, pass)
        }) {
            tracing::error!(error = %e, "desenhar o frame falhou");
            return;
        }
        tracing::info!(spans = spans.len(), "buffer desenhado no xdg_toplevel");
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "flush falhou");
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
        match gpu {
            Ok(g) => match ronoctua_render::Text::new(
                &g.device,
                &g.queue,
                g.config.format,
                // SIZE_LARGE and not SIZE_BASE: the footer is peripheral
                // reference, the buffer is what one reads. Having both at the same
                // size made the content look like a footnote.
                tokens::font::SIZE_LARGE,
            ) {
                Ok(t) => {
                    // THE IMAGE PIPELINE is born along with the text one, and its
                    // failure does NOT take the buffer down: without it what is lost
                    // is the vector preview, and the text goes on drawing.
                    match ronoctua_render::image::Images::new(&g.device, g.config.format) {
                        Ok(im) => self.images = Some(im),
                        Err(e) => tracing::error!(error = %e, "pipeline de imagem falhou"),
                    }
                    // THE QUAD PIPELINE, for `Command::Selection`. Same rule as
                    // the image one: its failure costs the highlight, not the
                    // buffer.
                    match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
                        Ok(q) => self.quads = Some(q),
                        Err(e) => tracing::error!(error = %e, "pipeline de quad falhou"),
                    }
                    self.text = Some(t);
                    self.gpu = Some(g);
                    tracing::info!("contexto grafico do buffer pronto");
                }
                Err(e) => tracing::error!(error = %e, "desenhador falhou"),
            },
            Err(e) => tracing::error!(error = %e, "sem contexto grafico"),
        }
    }
}

impl Host {
    /// The cell, in pixels: width and height.
    ///
    /// ONE place only, because it is used by two calculations that **have** to agree:
    /// how many cells fit in the window ([`Self::context_of`]) and which cell
    /// a pixel landed in ([`Self::cell_at`]). Diverging, the click hits one
    /// glyph and the drawing shows another — and the error grows with the distance from the
    /// origin, which is the worst kind of error to reproduce.
    /// The focused rows, as rectangles.
    ///
    /// It uses the SAME cell as the text and the click ([`Self::cell`]): a second
    /// calculation would paint the row in one place while the click found it in
    /// another, with the error growing away from the origin.
    fn selections(&self) -> Vec<ronoctua_render::quad::Quad> {
        use ronoctua_protocol::Command;
        let (_, line_height) = Self::cell();
        self.list
            .commands
            .iter()
            .filter_map(|c| {
                let Command::Selection { line, color } = c else {
                    return None;
                };
                let token = token_color(color.as_str()).or_else(|| {
                    tracing::error!(token = color.as_str(), "cor invalida numa selecao");
                    None
                })?;
                Some(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(line.saturating_add(tokens::space::BUFFER_MARGINY)) * line_height,
                    // THE WHOLE ROW, edge to edge. Invariant 15: it is a change
                    // of colour, not a box — no width to fit, no radius.
                    width: self.width as f32,
                    height: line_height,
                    color: token,
                })
            })
            .collect()
    }

    fn cell() -> (f32, f32) {
        (
            tokens::font::SIZE_LARGE * tokens::font::CELL_WIDTH_EM,
            tokens::font::SIZE_LARGE * tokens::font::LINE_HEIGHT,
        )
    }

    /// Which cell OF THE BUFFER a point in the window landed in.
    ///
    /// `None` when the point is in the margin: there is no content there, and returning
    /// cell 0 would make clicking the border select the first item.
    ///
    /// The origin is the **buffer's** (0,0) cell, and not the window's corner — it is the
    /// same origin `spans_of` uses when drawing, and that is why the margin
    /// comes in here as a subtraction.
    fn cell_at(x: f64, y: f64) -> Option<(u16, u16)> {
        let (width, height) = Self::cell();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / f64::from(width)) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / f64::from(height)) as u32;
        Some((
            u16::try_from(column.checked_sub(u32::from(tokens::space::BUFFER_MARGINX))?).ok()?,
            u16::try_from(line.checked_sub(u32::from(tokens::space::BUFFER_MARGINY))?).ok()?,
        ))
    }

    /// How many columns and rows fit in the window right now.
    ///
    /// Integer arithmetic over the cell, as in the footer: measuring text
    /// here would trade a calculation for a shaping on every `configure`.
    fn context_of(&self) -> (u16, u16) {
        // THE GRID IS ALWAYS THE NORMAL SIZE'S. A `large` run occupies
        // more than one cell, and what spaces it is whatever draws — that is how
        // the browser's grid came to move two rows at a time.
        let (cell_width_px, altura_celula) = Self::cell();
        // It subtracts the margins on BOTH sides: the content fits in what is left,
        // and not in what the window measures.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let columns = ((self.width as f32 / cell_width_px) as u32)
            .saturating_sub(u32::from(tokens::space::BUFFER_MARGINX) * 2)
            .clamp(1, u32::from(u16::MAX)) as u16;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = ((self.height as f32 / altura_celula) as u32)
            .saturating_sub(u32::from(tokens::space::BUFFER_MARGINY) * 2)
            .clamp(1, u32::from(u16::MAX)) as u16;
        (columns, lines)
    }

    /// Each scale's heights, in cells.
    ///
    /// Computed from the SAME tokens `scale_in_px` uses: if the two calculations
    /// diverge, the buffer reserves one number of cells and the run occupies
    /// another — which was exactly the clipped-glyph defect.
    fn heights() -> ronoctua_protocol::Heights {
        ronoctua_protocol::Heights::from_sizes(
            tokens::font::SIZE_SMALL,
            tokens::font::SIZE_BASE,
            tokens::font::SIZE_LARGE,
            tokens::font::SIZE_XLARGE,
            tokens::font::SIZE_XXLARGE,
            tokens::font::LINE_HEIGHT,
        )
    }

    /// Asks the component for a new frame and redraws.
    fn request_redraw(&mut self) {
        let (columns, lines) = self.context_of();
        match self.instance.render(columns, lines, Self::heights()) {
            // THE GATE HOLDS ON EVERY FRAME, and not only the first.
            //
            // A component can emit the gated primitive only after
            // a key — the sheet's preview is exactly that case: it does not
            // exist in the grid. Checking once would be checking the frame in which
            // it does not appear. The whole list is discarded, and not only the
            // command: drawing half of what the buffer asked for is worse than not
            // drawing, because it looks as if it works.
            Ok(l) => match l.validate(&self.capabilities) {
                Ok(()) => self.list = l,
                Err(e) => {
                    tracing::error!(error = %e, "quadro recusado: primitiva sem capacidade");
                    return;
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "o render do componente falhou");
                return;
            }
        }
        self.paint();
    }

    /// Delivers a key to the component and runs what it ASKS for.
    fn deliver(&mut self, input_of: &ronoctua_protocol::Input) {
        let effects = match self.instance.event(input_of) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, "o evento do componente falhou");
                return;
            }
        };
        // INPUT AND EFFECTS IN THE LOG, always.
        //
        // Without this, "the key did nothing" does not tell "it did not arrive" from
        // "it arrived and the component returned an empty list" — and the two are
        // fixed in opposite places. It was that doubt that cost a whole
        // round of VM testing.
        tracing::debug!(input = ?input_of, effects = effects.len(), "entrada entregue");
        let mut verdict = None;
        for effect in effects {
            match effect {
                ronoctua_protocol::Effect::Redraw => self.request_redraw(),
                // The component does NOT touch the clipboard: it asks,
                // and the host does it (invariant 11).
                ronoctua_protocol::Effect::Copy(t) => {
                    let ok = match copy_to_clipboard(&t) {
                        Ok(()) => {
                            tracing::info!(bytes = t.len(), "copiado");
                            true
                        }
                        // LOUD, and not a `debug`. This here WAS a
                        // `tracing::info!` that copied nothing, and nobody
                        // noticed because copying silently and not copying
                        // silently are indistinguishable.
                        Err(e) => {
                            tracing::error!(error = %e, "copiar FALHOU");
                            false
                        }
                    };
                    // THE VERDICT GOES BACK TO THE COMPONENT, so it can
                    // say on screen what happened. What knows whether it worked is
                    // this side; without the way back, the buffer could only show
                    // an optimistic "copied!" — which lies precisely when it
                    // matters. Outside the loop so as not to re-enter `event` in the
                    // middle of the effect list.
                    verdict = Some(ok);
                }
                ronoctua_protocol::Effect::Command(id) => {
                    tracing::info!(command = %id, "comando pedido pelo buffer");
                }
                // WHAT ACTS ON THE MACHINE, and the gate in front of it.
                //
                // The check is here and not inside `ask` so that the refusal is
                // one line for every request there will ever be — and so that
                // it says WHICH capability was missing, instead of the request
                // quietly doing nothing.
                ronoctua_protocol::Effect::Ask(demand) => {
                    if let Some(needed) = ronoctua_protocol::Effect::Ask(demand).requires()
                        && !self.capabilities.contains(&needed)
                    {
                        tracing::error!(
                            request = ?demand.what,
                            capability = ?needed,
                            "pedido recusado: o buffer nao declarou a capacidade"
                        );
                        continue;
                    }
                    self.ask(demand);
                }
                ronoctua_protocol::Effect::Close => self.quit = true,
                // THE BUFFER DOES NOT TOUCH THE DISK: it asks, and the host
                // writes (invariant 11) — the same arrangement as `Copy`.
                ronoctua_protocol::Effect::Save(text) => {
                    let ok = match &self.file {
                        Some(path) => match std::fs::write(path, &text) {
                            Ok(()) => {
                                tracing::info!(bytes = text.len(), path = %path.display(), "salvo");
                                true
                            }
                            // LOUD, for the same reason as `Copy`: saving in
                            // silence and failing in silence look identical, and
                            // the one that loses work is the second.
                            Err(e) => {
                                tracing::error!(error = %e, path = %path.display(), "salvar FALHOU");
                                false
                            }
                        },
                        None => {
                            tracing::error!(
                                "o buffer pediu para salvar e nao foi aberto com arquivo"
                            );
                            false
                        }
                    };
                    // The verdict goes back, so the buffer can say on screen
                    // what happened instead of showing an optimistic "saved!".
                    verdict = Some(ok);
                }
            }
        }
        if let Some(ok) = verdict {
            self.deliver(&ronoctua_protocol::Input::Copied(ok));
        }
    }
}

impl System {
    /// The system half, before anything has been read.
    fn fresh() -> Self {
        Self {
            places: ronoctua_memory::Places::default(),
            guard: ronoctua_protocol::Guard::default(),
            consumers: Vec::new(),
            scanning: false,
            progress: 0,
            last_scan: None,
            cancel: None,
            reports: None,
            ram_total: 0,
        }
    }
}

/// The system half of the host: the clock, the scans and the policy.
impl Host {
    /// Reads the cheap numbers and hands them to the component.
    ///
    /// Everything here is a pseudo-file: it costs microseconds and never
    /// touches the disk, which is why a clock may call it. The policy and the
    /// consumers are NOT read here — they are what the last answer left behind.
    fn sample(&mut self) {
        let Some(s) = self.system.as_mut() else {
            return;
        };
        let mut survey = ronoctua_memory::survey(&s.places);
        // The machine's RAM, kept for whoever turns systemd's bytes back into a
        // percentage. Read from the survey so there is one reading, not two.
        if let Some(total) = survey
            .readings
            .iter()
            .find(|r| r.of == ronoctua_protocol::Gauge::RamTotal)
            .map(|r| r.bytes)
        {
            s.ram_total = total;
        }
        survey.guard = s.guard;
        survey.consumers.clone_from(&s.consumers);
        survey.scanning = s.scanning;
        survey.progress = s.progress;
        self.deliver(&ronoctua_protocol::Input::Survey(survey));
    }

    /// Asks systemd what is in force. Spawns processes — never on the clock.
    fn refresh_guard(&mut self) {
        let Some(s) = self.system.as_mut() else {
            return;
        };
        s.guard = ronoctua_memory::guard::state(s.ram_total);
    }

    /// A scan's progress or its result.
    fn scanned(&mut self, m: Scanned) {
        {
            let Some(s) = self.system.as_mut() else {
                return;
            };
            match m {
                Scanned::Step(p) => s.progress = p,
                Scanned::Done(list) => {
                    s.consumers = list;
                    s.scanning = false;
                    s.progress = 1000;
                    s.cancel = None;
                }
            }
        }
        // The screen only moves when the survey goes back in — the component
        // holds no state the host can write to.
        self.sample();
    }

    /// Does what the buffer asked.
    ///
    /// Every arm that changes the machine reports its verdict the same way a
    /// copy or a save does: through the log, and through the next survey, which
    /// carries the state as systemd reports it — never as we requested it.
    fn ask(&mut self, demand: ronoctua_protocol::Demand) {
        use ronoctua_protocol::Request as R;
        match demand.what {
            R::Refresh => {
                self.refresh_guard();
                self.sample();
            }
            R::ScanRam => self.start_scan(Scan::Ram),
            R::ScanDisk => self.start_scan(Scan::Disk),
            R::CancelScan => self.cancel_scan(),
            R::GuardTolerant => self.switch_guard(ronoctua_protocol::GuardMode::Tolerant),
            R::GuardDefault => self.switch_guard(ronoctua_protocol::GuardMode::Strict),
            R::GuardPersist => {
                let mode = self
                    .system
                    .as_ref()
                    .map_or(ronoctua_protocol::GuardMode::Unknown, |s| s.guard.mode);
                match ronoctua_memory::guard::persist(mode) {
                    Ok(()) => tracing::info!(?mode, "perfil gravado no host.nix"),
                    Err(e) => tracing::error!(error = %e, "gravar o perfil FALHOU"),
                }
                self.refresh_guard();
                self.sample();
            }
            R::StopConsumer => self.stop_consumer(demand.target),
            R::PauseJob | R::ResumeJob | R::CancelJob | R::ForgetJob => {
                self.job_demand(demand.what, demand.target);
            }
            R::SweepJobs => Self::tell_queue(&ronoctua_transfers::Ask::Sweep),
            R::SweepStore | R::SweepGenerations | R::SweepJournal => {
                match ronoctua_memory::scan::sweep(demand.what) {
                    Ok(()) => tracing::info!(what = ?demand.what, "faxina concluida"),
                    Err(e) => tracing::error!(error = %e, what = ?demand.what, "faxina FALHOU"),
                }
                // The freed space only shows up on the next scan; the readings
                // that come from `statvfs` update on their own.
                self.sample();
            }
        }
    }

    /// A fresh queue arrived from the unit.
    ///
    /// The whole list, every time, and never a delta: the buffer holds no
    /// state that could go out of step, and the list is short enough that
    /// sending it whole costs less than being wrong about it.
    fn queue_arrived(&mut self, jobs: &[ronoctua_transfers::Job]) {
        let queue = crate::transfers::queue_of(jobs);
        if let Some(q) = self.queue.as_mut() {
            q.known.clear();
            q.known.extend(queue.jobs.iter().map(|j| j.id));
        }
        self.deliver(&ronoctua_protocol::Input::Queue(queue));
    }

    /// Passes one of the buffer's demands on to the unit, once the id checks out.
    fn job_demand(&mut self, what: ronoctua_protocol::Request, target: u64) {
        use ronoctua_protocol::Request as R;
        let Some(q) = self.queue.as_ref() else {
            return;
        };
        if !q.known.contains(&target) {
            // NOT an error the buffer can act on, and not a panic either: the
            // job finished between the draw and the key, which is ordinary. The
            // next ledger will already show it gone.
            tracing::warn!(id = target, "pedido para um trabalho que nao esta na lista");
            return;
        }
        let ask = match what {
            R::PauseJob => ronoctua_transfers::Ask::Pause { id: target },
            R::ResumeJob => ronoctua_transfers::Ask::Resume { id: target },
            R::CancelJob => ronoctua_transfers::Ask::Cancel { id: target },
            R::ForgetJob => ronoctua_transfers::Ask::Forget { id: target },
            // Unreachable by construction: `ask` only routes those four here.
            _ => return,
        };
        Self::tell_queue(&ask);
    }

    /// Says it to the unit, and says so in the log when it cannot.
    ///
    /// `tell` and not `ask`: none of these answer, and waiting for a reply that
    /// never comes would freeze this window's loop — which is the whole of the
    /// bug the footer's own split exists to avoid.
    fn tell_queue(what: &ronoctua_transfers::Ask) {
        if let Err(e) = ronoctua_transfers::client::tell(what) {
            tracing::error!(error = %e, ?what, "a fila nao aceitou o pedido");
        }
    }

    /// Applies a profile for this session, and reads back what took.
    fn switch_guard(&mut self, mode: ronoctua_protocol::GuardMode) {
        let table = ronoctua_memory::guard::profiles();
        match table.of(mode) {
            Some(p) => match ronoctua_memory::guard::apply(p) {
                Ok(()) => tracing::info!(?mode, "perfil aplicado para esta sessao"),
                // THE MANUAL WARNS that not every property may be changed at
                // runtime. If this one may not, the screen has to say so — which
                // it does by the next survey carrying the UNCHANGED state.
                Err(e) => tracing::error!(error = %e, ?mode, "o systemd recusou a troca"),
            },
            None => tracing::error!(?mode, "a tabela de perfis nao tem esse perfil"),
        }
        self.refresh_guard();
        self.sample();
    }

    /// Closes the consumer at a 1-based index of the last survey.
    fn stop_consumer(&mut self, target: u64) {
        let Some(s) = self.system.as_ref() else {
            return;
        };
        let Some(who) = usize::try_from(target)
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| s.consumers.get(n))
            .cloned()
        else {
            tracing::error!(target, "indice fora da lista da ultima amostra");
            return;
        };
        let places = s.places.clone();
        match ronoctua_memory::scan::stop(&places, &who) {
            Ok(()) => tracing::info!(name = %who.name, "encerrado"),
            // The refusal is the point of the message: the panel does not touch
            // what is outside `app.slice`, and saying so is how the person
            // learns the boundary instead of thinking the key is broken.
            Err(e) => tracing::error!(error = %e, name = %who.name, "encerrar FALHOU"),
        }
        // The list is stale the moment something leaves it: scan again, so the
        // screen does not offer to close what is already gone — and the SAME
        // scan, or the screen would change subject on its own.
        if let Some(again) = self.system.as_ref().and_then(|s| s.last_scan) {
            self.start_scan(again);
        }
    }

    /// Starts a scan, cancelling whichever was running.
    ///
    /// Starting while one runs CANCELS it — which is what makes an explicit
    /// stop optional rather than required, and keeps two threads from walking
    /// the same disk.
    fn start_scan(&mut self, what: Scan) {
        self.cancel_scan();
        let Some(s) = self.system.as_mut() else {
            return;
        };
        let Some(reports) = s.reports.clone() else {
            tracing::error!("sem canal de varredura; nada a fazer");
            return;
        };
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        s.cancel = Some(std::sync::Arc::clone(&cancel));
        s.last_scan = Some(what);
        s.scanning = true;
        s.progress = 0;
        let places = s.places.clone();

        // A THREAD, because this is the half that takes seconds. The loop that
        // draws is single-threaded, and waiting here would freeze the window —
        // the same trap `systemd-run --scope` set on the footer.
        let built = std::thread::Builder::new()
            .name(format!("ronoctua-scan-{what:?}").to_lowercase())
            .spawn(move || {
                let found = match what {
                    Scan::Ram => ronoctua_memory::scan::ram(&places),
                    Scan::Disk => {
                        let mut all = ronoctua_memory::scan::disk_metadata();
                        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
                        if let Some(home) = home {
                            all.extend(ronoctua_memory::scan::disk(&home, &cancel, |p| {
                                let _ = reports.send(Scanned::Step(p));
                            }));
                        }
                        all.sort_by_key(|c| std::cmp::Reverse(c.bytes));
                        all
                    }
                };
                // A closed channel means the window is gone; there is nobody to
                // tell, and that is not an error.
                let _ = reports.send(Scanned::Done(found));
            });
        if let Err(e) = built {
            tracing::error!(error = %e, "nao consegui subir a thread de varredura");
            s.scanning = false;
        }
        self.sample();
    }

    /// Tells a running scan to stop, and forgets it.
    fn cancel_scan(&mut self) {
        let Some(s) = self.system.as_mut() else {
            return;
        };
        if let Some(c) = s.cancel.take() {
            // The flag, and not just dropping the channel: a walk of a home
            // directory would keep consuming I/O long after the screen moved on.
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        s.scanning = false;
    }
}

/// Which scan.
#[derive(Debug, Clone, Copy)]
enum Scan {
    /// Who is holding RAM.
    Ram,
    /// Where the disk went.
    Disk,
}

impl SeatHandler for Host {
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
                Err(e) => tracing::warn!(error = %e, "sem teclado; o buffer nao recebe entrada"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                // With no pointer the buffer works entirely through the keyboard: the
                // click is a shortcut, not the only path.
                Err(e) => tracing::warn!(error = %e, "sem ponteiro; o clique nao vai funcionar"),
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

impl KeyboardHandler for Host {
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
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // `Ctrl+V` PASTES, and it is the only chord this host handles.
        //
        // It comes BEFORE the Ctrl guard below, otherwise it would fall into the `return` and
        // would do nothing — which is what happened: the search field ignored
        // `Ctrl+V` silently, and there was no way to tell that from "there was
        // nothing copied".
        //
        // What reads the clipboard is the HOST. The buffer does not read the
        // clipboard, as it does not read a file (invariant 11): it receives the text
        // already resolved.
        if self.mods.ctrl && event.keysym == Keysym::v {
            match paste_text() {
                Ok(t) if !t.is_empty() => self.deliver(&ronoctua_protocol::Input::Paste(t)),
                Ok(_) => tracing::debug!("area de transferencia vazia"),
                Err(e) => tracing::error!(error = %e, "colar FALHOU"),
            }
            return;
        }
        // With Ctrl the key is not text: it is a chord, and a chord belongs to the buffer's
        // keymap, which the HOST dispatches as `Input::Command`. Sending the
        // character would make `C-c` become a `c` in the search.
        if self.mods.ctrl || self.mods.logo {
            // THE BUFFER'S OWN KEYMAP OPENS HERE, and only here.
            //
            // The FIRST key of a sequence is a CHORD; the ones after it are
            // plain letters. It cost the editor to learn why: with the first
            // key plain, a keymap of `m s` swallowed EVERY `m` typed into the
            // file — and when the next letter did not continue the sequence,
            // both were dropped. No word with an `m` could be written.
            //
            // A chord is not text and a letter is, and that is the whole line.
            // `Ctrl` and not `Logo`: the compositor takes `Super` for itself.
            if opens_sequence(self.mods.ctrl, self.mods.logo, self.pending.is_empty())
                && let Some(c) = event.keysym.key_char()
                && let Some(input) = self.local_key(c)
            {
                self.deliver(&input);
            }
            return;
        }
        // A sequence ALREADY under way: from the second key on it is a plain
        // letter, as in `SPC m s`. `local_key` swallows what continues the
        // sequence and drops what kills it — see `advance_sequence`.
        if !self.pending.is_empty()
            && let Some(c) = event.utf8.as_deref().and_then(|s| s.chars().next())
        {
            if let Some(input) = self.local_key(c) {
                self.deliver(&input);
            }
            return;
        }
        let Some(e) = input_of_key(event.keysym, event.utf8.as_deref()) else {
            return;
        };
        self.deliver(&e);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // Repeating IS right here: holding the arrow scrolls the list, holding
        // backspace erases the search.
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

impl WindowHandler for Host {
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
        // A `None` size = "you choose". 640x400 is a reasonable guess for
        // a buffer that does not know its content yet.
        self.width = cfg.new_size.0.map_or(640, std::num::NonZeroU32::get);
        self.height = cfg.new_size.1.map_or(400, std::num::NonZeroU32::get);
        if let Some(g) = self.gpu.as_mut() {
            g.resize(self.width, self.height);
        }
        // IT RE-RENDERS with the actual size. The first `render` used a
        // guessed 80x24; without this the component would draw for a window
        // that is not its own — few lines on a large screen, exactly
        // what was seen in the glyph browser.
        self.request_redraw();
    }
}

impl CompositorHandler for Host {
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

impl OutputHandler for Host {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for Host {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

/// The left button from `linux/input-event-codes.h` (`BTN_LEFT`).
const LEFT_BUTTON: u32 = 0x110;

impl PointerHandler for Host {
    /// The click becomes a CELL and goes to the component.
    ///
    /// The left button only, and only on the press. The others do nothing on
    /// purpose: a context menu would be new chrome (invariant 14), and the middle
    /// one pasting text into a glyph browser would be a search typed
    /// by accident.
    ///
    /// There is no hover: highlighting the item under the mouse would ask for a frame per pixel
    /// of movement, and here the highlight already has an owner — it is the cursor, and it belongs to the
    /// component.
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            let PointerEventKind::Press { button, .. } = e.kind else {
                continue;
            };
            if button != LEFT_BUTTON {
                continue;
            }
            let Some((column, line)) = Self::cell_at(e.position.0, e.position.1) else {
                // A click on the margin. There is no content there, and sending cell 0
                // would make clicking the border select the first item.
                continue;
            };
            tracing::debug!(column, line, "clique entregue ao componente");
            self.deliver(&ronoctua_protocol::Input::Click { column, line });
        }
    }
}

smithay_client_toolkit::delegate_registry!(Host);
smithay_client_toolkit::delegate_dispatch2!(Host);

/// May a key OPEN a buffer sequence?
///
/// Pulled out so the rule can be tested without a compositor. It is ONE line,
/// and it is the line that decides whether the person can write the letter `m`
/// in a buffer whose keymap starts with `m`.
const fn opens_sequence(ctrl: bool, logo: bool, idle: bool) -> bool {
    // `logo` excluded and not ignored: `Super` belongs to the compositor, and a
    // buffer answering to it would steal a binding from niri.
    ctrl && !logo && idle
}

/// Resolves a character against a buffer's keymap.
///
/// Pulled out of [`Host::local_key`] so it can be tested WITHOUT a compositor:
/// the rest of that method is Wayland state, and this is the whole of the
/// decision.
fn advance_sequence(
    keymap: &[ronoctua_protocol::Binding],
    pending: &mut Vec<char>,
    c: char,
) -> Option<String> {
    if keymap.is_empty() {
        return None;
    }
    let mut wanted = pending.clone();
    wanted.push(c);
    let typed: String = wanted.iter().collect();

    if let Some(b) = keymap
        .iter()
        .find(|b| b.keys.split_whitespace().collect::<String>() == typed)
    {
        pending.clear();
        return Some(b.command.clone());
    }
    if keymap.iter().any(|b| {
        b.keys
            .split_whitespace()
            .collect::<String>()
            .starts_with(&typed)
    }) {
        *pending = wanted;
        return None;
    }
    // Neither: the sequence died. What was held back was never text — dropping
    // it is right, and re-injecting it would type `m` into the file a keystroke
    // after the person moved on.
    pending.clear();
    None
}

#[cfg(test)]
mod tests {
    /// The defect this gate exists for: the editor swallowed every `m`.
    ///
    /// A plain letter must NEVER open a sequence. When it did, typing `m` was
    /// held back and, if the next letter did not continue `m s`, both were
    /// dropped — no word with an `m` could be written into the file.
    #[test]
    fn a_plain_letter_does_not_open_a_sequence() {
        assert!(!super::opens_sequence(false, false, true));
    }

    #[test]
    fn a_chord_opens_a_sequence() {
        assert!(super::opens_sequence(true, false, true));
    }

    /// `Super` is the compositor's. A buffer answering to it would steal a
    /// binding from niri.
    #[test]
    fn super_does_not_open_a_sequence() {
        assert!(!super::opens_sequence(true, true, true));
    }

    /// With a sequence under way the gate is CLOSED: from the second key on the
    /// letters are plain, and it is the caller that routes them.
    #[test]
    fn a_sequence_under_way_does_not_open_another() {
        assert!(!super::opens_sequence(true, false, false));
    }

    use super::*;
    use ronoctua_protocol::{Color, Command, DisplayList, Text};

    /// The keymap the editor declares.
    fn buffer_keymap() -> Vec<ronoctua_protocol::Binding> {
        vec![ronoctua_protocol::Binding {
            keys: "m s".into(),
            command: "memory-refresh".into(),
        }]
    }

    #[test]
    fn the_sequence_only_fires_when_it_completes() {
        // It is what makes an editor SAVEABLE. The binding had been declared
        // since section 7.9 and nothing consumed it: the buffer would open,
        // edit, and lose the work.
        let km = buffer_keymap();
        let mut pending = Vec::new();
        assert_eq!(
            advance_sequence(&km, &mut pending, 'm'),
            None,
            "`m` sozinho nao dispara"
        );
        assert_eq!(pending, vec!['m'], "o prefixo tem de ficar pendente");
        assert_eq!(
            advance_sequence(&km, &mut pending, 's').as_deref(),
            Some("memory-refresh")
        );
        assert!(pending.is_empty(), "o disparo tem de limpar a sequencia");
    }

    #[test]
    fn a_dead_sequence_does_not_leak_into_the_text() {
        // `m` then `x`: no binding. Re-injecting the `m` would type it into the
        // file one keystroke after the person had moved on — the class of bug
        // that makes an editor untrustworthy.
        let km = buffer_keymap();
        let mut pending = Vec::new();
        advance_sequence(&km, &mut pending, 'm');
        assert_eq!(advance_sequence(&km, &mut pending, 'x'), None);
        assert!(pending.is_empty(), "sequencia morta tem de limpar");
    }

    #[test]
    fn a_buffer_with_no_keymap_swallows_nothing() {
        // Every other buffer types text. If an empty keymap held a character
        // back, the search in the glyph browser would lose its first letter.
        let mut pending = Vec::new();
        assert_eq!(advance_sequence(&[], &mut pending, 'm'), None);
        assert!(pending.is_empty());
    }

    /// An SVG in the form Lucide publishes: `currentColor` and an open stroke.
    const LUCIDE: &str = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" "#,
        r#"viewBox="0 0 24 24" fill="none" stroke="currentColor" "#,
        r#"stroke-width="2" stroke-linecap="round">"#,
        r#"<path d="M3 12h18" /><path d="M12 3v18" /></svg>"#
    );

    /// THE DEFECT: a black preview over a black background.
    ///
    /// `currentColor` **with no context is black**. Over the buffer's background the
    /// drawing would come out invisible — and invisible is indistinguishable from "this
    /// primitive is not implemented yet", which is the worst possible
    /// result: it looks as if nothing was done.
    #[test]
    fn the_current_color_becomes_the_token_and_the_drawing_shows_up() {
        let resolvido = resolver_current_color(LUCIDE, tokens::color::TEXT_PRIMARY);
        assert!(
            !resolvido.contains("currentColor"),
            "o `currentColor` sobreviveu: {resolvido}"
        );
        let [r, g, b, _] = tokens::color::TEXT_PRIMARY.to_rgba8();
        assert!(
            resolvido.contains(&format!("#{r:02X}{g:02X}{b:02X}")),
            "a cor do token nao entrou: {resolvido}"
        );

        // AND IT DRAWS. Finding the string is not drawing: a document `usvg`
        // refuses returns `None` in the same place, and the area stays empty.
        let side = 48;
        let rgba = ronoctua_notify::icon::from_svg(
            resolvido.as_bytes(),
            side,
            ronoctua_notify::icon::Fit::Fill,
        )
        .expect("o SVG da Lucide nao rasterizou");
        assert_eq!((rgba.width, rgba.height), (side, side));
        let opaque = rgba.pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(
            opaque > 100,
            "a cruz de duas linhas rasterizou com {opaque} pixels visiveis"
        );
    }

    /// THE DEFECT, SEEN ON SCREEN: the preview did not show the size.
    ///
    /// Lucide's SVG carries `width`/`height` in pixels and a fixed `viewBox` of
    /// 24. Rasterising to **fill** the box, a `width="24"` and a
    /// `width="512"` produce the same image — the size adjustment had
    /// no visible effect at all, and the preview confidently showed a size that
    /// was not the chosen one. `Natural` is what makes the row's number and the
    /// drawing say the same thing.
    #[test]
    fn the_preview_draws_at_the_documents_size() {
        let at = |px: u32| {
            LUCIDE.replace(
                r#"width="24" height="24""#,
                &format!(r#"width="{px}" height="{px}""#),
            )
        };
        // THROUGH THE PRODUCTION PATH, and not calling the rasteriser directly: the
        // wrong choice lives at the CALL SITE, and a test that passed
        // `Natural` by hand would stay green with the preview filling.
        //
        // The ceiling is generous: what is proved here is that the drawing FOLLOWS the
        // document, and not that it was clipped.
        let ceiling = 200;
        let preview = |px: u32| {
            rasterize_preview(&at(px), tokens::color::TEXT_PRIMARY, ceiling)
                .unwrap_or_else(|| panic!("o SVG de {px}px nao rasterizou"))
        };
        for px in [16u32, 24, 48, 128] {
            let r = preview(px);
            assert_eq!(
                (r.width, r.height),
                (px, px),
                "o documento pediu {px}px e saiu {}x{}",
                r.width,
                r.height
            );
        }
        // ABOVE THE CEILING it shrinks, like any viewer — the row's number
        // still says the actual size.
        let large = preview(512);
        assert_eq!((large.width, large.height), (ceiling, ceiling));

        // And the APPLICATION icon still FILLS: there the file's size
        // means nothing, and what matters is filling the cell.
        for px in [16u32, 512] {
            let r = ronoctua_notify::icon::from_svg(
                at(px).as_bytes(),
                18,
                ronoctua_notify::icon::Fit::Fill,
            )
            .expect("nao rasterizou");
            assert_eq!((r.width, r.height), (18, 18));
        }
    }

    /// And the AREA does not enlarge what already came out at the right size.
    ///
    /// `fit_into` grows to the edge; using it here would undo the natural
    /// rasterisation and would give the defect back — 24 px and 512 px would go back to being the same
    /// image, one step later.
    #[test]
    fn the_preview_area_centres_without_scaling_up() {
        use ronoctua_render::image::Area;
        let caixa = Area {
            x: 100.0,
            y: 10.0,
            width: 200.0,
            height: 200.0,
        };
        let small = caixa.centre(24, 24);
        assert_eq!((small.width, small.height), (24.0, 24.0));
        assert!(
            (small.x - 188.0).abs() < 0.01 && (small.y - 98.0).abs() < 0.01,
            "nao centrou: {small:?}"
        );
        // What does NOT fit shrinks, and then `centre` and `fit_into` agree.
        assert_eq!(caixa.centre(400, 400), caixa.fit_into(400, 400));
    }

    /// And the person's colour passes through intact: there it is **content**, not chrome.
    #[test]
    fn a_typed_colour_is_not_replaced() {
        let meu = LUCIDE.replace("currentColor", "#8AB4F8");
        assert_eq!(
            resolver_current_color(&meu, tokens::color::TEXT_PRIMARY),
            meu,
            "a cor escolhida foi trocada pela do token"
        );
    }

    /// A broken SVG is `None`, and the area stays empty — which is what one sees when
    /// typing a half-finished colour (`#8ab`). Inventing a drawing would be worse.
    #[test]
    fn an_invalid_svg_does_not_draw_and_does_not_panic() {
        assert!(
            ronoctua_notify::icon::from_svg(b"nao sou svg", 48, ronoctua_notify::icon::Fit::Fill)
                .is_none()
        );
        assert!(
            ronoctua_notify::icon::from_svg(LUCIDE.as_bytes(), 0, ronoctua_notify::icon::Fit::Fill)
                .is_none()
        );
    }

    #[test]
    fn the_key_is_translated_and_the_data_is_not() {
        // Invariant 11 and 8 meet here: the component sends a key
        // for a label and a literal for data, and what translates is the host.
        let t = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let mut d = DisplayList::fresh_one();
        d.push(Command::Text {
            line: 0,
            column: 0,
            text: Text::key("cmd-mx-open"),
            color: Color::name("text-primary"),
            scale: ronoctua_protocol::Scale::Normal,
            max_width: 0,
        });
        d.push(Command::Text {
            line: 1,
            column: 0,
            text: Text::datum("cmd-mx-open"),
            color: Color::name("text-primary"),
            scale: ronoctua_protocol::Scale::Normal,
            max_width: 0,
        });
        let v = spans_of(&d, &t, 80);
        assert_eq!(v[0].text, "Executar comando…");
        assert_eq!(v[1].text, "cmd-mx-open");
    }

    #[test]
    fn nothing_leaks_off_the_edge_nor_over_the_neighbour() {
        // THE DEFECT THIS CATCHES, and it showed up in a photo of the screen: the
        // invitation "Type to search for a Nerd Fonts glyph" running OVER
        // the counter `10473 / 10473`. Text over text is the worst possible
        // result — neither of the two stays legible, and there is no error
        // anywhere.
        //
        // There are TWO limits, and each one alone lets a case through: with no
        // budget, two runs collide; with no edge, a careless buffer
        // writes outside the window and the compositor does not clip.
        for columns in [10_u16, 20, 40, 80, 200] {
            // A long invitation with a short budget, and a counter at the end.
            let budget = columns.saturating_sub(12).max(1);
            let clipped = super::elide_to_budget(
                "Digite para buscar um glifo das Nerd Fonts",
                budget,
                2,
                columns,
            );
            let width = u16::try_from(clipped.chars().count()).unwrap_or(u16::MAX);
            assert!(
                width <= budget,
                "com {columns} colunas o texto passou do orcamento: \
                 {width} > {budget}"
            );
            assert!(
                2 + width <= columns,
                "com {columns} colunas o texto vazou da borda"
            );
        }
    }

    #[test]
    fn a_zero_budget_means_up_to_the_edge() {
        // `0` is "no budget of its own" — the common case, for whoever is the only
        // thing on the line. The edge still applies.
        let s = "x".repeat(100);
        let clipped = super::elide_to_budget(&s, 0, 5, 20);
        assert_eq!(clipped.chars().count(), 15, "devia ir ate a borda, e parou");
    }

    #[test]
    fn the_column_counts_up_to_the_edge() {
        // A run starting at column 70 of an 80-wide screen has ten
        // cells, and not eighty. Ignoring the column is like having no edge.
        let s = "y".repeat(50);
        assert_eq!(super::elide_to_budget(&s, 0, 70, 80).chars().count(), 10);
        // And at the exact edge, nothing is left — without panicking.
        assert!(super::elide_to_budget(&s, 0, 80, 80).is_empty());
        assert!(super::elide_to_budget(&s, 0, 90, 80).is_empty());
    }

    #[test]
    fn an_unknown_token_is_refused_and_not_papered_over() {
        // This used to return `TEXT_DIM` and carry on. The effect was a buffer with a
        // wrong name drawing nicely, and the defect only showing up when
        // someone found the colour odd.
        //
        // Today the guest cannot even produce an invalid name (over there it is an enum), and
        // if one arrives anyway, it does NOT become any colour.
        assert_eq!(token_color("nao-existe"), None);
        assert_eq!(token_color(""), None);
    }

    #[test]
    fn the_wit_names_resolve_on_the_host_side() {
        // Both sides come from the same generator. This test catches the divergence
        // that would appear if one of them started converting the name differently —
        // a dot instead of a hyphen, for example.
        for name in [
            "text-primary",
            "text-secondary",
            "rule-default",
            "surface-selection",
        ] {
            assert!(
                token_color(name).is_some(),
                "`{name}` nao resolve, mas e caso do enum WIT"
            );
        }
        // And the OLD form, with a dot, no longer resolves.
        assert_eq!(token_color("text.primary"), None);
    }
}
