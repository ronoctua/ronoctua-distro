//! The line that outlives the footer.
//!
//! # What it is for
//!
//! The owner moved the pointer across the hint line and the whole interface
//! closed — no modeline, no minibuffer, nothing. He named two defects in it and
//! only one of them was the crash: *"o outro é o erro de que a ronoctua-distro
//! não sabe lidar com isso, ou seja, acaba deixando o usuário sem interface"*.
//!
//! This is the answer to the second one. When the footer fails, systemd raises
//! this instead — two lines saying what happened and what can be done about it.
//!
//! # Why it is a second layer surface, and how that squares with invariant 14
//!
//! Invariant 14 reads "no chrome beyond the footer", and taken at the letter a
//! second layer-shell row is exactly that. The owner ruled on it: this is not a
//! SECOND chrome, it is the footer's own row drawn by another process. It
//! exists precisely when there is no chrome at all, it sits where the footer
//! sits, and it goes away on one key.
//!
//! # Why it cannot be the brain
//!
//! Because the brain does not draw, and that is written down:
//! `docs/ARCHITECTURE.md` says *"não desenha, não segura teclado, não abre
//! `/proc` e não reimplementa Nix"*. It is a decider with a queue and a task
//! registry, and a crash is not a scheduled task. What the owner actually asked
//! for — detection that costs nothing — is `OnFailure=`, where systemd hands
//! over an event it already has: no thread, no clock, no question.
//!
//! # Why it is a subcommand and not a crate
//!
//! Because everything it needs is already in this binary: the renderer, the
//! chrome, the tokens and both languages. A crate of its own would be a second
//! closure carrying the same wgpu, for a process that exists a few seconds a
//! year. It sits beside `rescue` in `main.rs`'s dispatch, which is the same
//! argument one file along.

use anyhow::{Context as _, Result};
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
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

/// The surface's namespace, so a compositor rule can tell it from the footer.
pub const NAMESPACE: &str = "ronoctua-lifeline";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// How many lines it takes: the sentence, and the things one can do.
///
/// Two, and not one: four options on the same row as the sentence would elide
/// one of them on a narrow screen, and the option that goes is the one nobody
/// then knows about.
const LINES: u16 = 2;

/// The margin inside the line, in cells — the footer's own.
const MARGIN: u16 = tokens::space::MODELINE_EDGELEFT;

/// The air between two options.
const AIR: u16 = 3;

/// What the line offers: the key's NOTATION, what it says, and what it does.
///
/// # Why a table and not four `if`s
///
/// Because `lifeline-reaches-nothing-the-keyboard-does-not` reads it. Every row
/// carries its own key, so "the mouse reaches nothing the keyboard does not" is
/// a property of the data rather than a promise in a comment — which is the
/// shape `files::OF_THE_VIEW` already has.
///
/// # Why a notation and not a `char`
///
/// It was a `char`, and `Esc` could not be one — so closing the line was a
/// keysym handled off to the side, drawn as `x` and reachable by two keys of
/// which the table knew one. The owner read the screen and asked for the key
/// that actually closes it: *"troque o atalho `x` para ESC"*. A notation holds
/// both, `display.rs` already writes `ESC` in a hint table exactly this way,
/// and the key is now written in ONE place again — which is the rule
/// `keys-are-never-text` exists for.
const OFFERED: [(&str, &str, Doing); 4] = [
    ("l", ronoctua_i18n::key::LIFELINE_LOGS, Doing::Logs),
    ("r", ronoctua_i18n::key::LIFELINE_RESTART, Doing::Restart),
    ("a", ronoctua_i18n::key::LIFELINE_AI, Doing::Ai),
    ("ESC", ronoctua_i18n::key::LIFELINE_CLOSE, Doing::Close),
];

/// The key, as it is drawn: the notation inside brackets.
///
/// # Why brackets, when the hint line draws none
///
/// The owner read the photograph and said it: *"os atalhos estão junto com o
/// texto, ou seja, não está bem distinto"*. Two things were wrong with it, and
/// the colour was the first — the key and its words were the SAME token, where
/// the hint line has always drawn them in two (see `chrome::Register`). That is
/// fixed below and would have been most of the answer on its own.
///
/// The brackets are the rest, and they earn their place here where they would
/// not on the hint line: these are things one PRESSES rather than a list one
/// learns, and the bracket is also the edge of what the mouse can click. A hint
/// line teaches forty keys and cannot afford two cells each.
fn written(notation: &str) -> String {
    // THROUGH `key_text`, which is the one renderer of a key in this tree: it
    // is what turns `<left>` into an arrow, and a second spelling here would be
    // the divergence `chrome::hint_spans` exists to prevent. The four in the
    // table are bare keys with no modifier, so today it hands them back
    // unchanged — `every_key_of_the_line_parses` is what keeps that honest.
    let said = ronoctua_keymap::Key::parse(notation)
        .map_or_else(|_| notation.to_owned(), |k| crate::chrome::key_text(&k));
    format!("[{said}]")
}

