//! THE INCUS WINDOW IN A PNG — `ronoctua-shell photograph --door incus`.
//!
//! # Why a camera of its own
//!
//! What fills this window is the daemon, asked by the view's feed. What is the
//! SAME is taken as it is: the production window, detached
//! (`incus_window::IncusWindow::detached`), driven through `door::Detached`
//! down the path a key and a pointer take, and a frame composed by
//! `door::compose`, the function `Door::paint` calls.
//!
//! # What makes the picture the same on every machine
//!
//! The daemon's answers are a SCENE handed in whole (`--scene`, the shapes
//! `ronoctua-incus` reads, as JSON — `incus_feed::Scene`), never this
//! machine's daemon: the feed answers on the spot from it, a deed is a yes
//! that changes nothing, a consent let is let in the scene, and a job is
//! KEPT and reported rather than queued — a photograph never touches the
//! person's consent or queue. Every date is UTC. A narrow window
//! (`--columns` under `space.containers.panesFrom`) is the stacked layout the
//! footer's region draws.
//!
//! Usage: `ronoctua-shell photograph --door incus [options]`, `--help` for
//! the list. Every word is this camera's, those before `--door incus` too
//! ([`taken`]).

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};
use crate::incus_window::{IncusWindow, Opening};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door incus [options] [steps]

  the daemon's answers are a scene handed in whole — never this machine's
  daemon, consent or queue

  --scene <file>        the daemon's answers, as JSON: instances, images,
                        profiles, networks, pools, projects, volumes (by
                        pool), remotes, catalogues (by remote), consented
                        (hosts let), cli (the CLI's path), refused (a reason
                        every answer is), events_ended (the daemon's
                        stream of events ended), deeds_refused (a reason
                        every deed is refused with)     (default: empty)
  --open <what>         instances, images or new        (default: instances)
  --out <file>          where to write                 (default: incus.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --width <px>          the window's width, in logical pixels
  --height <px>         the height, in logical pixels
                        (one of each pair; default: the window's own size)
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --page <id>           chrome, backdrop, near or absolute (default: chrome)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-l
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there
  --answer <text>       types this on the window's line and answers it

The scene's answers are taken after every step, and the frame is drawn once
they have arrived.";

/// One step of the walk.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// A key pressed and let go.
    Key(ronoctua_keymap::Key),
    /// The pointer rests on a cell.
    Hover(u16, u16),
    /// A press and a release on a cell, with the button, and twice when
    /// `double`.
    Click(u16, u16, u32, bool),
    /// What is typed on the window's line, answered.
    Answer(String),
}

/// A length the person gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    Cells(u16),
    Px(u32),
}

/// What was asked for.
#[derive(Debug)]
struct Asked {
    scene: Option<std::path::PathBuf>,
    open: Opening,
    out: std::path::PathBuf,
    width: Option<Measure>,
    height: Option<Measure>,
    language: String,
    theme: tokens::theme::Theme,
    page: tokens::theme::Ground,
    text: u16,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

/// A whole number inside `range`.
fn number<T>(flag: &str, raw: &str, range: std::ops::RangeInclusive<T>) -> Result<T>
where
    T: std::str::FromStr + PartialOrd + std::fmt::Display + Copy,
{
    let n: T = raw
        .trim()
        .parse()
        .ok()
        .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
    ensure!(
        range.contains(&n),
        "`{flag} {n}` is out of range; the range is {}..={}",
        range.start(),
        range.end()
    );
    Ok(n)
}

/// `column,line`, and what follows it.
fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16, Option<String>)> {
    let mut parts = raw.split(',');
    let (Some(column), Some(line)) = (parts.next(), parts.next()) else {
        bail!("`{flag}` wants `column,line`, not `{raw}`");
    };
    let rest = parts.next().map(|p| p.trim().to_owned());
    ensure!(parts.next().is_none(), "`{flag} {raw}` has a word too many");
    Ok((
        number(flag, column, 0..=999)?,
        number(flag, line, 0..=999)?,
        rest,
    ))
}

/// A length given ONCE: in cells or in pixels, never both.
fn once(slot: &mut Option<Measure>, given: Measure, cells: &str, px: &str) -> Result<()> {
    ensure!(
        slot.replace(given).is_none(),
        "`{cells}` and `{px}` are one length, given twice: give one of them, once"
    );
    Ok(())
}

