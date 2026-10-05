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
//! [`crate::scene_photograph::draw`].
//!
//! What the footer would read from the machine is the scene's, delivered by
//! the path the real thing takes: the window in focus as the compositor's
//! event, the state its buffer is in through a buffer socket this camera
//! serves (`leader::buffer_states`, `leader::buffer_keymap`), the screens as
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
                        command <id>` sends it";

/// The variable that says the camera is already in its room.
const IN_THE_ROOM: &str = "RONOCTUA_FOOTER_CAMERA_ROOM";

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

fn parse(words: &[String]) -> Result<Asked> {
    use crate::scene_photograph::{number, pair};
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
                me.tone = crate::leader::tone_of(Some(&word));
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

/// `ronoctua-shell photograph --door footer …`, to its end.
pub fn taken(rest: &[String]) -> ! {
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
            run(&w).map(|()| 0)
        } else {
            parse(&w).and_then(|_| in_a_room())
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

/// Runs this very command again with every XDG directory in a fresh room,
/// and gives back its exit code.
fn in_a_room() -> Result<i32> {
    let room = std::env::temp_dir().join(format!("ronoctua-footer-camera-{}", std::process::id()));
    let made = (|| -> Result<std::process::ExitStatus> {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut dirs = std::fs::DirBuilder::new();
        dirs.recursive(true).mode(0o700);
        for d in ["home", "config", "data", "state", "cache", "runtime"] {
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
    let _ = std::fs::remove_dir_all(&room);
    Ok(made?.code().unwrap_or(1))
}

/// In the room: the footer, walked and drawn.
fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(tokens::theme::Ground::Chrome);
    tokens::text::set(asked.text);
    crate::locale::prefs::set(crate::locale::Choice::default());
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
    // state and its `,` group come from (`leader::buffer_socket_path`).
    if window.is_some() && (asked.state.is_some() || !asked.local.is_empty()) {
        let path = crate::leader::buffer_socket_path(NO_PROCESS.unsigned_abs())?;
        buffer::serve(&path, buffer::Answers::of(&asked))?;
    }
    let screens = asked
        .screens
        .as_deref()
        .map(crate::scene_photograph::scene)
        .transpose()?;
    let (ours, theirs) =
        std::os::unix::net::UnixStream::pair().context("a socket pair for the stand-in")?;
    let width = asked.output.0;
    std::thread::spawn(move || stand_in::serve(theirs, width));
    let conn = wayland_client::Connection::from_socket(ours).context("a connection")?;
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
        steps: asked.steps.clone(),
    };
    let taken = crate::surface::photographed(&conn, scene)?;
    if let Some(said) = taken.said.as_ref() {
        println!("the minibuffer says: {said}");
    }
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let mut images =
        ronoctua_render::image::Images::new(&gpu.device, ronoctua_render::offscreen::FORMAT)?;
    crate::scene_photograph::draw(
        &mut gpu,
        &mut images,
        &taken.composed,
        taken.logical,
        &crate::scene_photograph::Film {
            list: asked.list,
            by: asked.scale,
            language: &asked.language,
            out: asked.out.clone(),
        },
        i32::from(asked.scale),
    )
}

/// The focused window's buffer, as far as the footer asks it: its state and
/// its `,` group, one answer per connection (`leader::buffer_states_at`,
/// `leader::buffer_keymap_at`).
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
                states.push_str(&format!("{now} {}\n", crate::leader::tone_word(asked.tone)));
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
                // wl_display.get_registry: the three globals.
                (DISPLAY, 1, Some(registry)) => {
                    self.registries.insert(registry);
                    for (name, interface, version) in GLOBALS {
                        let mut a = name.to_ne_bytes().to_vec();
                        string(&mut a, interface);
                        a.extend_from_slice(&version.to_ne_bytes());
                        out.extend(event(registry, 0, &a));
                    }
                }
                // wl_registry.bind(name, interface, version, id).
                (r, 0, Some(name)) if self.registries.contains(&r) => {
                    let padded = word(args, 4).map(|len| (len as usize + 3) & !3);
                    if let Some(new) = padded.and_then(|p| word(args, 12 + p))
                        && name == LAYER_SHELL
                    {
                        self.shells.insert(new);
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

    /// Serves one client until it leaves.
    pub fn serve(stream: UnixStream, width: u32) {
        let Ok(mut writer) = stream.try_clone() else {
            return;
        };
        let mut reader = stream;
        let mut stand = Stand::of(width);
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
