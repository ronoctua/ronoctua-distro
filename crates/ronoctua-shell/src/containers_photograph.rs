//! THE CONTAINERS' WINDOW IN A PNG — `ronoctua-shell photograph --door
//! containers`.
//!
//! # Why a camera of its own
//!
//! `crate::photograph` is the player's, and `crate::files_photograph` the
//! file manager's; what fills this window is podman, asked by the view's
//! feed. What is the SAME is taken as it is: the production window, detached
//! (`containers_window::ContainersWindow::detached`), driven through
//! `door::Detached` down the path a key and a pointer take, and a frame
//! composed by `door::compose`, the function `Door::paint` calls.
//!
//! # What makes the picture the same on every machine
//!
//! podman is the one the environment names — a photograph runs it with a
//! STAND-IN, a script that prints recorded answers — and the feed asks it on
//! the spot, so a frame is never taken before the answer it shows. The
//! firewall is the file given (`--firewall`), never this machine's, and
//! every date is UTC. A narrow window (`--columns` under
//! `space.containers.panesFrom`) is the stacked layout the footer's region
//! draws.
//!
//! Usage: `ronoctua-shell photograph --door containers [options]`, `--help`
//! for the list. Every word is this camera's, those before `--door
//! containers` too ([`taken`]).

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door containers [options] [steps]

  podman is the one the environment names (the variable below): for a
  photograph, a stand-in that prints recorded answers.

  --firewall <file>     the firewall's settings, as firewall.json keeps
                        them — never this machine's  (default: none written)
  --unreadable-firewall settings that exist and cannot be read
  --out <file>          where to write                 (default: containers.png)
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

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-i
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there

podman's answers are asked for after every step, and the frame is drawn
once they have arrived.";

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
    firewall: Option<std::path::PathBuf>,
    /// The settings are there and cannot be read.
    unreadable: bool,
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

/// Reads the words — all of them, `--door containers` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        firewall: None,
        unreadable: false,
        out: std::path::PathBuf::from("containers.png"),
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
                println!(
                    "{USAGE}\n\npodman's variable: {}",
                    ronoctua_containers::VARIABLE
                );
                std::process::exit(0);
            }
            "--door" => {
                let said = value()?;
                ensure!(
                    said == "containers",
                    "`--door {said}` is not this camera's door"
                );
            }
            "--firewall" => {
                let f = std::path::PathBuf::from(value()?);
                ensure!(f.is_file(), "`--firewall {}` is not a file", f.display());
                me.firewall = Some(f);
            }
            "--unreadable-firewall" => me.unreadable = true,
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
                // REFUSED RATHER THAN SNAPPED, as the player's camera refuses
                // it: a photograph at a size nobody asked for is worse than none.
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
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    Ok(me)
}

/// `ronoctua-shell photograph --door containers …`, to its end: the process
/// leaves with the answer, because the player's camera that handed the words
/// over has nothing to do with them. Every word of the command line is read
/// here — the file manager's camera's reason ([`words_of`]).
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
///
/// Refused when it is not, which is a caller that handed over words that did
/// not come from the command line: reading the line anyway would photograph
/// what somebody else asked for.
fn words_of(line: &[String], rest: &[String]) -> Result<Vec<String>> {
    ensure!(
        line.ends_with(rest),
        "the words handed to the containers' camera are not the end of the command line"
    );
    Ok(line.to_vec())
}