/// Reads the words — all of them, `--door incus` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        scene: None,
        open: Opening::Instances,
        out: std::path::PathBuf::from("incus.png"),
        width: None,
        height: None,
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
                ensure!(said == "incus", "`--door {said}` is not this camera's door");
            }
            "--scene" => {
                let f = std::path::PathBuf::from(value()?);
                ensure!(f.is_file(), "`--scene {}` is not a file", f.display());
                me.scene = Some(f);
            }
            "--open" => {
                let said = value()?;
                me.open = match said.as_str() {
                    "instances" => Opening::Instances,
                    other => Opening::of_words(&[other.to_owned()])?,
                };
            }
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => {
                let n = number("--columns", &value()?, 1..=1000)?;
                once(&mut me.width, Measure::Cells(n), "--columns", "--width")?;
            }
            "--lines" => {
                let n = number("--lines", &value()?, 1..=1000)?;
                once(&mut me.height, Measure::Cells(n), "--lines", "--height")?;
            }
            "--width" => {
                let px = number("--width", &value()?, 1..=16384)?;
                once(&mut me.width, Measure::Px(px), "--columns", "--width")?;
            }
            "--height" => {
                let px = number("--height", &value()?, 1..=16384)?;
                once(&mut me.height, Measure::Px(px), "--lines", "--height")?;
            }
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
                    .with_context(|| {
                        let known: Vec<&str> =
                            tokens::theme::Ground::ALL.iter().map(|g| g.id()).collect();
                        format!("unknown page `{id}`; the pages are {}", known.join(", "))
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
            "--key" => {
                let notation = value()?;
                let key = ronoctua_keymap::Key::parse(&notation)
                    .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                me.steps.push(Step::Key(key));
            }
            "--hover" => {
                let (column, line, rest) = cell_of("--hover", &value()?)?;
                ensure!(rest.is_none(), "`--hover` wants `column,line`");
                me.steps.push(Step::Hover(column, line));
            }
            "--click" => {
                let (column, line, rest) = cell_of("--click", &value()?)?;
                let (button, double) = match rest.as_deref() {
                    None | Some("left") => (LEFT_BUTTON, false),
                    Some("right") => (RIGHT_BUTTON, false),
                    Some("double") => (LEFT_BUTTON, true),
                    Some(other) => {
                        bail!("`{other}` is not a click; the clicks are left, right and double")
                    }
                };
                me.steps.push(Step::Click(column, line, button, double));
            }
            "--answer" => me.steps.push(Step::Answer(value()?)),
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    Ok(me)
}

/// `ronoctua-shell photograph --door incus …`, to its end: the process
/// leaves with the answer. Every word of the command line is read here —
/// the containers' camera's reason ([`words_of`]).
pub fn taken(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    match words_of(&line, rest).and_then(|words| run(&words)) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// The words this camera reads: the whole `line` after `photograph`, when
/// `rest` — what the player's camera handed over — is how it ends.
fn words_of(line: &[String], rest: &[String]) -> Result<Vec<String>> {
    ensure!(
        line.ends_with(rest),
        "the words handed to Incus's camera are not the end of the command line"
    );
    Ok(line.to_vec())
}

/// Draws the Incus window the words ask for into a PNG, or prints its
/// frame.
///
/// # Errors
///
/// A word not understood, a scene that is not one, no graphics adapter, or a
/// picture larger than the GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(asked.text);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let scene = match &asked.scene {
        Some(file) => crate::incus_feed::Scene::read(file)?,
        None => crate::incus_feed::Scene::default(),
    };
    let window = IncusWindow::detached(scene, asked.open);
    let mut d = Detached::new(ui, window);
    shoot(&mut d, &asked)
}

/// The size the window is laid out at.
fn size_of(d: &Detached<IncusWindow>, asked: &Asked) -> (u32, u32) {
    use crate::door::Content as _;
    let px = tokens::text::size();
    let (cell, line) = (
        tokens::metrics::cell_width(px),
        tokens::metrics::line_height(px),
    );
    let (default_w, default_h) = d.content.default_size();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = match asked.width {
        None => default_w,
        Some(Measure::Px(w)) => w,
        Some(Measure::Cells(n)) => (f32::from(n + 2 * d.content.margin()) * cell).ceil() as u32,
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = match asked.height {
        None => default_h,
        Some(Measure::Px(h)) => h,
        Some(Measure::Cells(n)) => (f32::from(n) * line).ceil() as u32,
    };
    (width, height)
}

/// Where the middle of a cell is, in logical pixels.
fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    let (cell, height) = (
        f64::from(tokens::metrics::cell_width(px)),
        f64::from(tokens::metrics::line_height(px)),
    );
    (
        (f64::from(column) + 0.5) * cell,
        (f64::from(line) + 0.5) * height,
    )
}

