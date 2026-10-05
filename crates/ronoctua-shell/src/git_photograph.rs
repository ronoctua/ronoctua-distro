//! THE GIT BROWSER'S WINDOW IN A PNG — `ronoctua-shell photograph --door
//! git`.
//!
//! The production window (`git_browser_window::GitWindow`), detached, driven
//! down the path a key and a pointer take (`door::Detached`), and drawn by
//! `door::compose`, the function `Door::paint` calls — the file manager's
//! camera's arrangement (`crate::files_photograph`), for this door. What
//! makes the picture the same on every machine is the repository: the one
//! given (`--dir`), read with git on this thread before every frame, so a
//! frame is never taken before the answers it shows; UTC for every date.
//!
//! Usage: `ronoctua-shell photograph --door git --dir <folder> [options]`,
//! `--help` for the list. As with the file manager's camera, every word is
//! this camera's and `--door git` first is the spelling that always works.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Detached, LEFT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door git --dir <folder> [options] [steps]

  --dir <folder>        a folder inside the repository     (required)
  --out <file>          where to write                 (default: git.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --page <id>           chrome, backdrop, near or absolute (default: chrome)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-2
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line>    a press and a release there

The repository is read after every step, and the frame is drawn once every
answer has arrived.";

/// One step of the walk.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Key(ronoctua_keymap::Key),
    Hover(u16, u16),
    Click(u16, u16),
}

/// What was asked for.
#[derive(Debug)]
struct Asked {
    dir: std::path::PathBuf,
    out: std::path::PathBuf,
    columns: Option<u16>,
    lines: Option<u16>,
    language: String,
    theme: tokens::theme::Theme,
    page: tokens::theme::Ground,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

fn number(flag: &str, raw: &str, most: u16) -> Result<u16> {
    let n: u16 = raw
        .trim()
        .parse()
        .ok()
        .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
    ensure!(
        (1..=most).contains(&n),
        "`{flag} {n}` is out of range; the range is 1..={most}"
    );
    Ok(n)
}

fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16)> {
    let mut parts = raw.split(',');
    let (Some(column), Some(line), None) = (parts.next(), parts.next(), parts.next()) else {
        bail!("`{flag}` wants `column,line`, not `{raw}`");
    };
    Ok((number(flag, column, 999)?, number(flag, line, 999)?))
}

