//! THE BELONGINGS' WINDOW IN A PNG — `ronoctua-shell photograph --door
//! belongings`.
//!
//! # Why a camera of its own
//!
//! The player's camera (`crate::photograph`) walks a player's states and the
//! file manager's (`crate::files_photograph`) its inspector; this window has
//! neither — what it draws is FILES, the ones the CLI and the doors write
//! (`belongings::Readings`). So the picture is taken over a folder laid out as
//! a machine lays them out (`--places`: `home/`, `home/.config/ronoctua/`,
//! `home/.local/state/ronoctua/`, `run/ronoctua-belongings/`), never this
//! machine's own, at a clock that does not move (`--now`). What is the SAME
//! is taken as it is: the production window, driven through
//! `door::Detached` down the path a key and a pointer take, a frame composed
//! by `door::compose`.
//!
//! # What it reaches
//!
//! Nothing past the door: the window's orders go nowhere
//! (`belongings_window::Reach::Nowhere`) and are printed (`would have sent:
//! prepare`). What a key WRITES — a decision, the settings, the passphrase's
//! file — it writes under `--places`, which is why the folder given is copied
//! first and the copy is what the walk changes.
//!
//! Usage: `ronoctua-shell photograph --door belongings --places <folder>
//! [options]`, `--help` for the list.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_belongings::places::Places;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::belongings_window::{BelongingsWindow, Reach};
use crate::door::{Content as _, Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door belongings --places <folder> [options] [steps]

  --places <folder>     a machine's belongings laid out under one folder:
                        home/, home/.config/ronoctua/ (belongings.json,
                        belongings-keys/), home/.local/state/ronoctua/
                        belongings/ (status.json, last.json) and
                        run/ronoctua-belongings/ (decisions.json, session/)
                        — copied first; the copy is what a key changes
                                                       (required)
  --now <seconds>       the clock, in seconds since the epoch (default:
                        1790000000)
  --out <file>          where to write                 (default: belongings.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --width <px>          the window's width, in logical pixels
  --height <px>         the height, in logical pixels
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --page <id>           chrome, backdrop, near or absolute (default: chrome)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --busy                a run of the belongings holds the lock while the
                        picture is taken (`running::hold` on the copy), so
                        the page draws a sync going on and a key refuses
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-l, a
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there";

/// One step of the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(ronoctua_keymap::Key),
    Hover(u16, u16),
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
    places: std::path::PathBuf,
    now: i64,
    out: std::path::PathBuf,
    width: Option<Measure>,
    height: Option<Measure>,
    language: String,
    theme: tokens::theme::Theme,
    page: tokens::theme::Ground,
    text: u16,
    scale: u16,
    list: bool,
    busy: bool,
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

/// A length given ONCE.
fn once(slot: &mut Option<Measure>, given: Measure, cells: &str, px: &str) -> Result<()> {
    ensure!(
        slot.replace(given).is_none(),
        "`{cells}` and `{px}` are one length, given twice: give one of them, once"
    );
    Ok(())
}

/// Reads the words — all of them, `--door belongings` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut places = None;
    let mut me = Asked {
        places: std::path::PathBuf::new(),
        now: 1_790_000_000,
        out: std::path::PathBuf::from("belongings.png"),
        width: None,
        height: None,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        page: tokens::theme::Ground::Chrome,
        text: tokens::text::NORMAL,
        scale: 1,
        list: false,
        busy: false,
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
                    said == "belongings",
                    "`--door {said}` is not this camera's door"
                );
            }
            "--places" => {
                let d = std::path::PathBuf::from(value()?);
                ensure!(d.is_dir(), "`--places {}` is not a folder", d.display());
                places = Some(d);
            }
            "--now" => me.now = number("--now", &value()?, 0..=i64::from(u32::MAX))?,
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
            "--busy" => me.busy = true,
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
    me.places = places.context("the belongings are read from a folder: `--places <folder>`")?;
    Ok(me)
}

/// `ronoctua-shell photograph --door belongings …`, to its end. Every word
/// of the command line is read, the ones before `--door belongings` too —
/// the file manager's camera's reason.
pub fn taken(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let run_it = || -> Result<()> {
        ensure!(
            line.ends_with(rest),
            "the words handed to the belongings' camera are not the end of the command line"
        );
        run(&line)
    };
    match run_it() {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// Copies a folder whole, links as links, modes kept.
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

/// The clock of this photograph.
static NOW: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

fn fixed_now() -> i64 {
    NOW.load(std::sync::atomic::Ordering::Relaxed)
}

/// Draws the belongings the words ask for into a PNG, or prints its frame.
///
/// # Errors
///
/// A word not understood, a folder that is not one, no graphics adapter, or a
/// picture larger than the GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(asked.text);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    // THE COPY, beside the picture: what a key writes lands there, and the
    // folder given stays as it was.
    let copy = asked.out.with_extension("places");
    let _ = std::fs::remove_dir_all(&copy);
    copy_tree(&asked.places, &copy)?;
    // A RUN IN FLIGHT, staged: the lock held on the copy for as long as the
    // picture takes — the same lock the CLI holds, read by the same
    // `running::busy` the door asks.
    let held = asked
        .busy
        .then(|| ronoctua_belongings::running::hold(&Places::under(&copy)))
        .transpose()
        .context("could not hold the belongings' lock on the copy")?;
    NOW.store(asked.now, std::sync::atomic::Ordering::Relaxed);
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let window = BelongingsWindow::over(
        Some(Places::under(&copy)),
        fixed_now,
        ronoctua_files::when::Zone::utc(),
        Reach::Nowhere(Vec::new()),
        Vec::new(),
    );
    let shot = shoot(&mut Detached::new(ui, window), &asked);
    drop(held);
    let _ = std::fs::remove_dir_all(&copy);
    shot
}

/// The size the window is laid out at.
fn size_of(d: &Detached<BelongingsWindow>, asked: &Asked) -> (u32, u32) {
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

/// The pointer's frames for one step.
fn frames_of(
    at: (u16, u16),
    button: Option<(u32, bool)>,
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

/// The window, walked and drawn.
fn shoot(d: &mut Detached<BelongingsWindow>, asked: &Asked) -> Result<()> {
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
    }
    let composed = d.compose(&gpu.device, &gpu.queue, &mut images);

    for op in d.content.sent() {
        println!("would have sent: {}", op.word());
    }
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    if asked.list {
        println!(
            "{}x{} px logical, {} columns by {} lines, scale {}",
            d.ui.width, d.ui.height, d.ui.columns, d.ui.lines, d.ui.scale
        );
        for s in &composed.spans {
            println!(
                "run line {} column {} {:?} {:?} {:?}",
                s.line, s.column, s.size, s.color, s.text
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
        "the picture asked for is {}x{} px and this GPU takes at most {} on either side",
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
    Ok(())
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
            "--door",
            "belongings",
            "--places",
            &dir,
            "--key",
            "C-l",
            "--click",
            "3,4,double",
            "--now",
            "5",
            "--busy",
        ]))
        .expect("the words");
        assert_eq!(asked.places, here);
        assert_eq!(asked.now, 5);
        assert!(asked.busy);
        assert_eq!(asked.steps.len(), 2);
        assert!(parse(&words(&["--door", "belongings"])).is_err());
        assert!(parse(&words(&["--door", "files", "--places", &dir])).is_err());
    }
}