/// The pointer's frames for one step: it enters the first time and moves
/// after, then presses and lets go — twice for a double click.
fn frames_of(
    at: (u16, u16),
    button: Option<(u32, bool)>,
    entered: &mut bool,
    ui: &Ui,
) -> Result<Vec<Vec<PointerEvent>>> {
    let position = middle_of(at.0, at.1);
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
    if let Some((button, double)) = button {
        for _ in 0..if double { 2 } else { 1 } {
            out.push(event(PointerEventKind::Press {
                time: 0,
                button,
                serial: 0,
            }));
            out.push(event(PointerEventKind::Release {
                time: 0,
                button,
                serial: 0,
            }));
        }
    }
    Ok(out)
}

/// Draws a frame the way the door settles one: composed, and composed again
/// for as long as the scene's answers keep arriving.
fn settled(
    d: &mut Detached<IncusWindow>,
    gpu: &ronoctua_render::offscreen::Offscreen,
    images: &mut ronoctua_render::image::Images,
) -> crate::door::Composed {
    let mut composed = d.compose(&gpu.device, &gpu.queue, images);
    for _ in 0..4 {
        if !d.content.take_now(&mut d.ui) {
            break;
        }
        d.turn();
        composed = d.compose(&gpu.device, &gpu.queue, images);
    }
    composed
}