/// What one of them does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    /// Open the journal in the rescue console — a terminal, which is what the
    /// `Ctrl+Alt+Del` door already is.
    Logs,
    /// Ask systemd to start the unit again. It is the person's OWN user unit,
    /// so nothing here asks polkit anything.
    Restart,
    /// DRAWN AND REFUSING. The owner asked for the option to exist and for its
    /// implementation to wait: *"deixar a implementação dessa funcionalidade de
    /// no pending, pois será implementada mais tarde"*. A row that is there and
    /// says why it cannot act is the shape the format buffer's XFS row already
    /// has; a row that is missing is a feature nobody knows was planned.
    Ai,
    /// Take the line away.
    Close,
}

/// One option, as it was DRAWN — so the pointer asks the same table the
/// keyboard does.
#[derive(Debug, Clone)]
struct Placed {
    at: u16,
    cells: u16,
    doing: Doing,
}

/// What the unit said about itself, asked ONCE.
///
/// # Why once, and why `systemctl` and not the bus
///
/// Once because this process exists for one event: it is raised BY the failure,
/// so there is nothing to poll for. `systemctl` because the answer is four
/// properties of one unit and the alternative is a D-Bus connection with a
/// proxy and a match rule, alive for the length of one question.
struct Fallen {
    /// `Result=` — `core-dump`, `exit-code`, `signal`, `oom-kill`…
    why: String,
    /// Whether it is running again. The footer has `Restart=on-failure`, so by
    /// the time this draws it usually IS — and the sentence has to say which of
    /// the two happened rather than assume.
    back: bool,
    /// Whether systemd knows this unit at all.
    ///
    /// # Why the line has to ask
    ///
    /// Because the nested loop has no unit: `nix/footer-single.nix` starts the
    /// footer as an ordinary background child, and `nix/dev.nix` waits on niri.
    /// The line is raised there too — it is where the owner met this — and a
    /// `restart` button that asked systemd about a unit it has never heard of
    /// would come back "systemd would not start it", which is a sentence about
    /// the wrong thing.
    loaded: bool,
}

impl Fallen {
    /// Asks the unit, with a fallback that is honest rather than empty.
    fn of(unit: &str) -> Self {
        let said = std::process::Command::new(SYSTEMCTL)
            .args([
                "--user",
                "show",
                "-p",
                "Result",
                "-p",
                "ActiveState",
                "-p",
                "LoadState",
                unit,
            ])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let value = |key: &str| {
            said.lines()
                .find_map(|l| l.strip_prefix(key))
                .unwrap_or_default()
                .to_owned()
        };
        let loaded = value("LoadState=") == "loaded";
        Self {
            // A UNIT NOBODY LOADED HAS NO RESULT, and printing `success` for it
            // would be systemd answering about a unit it does not have.
            why: if loaded {
                value("Result=")
            } else {
                String::new()
            },
            back: value("ActiveState=") == "active",
            loaded,
        }
    }
}

/// The `systemctl`, by path and never by name — `crate::brain`'s own constant,
/// written the same way for the same reason.
///
/// A unit's `PATH` is minimal (`AGENTS.md`), and a bare name fails far from its
/// cause. Outside Nix — `cargo run` — it falls back to the name and the shell's
/// `PATH` resolves it.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// And the `journalctl`.
///
/// # Why this costs nothing
///
/// `option_env!` with a store path is a reference the COMPILE pays, which is
/// why the icons and the buffers come from the environment instead. Here there
/// is nothing to pay: `journalctl` is the same `${systemd}` store path
/// `RONOCTUA_SYSTEMCTL` above already names, so the reference exists either way.
const JOURNALCTL: &str = match option_env!("RONOCTUA_JOURNALCTL") {
    Some(c) => c,
    None => "journalctl",
};