/// Draws the containers' window the words ask for into a PNG, or prints its
/// frame.
///
/// # Errors
///
/// A word not understood, a firewall file that is not one, no graphics
/// adapter, or a picture larger than the GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    // THE THEME, THE PAGE, THE SIZE OF THE TEXT AND THE DATE'S SHAPE, before
    // anything resolves a colour or a cell — the ones asked, never the
    // person's files.
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(asked.text);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    ensure!(
        !(asked.unreadable && asked.firewall.is_some()),
        "`--firewall` and `--unreadable-firewall` are two answers to one question: give one"
    );
    let firewall = match &asked.firewall {
        None if asked.unreadable => crate::containers::Firewall {
            unreadable: true,
            ..crate::containers::Firewall::default()
        },
        None => crate::containers::Firewall::default(),
        Some(file) => {
            let raw = std::fs::read(file)
                .with_context(|| format!("could not read `{}`", file.display()))?;
            let settings = ronoctua_security::firewall::Settings::of_bytes(&raw).map_err(|r| {
                anyhow::anyhow!("`{}` is not firewall settings: {:?}", file.display(), r.why)
            })?;
            crate::containers::Firewall {
                settings: Some(settings),
                status: None,
                unreadable: false,
            }
        }
    };
    let window = crate::containers_window::ContainersWindow::detached(
        ronoctua_containers::Podman::from_env(),
        firewall,
    );
    let mut d = Detached::new(ui, window);
    shoot(&mut d, &asked)
}

/// The size the window is laid out at.
fn size_of(d: &Detached<crate::containers_window::ContainersWindow>, asked: &Asked) -> (u32, u32) {
    use crate::door::Content as _;
    let px = tokens::text::size();
    let (cell, line) = (
        tokens::metrics::cell_width(px),
        tokens::metrics::line_height(px),
    );
    let (default_w, default_h) = d.content.default_size();
    // ROUNDED UP, as the player's camera rounds.
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
/// for as long as podman's answers keep arriving — never more than a handful
/// of times, since the feed asks on the spot.
fn settled(
    d: &mut Detached<crate::containers_window::ContainersWindow>,
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
fn shoot(
    d: &mut Detached<crate::containers_window::ContainersWindow>,
    asked: &Asked,
) -> Result<()> {
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
    // THE ANSWER FIRST, THEN THE FRAME: a window on a screen draws before
    // podman answers and again when it does, and the second frame is the one
    // a person looks at.
    let mut composed = settled(d, &gpu, &mut images);
    let mut entered = false;
    for step in &asked.steps {
        ensure!(
            !d.ui.quit,
            "the window closed at the step before `{step:?}`, and a step after it reaches nothing"
        );
        match *step {
            Step::Key(key) => d.press(key),
            Step::Hover(column, line) => {
                for events in frames_of((column, line), None, &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
            Step::Click(column, line, button, double) => {
                for events in
                    frames_of((column, line), Some((button, double)), &mut entered, &d.ui)?
                {
                    d.pointer(&events);
                }
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
        for (key, a) in &composed.pictures {
            println!(
                "picture {key} {:.1},{:.1} {:.1}x{:.1}",
                a.x, a.y, a.width, a.height
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
        "{}: {}x{} px, {} columns by {} lines, scale {}, {} runs, {} pictures, language {}, \
         theme {} on the {} page",
        asked.out.display(),
        gpu.width,
        gpu.height,
        d.ui.columns,
        d.ui.lines,
        asked.scale,
        composed.spans.len(),
        composed.pictures.len(),
        d.ui.texts.language(),
        asked.theme.id(),
        asked.page.id(),
    );
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    // THE CHILDREN ALIVE on this frame: a log and the stats live only while
    // their tab is shown.
    match d.content.following() {
        (None, false) => println!("no stream is followed"),
        (log, stats) => println!(
            "followed: log {}, stats {}",
            log.unwrap_or("none"),
            if stats { "yes" } else { "no" }
        ),
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
            "containers",
            "--key",
            "TAB",
            "--click",
            "3,4,double",
            "--columns",
            "140",
            "--language",
            "en-GB",
        ]))
        .expect("the words");
        assert_eq!(asked.width, Some(Measure::Cells(140)));
        assert_eq!(asked.steps.len(), 2);
        assert_eq!(asked.language, "en-GB");
        assert!(
            parse(&words(&["--door", "files"])).is_err(),
            "another door was taken"
        );
        assert!(
            parse(&words(&["--firewall", "/nonexistent/firewall.json"])).is_err(),
            "a firewall file that is not there was taken"
        );
        let line = words(&["--out", "a.png", "--door", "containers"]);
        let asked = parse(&words_of(&line, &line[2..]).expect("the line")).expect("the words");
        assert_eq!(asked.out, std::path::PathBuf::from("a.png"));
    }
}
