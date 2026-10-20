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
pub(crate) const OFFERED: [(&str, &str, Doing); 4] = [
    ("l", crate::key::LIFELINE_LOGS, Doing::Logs),
    ("r", crate::key::LIFELINE_RESTART, Doing::Restart),
    ("a", crate::key::LIFELINE_AI, Doing::Ai),
    ("ESC", crate::key::LIFELINE_CLOSE, Doing::Close),
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
pub(crate) enum Doing {
    /// Open the journal in the rescue console — a terminal, which is what the
    /// `Ctrl+Alt+Del` door already is.
    Logs,
    /// Ask systemd to start the unit again. It is the person's OWN user unit,
    /// so nothing here asks polkit anything.
    Restart,
    /// Ask Claude Code what happened. NOTHING LEAVES BEFORE IT IS SEEN: this
    /// gathers what would go — the unit, its `Result=`, the last lines of its
    /// journal and its core dump's summary — into a file only this person can
    /// read, opens it in a terminal, and asks, with "no" first.
    Ai,
    /// No: nothing leaves, and the file goes.
    AiNo,
    /// Yes: Claude Code, in a terminal, told where the file is.
    AiYes,
    /// Take the line away.
    Close,
}

/// The question the line asks once the report is on screen — "no" FIRST,
/// with the cursor born on it (`Lifeline::answer`): a question about
/// something leaving this machine answers NO to a stray key, and a click only
/// picks an answer — the second click on the pick gives it.
pub(crate) const ASKED: [(&str, &str, Doing); 2] = [
    ("n", crate::key::LIFELINE_AI_NO, Doing::AiNo),
    ("y", crate::key::LIFELINE_AI_YES, Doing::AiYes),
];

/// The report's file, in the runtime directory (`0600`, `create_new`).
const REPORT_FILE: &str = "lifeline-report.md";

/// How many lines of the unit's journal go into the report.
const JOURNAL_LINES: &str = "40";

/// Claude Code, by the store path the unit hands over (`RONOCTUA_CLAUDE`,
/// set only where the agents are installed) — and NOTHING where it did not.
///
/// # Never by name
///
/// This line runs as a unit, whose `PATH` is minimal: a bare `claude` would
/// be spawned inside the rescue console, fail THERE, and the line — which only
/// sees that the console started — would say Claude Code had opened. So no
/// path is no Claude, and the option says so before anything is gathered.
fn claude() -> Option<std::path::PathBuf> {
    program_at(std::env::var_os("RONOCTUA_CLAUDE"))
}

/// The pager the report is shown in, by the store path the unit hands over
/// (`RONOCTUA_LESS`); no path, no pager — and then no report leaves either.
fn pager() -> Option<std::path::PathBuf> {
    program_at(std::env::var_os("RONOCTUA_LESS"))
}

/// A program at a path that is there — or none.
fn program_at(path: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    path.filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file())
}

/// The `coredumpctl` beside the `systemctl` this line already names: one
/// `${systemd}`, so one store path and no second variable.
fn coredumpctl() -> std::path::PathBuf {
    std::path::Path::new(SYSTEMCTL).with_file_name("coredumpctl")
}

/// What the report says, whole — the sections in the person's language, and
/// what the machine said under each as it said it (DATA: a journal line is
/// what the machine wrote, the way the brain's log line is).
#[must_use]
fn report(
    unit: &str,
    result: &str,
    journal: &str,
    core: &str,
    texts: &ronoctua_i18n::Texts,
) -> String {
    use crate::key;
    let none = texts.t(key::LIFELINE_REPORT_NOTHING);
    let or_none = |s: &str| {
        if s.trim().is_empty() {
            none.clone()
        } else {
            s.trim_end().to_owned()
        }
    };
    format!(
        "# {}\n\n{}: {unit}\n{}: {}\n\n## {}\n\n```\n{}\n```\n\n## {}\n\n```\n{}\n```\n",
        texts.t(key::LIFELINE_REPORT_TITLE),
        texts.t(key::LIFELINE_REPORT_UNIT),
        texts.t(key::LIFELINE_REPORT_RESULT),
        or_none(result),
        texts.t(key::LIFELINE_REPORT_JOURNAL),
        or_none(journal),
        texts.t(key::LIFELINE_REPORT_CORE),
        or_none(core),
    )
}