/// Raises the line for a unit that failed.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `wlr-layer-shell`.
pub fn host_it(unit: &str) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<Lifeline> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let layer_shell = LayerShell::bind(&globals, &qh).context(
        "the compositor does not expose wlr-layer-shell — this line is the footer's own row, \
         drawn by another process, and there is nowhere else to put it",
    )?;

    let height = tokens::metrics::line_height(tokens::text::size()) * f32::from(LINES);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = height.ceil() as u32;

    let surface_of = compositor.create_surface(&qh);
    // NO OUTPUT NAMED, which is the one place this differs from the footer on
    // purpose. The footer is one surface per monitor because it is where one
    // reads WHERE ONE IS; this is a message, and a message said twice on two
    // screens is a message said twice.
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface_of,
        // `Top` and not `Overlay`, for the footer's reason: above the windows
        // and below whatever locks the screen.
        Layer::Top,
        Some(NAMESPACE),
        None,
    );
    layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_size(0, height);
    layer.set_exclusive_zone(height as i32);
    // ON DEMAND, and this is the honest middle.
    //
    // `None` would make every letter unreachable and leave the mouse able to do
    // what the keyboard cannot, which this system does not allow. `Exclusive`
    // would take the keyboard from the window somebody is in the middle of
    // using, to show them a line they did not ask for. `OnDemand` gives it the
    // keyboard when it is clicked, exactly as a window gets it when focused.
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.commit();

    let mut cycle: EventLoop<Lifeline> = EventLoop::try_new().context("calloop")?;
    let texts = std::rc::Rc::new(crate::language::texts());
    let mut state = Lifeline {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointer: None,
        layer,
        asked: (0, height),
        width: 0,
        height,
        scale: 1,
        scale_told: 1,
        columns: 0,
        gpu: None,
        text: None,
        quads: None,
        fallen: Fallen::of(unit),
        unit: unit.to_owned(),
        placed: Vec::new(),
        under_pointer: None,
        said: None,
        texts,
        quit: false,
        owed: false,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            if std::mem::take(&mut e.owed) {
                e.paint();
            }
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The line.
struct Lifeline {
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    layer: LayerSurface,
    /// What the compositor configured, kept apart from what the buffer can
    /// show — see `crate::hidpi`.
    asked: (u32, u32),
    width: u32,
    height: u32,
    scale: i32,
    scale_told: i32,
    columns: u16,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    /// What the unit said, asked once when this process started.
    fallen: Fallen,
    unit: String,
    /// Where each option was DRAWN, for the pointer. The rule `band_placed`
    /// and `modeline_placed` already follow: what the cursor is over has to be
    /// what was drawn.
    placed: Vec<Placed>,
    under_pointer: Option<Doing>,
    /// What the last gesture had to say — the refusal of the option that is not
    /// built yet, most of the time.
    said: Option<String>,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    quit: bool,
    owed: bool,
}

impl Lifeline {
    /// Asks for a frame once the current batch of events is done.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// The sentence: what happened, and whether it came back.
    fn sentence(&self) -> String {
        let key = if self.fallen.back {
            ronoctua_i18n::key::LIFELINE_FELL_AND_CAME_BACK
        } else {
            ronoctua_i18n::key::LIFELINE_FELL
        };
        self.texts.t(key)
    }

    /// And the unit's own words, for the right-hand end of the first line.
    ///
    /// The unit's name and `Result=` are DATA, the way a filename is: they are
    /// what the machine said, not a sentence this project wrote, so they cross
    /// as arguments of a key (invariant 8 intact).
    fn evidence(&self) -> String {
        // AND WITH NOTHING TO SAY, THE DOT DOES NOT GET SAID EITHER.
        //
        // Photographed: a transient unit is gone from systemd by the time this
        // asks, so `Result=` comes back empty and the line read
        // `footer.service · ` — a separator hanging off the end with nothing
        // after it. A dot between one thing and nothing separates nothing,
        // which is the argument `chrome::Piece::separator` already makes about
        // the hint line's own.
        if self.fallen.why.is_empty() {
            return self.texts.t_with(
                ronoctua_i18n::key::LIFELINE_UNIT,
                &[("unit", self.unit.as_str())],
            );
        }
        self.texts.t_with(
            ronoctua_i18n::key::LIFELINE_WHY,
            &[
                ("unit", self.unit.as_str()),
                ("why", self.fallen.why.as_str()),
            ],
        )
    }

    /// Carries out an option.
    fn act(&mut self, doing: Doing) {
        match doing {
            Doing::Close => self.quit = true,
            Doing::Ai => {
                self.said = Some(self.texts.t(ronoctua_i18n::key::LIFELINE_AI_NOT_YET));
                self.ask_paint();
            }
            Doing::Restart if !self.fallen.loaded => {
                self.said = Some(self.texts.t(ronoctua_i18n::key::LIFELINE_NOT_A_UNIT));
                self.ask_paint();
            }
            Doing::Restart => {
                let done = std::process::Command::new(SYSTEMCTL)
                    .args(["--user", "restart", &self.unit])
                    .status()
                    .is_ok_and(|s| s.success());
                self.said = Some(self.texts.t(if done {
                    ronoctua_i18n::key::LIFELINE_RESTARTING
                } else {
                    ronoctua_i18n::key::LIFELINE_WOULD_NOT_START
                }));
                self.ask_paint();
            }
            Doing::Logs => {
                // THE RESCUE CONSOLE, which is a terminal — the door
                // `Ctrl+Alt+Del` already opens. A second text viewer would be a
                // second thing to keep, and the journal already pages, searches
                // and colours.
                //
                // A CHILD AND NOT THIS PROCESS: hosting the terminal here would
                // put a second surface and a second event loop inside the line,
                // and the line has to be able to go away while the log stays.
                let me = std::env::current_exe().unwrap_or_else(|_| "ronoctua-shell".into());
                let started = std::process::Command::new(me)
                    .arg("rescue")
                    .arg(JOURNALCTL)
                    .args(["--user", "-u", &self.unit, "-e"])
                    .spawn();
                let said = match started {
                    Ok(_) => ronoctua_i18n::key::LIFELINE_LOGS_OPENED,
                    Err(e) => {
                        tracing::warn!(error = %e, "could not open the journal");
                        ronoctua_i18n::key::LIFELINE_NO_JOURNALCTL
                    }
                };
                self.said = Some(self.texts.t(said));
                self.ask_paint();
            }
        }
    }

    /// Which option a point is on.
    fn at(&self, x: f64, y: f64) -> Option<Doing> {
        let cell = f64::from(crate::chrome::cell_width());
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if cell <= 0.0 || line <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let row = (y / line) as u16;
        if row + 1 != LINES {
            return None;
        }
        let at = u16::try_from((x / cell) as u64).ok()?;
        self.placed
            .iter()
            .find(|p| at >= p.at && at < p.at + p.cells)
            .map(|p| p.doing)
    }
}

impl Lifeline {
    /// How many cells the line has, both margins already off.
    fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let usable = (self.width as f32) - 2.0 * cell * f32::from(MARGIN);
        self.columns = tokens::metrics::columns(usable.max(0.0), size);
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast()),
            std::ptr::NonNull::new(self.layer.wl_surface().id().as_ptr().cast()),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        // SAFETY: `conn` and `layer` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what produced them.
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
        let Ok(mut g) = gpu else {
            tracing::error!("no usable GPU; the line cannot draw");
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
        // THE ONE RECTANGLE THIS LINE DRAWS: its own ground. Without it the
        // sentence would read over whatever the desk happens to be, which for a
        // message about the interface being gone is the one case where the
        // background cannot be assumed.
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "assembling the quads failed"),
        }
        self.gpu = Some(g);
    }

    /// Lays the two lines out, and writes down where each option landed.
    fn frame(&mut self) -> Vec<ronoctua_render::Span> {
        let size = tokens::text::size();
        let room = self.columns;
        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let px = ronoctua_render::Size::Px(size);

        // LINE 0: the mark, the sentence, and what the unit said about itself.
        spans.push(ronoctua_render::Span {
            size: px,
            text: tokens::icon::WARNING.to_string(),
            column: MARGIN,
            line: 0,
            color: tokens::color::STATE_ERROR,
            edge: false,
            foot: false,
        });
        // THE EVIDENCE IS MEASURED FIRST, because the sentence yields to it: a
        // cut `Result=core-dump` says nothing, and a cut sentence still reads.
        let evidence = self.evidence();
        let wide = ronoctua_render::layout::width(&evidence);
        let said = self.said.clone().unwrap_or_else(|| self.sentence());
        let left = MARGIN + 2;
        let for_the_sentence = room.saturating_sub(wide + AIR);
        spans.push(ronoctua_render::Span {
            size: px,
            text: ronoctua_render::layout::elide_end(&said, for_the_sentence),
            column: left,
            line: 0,
            color: tokens::color::TEXT_PRIMARY,
            edge: false,
            foot: false,
        });
        if wide > 0 && wide < room {
            spans.push(ronoctua_render::Span {
                size: px,
                text: evidence,
                column: MARGIN + room.saturating_sub(wide),
                line: 0,
                color: tokens::color::TEXT_SUBTLE_DEFAULT,
                edge: false,
                foot: false,
            });
        }

        // LINE 1: the options, each with its letter in front of it.
        self.placed.clear();
        let mut at = left;
        for (notation, id, doing) in OFFERED {
            let key = written(notation);
            let label = self.texts.t(id);
            let wide = ronoctua_render::layout::width(&key);
            let cells = wide + 1 + ronoctua_render::layout::width(&label);
            if at + cells > MARGIN + room {
                // WHAT DOES NOT FIT IS NOT DRAWN, and it is not silently
                // dropped either: it keeps its key, so the keyboard still
                // reaches it on a narrow screen while the mouse does not. That
                // asymmetry is the right way round — the keyboard is the one
                // that cannot run out of room.
                break;
            }
            // THE KEY IN THE KEY'S OWN REGISTER, and the words in the words'.
            //
            // The two used to be `TEXT_SUBTLE_STRONG` BOTH, and the owner read
            // the photograph and said so: *"não está bem distinto"*. The hint
            // line has always drawn them in two registers — `Register::color`
            // is the one place that decides which — and this line simply was
            // not asking it for the second half.
            spans.push(ronoctua_render::Span {
                size: px,
                text: key,
                column: at,
                line: 1,
                color: crate::chrome::Register::Key.color(),
                edge: false,
                foot: false,
            });
            spans.push(ronoctua_render::Span {
                size: px,
                text: label,
                column: at + wide + 1,
                line: 1,
                // AND THE ONE THAT CANNOT ACT READS AS SUCH. It is drawn, it
                // has a key, and pressing it says why — what it does not do is
                // look like the three that work.
                color: if doing == Doing::Ai {
                    tokens::color::TEXT_DIM
                } else if self.under_pointer == Some(doing) {
                    // AND THE POINTER LIGHTS THE ONE IT IS ON, which is the
                    // only thing on this line that says the mouse is being
                    // listened to at all.
                    tokens::color::TEXT_HERE_ACTS
                } else {
                    crate::chrome::Register::Prose.color()
                },
                edge: false,
                foot: false,
            });
            self.placed.push(Placed { at, cells, doing });
            at += cells + AIR;
        }
        spans
    }

    /// Draws a frame.
    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let spans = self.frame();
        #[allow(clippy::cast_precision_loss)]
        let ground = ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: self.width as f32,
            height: self.height as f32,
            color: tokens::color::SURFACE_CHROME,
        };
        let surface = self.layer.wl_surface().clone();
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
            // LOGICAL, where the text above took the PHYSICAL surface — see the
            // note the window doors carry: a logical rectangle over a logical
            // viewport lands where a physical one over a physical one would.
            q.prepare(&gpu.device, &gpu.queue, width, height, &[ground]);
        }
        let quads_ref = self.quads.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw(pass);
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
}

