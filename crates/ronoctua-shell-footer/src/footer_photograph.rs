//! THE FOOTER IN A PNG — `ronoctua-shell photograph --door footer`.
//!
//! # Why the footer needed a camera of its own
//!
//! Every window is a `door::Content`, and a door is photographed DETACHED:
//! its content needs no compositor. The footer is not a door. It is a
//! layer-shell surface per output, with its own handlers, and the region's
//! views, the modeline and the minibuffer are methods of one `Footer` that
//! is only ever built over a live connection. So the modeline's state tone,
//! the minibuffer's prefix with a submenu built at run time, and every region
//! view with a window twin could be seen only in a VM.
//!
//! # How
//!
//! The production footer, assembled by the function `surface::run` assembles
//! it with, over a socket pair whose other end is a STAND-IN COMPOSITOR
//! ([`stand_in`]): it announces `wl_compositor`, `zwlr_layer_shell_v1` and one
//! `wl_output`, answers every `sync`, and answers every height the footer asks
//! (`set_size`) with the `configure` a compositor would send — which is what
//! the footer paints on. The screen is marked as a photograph's, so its frame
//! is KEPT instead of presented (`Screen::photographing`), composed by the very
//! loop that presents it, and developed here by the scenes' camera's
//! [`crate::photo::draw`].
//!
//! What the footer would read from the machine is the scene's, delivered by
//! the path the real thing takes: the window in focus as the compositor's
//! event, the state its buffer is in through a buffer socket this camera
//! serves (`ipc::buffer_states`, `ipc::buffer_keymap`), the screens as
//! the display thread's reading, the clipboard as its queue.
//!
//! # Nothing of this machine
//!
//! The footer reads and writes preferences under the XDG directories, so the
//! camera runs ITSELF again in a room of its own: every XDG directory and
//! `HOME` point into a fresh directory that goes when the picture is taken,
//! and no compositor, D-Bus or watchdog address is inherited. (A child process
//! is the only way: changing the environment of a running process is `unsafe`
//! in this edition, and the crate denies it.)

use crate::photo::{IN_THE_ROOM, MACHINE_DIR, MACHINE_STATE};
use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;

const USAGE: &str = "\
usage: ronoctua-shell photograph --door footer [options] [steps]

  the footer itself — modeline, minibuffer and the region — over a stand-in
  compositor with one monitor

  --output <w>x<h>      the monitor, in logical pixels (default: 1280x800)
  --app <id>            a window in focus, with this app_id (one of ours is
                        br.ronoctua.<name>)
  --title <text>        its title
  --state <id>          the state that window's buffer says it is in
  --tone <word>         the state's tone: standard, accent, success, warning
                        or error                       (default: standard)
  --states <id:key,...> the states it lists, for LEADER w s
  --local <keys>=<id>   a binding of its `,` group — repeatable
  --screens <scene>     a reading of the screens: one, two, stacked or dark
  --entry <text>        an entry of the clipboard's history, newest first —
                        repeatable
  --mind <file>         what the brain says, as its socket sends it (one
                        mind, JSON): the modeline's marks, the strip's
                        notices — never this machine's brain
  --player <file>       what the player says, as its watch hears it (one
                        state, JSON): the modeline's pair and what the
                        player's region shows — never this machine's player
  --queue <file>        the queue's outline the player sends beside it
  --room <folder>       the room itself, laid out as a machine's roots
                        (home/, config/, data/, state/, cache/, runtime/,
                        and three of the machine's own places: machine/ —
                        the machine directory —, machine-state/ — its
                        record's folder — and run/ for /run; each made when
                        missing) and left as it is after — what a view
                        reads from files, planted; without it the room is
                        fresh and goes with the picture
  --software <host>     the software centre's reading of the room, as the
                        machine <host> reads it: the person's programs and
                        declaration (state/), the system's reports (sys/)
                        and launcher entries (share/) — no program asked,
                        the window's camera's reading (`photograph --door
                        software`)
  --accounts-marks      the LLM accounts' marks, made by their brain module
                        over the room's files (`--room`) and laid over
                        `--mind`'s: never a mark written by hand
  --out <file>          where to write                 (default: footer.png)
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --leader              the leader, as the compositor's bind sends it
  --key <notation>      a key, as the keymap writes it: w, s, <down>, RET
  --command <id>        a command of the registry, as `ronoctua-shell
                        command <id>` sends it
  --wheel <lines>       the wheel over the region, in lines (negative: up)
  --click <line>        a press and a release on line <line> of the region
                        as it is drawn, counted from its top (0) — twice
                        for the second click, which acts
  --hover <col,line>    the pointer rests on a cell of the footer's surface,
                        counted from its top left (0,0) as `--list` counts
                        its runs: it comes in on the first pointer step and
                        moves after, the hand of a seat the stand-in offers
  --press <col,line[,double]>  the pointer moves to a cell and presses there
                        in one frame, as a quick click arrives, and lets go
                        in the next — twice for double
  --sudo <shot>         the sudo bar up, asking: password, touch, finger,
                        use-password, wrong, checking or window — a deed of
                        the camera's, never a conversation
  --sudo-lit <control>  the bar's control under a pointer: answer,
                        use-password or cancel