/// Writes the report where only this person can read it: created `0600`
/// in one call (`create_new` + mode), never made readable first and narrowed
/// after. A report left by an earlier line is taken away first.
fn write_report(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(text.as_bytes())
}

/// Gathers what the report says the machine said — the journal's last
/// lines and the core dump's summary — on a thread: two short processes,
/// neither on the line's loop. Either one that answers nothing is empty, and
/// the report says so in its place.
fn gather(unit: &str) -> (String, String) {
    let journal = std::process::Command::new(JOURNALCTL)
        .args([
            "--user",
            "-u",
            unit,
            "-n",
            JOURNAL_LINES,
            "--no-pager",
            "-o",
            "short-iso",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let core = std::process::Command::new(coredumpctl())
        .args([
            "--no-pager",
            "info",
            "-1",
            &format!("COREDUMP_USER_UNIT={unit}"),
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    (journal, core)
}

/// One option, as it was DRAWN — so the pointer asks the same table the
/// keyboard does.
#[derive(Debug, Clone)]
pub(crate) struct Placed {
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
    /// footer as an ordinary background child, and `nix/dev.nix` waits on the
    /// compositor.
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

/// The `systemctl`, by path and never by name — the brain link's own constant
/// (`ronoctua_shell_door::brainlink`), written the same way for the same
/// reason.
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
    // WHAT THE AI OPTION GATHERS, back from its thread.
    let (gathered_sender, gathered) = channel::channel::<(String, String)>();
    cycle
        .handle()
        .insert_source(gathered, |event, (), state: &mut Lifeline| {
            if let channel::Event::Msg((journal, core)) = event {
                state.gathered_back(&journal, &core);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the report's channel: {e}"))?;
    let mut state = Lifeline {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboards: crate::seats::Seats::default(),
        key_here: Modifiers::default(),
        pointers: crate::seats::Seats::default(),
        shapes: crate::cursor_shape::Shapes::bind(&globals, &qh),
        here: PerSeat::default(),
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
        said: None,
        texts,
        quit: false,
        owed: false,
        gathered: gathered_sender,
        asking: None,
        answer: crate::aim::Aim::none(),
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    // AND THE SIZE OF THE TEXT, which this line kept from the moment it was
    // raised while every other door followed the person's choice.
    //
    // It is the one door that can be on screen BESIDE the footer — that is
    // what it is for — so two different sizes on two lines of the same edge is
    // exactly the photograph that made this system have one size at all. The
    // height is a function of the size too (`LINES` of `line_height`), so the
    // surface is asked for a new one rather than only repainted.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    cycle
        .handle()
        .insert_source(size_changed, |event, (), state: &mut Lifeline| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            // THE ONE PLACE THAT KNOWS THE BATCH IS OVER.
            e.settle();
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
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    /// The modifiers of the keyboard whose events are being handled — the
    /// per-seat state `keyboards` swaps in (`crate::seats`).
    key_here: Modifiers,
    /// Every pointer this window was offered — one per cursor — with the
    /// state of each while another is `here`. See `crate::seats`.
    pointers: crate::seats::Seats<PerSeat>,
    /// The arrow each pointer is asked for on entering. See `crate::cursor_shape`.
    shapes: crate::cursor_shape::Shapes,
    /// The state of the pointer whose events are being handled now.
    here: PerSeat,
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
    /// What the last gesture had to say — the refusal of the option that is not
    /// built yet, most of the time.
    said: Option<String>,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    quit: bool,
    owed: bool,
    /// Where the gathered journal and core dump come back to the loop.
    gathered: channel::Sender<(String, String)>,
    /// The report on screen, while the line asks whether it may leave.
    asking: Option<std::path::PathBuf>,
    /// The answer the question's cursor is on: born on "no" (`ASKED`'s
    /// first), moved by a click, and given by `RET` or by a click on the
    /// person's pick (`crate::aim`) — the question rose by itself, after the
    /// report opened, and one click on "yes" sent it.
    answer: crate::aim::Aim<Doing>,
}

/// What this window keeps for ONE pointer — see `crate::seats`.
#[derive(Debug, Default)]
struct PerSeat {
    under_pointer: Option<Doing>,
}

impl Lifeline {
    /// Asks for a frame once the current batch of events is done.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Draws the owed frame, if one is owed.
    ///
    /// The drain was written inline in the loop's callback, which worked and
    /// was invisible to `one-frame-model` — the check reads for this function
    /// by name, because a door that spells the model differently is a door
    /// nobody can audit at a glance.
    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
        }
    }

    /// The sentence: what happened, and whether it came back.
    fn sentence(&self) -> String {
        let key = if self.fallen.back {
            crate::key::LIFELINE_FELL_AND_CAME_BACK
        } else {
            crate::key::LIFELINE_FELL
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
            return self
                .texts
                .t_with(crate::key::LIFELINE_UNIT, &[("unit", self.unit.as_str())]);
        }
        self.texts.t_with(
            crate::key::LIFELINE_WHY,
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
            // GATHERED OFF THE LOOP, shown before anything leaves.
            // NO CLAUDE, NO ERRAND: said before anything is gathered, so the
            // option never walks the person to a "yes" that cannot happen.
            Doing::Ai if claude().is_none() || pager().is_none() => {
                self.said = Some(self.texts.t(crate::key::LIFELINE_AI_NO_CLAUDE));
                self.ask_paint();
            }
            Doing::Ai => {
                let (told, unit) = (self.gathered.clone(), self.unit.clone());
                let born = std::thread::Builder::new()
                    .name("lifeline-report".to_owned())
                    .spawn(move || {
                        let _ = told.send(gather(&unit));
                    });
                self.said = Some(self.texts.t(if born.is_ok() {
                    crate::key::LIFELINE_AI_GATHERING
                } else {
                    crate::key::LIFELINE_AI_NO_REPORT
                }));
                self.ask_paint();
            }
            // NO: nothing left, and the file goes with the question.
            Doing::AiNo => {
                if let Some(path) = self.asking.take() {
                    let _ = std::fs::remove_file(path);
                }
                self.said = Some(self.texts.t(crate::key::LIFELINE_AI_KEPT));
                self.ask_paint();
            }
            // YES: Claude Code in a terminal, told where the report is — the
            // prompt in the person's language, the report in their file.
            Doing::AiYes => {
                let Some(path) = self.asking.take() else {
                    return;
                };
                let Some(claude) = claude() else {
                    let _ = std::fs::remove_file(path);
                    self.said = Some(self.texts.t(crate::key::LIFELINE_AI_NO_CLAUDE));
                    self.ask_paint();
                    return;
                };
                let prompt = self.texts.t_with(
                    crate::key::LIFELINE_AI_PROMPT,
                    &[
                        ("unit", self.unit.as_str()),
                        ("path", &path.to_string_lossy()),
                    ],
                );
                let me = std::env::current_exe().unwrap_or_else(|_| "ronoctua-shell".into());
                let started = std::process::Command::new(me)
                    .arg("rescue")
                    .arg(claude)
                    .arg(prompt)
                    .spawn();
                self.said = Some(self.texts.t(match started {
                    Ok(_) => crate::key::LIFELINE_AI_OPENED,
                    Err(e) => {
                        tracing::warn!(error = %e, "could not open Claude Code");
                        crate::key::LIFELINE_AI_NO_CLAUDE
                    }
                }));
                self.ask_paint();
            }
            Doing::Restart if !self.fallen.loaded => {
                self.said = Some(self.texts.t(crate::key::LIFELINE_NOT_A_UNIT));
                self.ask_paint();
            }
            Doing::Restart => {
                let done = std::process::Command::new(SYSTEMCTL)
                    .args(["--user", "restart", &self.unit])
                    .status()
                    .is_ok_and(|s| s.success());
                self.said = Some(self.texts.t(if done {
                    crate::key::LIFELINE_RESTARTING
                } else {
                    crate::key::LIFELINE_WOULD_NOT_START
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
                    Ok(_) => crate::key::LIFELINE_LOGS_OPENED,
                    Err(e) => {
                        tracing::warn!(error = %e, "could not open the journal");
                        crate::key::LIFELINE_NO_JOURNALCTL
                    }
                };
                self.said = Some(self.texts.t(said));
                self.ask_paint();
            }
        }
    }

    /// The journal and the core dump came back: the report is written where
    /// only this person reads it, opened in a terminal, and the line asks.
    fn gathered_back(&mut self, journal: &str, core: &str) {
        let text = report(&self.unit, &self.fallen.why, journal, core, &self.texts);
        let Some(path) = ronoctua_xdg::runtime_file(REPORT_FILE) else {
            self.said = Some(self.texts.t(crate::key::LIFELINE_AI_NO_REPORT));
            self.ask_paint();
            return;
        };
        if let Err(e) = write_report(&path, &text) {
            tracing::warn!(error = %e, "could not write the report");
            self.said = Some(self.texts.t(crate::key::LIFELINE_AI_NO_REPORT));
            self.ask_paint();
            return;
        }
        // WHAT WOULD LEAVE, ON SCREEN FIRST, in the rescue console — a
        // terminal, which pages and searches.
        let me = std::env::current_exe().unwrap_or_else(|_| "ronoctua-shell".into());
        let shown = match pager() {
            Some(pager) => std::process::Command::new(me)
                .arg("rescue")
                .arg(pager)
                .arg(&path)
                .spawn()
                .map(drop),
            None => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
        };
        if let Err(e) = shown {
            tracing::warn!(error = %e, "could not show the report");
            let _ = std::fs::remove_file(&path);
            self.said = Some(self.texts.t(crate::key::LIFELINE_AI_NO_REPORT));
            self.ask_paint();
            return;
        }
        self.asking = Some(path);
        self.answer = crate::aim::Aim::born(ASKED[0].2);
        self.said = Some(self.texts.t(crate::key::LIFELINE_AI_SHOWN));
        self.ask_paint();
    }

    /// The options on the second line now: the four, or — while the report
    /// is on screen — the question, "no" first.
    fn offered(&self) -> &'static [(&'static str, &'static str, Doing)] {
        if self.asking.is_some() {
            &ASKED
        } else {
            &OFFERED
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
    /// The person chose another size, anywhere on the system.
    ///
    /// The rasteriser is told, the surface asks for the height that size now
    /// needs, and the line is repainted. `toolkits::reload` has already put
    /// the new value in the tokens by the time this runs — it is what the
    /// channel's guard calls.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(tokens::text::size());
        }
        let height = tokens::metrics::line_height(tokens::text::size()) * f32::from(LINES);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let height = height.ceil() as u32;
        self.layer.set_size(0, height);
        self.layer
            .set_exclusive_zone(i32::try_from(height).unwrap_or(i32::MAX));
        self.layer.commit();
        self.recount();
        self.ask_paint();
    }

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
                // OPAQUE: only the footer's hidden modes are see-through.
                false,
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
            self.texts.language(),
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

    /// Lays the two lines out, and writes down where each option landed —
    /// by [`line_of`], which the photograph draws too.
    fn frame(&mut self) -> Vec<ronoctua_render::Span> {
        let lit: Vec<Doing> = self
            .pointers
            .lit(&self.here)
            .filter_map(|(_, s)| s.under_pointer)
            .collect();
        let said = self.said.clone().unwrap_or_else(|| self.sentence());
        let (spans, placed) = line_of(&Line {
            said: &said,
            evidence: &self.evidence(),
            offered: self.offered(),
            cursor: self.asking.as_ref().and(self.answer.on().copied()),
            lit: &lit,
            columns: self.columns,
            texts: &self.texts,
        });
        self.placed = placed;
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
            tokens::color::SURFACE_CHROME.to_linear(),
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

/// What the line says, and with what — everything [`line_of`] reads.
pub(crate) struct Line<'a> {
    /// The sentence: what happened, or what the last gesture had to say.
    pub said: &'a str,
    /// The unit's own words, for the right-hand end.
    pub evidence: &'a str,
    /// The options on the second line.
    pub offered: &'static [(&'static str, &'static str, Doing)],
    /// The answer the question's cursor is on, while the question is up.
    pub cursor: Option<Doing>,
    /// The options a pointer rests on.
    pub lit: &'a [Doing],
    /// How many cells the line has.
    pub columns: u16,
    /// The words.
    pub texts: &'a ronoctua_i18n::Texts,
}

/// The two lines, laid out, and where each option landed — the line's own
/// frame and the photograph's (its `Camera`).
pub(crate) fn line_of(l: &Line<'_>) -> (Vec<ronoctua_render::Span>, Vec<Placed>) {
    let size = tokens::text::size();
    let room = l.columns;
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
    let evidence = l.evidence.to_owned();
    let wide = ronoctua_render::layout::width(&evidence);
    let said = l.said;
    let left = MARGIN + 2;
    let for_the_sentence = room.saturating_sub(wide + AIR);
    spans.push(ronoctua_render::Span {
        size: px,
        text: ronoctua_render::layout::elide_end(said, for_the_sentence),
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
    let mut placed: Vec<Placed> = Vec::new();
    let mut at = left;
    for &(notation, id, doing) in l.offered {
        let key = written(notation);
        let label = l.texts.t(id);
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
            // THE QUESTION'S CURSOR in the cursor's ink — born on "no",
            // the answer `RET` gives, and moved by a click.
            color: if l.cursor == Some(doing) {
                tokens::color::TEXT_HERE_ACTS
            } else if l.lit.contains(&doing) {
                // AND THE POINTER LIGHTS THE ONE IT IS ON, which is the
                // only thing on this line that says the mouse is being
                // listened to at all — ANY cursor's, since every one of
                // them can press it. The lit tone of the words, and not
                // the cursor's ink: a light under the pointer is not the
                // cursor, and on the question it would read as the answer
                // having moved to "yes".
                crate::chrome::Register::ProseLit.color()
            } else {
                crate::chrome::Register::Prose.color()
            },
            edge: false,
            foot: false,
        });
        placed.push(Placed { at, cells, doing });
        at += cells + AIR;
    }
    (spans, placed)
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
        // MARKED, NOT PAINTED — the same model every other door carries. A
        // configure arrives with the surface's first size and again on every
        // resize, and painting here meant the resize gesture waited for the
        // monitor once per step.
        self.ask_paint();
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
        if capability == Capability::Keyboard {
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(k) => {
                    // EVERY KEYBOARD, and not only the first: one per seat, and in
                    // Independent mode one per cursor.
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.keyboards.took(&seat, k, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no keyboard"),
            }
        }
        if capability == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => {
                    // EVERY POINTER, and not only the first: one per cursor.
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.shapes.took(&seat, &p, qh);
                    self.pointers.took(&seat, p, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no pointer"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            self.keyboards.lost(&seat, &mut self.key_here);
        }
        if capability == Capability::Pointer {
            self.shapes.lost(&seat);
            self.pointers.lost(&seat, &mut self.here);
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Lifeline {
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        // THE NOTATION, AND THEN THE TABLE. It is `audio_window`'s shape: the
        // door turns what its toolkit handed it into the spelling the table
        // holds, and the table is the only place a key is written.
        //
        // `Esc` used to be handled off to the side, which is how the line came
        // to draw `x` for a gesture that two keys performed and the table knew
        // one of. Now it is a row like the others.
        // A QUESTION ON SCREEN ANSWERS NO to `Esc`, the one answer that
        // sends nothing anywhere — and `RET` gives the answer the cursor is
        // on, which is born on "no".
        if self.asking.is_some() {
            match event.keysym {
                Keysym::Escape => {
                    self.act(Doing::AiNo);
                    return;
                }
                Keysym::Return | Keysym::KP_Enter => {
                    self.act(self.answer.on().copied().unwrap_or(Doing::AiNo));
                    return;
                }
                _ => {}
            }
        }
        let typed = match event.keysym {
            Keysym::Escape => "ESC".to_owned(),
            _ => match event.keysym.key_char() {
                Some(c) => c.to_ascii_lowercase().to_string(),
                None => return,
            },
        };
        if let Some((_, _, doing)) = self
            .offered()
            .iter()
            .find(|(notation, _, _)| *notation == typed)
        {
            self.act(*doing);
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
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

impl PointerHandler for Lifeline {
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
        for serial in crate::cursor_shape::enters(events) {
            self.shapes.entered(self.pointers.seat_of(pointer), serial);
        }
        self.pointers.switch_to(pointer, &mut self.here);
        self.pointer_frame_here(conn, qh, events);
    }
}

impl Lifeline {
    /// One pointer's frame, with its state already `here`.
    fn pointer_frame_here(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        events: &[PointerEvent],
    ) {
        let before = self.here.under_pointer;
        let mut pressed = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.here.under_pointer = self.at(e.position.0, e.position.1);
                }
                PointerEventKind::Leave { .. } => self.here.under_pointer = None,
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    pressed = self.at(e.position.0, e.position.1);
                }
                _ => {}
            }
        }
        // THE QUESTION PICKS BEFORE IT ANSWERS: a click puts the cursor on
        // an answer, and a click on the person's pick gives it — the cursor
        // born on "no" is the screen's, so it takes two as well.
        if let Some(doing) = pressed
            && self.asking.is_some()
        {
            if self.answer.clicked(doing) {
                self.act(doing);
            } else {
                self.ask_paint();
            }
            return;
        }
        if let Some(doing) = pressed {
            self.act(doing);
            return;
        }
        // A FRAME ONLY WHEN THE ACCENT MOVED. A `Motion` arrives per pixel, and
        // the only thing the pointer draws here is which option it is on.
        if before != self.here.under_pointer {
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

/// The lifeline's camera (`photograph --door lifeline`): the line as it
/// rises for a unit that fell — [`line_of`], the function its own paint
/// calls, over the ground its paint clears to. It takes no steps: it is
/// drawn as it rises.
#[derive(Debug)]
pub(crate) struct Camera {
    /// The unit that fell.
    unit: String,
    /// systemd's `Result=`.
    result: String,
    /// Whether the question about sending the report is up.
    asking: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            unit: "ronoctua-footer.service".to_owned(),
            result: "core-dump".to_owned(),
            asking: false,
        }
    }
}

impl crate::photo::scenes::Scene for Camera {
    const DOOR: &'static str = "lifeline";
    const IS: &'static str = "the line that outlives the footer, for a unit that fell";
    const WORDS: &'static str = "\
  --unit <name>         the unit         (default: ronoctua-footer.service)
  --result <word>       systemd's Result= (default: core-dump)
  --asking              the question about sending the report is up";
    const WALKS: bool = false;

    fn word(&mut self, flag: &str, value: &mut dyn FnMut() -> Result<String>) -> Result<bool> {
        match flag {
            "--unit" => self.unit = value()?,
            "--result" => self.result = value()?,
            "--asking" => self.asking = true,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn shoot(self, mut ui: crate::door::Ui, asked: &crate::photo::scenes::Asked) -> Result<()> {
        use crate::key;
        let texts = std::rc::Rc::clone(&ui.texts);
        let height = tokens::metrics::line_height(tokens::text::size()) * 2.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = (
            match asked.width {
                Some(crate::photo::scenes::Measure::Px(w)) => w,
                Some(crate::photo::scenes::Measure::Cells(n)) => {
                    (f32::from(n) * tokens::metrics::cell_width(tokens::text::size())).ceil() as u32
                }
                None => 1600,
            },
            height.ceil() as u32,
        );
        (ui.width, ui.height) = size;
        let cell = tokens::metrics::cell_width(tokens::text::size());
        #[allow(clippy::cast_precision_loss)]
        let usable = size.0 as f32 - 2.0 * cell * f32::from(tokens::space::MODELINE_EDGELEFT);
        let columns = tokens::metrics::columns(usable.max(0.0), tokens::text::size());
        let said = texts.t(if self.asking {
            key::LIFELINE_AI_SHOWN
        } else {
            key::LIFELINE_FELL
        });
        let evidence = texts.t_with(
            key::LIFELINE_WHY,
            &[("unit", self.unit.as_str()), ("why", self.result.as_str())],
        );
        let (spans, _) = line_of(&Line {
            said: &said,
            evidence: &evidence,
            offered: if self.asking { &ASKED } else { &OFFERED },
            cursor: self.asking.then_some(ASKED[0].2),
            lit: &[],
            columns,
            texts: &texts,
        });
        #[allow(clippy::cast_precision_loss)]
        let composed = crate::door::Composed {
            spans,
            quads: vec![ronoctua_render::quad::Quad {
                x: 0.0,
                y: 0.0,
                width: size.0 as f32,
                height: size.1 as f32,
                color: tokens::color::SURFACE_CHROME,
            }],
            split: 1,
            pictures: Vec::new(),
        };
        let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
        let mut images =
            ronoctua_render::image::Images::new(&gpu.device, ronoctua_render::offscreen::FORMAT)?;
        crate::photo::draw(
            &mut gpu,
            &mut images,
            &composed,
            size,
            &asked.film(),
            ui.scale,
        )
    }
}

/// `ronoctua-shell photograph --door lifeline …`, to its end.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::scenes::taken(rest, Camera::default())
}

#[cfg(test)]
mod tests {
    use super::Camera;
    use super::{Doing, OFFERED, program_at};

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

    /// FAILS IF: the question's cursor is not drawn on its answer in the
    /// cursor's ink, or the pointer's light reads as the cursor — on the
    /// question it would say the answer had moved to "yes".
    #[test]
    fn the_cursor_is_not_the_pointers_light() {
        use super::{ASKED, Line, line_of};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ink = |cursor: Option<Doing>, lit: &[Doing], of: Doing| {
            let (spans, _) = line_of(&Line {
                said: "said",
                evidence: "",
                offered: &ASKED,
                cursor,
                lit,
                columns: 120,
                texts: &t,
            });
            let label = t.t(ASKED.iter().find(|a| a.2 == of).expect("an answer").1);
            spans
                .iter()
                .find(|s| s.text == label)
                .map(|s| s.color)
                .expect("the answer is drawn")
        };
        let here = super::tokens::color::TEXT_HERE_ACTS;
        assert_eq!(ink(Some(Doing::AiNo), &[], Doing::AiNo), here);
        assert_ne!(ink(Some(Doing::AiNo), &[], Doing::AiYes), here);
        let lit = ink(Some(Doing::AiNo), &[Doing::AiYes], Doing::AiYes);
        assert_ne!(lit, here, "the light under the pointer is the cursor's ink");
        assert_ne!(lit, ink(Some(Doing::AiNo), &[], Doing::AiYes), "no light");
        // AND BORN ON "NO", whose first click only picks it.
        let mut answer = crate::aim::Aim::born(ASKED[0].2);
        assert!(!answer.clicked(Doing::AiYes), "one click said yes");
        assert!(answer.clicked(Doing::AiYes));
    }

    /// FAILS IF: the question about sending the report does not put "no"
    /// first — the answer a stray key gives — or its keys collide.
    #[test]
    fn the_question_puts_no_first() {
        use super::ASKED;
        assert_eq!(ASKED[0].2, Doing::AiNo, "the question does not start on no");
        assert_eq!(ASKED[1].2, Doing::AiYes);
        assert_ne!(ASKED[0].0, ASKED[1].0);
        for (notation, _, _) in ASKED {
            assert!(ronoctua_keymap::Key::parse(notation).is_ok(), "{notation}");
        }
    }

    /// FAILS IF: the report leaves out what it promises — the unit, how it
    /// ended, the journal, the core dump — or says nothing where a part came
    /// back empty instead of saying so.
    #[test]
    fn the_report_carries_the_four_things_and_says_what_is_missing() {
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let r = super::report(
            "ronoctua-footer.service",
            "watchdog",
            "out 04 12:00 shell: drawing\n",
            "",
            &t,
        );
        assert!(
            r.contains("ronoctua-footer.service") && r.contains("watchdog"),
            "{r}"
        );
        assert!(r.contains("shell: drawing"), "{r}");
        assert!(
            r.contains(&t.t(crate::key::LIFELINE_REPORT_NOTHING)),
            "an empty core dump said nothing: {r}"
        );
        assert!(r.contains(&t.t(crate::key::LIFELINE_REPORT_JOURNAL)));
    }

    /// FAILS IF: a missing or empty variable falls back to a bare name,
    /// which a unit's minimal `PATH` would never find.
    #[test]
    fn no_path_is_no_program() {
        assert_eq!(program_at(None), None);
        assert_eq!(program_at(Some("".into())), None);
        assert_eq!(program_at(Some("claude".into())), None);
        assert_eq!(program_at(Some("/nonexistent/bin/claude".into())), None);
        let here = std::env::current_exe().expect("this test");
        assert_eq!(program_at(Some(here.clone().into())), Some(here));
    }

    /// FAILS IF: the report is ever readable by anybody but its owner — it
    /// carries paths and a backtrace.
    #[test]
    fn the_report_is_born_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("ronoctua-lifeline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a directory");
        let path = dir.join("report.md");
        std::fs::write(&path, "an old one").expect("an old report");
        super::write_report(&path, "the new one").expect("written");
        let mode = std::fs::metadata(&path)
            .expect("there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "the new one");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the lifeline's camera is given a walk it cannot take: the
    /// line is drawn as it rises.
    #[test]
    fn the_lifeline_takes_no_steps() {
        let words = |all: &[&str]| all.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        let parse = |w: &[&str]| crate::photo::scenes::parse(&words(w), &mut Camera::default());
        assert!(parse(&["--door", "lifeline", "--key", "a"]).is_err());
        assert!(parse(&["--door", "lifeline", "--asking"]).is_ok());
    }
}