impl LayerShellHandler for Lifeline {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        if cfg.new_size.0 != 0 {
            self.asked.0 = cfg.new_size.0;
        }
        if cfg.new_size.1 != 0 {
            self.asked.1 = cfg.new_size.1;
        }
        self.width = self.asked.0;
        self.height = self.asked.1;
        self.recount();
        self.assemble_gpu();
        if let Some(g) = self.gpu.as_mut() {
            let (shown_w, shown_h) = crate::hidpi::resized(g, self.asked, self.scale);
            self.width = shown_w;
            self.height = shown_h;
            self.recount();
        }
        self.paint();
        self.layer.commit();
    }
}

impl CompositorHandler for Lifeline {
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
            // AND THE LINE KEEPS WHAT THE BUFFER CAN SHOW, which below the
            // GPU's ceiling is what the compositor configured — see
            // `crate::hidpi`.
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
        _: wayland_client::protocol::wl_output::Transform,
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

impl OutputHandler for Lifeline {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl SeatHandler for Lifeline {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        // NO REPEAT ASKED FOR, and that is not the oversight
        // `every-window-repeats` exists to catch: nothing here is a list one
        // walks. Four letters, each one a single press.
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "no keyboard"),
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                Err(e) => tracing::warn!(error = %e, "no pointer"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if capability == Capability::Pointer
            && let Some(p) = self.pointer.take()
        {
            p.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Lifeline {
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // THE NOTATION, AND THEN THE TABLE. It is `audio_window`'s shape: the
        // door turns what its toolkit handed it into the spelling the table
        // holds, and the table is the only place a key is written.
        //
        // `Esc` used to be handled off to the side, which is how the line came
        // to draw `x` for a gesture that two keys performed and the table knew
        // one of. Now it is a row like the others.
        let typed = match event.keysym {
            Keysym::Escape => "ESC".to_owned(),
            _ => match event.keysym.key_char() {
                Some(c) => c.to_ascii_lowercase().to_string(),
                None => return,
            },
        };
        if let Some((_, _, doing)) = OFFERED.iter().find(|(notation, _, _)| *notation == typed) {
            self.act(*doing);
        }
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

    /// NOTHING REPEATS HERE, and this is the one door where that is right.
    ///
    /// `every-window-repeats` exists because nine doors walked lists with a key
    /// that never repeated. This walks nothing: four letters, each a single
    /// press, and a held `r` that restarted the interface four times would be
    /// the opposite of a rescue.
    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

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

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for Lifeline {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = self.under_pointer;
        let mut pressed = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.under_pointer = self.at(e.position.0, e.position.1);
                }
                PointerEventKind::Leave { .. } => self.under_pointer = None,
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    pressed = self.at(e.position.0, e.position.1);
                }
                _ => {}
            }
        }
        if let Some(doing) = pressed {
            self.act(doing);
            return;
        }
        // A FRAME ONLY WHEN THE ACCENT MOVED. A `Motion` arrives per pixel, and
        // the only thing the pointer draws here is which option it is on.
        if before != self.under_pointer {
            self.ask_paint();
        }
    }
}

