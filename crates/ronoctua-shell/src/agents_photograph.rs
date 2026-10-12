//! THE LLM ACCOUNTS' WINDOW IN A PNG — `ronoctua-shell photograph --door
//! agents`.
//!
//! # Why a camera of its own
//!
//! What this window draws is FILES — the accounts' list, each account's
//! credentials, the numbers the status line and the usage unit kept, the
//! token count — read through `ronoctua-llm`, which finds them under the
//! XDG roots and the home. So the picture is taken over a machine laid out
//! under one folder (`--places`: `home/`, `config/`, `state/`, `cache/`),
//! never this machine's own: the camera copies it and runs ITSELF again in a
//! room whose `HOME` and XDG roots are the copy (changing the environment of
//! a running process is `unsafe` in this edition, and the crate denies it —
//! the footer's camera's reason). What is the SAME is taken as it is: the
//! production window, driven through `door::Detached` down the path a key
//! and a pointer take, read by `agents::read` on the spot.
//!
//! # What it reaches
//!
//! Nothing past the door: a session, a login and a refresh go nowhere
//! (`agents_window::Reach::Nowhere`) and are printed (`would have: …`). What a
//! key WRITES — the list, a status line, the lines — it writes in the copy.
//!
//! Usage: `ronoctua-shell photograph --door agents --places <folder>
//! [options]`, `--help` for the list.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::agents_window::{AgentsWindow, Reach, Reader};
use crate::door::{Content as _, Detached, LEFT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door agents --places <folder> [options] [steps]

  --places <folder>     a machine laid out under one folder — home/ (the
                        agents' folders), config/, state/ and cache/ (the
                        XDG roots: the list of accounts, the kept numbers,
                        the token counts) — copied first; the copy is what
                        a key changes                        (required)
  --place <n>           open on the account at place n of the list
  --out <file>          where to write                 (default: agents.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --page <id>           chrome, backdrop, near or absolute (default: chrome)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-n
  --hover <col,line>    rests the pointer on a cell of the window
  --click <col,line>    a press and a release there";

/// The variable that says the camera is already in its room.
const IN_THE_ROOM: &str = "RONOCTUA_AGENTS_CAMERA_ROOM";

/// The folder the room was copied from, for the list's folders to follow.
const COPIED_FROM: &str = "RONOCTUA_AGENTS_CAMERA_FROM";

/// One step of the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(ronoctua_keymap::Key),
    Hover(u16, u16),
    Click(u16, u16),
}

/// What was asked for.
#[derive(Debug)]
struct Asked {
    places: std::path::PathBuf,
    place: Option<usize>,
    out: std::path::PathBuf,
    columns: Option<u16>,
    lines: Option<u16>,
    language: String,
    theme: tokens::theme::Theme,
    page: tokens::theme::Ground,
    text: u16,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

/// `column,line`.
fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16)> {
    let Some((column, line)) = raw.split_once(',') else {
        bail!("`{flag}` wants `column,line`, not `{raw}`");
    };
    Ok((
        crate::scene_photograph::number(flag, column, 0..=999)?,
        crate::scene_photograph::number(flag, line, 0..=999)?,
    ))
}

/// Reads the words — all of them, `--door agents` included.
fn parse(words: &[String]) -> Result<Asked> {
    use crate::scene_photograph::number;
    let mut places = None;
    let mut me = Asked {
        places: std::path::PathBuf::new(),
        place: None,
        out: std::path::PathBuf::from("agents.png"),
        columns: None,
        lines: None,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        page: tokens::theme::Ground::Chrome,
        text: tokens::text::NORMAL,
        scale: 1,
        list: false,
        steps: Vec::new(),
    };
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
                    said == "agents",
                    "`--door {said}` is not this camera's door"
                );
            }
            "--places" => {
                let d = std::path::PathBuf::from(value()?);
                ensure!(d.is_dir(), "`--places {}` is not a folder", d.display());
                places = Some(d);
            }
            "--place" => me.place = Some(number("--place", &value()?, 1..=99)?),
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => me.columns = Some(number("--columns", &value()?, 1..=1000)?),
            "--lines" => me.lines = Some(number("--lines", &value()?, 1..=1000)?),
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme = tokens::theme::by_id(&id).with_context(|| {
                    let known: Vec<&str> = tokens::theme::all().map(|t| t.id()).collect();
                    format!("unknown theme `{id}`; the themes are {}", known.join(", "))
                })?;
            }
            "--page" => {
                let id = value()?;
                me.page = tokens::theme::Ground::ALL
                    .into_iter()
                    .find(|g| g.id() == id)
                    .with_context(|| format!("unknown page `{id}`"))?;
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
            "--key" => {
                let notation = value()?;
                let key = ronoctua_keymap::Key::parse(&notation)
                    .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                me.steps.push(Step::Key(key));
            }
            "--hover" => {
                let (column, line) = cell_of("--hover", &value()?)?;
                me.steps.push(Step::Hover(column, line));
            }
            "--click" => {
                let (column, line) = cell_of("--click", &value()?)?;
                me.steps.push(Step::Click(column, line));
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    me.places = places.context("the accounts are read from a folder: `--places <folder>`")?;
    Ok(me)
}

/// `ronoctua-shell photograph --door agents …`, to its end. Every word of
/// the command line is read, the ones before `--door agents` too.
pub fn taken(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let done = (|| -> Result<i32> {
        ensure!(
            line.ends_with(rest),
            "the words handed to the accounts' camera are not the end of the command line"
        );
        let asked = parse(&line)?;
        if std::env::var_os(IN_THE_ROOM).is_some() {
            run(&asked).map(|()| 0)
        } else {
            in_a_room(&asked)
        }
    })();
    match done {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// Copies a folder whole, links as links.
fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("could not make {}", to.display()))?;
    for entry in
        std::fs::read_dir(from).with_context(|| format!("could not read {}", from.display()))?
    {
        let entry = entry?;
        let kind = entry.file_type()?;
        let there = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &there)?;
        } else if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(entry.path())?, &there)?;
        } else {
            std::fs::copy(entry.path(), &there)
                .with_context(|| format!("could not copy {}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Where the room goes — beside the picture, `<out>.places` — and the folder
/// it is copied from, both whole paths. REFUSED when the two are one folder
/// or one is inside the other: the room is deleted before the copy, so a
/// room holding the folder would delete it, and a room inside the folder
/// would be copied into itself until the disk is full.
fn room_for(
    places: &std::path::Path,
    out: &std::path::Path,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let from = places
        .canonicalize()
        .with_context(|| format!("the folder `{}`", places.display()))?;
    let room = out.with_extension("places");
    let beside = room
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let beside = beside
        .canonicalize()
        .with_context(|| format!("the folder of `{}`", out.display()))?;
    let name = room
        .file_name()
        .with_context(|| format!("`{}` names no file", out.display()))?;
    let room = beside.join(name);
    ensure!(
        room != from && !room.starts_with(&from) && !from.starts_with(&room),
        "the room `{}` (beside `--out`) and `--places {}` overlap: the room is deleted and \
         copied into — write the picture somewhere outside the folder",
        room.display(),
        from.display()
    );
    Ok((room, from))
}

/// Runs this very command again in a room: the folder copied beside the
/// picture, and every root of the machine pointed into the copy.
fn in_a_room(asked: &Asked) -> Result<i32> {
    let (room, from) = room_for(&asked.places, &asked.out)?;
    let _ = std::fs::remove_dir_all(&room);
    copy_tree(&from, &room)?;
    for d in ["home", "config", "state", "cache"] {
        std::fs::create_dir_all(room.join(d))?;
    }
    let me = std::env::current_exe().context("this program's own path")?;
    let mut child = std::process::Command::new(me);
    child
        .args(std::env::args_os().skip(1))
        .env(IN_THE_ROOM, &room)
        .env(COPIED_FROM, &from)
        .env("HOME", room.join("home"))
        .env("XDG_CONFIG_HOME", room.join("config"))
        .env("XDG_STATE_HOME", room.join("state"))
        .env("XDG_CACHE_HOME", room.join("cache"));
    // NOTHING OF THIS SESSION: not its instance's roots, not its agents.
    for gone in [
        "RONOCTUA_INSTANCE",
        "RONOCTUA_CLAUDE",
        "RONOCTUA_CODEX",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        child.env_remove(gone);
    }
    let status = child.status().context("the camera in its room")?;
    let _ = std::fs::remove_dir_all(&room);
    Ok(status.code().unwrap_or(1))
}

/// The pointer's frames for one step.
fn frames_of(
    at: (u16, u16),
    click: bool,
    entered: &mut bool,
    ui: &Ui,
) -> Result<Vec<Vec<PointerEvent>>> {
    let px = tokens::text::size();
    let position = (
        (f64::from(at.0) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
        (f64::from(at.1) + 0.5) * f64::from(tokens::metrics::line_height(px)),
    );
    ensure!(
        position.0 < f64::from(ui.width) && position.1 < f64::from(ui.height),
        "the cell {},{} is outside the window, which is {}x{} px",
        at.0,
        at.1,
        ui.width,
        ui.height
    );
    let event = |kind| {
        vec![PointerEvent {
            surface: ui.surface.clone(),
            position,
            kind,
        }]
    };
    let mut out = vec![event(if std::mem::replace(entered, true) {
        PointerEventKind::Motion { time: 0 }
    } else {
        PointerEventKind::Enter { serial: 0 }
    })];
    if click {
        out.push(event(PointerEventKind::Press {
            time: 0,
            button: LEFT_BUTTON,
            serial: 0,
        }));
        out.push(event(PointerEventKind::Release {
            time: 0,
            button: LEFT_BUTTON,
            serial: 0,
        }));
    }
    Ok(out)
}

/// The list's folders under the folder given, moved into the copy — an
/// account's folder is a path, and the copy is what a key may change.
/// Through the crate's own store: its file and its shape are its own.
fn follow_the_copy() -> Result<()> {
    let (Some(room), Some(from)) = (
        std::env::var_os(IN_THE_ROOM).map(std::path::PathBuf::from),
        std::env::var_os(COPIED_FROM).map(std::path::PathBuf::from),
    ) else {
        return Ok(());
    };
    let store = ronoctua_llm::Accounts::store();
    let Some(mut list) = store.parsed() else {
        return Ok(());
    };
    let mut moved = false;
    for a in &mut list.accounts {
        if let Ok(rest) = a.dir.strip_prefix(&from) {
            a.dir = room.join(rest);
            moved = true;
        }
    }
    if moved {
        store
            .write(&list)
            .context("the copy's list of accounts could not be written")?;
    }
    Ok(())
}

/// In the room: the window, walked and drawn.
fn run(asked: &Asked) -> Result<()> {
    follow_the_copy()?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(asked.text);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let window = AgentsWindow::over(
        ronoctua_files::when::Zone::utc(),
        Reader::Here(crate::agents::read),
        Reach::Nowhere(Vec::new()),
        asked.place,
    );
    let mut d = Detached::new(ui, window);
    let px = tokens::text::size();
    let (default_w, default_h) = d.content.default_size();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size = (
        asked.columns.map_or(default_w, |n| {
            (f32::from(n + 2 * d.content.margin()) * tokens::metrics::cell_width(px)).ceil() as u32
        }),
        asked.lines.map_or(default_h, |n| {
            (f32::from(n) * tokens::metrics::line_height(px)).ceil() as u32
        }),
    );
    d.lay_out_at(size);
    d.turn();
    let mut entered = false;
    for step in &asked.steps {
        ensure!(
            !d.ui.quit,
            "the window closed at the step before `{step:?}`, and a step after it reaches nothing"
        );
        match *step {
            Step::Key(key) => d.press(key),
            Step::Hover(column, line) => {
                for events in frames_of((column, line), false, &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
            Step::Click(column, line) => {
                for events in frames_of((column, line), true, &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
        }
        d.turn();
    }
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let mut images =
        ronoctua_render::image::Images::new(&gpu.device, ronoctua_render::offscreen::FORMAT)?;
    let composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    for done in d.content.done() {
        println!("would have: {done}");
    }
    for launch in d.ui.launches() {
        println!("would have started: {launch:?}");
    }
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    crate::scene_photograph::draw(
        &mut gpu,
        &mut images,
        &composed,
        (d.ui.width, d.ui.height),
        &crate::scene_photograph::Film {
            list: asked.list,
            by: asked.scale,
            language: &asked.language,
            out: asked.out.clone(),
        },
        i32::from(asked.scale),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn the_words_are_the_walk_and_a_folder_is_required() {
        let here = std::env::temp_dir();
        let dir = here.to_string_lossy().into_owned();
        let asked = parse(&words(&[
            "--door", "agents", "--places", &dir, "--key", "C-n", "--click", "3,4", "--place", "2",
        ]))
        .expect("the words");
        assert_eq!(asked.places, here);
        assert_eq!(asked.place, Some(2));
        assert_eq!(asked.steps.len(), 2);
        assert!(parse(&words(&["--door", "agents"])).is_err());
        assert!(parse(&words(&["--door", "files", "--places", &dir])).is_err());
    }

    /// FAILS IF: the room — deleted, then copied into — may be the folder
    /// given, sit inside it (a copy into itself until the disk is full) or
    /// hold it (the folder deleted before it is copied).
    #[test]
    fn the_room_never_overlaps_the_folder() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-agents-room-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let places = root.join("photos");
        std::fs::create_dir_all(places.join("home")).unwrap();
        // INSIDE: `--places photos --out photos/agents.png`.
        assert!(room_for(&places, &places.join("agents.png")).is_err());
        // THE SAME: `--places x.places --out x.png`.
        let same = root.join("x.places");
        std::fs::create_dir_all(&same).unwrap();
        assert!(room_for(&same, &root.join("x.png")).is_err());
        // HOLDING IT: the folder given inside the room.
        let held = root.join("y.places").join("inner");
        std::fs::create_dir_all(&held).unwrap();
        assert!(room_for(&held, &root.join("y.png")).is_err());
        // BESIDE: fine, and whole paths.
        std::fs::create_dir_all(root.join("shots")).unwrap();
        let (room, from) = room_for(&places, &root.join("shots").join("a.png")).expect("beside");
        assert_eq!(
            room,
            root.canonicalize().unwrap().join("shots").join("a.places")
        );
        assert_eq!(from, places.canonicalize().unwrap());
        assert!(places.join("home").is_dir(), "the folder was touched");
        let _ = std::fs::remove_dir_all(root);
    }
}