/// The window, walked and drawn.
#[allow(clippy::too_many_lines)]
fn shoot(d: &mut Detached<IncusWindow>, asked: &Asked) -> Result<()> {
    use crate::door::Content as _;
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let format = ronoctua_render::offscreen::FORMAT;
    let mut text = ronoctua_render::Text::new(
        &gpu.device,
        &gpu.queue,
        format,
        d.ui.scale,
        tokens::text::size(),
        &asked.language,
    )?;
    let mut quads = ronoctua_render::quad::Quads::new(&gpu.device, format)?;
    let mut images = ronoctua_render::image::Images::new(&gpu.device, format)?;

    let size = size_of(d, asked);
    d.lay_out_at(size);
    d.turn();
    let mut composed = settled(d, &gpu, &mut images);
    let mut entered = false;
    for step in &asked.steps {
        ensure!(
            !d.ui.quit,
            "the window closed at the step before `{step:?}`, and a step after it reaches nothing"
        );
        match step {
            Step::Key(key) => d.press(*key),
            Step::Hover(column, line) => {
                for events in frames_of((*column, *line), None, &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
            Step::Click(column, line, button, double) => {
                for events in frames_of(
                    (*column, *line),
                    Some((*button, *double)),
                    &mut entered,
                    &d.ui,
                )? {
                    d.pointer(&events);
                }
            }
            // THE LINE ANSWERED as the door answers it: what it asked, with
            // what is typed.
            Step::Answer(said) => {
                let id =
                    d.ui.asking.take().map(|l| l.id).with_context(|| {
                        format!("`--answer {said}`: nothing is asked on the line")
                    })?;
                d.content.answered(&mut d.ui, id, said);
            }
        }
        d.turn();
        if d.ui.quit {
            break;
        }
        composed = settled(d, &gpu, &mut images);
    }

    if asked.list {
        println!(
            "{}x{} px logical, {} columns by {} lines, scale {}",
            d.ui.width, d.ui.height, d.ui.columns, d.ui.lines, d.ui.scale
        );
        for s in &composed.spans {
            println!(
                "run line {} column {}{}{} {:?} {:?} {:?}",
                s.line,
                s.column,
                if s.edge { " from-the-right" } else { "" },
                if s.foot { " from-the-bottom" } else { "" },
                s.size,
                s.color,
                s.text
            );
        }
        return Ok(());
    }

    // THE TEXTURE IS PHYSICAL, as a door's buffer is.
    let logical = (d.ui.width, d.ui.height);
    let by = u32::from(asked.scale);
    let physical = (logical.0.saturating_mul(by), logical.1.saturating_mul(by));
    gpu.resize(physical.0, physical.1);
    ensure!(
        (gpu.width, gpu.height) == physical,
        "the picture asked for is {}x{} px and this GPU takes at most {} on either side; ask \
         for a smaller window or a smaller --scale",
        physical.0,
        physical.1,
        gpu.ceiling,
    );
    text.prepare(
        &gpu.device,
        &gpu.queue,
        gpu.width,
        gpu.height,
        ronoctua_render::Atlas::Pinned,
        &composed.spans,
    )?;
    quads.prepare(
        &gpu.device,
        &gpu.queue,
        logical.0,
        logical.1,
        &composed.quads,
    );
    images.prepare(&gpu.queue, logical.0, logical.1, &composed.pictures);
    let (drawn, rects, pictures) = (&text, &quads, &images);
    gpu.frame(tokens::color::SURFACE_CHROME.to_linear(), |pass| {
        composed.draw(pass, drawn, rects, pictures)
    })?;
    let rgba = gpu.read_rgba()?;
    ronoctua_render::offscreen::write_png(&asked.out, &rgba, gpu.width, gpu.height)?;
    println!(
        "{}: {}x{} px, {} columns by {} lines, scale {}, {} runs, language {}, theme {} on the {} \
         page",
        asked.out.display(),
        gpu.width,
        gpu.height,
        d.ui.columns,
        d.ui.lines,
        asked.scale,
        composed.spans.len(),
        d.ui.texts.language(),
        asked.theme.id(),
        asked.page.id(),
    );
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    if let Some(line) = d.ui.asking.as_ref() {
        println!("the window's line asks: {}", line.id);
    }
    let (events, queued, rereading) = d.content.report();
    println!(
        "the daemon's events are {}",
        if events { "followed" } else { "not followed" }
    );
    if !rereading.is_empty() {
        println!(
            "the clock would read the state of {} again",
            rereading.join(", ")
        );
    }
    for work in queued {
        println!("would have queued: {work:?}");
    }
    for launch in d.ui.launches() {
        println!("would have started: {launch:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn the_words_are_the_walk_and_the_door_is_this_one() {
        let asked = parse(&words(&[
            "--door",
            "incus",
            "--open",
            "new",
            "--key",
            "TAB",
            "--answer",
            "web",
            "--click",
            "3,4,double",
            "--columns",
            "140",
            "--language",
            "en-GB",
        ]))
        .expect("the words");
        assert_eq!(asked.width, Some(Measure::Cells(140)));
        assert_eq!(asked.open, Opening::New);
        assert_eq!(asked.steps.len(), 3);
        assert!(parse(&words(&["--door", "files"])).is_err());
        assert!(parse(&words(&["--scene", "/nonexistent/scene.json"])).is_err());
        assert!(parse(&words(&["--open", "everything"])).is_err());
        let line = words(&["--out", "a.png", "--door", "incus"]);
        let asked = parse(&words_of(&line, &line[2..]).expect("the line")).expect("the words");
        assert_eq!(asked.out, std::path::PathBuf::from("a.png"));
    }

    /// FAILS IF: a scene written as the help says is not read — every field
    /// the view draws from, in the crate's own shapes.
    #[test]
    fn a_scene_reads_the_crate_s_shapes() {
        let raw = r#"{
            "instances": [{"name":"web","kind":"container","status":"running","project":"default",
              "description":"","profiles":["default"],"created_at":"2026-10-04T05:24:11Z",
              "image":"Debian","limits":{"cpu":2,"memory_bytes":null,"processes":null},
              "state":{"ipv4":["10.218.42.5"],"ipv6":[],"cpu_ns":1,"memory_bytes":2,"processes":3,"disk_bytes":0},
              "snapshots":[]}],
            "remotes": [{"name":"images","address":"https://images.linuxcontainers.org","protocol":"simple-streams","public":true}],
            "catalogues": {"images": [{"alias":"alpine/3.22","description":"Alpine","kind":"container","fingerprint":"f00d","size_bytes":3}]},
            "volumes": {"default": [{"name":"data","kind":"custom"}]},
            "consented": ["images.linuxcontainers.org"],
            "cli": "/nix/store/x-incus/bin/incus"
        }"#;
        let scene: crate::incus_feed::Scene = serde_json::from_str(raw).expect("a scene");
        assert_eq!(scene.instances.len(), 1);
        assert_eq!(scene.catalogues["images"].len(), 1);
        assert!(
            serde_json::from_str::<crate::incus_feed::Scene>(r#"{"instance":[]}"#).is_err(),
            "a word that is not a field is refused"
        );
    }
}