impl ProvidesRegistryState for Lifeline {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(Lifeline);
smithay_client_toolkit::delegate_dispatch2!(Lifeline);

#[cfg(test)]
mod tests {
    use super::{Doing, OFFERED};

    /// EVERY OPTION HAS A LETTER, and no two share one.
    ///
    /// It is the same promise `files-everything-has-a-key` makes one screen
    /// along: the mouse reaches nothing the keyboard does not. Here it is
    /// cheap to keep because the options are a table — a `match` with four arms
    /// could not be read this way, which is why it is not one.
    #[test]
    fn every_option_has_a_key_of_its_own() {
        let mut keys: Vec<&str> = OFFERED.iter().map(|(k, _, _)| *k).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), OFFERED.len(), "two options share a key");
        for (_, id, _) in OFFERED {
            assert!(!id.is_empty());
        }
    }

    /// EVERY KEY OF THE LINE PARSES, which is what lets the drawing go through
    /// `chrome::key_text` — the one renderer of a key in this tree — instead of
    /// spelling one of its own. A notation that did not parse would fall back
    /// to the raw string silently, and an arrow would come out as `<left>`.
    #[test]
    fn every_key_of_the_line_parses() {
        for (notation, _, _) in OFFERED {
            let parsed = ronoctua_keymap::Key::parse(notation)
                .unwrap_or_else(|_| panic!("`{notation}` is not a key this tree can read"));
            assert_eq!(
                parsed.notation(),
                notation,
                "`{notation}` does not survive a round trip"
            );
        }
    }

    /// AND THE KEY IS DRAWN APART FROM ITS WORDS.
    ///
    /// The owner read the photograph and said the two ran together — they were
    /// the same token. `Register::color` is the one place that decides, and the
    /// two it hands back have to differ or the line is one grey run again.
    #[test]
    fn the_key_does_not_read_as_its_own_words() {
        assert_ne!(
            crate::chrome::Register::Key.color(),
            crate::chrome::Register::Prose.color(),
            "a key and its words resolve to one colour"
        );
        assert_eq!(super::written("ESC"), "[ESC]");
        assert_eq!(super::written("l"), "[l]");
    }

    /// AND THE FOUR THE OWNER ASKED FOR ARE THE FOUR THAT ARE THERE.
    #[test]
    fn the_line_offers_what_was_asked_for() {
        let doings: Vec<Doing> = OFFERED.iter().map(|(_, _, d)| *d).collect();
        assert_eq!(
            doings,
            vec![Doing::Logs, Doing::Restart, Doing::Ai, Doing::Close],
            "the line stopped offering one of the four"
        );
    }
}