/// Reads the words — all of them, `--door git` included.
fn parse(words: &[String]) -> Result<Asked> {
    let mut dir = None;
    let mut me = Asked {
        dir: std::path::PathBuf::new(),
        out: std::path::PathBuf::from("git.png"),
        columns: None,
        lines: None,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        page: tokens::theme::Ground::Chrome,
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
                ensure!(said == "git", "`--door {said}` is not this camera's door");
            }
            "--dir" => {
                let d = std::path::PathBuf::from(value()?);
                ensure!(d.is_dir(), "`--dir {}` is not a folder", d.display());
                dir = Some(d);
            }
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => me.columns = Some(number("--columns", &value()?, 1000)?),
            "--lines" => me.lines = Some(number("--lines", &value()?, 1000)?),
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
            "--scale" => {
                let most = u16::try_from(ronoctua_render::MOST_SCALE).unwrap_or(1);
                me.scale = number("--scale", &value()?, most)?;
            }
            "--list" => me.list = true,
            "--key" => {
                let notation = value()?;
                let key = ronoctua_keymap::Key::parse(&notation)
                    .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                me.steps.push(Step::Key(key));
            }
            "--hover" => {
                let (c, l) = cell_of("--hover", &value()?)?;
                me.steps.push(Step::Hover(c, l));
            }
            "--click" => {
                let (c, l) = cell_of("--click", &value()?)?;
                me.steps.push(Step::Click(c, l));
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    me.dir = dir.context("the browser reads a repository: give a folder in it with `--dir`")?;
    Ok(me)
}

/// `ronoctua-shell photograph --door git …`, to its end.
pub fn taken(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let words = if line.ends_with(rest) {
        line
    } else {
        rest.to_vec()
    };
    match run(&words) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// Draws the browser the words ask for into a PNG, or prints its frame.
///
/// # Errors
///
/// A word not understood, a folder that is not one, no graphics adapter, or
/// a picture larger than the GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(tokens::text::NORMAL);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let (sender, _backs) = smithay_client_toolkit::reexports::calloop::channel::channel::<
        crate::git_browser_window::Back,
    >();
    let window = crate::git_browser_window::GitWindow::over(
        &asked.dir,
        ui.texts.clone(),
        ronoctua_files::when::Zone::utc(),
        sender,
    )
    .at_once();
    ensure!(
        window.repo().is_some(),
        "`--dir {}` is in no repository",
        asked.dir.display()
    );
    let mut d = Detached::new(ui, window);
    shoot(&mut d, &asked)
}

fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    (
        (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
        (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
    )
}

fn frames_of(
    at: (u16, u16),
    click: bool,
    entered: &mut bool,
    ui: &Ui,
) -> Result<Vec<Vec<PointerEvent>>> {
    let position = middle_of(at.0, at.1);
    ensure!(
        position.0 < f64::from(ui.width) && position.1 < f64::from(ui.height),
        "the cell {},{} is outside the window",
        at.0,
        at.1
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

/// The window, walked and drawn.
fn shoot(d: &mut Detached<crate::git_browser_window::GitWindow>, asked: &Asked) -> Result<()> {
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
    // THE ANSWERS FIRST, THEN THE FRAME — twice, because the first frame
    // lays the region out and what it lays out decides what is asked.
    let _ = d.compose(&gpu.device, &gpu.queue, &mut images);
    d.content.ask_now();
    let mut composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    let mut entered = false;
    for step in &asked.steps {
        ensure!(
            !d.ui.quit,
            "the window closed before `{step:?}`, and a step after it reaches nothing"
        );
        match *step {
            Step::Key(key) => d.press(key),
            Step::Hover(c, l) => {
                for events in frames_of((c, l), false, &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
            Step::Click(c, l) => {
                for events in frames_of((c, l), true, &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
        }
        d.turn();
        d.content.ask_now();
        let _ = d.compose(&gpu.device, &gpu.queue, &mut images);
        d.content.ask_now();
        composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    }
    if asked.list {
        println!(
            "{}x{} px logical, {} columns by {} lines",
            d.ui.width, d.ui.height, d.ui.columns, d.ui.lines
        );
        for s in &composed.spans {
            println!(
                "run line {} column {} {:?} {:?}",
                s.line, s.column, s.color, s.text
            );
        }
        return Ok(());
    }
    let logical = (d.ui.width, d.ui.height);
    let by = u32::from(asked.scale);
    let physical = (logical.0.saturating_mul(by), logical.1.saturating_mul(by));
    gpu.resize(physical.0, physical.1);
    ensure!(
        (gpu.width, gpu.height) == physical,
        "the picture asked for is {}x{} px and this GPU takes at most {}",
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
        "{}: {}x{} px, {} columns by {} lines, language {}",
        asked.out.display(),
        gpu.width,
        gpu.height,
        d.ui.columns,
        d.ui.lines,
        d.ui.texts.language(),
    );
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_are_the_walk_and_a_folder_is_required() {
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let w = |all: &[&str]| all.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        let asked = parse(&w(&[
            "--door",
            "git",
            "--dir",
            &dir,
            "--key",
            "RET",
            "--click",
            "3,4",
            "--columns",
            "180",
        ]))
        .unwrap();
        assert_eq!(asked.columns, Some(180));
        assert_eq!(asked.steps.len(), 2);
        assert!(parse(&w(&["--door", "git"])).is_err(), "no folder");
        assert!(parse(&w(&["--door", "files", "--dir", &dir])).is_err());
        assert!(parse(&w(&["--dir", &dir, "--click", "3"])).is_err());
    }
}