Three views read the scene a variable names, in their window camera's shape,
never this machine's: RONOCTUA_PHOTOGRAPH_INCUS_SCENE (`--door incus
--scene`), RONOCTUA_PHOTOGRAPH_SECURITY_SCENE (`--door security --state`) and
RONOCTUA_PHOTOGRAPH_STICK_SCENE (`--door stick --scene`).";

/// The PID the window in focus is said to have: the kernel's default
/// `pid_max`, which no process can have — so nothing under `/proc` answers
/// for it, and the only one that does is this camera's buffer socket.
const NO_PROCESS: i32 = 4_194_304;

/// One step of the walk.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Step {
    Leader,
    Key(ronoctua_keymap::Key),
    Command(String),
    /// The wheel over the region, in lines.
    Wheel(i32),
    /// A click on a line of the region as drawn, from its top.
    Click(usize),
    /// The pointer resting on a cell of the surface: its column and line.
    Hover(u16, u16),
    /// The pointer moving to a cell and pressing there in one frame, and
    /// letting go in the next — twice when `double`.
    Press(u16, u16, bool),
    /// The sudo bar up, asking (`crate::sudo_bar::Shot`).
    Sudo(crate::sudo_bar::Shot),
    /// A control of the sudo bar under a pointer.
    SudoLit(crate::sudo_bar::Control),
}

/// What the footer is given, and walked.
pub(crate) struct Scene {
    pub texts: ronoctua_i18n::Texts,
    /// The monitor, in logical pixels.
    pub output: (u32, u32),
    pub scale: i32,
    pub window: Option<ronoctua_compositor::Window>,
    pub screens: Option<ronoctua_display::Snapshot>,
    pub clipboard: Vec<ronoctua_clipboard::Entry>,
    /// What the brain says, delivered as its socket's thread delivers it.
    pub mind: Option<ronoctua_brain::Mind>,
    /// What the player's watch hears — the queue's outline, then the state —
    /// delivered as its thread delivers them.
    pub player: Vec<ronoctua_player::client::Heard>,
    pub steps: Vec<Step>,
}

/// The footer's last frame.
pub(crate) struct Taken {
    pub composed: crate::door::Composed,
    /// The surface's logical size when it was composed.
    pub logical: (u32, u32),
    /// What the minibuffer was saying.
    pub said: Option<String>,
}

/// What was asked for.
#[derive(Debug)]
struct Asked {
    output: (u32, u32),
    app: Option<String>,
    title: Option<String>,
    state: Option<String>,
    tone: ronoctua_protocol::Tone,
    states: Vec<(String, char)>,
    local: Vec<(String, String)>,
    screens: Option<String>,
    entries: Vec<String>,
    mind: Option<std::path::PathBuf>,
    player: Option<std::path::PathBuf>,
    queue: Option<std::path::PathBuf>,
    room: Option<std::path::PathBuf>,
    software: Option<String>,
    accounts_marks: bool,
    out: std::path::PathBuf,
    language: String,
    theme: tokens::theme::Theme,
    text: u16,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

/// The tones, as the states socket spells them.
const TONES: [&str; 5] = ["standard", "accent", "success", "warning", "error"];

/// `column,line`, and what follows it — a cell of the surface.
fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16, Option<String>)> {
    let mut parts = raw.split(',');
    let (Some(column), Some(line)) = (parts.next(), parts.next()) else {
        bail!("`{flag}` wants `column,line`, not `{raw}`");
    };
    let rest = parts.next().map(|p| p.trim().to_owned());
    ensure!(parts.next().is_none(), "`{flag} {raw}` has a word too many");
    Ok((
        crate::photo::number(flag, column, 0..=999)?,
        crate::photo::number(flag, line, 0..=999)?,
        rest,
    ))
}

impl Asked {
    /// Whether the walk points: the stand-in then offers a seat with a
    /// pointer, and only then — a walk that does not point sees the same
    /// compositor every other photograph sees.
    fn points(&self) -> bool {
        self.steps
            .iter()
            .any(|s| matches!(s, Step::Hover(..) | Step::Press(..)))
    }
}

fn parse(words: &[String]) -> Result<Asked> {
    use crate::photo::{number, pair};
    let mut me = Asked {
        output: (1280, 800),
        app: None,
        title: None,
        state: None,
        tone: ronoctua_protocol::Tone::Standard,
        states: Vec::new(),
        local: Vec::new(),
        screens: None,
        entries: Vec::new(),
        mind: None,
        player: None,
        queue: None,
        room: None,
        software: None,
        accounts_marks: false,
        out: std::path::PathBuf::from("footer.png"),
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        text: tokens::text::NORMAL,
        scale: 1,
        list: false,
        steps: Vec::new(),
    };
    let mut door = false;
    let mut words = words.iter();
    while let Some(flag) = words.next() {
        let mut value = || {
            words
                .next()
                .cloned()
                .with_context(|| format!("`{flag}` with no value"))
        };
        match flag.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--door" => {
                let said = value()?;
                ensure!(
                    said == "footer",
                    "`--door {said}` is not this camera's door"
                );
                door = true;
            }
            "--output" => me.output = pair("--output", &value()?)?,
            "--app" => me.app = Some(value()?),
            "--title" => me.title = Some(value()?),
            "--state" => me.state = Some(value()?),
            "--tone" => {
                let word = value()?;
                ensure!(
                    TONES.contains(&word.as_str()),
                    "`--tone {word}` is not a tone; the tones are {}",
                    TONES.join(", ")
                );
                me.tone = crate::ipc::tone_of(Some(&word));
            }
            "--states" => {
                for one in value()?.split(',') {
                    let (id, key) = one
                        .split_once(':')
                        .with_context(|| format!("`--states` wants `id:key`, not `{one}`"))?;
                    let mut keys = key.chars();
                    let (Some(letter), None) = (keys.next(), keys.next()) else {
                        bail!("the key of `{one}` is not one letter");
                    };
                    me.states.push((id.to_owned(), letter));
                }
            }
            "--local" => {
                let raw = value()?;
                let (keys, id) = raw
                    .split_once('=')
                    .with_context(|| format!("`--local` wants `<keys>=<id>`, not `{raw}`"))?;
                me.local.push((keys.to_owned(), id.to_owned()));
            }
            "--screens" => me.screens = Some(value()?),
            "--entry" => me.entries.push(value()?),
            "--mind" => me.mind = Some(std::path::PathBuf::from(value()?)),
            "--player" => me.player = Some(std::path::PathBuf::from(value()?)),
            "--queue" => me.queue = Some(std::path::PathBuf::from(value()?)),
            "--room" => me.room = Some(std::path::PathBuf::from(value()?)),
            "--software" => me.software = Some(value()?),
            "--accounts-marks" => me.accounts_marks = true,
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme = tokens::theme::by_id(&id).with_context(|| {
                    let known: Vec<&str> = tokens::theme::all().map(|t| t.id()).collect();
                    format!("unknown theme `{id}`; the themes are {}", known.join(", "))
                })?;
            }
            "--text" => {
                let asked = number(
                    "--text",
                    &value()?,
                    tokens::font::SCALE_FLOOR..=tokens::font::SCALE_CEILING,
                )?;
                let held = tokens::text::allowed(asked);
                ensure!(
                    held == asked,
                    "`--text {asked}` is not a size the person can reach; the nearest is {held}"
                );
                me.text = held;
            }
            "--scale" => {
                let most = u16::try_from(ronoctua_render::MOST_SCALE).unwrap_or(1);
                me.scale = number("--scale", &value()?, 1..=most)?;
            }
            "--list" => me.list = true,
            "--leader" => me.steps.push(Step::Leader),
            "--key" => {
                let notation = value()?;
                let key = ronoctua_keymap::Key::parse(&notation)
                    .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                me.steps.push(Step::Key(key));
            }
            "--command" => me.steps.push(Step::Command(value()?)),
            "--wheel" => me
                .steps
                .push(Step::Wheel(number("--wheel", &value()?, -1000..=1000)?)),
            "--click" => me
                .steps
                .push(Step::Click(number("--click", &value()?, 0..=1000)?)),
            "--hover" => {
                let (column, line, rest) = cell_of("--hover", &value()?)?;
                ensure!(rest.is_none(), "`--hover` wants `column,line`");
                me.steps.push(Step::Hover(column, line));
            }
            "--press" => {
                let (column, line, rest) = cell_of("--press", &value()?)?;
                let double = match rest.as_deref() {
                    None | Some("left") => false,
                    Some("double") => true,
                    Some(other) => {
                        bail!("`{other}` is not a press; the presses are left and double")
                    }
                };
                me.steps.push(Step::Press(column, line, double));
            }
            "--sudo" => {
                let word = value()?;
                let shot = crate::sudo_bar::Shot::of_word(&word).with_context(|| {
                    let words: Vec<&str> =
                        crate::sudo_bar::Shot::ALL.iter().map(|(_, w)| *w).collect();
                    format!("`--sudo {word}` is not one of {}", words.join(", "))
                })?;
                me.steps.push(Step::Sudo(shot));
            }
            "--sudo-lit" => {
                let word = value()?;
                me.steps.push(Step::SudoLit(match word.as_str() {
                    "answer" => crate::sudo_bar::Control::Answer,
                    "use-password" => crate::sudo_bar::Control::UsePassword,
                    "cancel" => crate::sudo_bar::Control::Cancel,
                    _ => bail!("`--sudo-lit {word}` is not answer, use-password or cancel"),
                }));
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    ensure!(door, "which door? `--door footer`");
    ensure!(
        me.app.is_some()
            || (me.title.is_none()
                && me.state.is_none()
                && me.states.is_empty()
                && me.local.is_empty()),
        "`--title`, `--state`, `--states` and `--local` describe the window in focus, and there is none without `--app`"
    );
    Ok(me)
}

/// WHAT A FEATURE PLANTS IN THE FOOTER'S CAMERA — handed in by the binary,
/// the one crate that names the features: the footer's camera knows the words
/// (`--accounts-marks`, `--software <host>`) and what they plant is the
/// feature's own, made by the feature's own code.
#[derive(Clone, Copy)]
pub struct Plantings {
    /// The LLM accounts' marks, made by their brain module over the room's
    /// files at `now` (seconds since the epoch), laid over `mind`.
    pub accounts_marks: fn(mind: &mut ronoctua_brain::Mind, now: i64),
    /// The software centre's reading of `room`, as the machine `host` reads
    /// it, kept for the next opening of its region — where its feed's thread
    /// would put one.
    pub software: fn(room: &std::path::Path, host: &str) -> Result<()>,
}

impl std::fmt::Debug for Plantings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plantings").finish_non_exhaustive()
    }
}

/// `ronoctua-shell photograph --door footer …`, to its end — the regions,
/// the residents and the plantings the binary composes (`composition`).
pub fn taken(rest: &[String], composition: &crate::Composition) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let words = if line.ends_with(rest) {
        Ok(line)
    } else {
        Err(anyhow::anyhow!(
            "the words handed to the footer's camera are not the end of the command line"
        ))
    };
    let done = words.and_then(|w| {
        if std::env::var_os(IN_THE_ROOM).is_some() {
            run(&w, composition).map(|()| 0)
        } else {
            parse(&w).and_then(|asked| in_a_room(asked.room.as_deref()))
        }
    });
    match done {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// Runs this very command again with every XDG directory in a room — a
/// fresh one, or the one given (`--room`, kept after) — and gives back its
/// exit code.
fn in_a_room(given: Option<&std::path::Path>) -> Result<i32> {
    let room = match given {
        Some(folder) => folder
            .canonicalize()
            .with_context(|| format!("the room `{}`", folder.display()))?,
        None => std::env::temp_dir().join(format!("ronoctua-footer-camera-{}", std::process::id())),
    };
    let made = (|| -> Result<std::process::ExitStatus> {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut dirs = std::fs::DirBuilder::new();
        dirs.recursive(true).mode(0o700);
        for d in [
            "home",
            "config",
            "data",
            "state",
            "cache",
            "runtime",
            MACHINE_DIR,
            MACHINE_STATE,
            "run",
        ] {
            dirs.create(room.join(d))
                .with_context(|| format!("could not make the room `{}`", room.display()))?;
        }
        let me = std::env::current_exe().context("this program's own path")?;
        let mut child = std::process::Command::new(me);
        child
            .args(std::env::args_os().skip(1))
            .env(IN_THE_ROOM, &room)
            .env("HOME", room.join("home"))
            .env("XDG_CONFIG_HOME", room.join("config"))
            .env("XDG_DATA_HOME", room.join("data"))
            .env("XDG_STATE_HOME", room.join("state"))
            .env("XDG_CACHE_HOME", room.join("cache"))
            .env("XDG_RUNTIME_DIR", room.join("runtime"));
        for gone in [
            "WAYLAND_DISPLAY",
            "WAYLAND_SOCKET",
            "SWAYSOCK",
            "SCROLLSOCK",
            "DBUS_SESSION_BUS_ADDRESS",
            "NOTIFY_SOCKET",
            "WATCHDOG_USEC",
            "WATCHDOG_PID",
            "RONOCTUA_INSTANCE",
        ] {
            child.env_remove(gone);
        }
        child.status().context("the camera in its room")
    })();
    if given.is_none() {
        let _ = std::fs::remove_dir_all(&room);
    }
    Ok(made?.code().unwrap_or(1))
}

/// In the room: the footer, walked and drawn.
fn run(words: &[String], composition: &crate::Composition) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(tokens::theme::Ground::Chrome);
    tokens::text::set(asked.text);
    crate::when::prefs::set(crate::when::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let window = asked.app.as_ref().map(|app| ronoctua_compositor::Window {
        id: 1,
        title: asked.title.clone(),
        app_id: Some(app.clone()),
        pid: Some(NO_PROCESS),
        is_focused: true,
        floating: false,
        fullscreen: false,
        sticky: false,
        drawer: false,
        hidden: false,
        trailmark: false,
        rect: None,
    });
    // THE BUFFER'S SOCKET, served before the footer asks: it is where its
    // state and its `,` group come from (`ipc::buffer_socket_path`).
    if window.is_some() && (asked.state.is_some() || !asked.local.is_empty()) {
        let path = crate::ipc::buffer_socket_path(NO_PROCESS.unsigned_abs())?;
        buffer::serve(&path, buffer::Answers::of(&asked))?;
    }
    let screens = asked
        .screens
        .as_deref()
        .map(crate::photo::scene)
        .transpose()?;
    let (ours, theirs) =
        std::os::unix::net::UnixStream::pair().context("a socket pair for the stand-in")?;
    let width = asked.output.0;
    let points = asked.points();
    std::thread::spawn(move || stand_in::serve(theirs, width, points));
    let conn = wayland_client::Connection::from_socket(ours).context("a connection")?;
    // THE BRAIN'S MIND, read as its socket's line is: one JSON object.
    let mind = asked
        .mind
        .as_ref()
        .map(|file| -> Result<ronoctua_brain::Mind> {
            let text = std::fs::read_to_string(file)
                .with_context(|| format!("could not read `{}`", file.display()))?;
            serde_json::from_str(&text)
                .with_context(|| format!("`{}` is not a mind of the brain", file.display()))
        })
        .transpose()?;
    // THE ACCOUNTS' MARKS, as their module makes them from the room's files
    // — the same reading and the same colouring the brain does.
    let mind = if asked.accounts_marks {
        let now = crate::when::now_s();
        let mut mind = mind.unwrap_or_default();
        if mind.now == 0 {
            mind.now = now;
        }
        (composition.plantings.accounts_marks)(&mut mind, now);
        Some(mind)
    } else {
        mind
    };
    // WHAT THE PLAYER'S WATCH WOULD HEAR, read as its socket's lines are:
    // the queue's outline first, as a connection hears it, then the state.
    let read = |file: &std::path::Path| -> Result<String> {
        std::fs::read_to_string(file)
            .with_context(|| format!("could not read `{}`", file.display()))
    };
    let mut player = Vec::new();
    if let Some(file) = asked.queue.as_deref() {
        let outline: ronoctua_player::queue::Outline = serde_json::from_str(&read(file)?)
            .with_context(|| format!("`{}` is not an outline of the queue", file.display()))?;
        player.push(ronoctua_player::client::Heard::Queue(Box::new(outline)));
    }
    if let Some(file) = asked.player.as_deref() {
        let state: ronoctua_player::wire::State = serde_json::from_str(&read(file)?)
            .with_context(|| format!("`{}` is not a state of the player", file.display()))?;
        player.push(ronoctua_player::client::Heard::State(Box::new(state)));
    }
    // THE SOFTWARE CENTRE'S READING of this very room — the window's camera's,
    // so one room is one machine in both doors — kept where its feed's thread
    // would put one, for the view a step opens.
    if let Some(host) = asked.software.as_deref() {
        let room = std::env::var_os(IN_THE_ROOM)
            .map(std::path::PathBuf::from)
            .context("the camera is not in its room")?;
        (composition.plantings.software)(&room, host)?;
    }
    let scene = Scene {
        texts,
        output: asked.output,
        scale: i32::from(asked.scale),
        window,
        screens,
        clipboard: asked
            .entries
            .iter()
            .map(|e| ronoctua_clipboard::Entry::of_text(e.clone()))
            .collect(),
        mind,
        player,
        steps: asked.steps.clone(),
    };
    let taken = crate::surface::photographed(&conn, scene, composition)?;
    if let Some(said) = taken.said.as_ref() {
        println!("the minibuffer says: {said}");
    }
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let mut images =
        ronoctua_render::image::Images::new(&gpu.device, ronoctua_render::offscreen::FORMAT)?;
    crate::photo::draw(
        &mut gpu,
        &mut images,
        &taken.composed,
        taken.logical,
        &crate::photo::Film {
            list: asked.list,
            by: asked.scale,
            language: &asked.language,
            out: asked.out.clone(),
        },
        i32::from(asked.scale),
    )
}

/// The focused window's buffer, as far as the footer asks it: its state and
/// its `,` group, one answer per connection (`ipc::buffer_states_at`,
/// `ipc::buffer_keymap_at`).
mod buffer {
    use anyhow::{Context as _, Result};
    use std::io::{BufRead as _, Write as _};

    /// What the socket answers, already spelled.
    pub struct Answers {
        keymap: String,
        states: String,
    }

    impl Answers {
        pub(super) fn of(asked: &super::Asked) -> Self {
            let mut keymap = String::new();
            for (keys, id) in &asked.local {
                keymap.push_str(&format!("{keys} {id}\n"));
            }
            let mut states = String::new();
            if let Some(now) = asked.state.as_ref() {
                states.push_str(&format!("{now} {}\n", crate::ipc::tone_word(asked.tone)));
                for (id, letter) in &asked.states {
                    states.push_str(&format!("{id} {letter}\n"));
                }
            }
            Self { keymap, states }
        }
    }

    /// Listens at `path` for as long as the camera lives.
    pub fn serve(path: &std::path::Path, answers: Answers) -> Result<()> {
        let listener = std::os::unix::net::UnixListener::bind(path)
            .with_context(|| format!("the buffer's socket `{}`", path.display()))?;
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut asked = String::new();
                let mut reader = std::io::BufReader::new(&stream);
                if reader.read_line(&mut asked).is_err() {
                    continue;
                }
                let said = match asked.trim() {
                    "keymap" => answers.keymap.as_str(),
                    "states" => answers.states.as_str(),
                    _ => "",
                };
                let _ = (&stream).write_all(said.as_bytes());
            }
        });
        Ok(())
    }
}

/// A compositor that answers only what the footer waits for.
///
/// The Wayland wire is a header — the object, then the size and the opcode
/// in one word — and arguments in 32-bit words, strings length-prefixed and
/// padded. Everything the footer sends is read and dropped except: the
/// display's `sync` (answered with `done` and the id given back) and
/// `get_registry` (three globals), the registry's `bind` (to know which id
/// is the layer shell), the shell's `get_layer_surface`, and a layer
/// surface's `set_size`, answered with the `configure` a compositor gives —
/// the monitor's width when the footer asks for none, and its height.
///
/// A walk that points is offered a fourth global, a seat, which says the
/// moment it is bound that it has a pointer and what it is called: the
/// footer makes its hand from it (`SeatHandler::new_capability`), and the
/// camera hands that hand its frames. No event of the pointer's travels on
/// the wire: the frames are the camera's, delivered where a compositor's
/// arrive (`PointerHandler::pointer_frame`).
pub(crate) mod stand_in {
    use std::collections::HashSet;
    use std::io::{Read as _, Write as _};
    use std::os::unix::net::UnixStream;

    const DISPLAY: u32 = 1;
    const LAYER_SHELL: u32 = 2;
    /// `(name, interface, version)`.
    const GLOBALS: [(u32, &str, u32); 3] = [
        (1, "wl_compositor", 6),
        (LAYER_SHELL, "zwlr_layer_shell_v1", 4),
        (3, "wl_output", 4),
    ];
    /// The seat a walk that points is offered: `(name, interface, version)`.
    const SEAT: (u32, &str, u32) = (4, "wl_seat", 7);
    /// `wl_seat.capability.pointer`.
    const A_POINTER: u32 = 1;

    fn word(bytes: &[u8], at: usize) -> Option<u32> {
        let b: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_ne_bytes(b))
    }

    fn event(to: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
        let size = u32::try_from(8 + args.len()).unwrap_or(u32::MAX);
        let mut m = Vec::with_capacity(8 + args.len());
        m.extend_from_slice(&to.to_ne_bytes());
        m.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
        m.extend_from_slice(args);
        m
    }

    fn string(out: &mut Vec<u8>, s: &str) {
        let len = u32::try_from(s.len() + 1).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_ne_bytes());
        out.extend_from_slice(s.as_bytes());
        out.push(0);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }

    /// The ids the client made, by what they are.
    #[derive(Default)]
    pub struct Stand {
        width: u32,
        serial: u32,
        /// Whether a seat with a pointer is offered.
        points: bool,
        registries: HashSet<u32>,
        shells: HashSet<u32>,
        layers: HashSet<u32>,
    }

    impl Stand {
        #[must_use]
        pub fn of(width: u32) -> Self {
            Self {
                width,
                ..Self::default()
            }
        }

        /// The same stand-in, offering a seat with a pointer.
        #[must_use]
        pub const fn pointing(mut self) -> Self {
            self.points = true;
            self
        }

        /// What one request is answered with — nothing, mostly.
        pub fn answer(&mut self, msg: &[u8]) -> Vec<u8> {
            let (Some(id), Some(head)) = (word(msg, 0), word(msg, 4)) else {
                return Vec::new();
            };
            let opcode = head & 0xffff;
            let args = msg.get(8..).unwrap_or_default();
            let first = word(args, 0);
            let mut out = Vec::new();
            match (id, opcode, first) {
                // wl_display.sync: done, and the id is free again.
                (DISPLAY, 0, Some(callback)) => {
                    self.serial = self.serial.wrapping_add(1);
                    out.extend(event(callback, 0, &self.serial.to_ne_bytes()));
                    out.extend(event(DISPLAY, 1, &callback.to_ne_bytes()));
                }
                // wl_display.get_registry: the three globals, and the seat
                // for a walk that points.
                (DISPLAY, 1, Some(registry)) => {
                    self.registries.insert(registry);
                    let seat = self.points.then_some(SEAT);
                    for (name, interface, version) in GLOBALS.into_iter().chain(seat) {
                        let mut a = name.to_ne_bytes().to_vec();
                        string(&mut a, interface);
                        a.extend_from_slice(&version.to_ne_bytes());
                        out.extend(event(registry, 0, &a));
                    }
                }
                // wl_registry.bind(name, interface, version, id).
                (r, 0, Some(name)) if self.registries.contains(&r) => {
                    let padded = word(args, 4).map(|len| (len as usize + 3) & !3);
                    if let Some(new) = padded.and_then(|p| word(args, 12 + p)) {
                        if name == LAYER_SHELL {
                            self.shells.insert(new);
                        }
                        // THE SEAT SAYS WHAT IT HAS, and then its name, the
                        // moment it is bound — a compositor's order.
                        if self.points && name == SEAT.0 {
                            out.extend(event(new, 0, &A_POINTER.to_ne_bytes()));
                            let mut a = Vec::new();
                            string(&mut a, "seat0");
                            out.extend(event(new, 1, &a));
                        }
                    }
                }
                // zwlr_layer_shell_v1.get_layer_surface(id, …).
                (s, 0, Some(layer)) if self.shells.contains(&s) => {
                    self.layers.insert(layer);
                }
                // zwlr_layer_surface_v1.set_size(width, height): configure.
                (l, 0, Some(width)) if self.layers.contains(&l) => {
                    let height = word(args, 4).unwrap_or(0);
                    let width = if width == 0 { self.width } else { width };
                    self.serial = self.serial.wrapping_add(1);
                    let mut a = self.serial.to_ne_bytes().to_vec();
                    a.extend_from_slice(&width.to_ne_bytes());
                    a.extend_from_slice(&height.to_ne_bytes());
                    out.extend(event(l, 0, &a));
                }
                _ => {}
            }
            out
        }
    }

    /// Serves one client until it leaves — offering it a seat with a
    /// pointer when `points`.
    pub fn serve(stream: UnixStream, width: u32, points: bool) {
        let Ok(mut writer) = stream.try_clone() else {
            return;
        };
        let mut reader = stream;
        let mut stand = if points {
            Stand::of(width).pointing()
        } else {
            Stand::of(width)
        };
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            let n = match reader.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            pending.extend_from_slice(&chunk[..n]);
            while let Some(size) = word(&pending, 4).map(|h| (h >> 16) as usize) {
                if size < 8 {
                    return;
                }
                if pending.len() < size {
                    break;
                }
                let msg: Vec<u8> = pending.drain(..size).collect();
                let said = stand.answer(&msg);
                if !said.is_empty() && writer.write_all(&said).is_err() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// FAILS IF: a word of the camera is lost, a step is taken out of
    /// order, or a description of a window is accepted with no window.
    #[test]
    fn the_words_are_the_scene_and_the_walk() {
        let a = parse(&words(&[
            "--door",
            "footer",
            "--app",
            "br.ronoctua.files",
            "--state",
            "permission",
            "--tone",
            "warning",
            "--states",
            "browse:b,permission:p",
            "--local",
            "f s=save",
            "--leader",
            "--key",
            "w",
            "--command",
            "clipboard-open",
        ]))
        .expect("the words parse");
        assert_eq!(a.app.as_deref(), Some("br.ronoctua.files"));
        assert_eq!(a.tone, ronoctua_protocol::Tone::Warning);
        assert_eq!(
            a.states,
            vec![("browse".to_owned(), 'b'), ("permission".to_owned(), 'p')]
        );
        assert_eq!(a.local, vec![("f s".to_owned(), "save".to_owned())]);
        assert_eq!(a.steps.len(), 3);
        assert_eq!(a.steps[0], Step::Leader);
        assert_eq!(a.steps[2], Step::Command("clipboard-open".to_owned()));
        assert!(parse(&words(&["--door", "footer", "--state", "x"])).is_err());
        assert!(parse(&words(&["--door", "footer", "--tone", "loud"])).is_err());
        assert!(parse(&words(&["--door", "window"])).is_err());
        assert!(!a.points(), "a walk with no pointer step points");
        // THE POINTER'S STEPS, in their order, and a walk that has one points.
        let a = parse(&words(&[
            "--door",
            "footer",
            "--hover",
            "4,2",
            "--press",
            "5,3,double",
            "--press",
            "6,1",
        ]))
        .expect("the words parse");
        assert_eq!(
            a.steps,
            vec![
                Step::Hover(4, 2),
                Step::Press(5, 3, true),
                Step::Press(6, 1, false)
            ]
        );
        assert!(a.points());
        assert!(parse(&words(&["--door", "footer", "--hover", "4"])).is_err());
        assert!(parse(&words(&["--door", "footer", "--hover", "4,2,double"])).is_err());
        assert!(parse(&words(&["--door", "footer", "--press", "4,2,triple"])).is_err());
    }

    /// FAILS IF: a walk that points is offered no seat, the seat does not
    /// say it has a pointer the moment it is bound — the footer would make
    /// no hand — or a walk that does not point is offered one, and every
    /// other photograph would see another compositor.
    #[test]
    fn a_seat_is_offered_only_to_a_walk_that_points() {
        fn request(id: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
            let size = u32::try_from(8 + args.len()).expect("small");
            let mut m = id.to_ne_bytes().to_vec();
            m.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
            m.extend_from_slice(args);
            m
        }
        let registry = request(1, 1, &2_u32.to_ne_bytes());
        let said = stand_in::Stand::of(1280).answer(&registry);
        assert!(!said.windows(7).any(|w| w == b"wl_seat"), "a seat offered");
        let mut s = stand_in::Stand::of(1280).pointing();
        let said = s.answer(&registry);
        assert!(said.windows(7).any(|w| w == b"wl_seat"), "no seat offered");
        // bind(4, "wl_seat", 7, new id 9): what it has, then its name.
        let mut bind = 4_u32.to_ne_bytes().to_vec();
        bind.extend_from_slice(&8_u32.to_ne_bytes());
        bind.extend_from_slice(b"wl_seat\0");
        bind.extend_from_slice(&7_u32.to_ne_bytes());
        bind.extend_from_slice(&9_u32.to_ne_bytes());
        let said = s.answer(&request(2, 0, &bind));
        assert_eq!(said.len(), 12 + 20, "two events");
        assert_eq!(&said[0..4], &9_u32.to_ne_bytes(), "to the seat");
        // THE HEADER: twelve bytes, opcode 0 (`capabilities`).
        assert_eq!(&said[4..8], &(12_u32 << 16).to_ne_bytes(), "capabilities");
        assert_eq!(&said[8..12], &1_u32.to_ne_bytes(), "a pointer");
        assert_eq!(&said[12..16], &9_u32.to_ne_bytes());
        assert!(said[12..].windows(5).any(|w| w == b"seat0"), "its name");
    }

    /// FAILS IF: the stand-in leaves a `sync` unanswered, announces no
    /// layer shell, or answers a height the footer asks with any other
    /// height — the footer would wait forever, or paint at a size nobody
    /// asked for.
    #[test]
    fn the_stand_in_answers_what_the_footer_waits_for() {
        fn request(id: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
            stand_in_event(id, opcode, args)
        }
        fn stand_in_event(id: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
            let size = u32::try_from(8 + args.len()).expect("small");
            let mut m = id.to_ne_bytes().to_vec();
            m.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
            m.extend_from_slice(args);
            m
        }
        let mut s = stand_in::Stand::of(1280);
        // sync(new id 3): done on 3, then delete_id(3) on the display.
        let said = s.answer(&request(1, 0, &3_u32.to_ne_bytes()));
        assert_eq!(said.len(), 24);
        assert_eq!(&said[0..4], &3_u32.to_ne_bytes());
        assert_eq!(&said[20..24], &3_u32.to_ne_bytes());
        // get_registry(2): three globals, the layer shell among them.
        let said = s.answer(&request(1, 1, &2_u32.to_ne_bytes()));
        assert!(
            said.windows(19).any(|w| w == b"zwlr_layer_shell_v1"),
            "no layer shell announced"
        );
        // bind(2, "zwlr_layer_shell_v1", 4, new id 5).
        let mut bind = 2_u32.to_ne_bytes().to_vec();
        bind.extend_from_slice(&20_u32.to_ne_bytes());
        bind.extend_from_slice(b"zwlr_layer_shell_v1\0");
        bind.extend_from_slice(&4_u32.to_ne_bytes());
        bind.extend_from_slice(&5_u32.to_ne_bytes());
        assert!(s.answer(&request(2, 0, &bind)).is_empty());
        // get_layer_surface(new id 7, …), then set_size(0, 54).
        let mut get = 7_u32.to_ne_bytes().to_vec();
        get.extend_from_slice(&6_u32.to_ne_bytes());
        assert!(s.answer(&request(5, 0, &get)).is_empty());
        let mut size = 0_u32.to_ne_bytes().to_vec();
        size.extend_from_slice(&54_u32.to_ne_bytes());
        let said = s.answer(&request(7, 0, &size));
        assert_eq!(said.len(), 20, "one configure");
        assert_eq!(&said[0..4], &7_u32.to_ne_bytes());
        assert_eq!(
            &said[12..16],
            &1280_u32.to_ne_bytes(),
            "the monitor's width"
        );
        assert_eq!(&said[16..20], &54_u32.to_ne_bytes(), "the height asked");
        // Anything else is read and dropped.
        assert!(s.answer(&request(9, 3, &[])).is_empty());
    }
}
