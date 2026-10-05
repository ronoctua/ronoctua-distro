//! THE ICONS APP'S WINDOW IN A PNG — `ronoctua-shell photograph --door
//! icons`.
//!
//! A camera of its own, for the file manager's reason (`crate::
//! files_photograph`): the player's camera walks a daemon's states, and this
//! window has none — what fills it is the three tables. What is the SAME is
//! taken as it is: the production window (`icons_window::IconsWindow`),
//! detached (`door::Detached`), walked down the path a key and a pointer take,
//! and a frame composed by `door::compose`, the function `Door::paint` calls.
//!
//! The tables are read on the spot (`ronoctua_iconset::shared`, from
//! `RONOCTUA_ICONSET` — the `.#png` launcher sets it for `--door`), and
//! without them the window draws what it draws on a screen without them: the
//! refusal, on its line.
//!
//! Usage: `ronoctua-shell photograph --door icons [options] [steps]`, `--help`
//! for the list.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Detached, LEFT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door icons [options] [steps]

  --screen <screen>     nerd-fonts, lucide or unicode  (default: nerd-fonts)
  --out <file>          where to write                 (default: icons.png)
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

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-2, a
  --type <words>        each character a key, as typed
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,double]>  a press and a release there";

/// One step of the walk.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// A key pressed and let go.
    Key(ronoctua_keymap::Key),
    /// The pointer rests on a cell.
    Hover(u16, u16),
    /// A press and a release on a cell, twice when `double`.
    Click(u16, u16, bool),
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
    screen: ronoctua_iconset::Collection,
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

/// A typed character as the key the keymap writes for it.
fn key_of_char(c: char) -> ronoctua_keymap::Key {
    let sym = if c == ' ' {
        ronoctua_keymap::Sym::Space
    } else {
        ronoctua_keymap::Sym::Char(c)
    };
    ronoctua_keymap::Key::simple(sym)
}

/// Reads the words — all of them, `--door icons` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        screen: ronoctua_iconset::Collection::NerdFonts,
        out: std::path::PathBuf::from("icons.png"),
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
                ensure!(said == "icons", "`--door {said}` is not this camera's door");
            }
            "--screen" => {
                let said = value()?;
                me.screen = crate::icons::screen_named(&said).with_context(|| {
                    format!(
                        "`{said}` is not a screen; the screens are nerd-fonts, lucide and unicode"
                    )
                })?;
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
                // REFUSED RATHER THAN SNAPPED, as the other cameras refuse it.
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
            "--type" => {
                for c in value()?.chars() {
                    me.steps.push(Step::Key(key_of_char(c)));
                }
            }
            "--hover" => {
                let (column, line, rest) = cell_of("--hover", &value()?)?;
                ensure!(rest.is_none(), "`--hover` wants `column,line`");
                me.steps.push(Step::Hover(column, line));
            }
            "--click" => {
                let (column, line, rest) = cell_of("--click", &value()?)?;
                let double = match rest.as_deref() {
                    None | Some("left") => false,
                    Some("double") => true,
                    Some(other) => {
                        bail!("`{other}` is not a click; the clicks are left and double")
                    }
                };
                me.steps.push(Step::Click(column, line, double));
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    Ok(me)
}

/// `ronoctua-shell photograph --door icons …`, to its end: the process leaves
/// with the answer. Every word of the line is this camera's, as the file
/// manager's reads them (`files_photograph::taken`).
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

/// Draws the window the words ask for into a PNG, or prints its frame.
///
/// # Errors
///
/// A word not understood, no graphics adapter, or a picture larger than the
/// GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(asked.text);
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let mut view = crate::icons::View::new(asked.screen, &ui.texts);
    // ON THE SPOT, and not on a thread: a photograph waits for the tables a
    // window would draw once they arrived.
    view.took(ronoctua_iconset::shared());
    let window = crate::icons_window::IconsWindow::over(view);
    let mut d = Detached::new(ui, window);
    shoot(&mut d, &asked)
}

/// The size the window is laid out at.
fn size_of(d: &Detached<crate::icons_window::IconsWindow>, asked: &Asked) -> (u32, u32) {
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

/// The pointer's frames for one step: it enters the first time and moves
/// after, then presses and lets go — twice for a double click.
fn frames_of(
    at: (u16, u16),
    click: Option<bool>,
    entered: &mut bool,
    ui: &Ui,
) -> Result<Vec<Vec<PointerEvent>>> {
    let px = tokens::text::size();
    let (cell, height) = (
        f64::from(tokens::metrics::cell_width(px)),
        f64::from(tokens::metrics::line_height(px)),
    );
    let position = (
        (f64::from(at.0) + 0.5) * cell,
        (f64::from(at.1) + 0.5) * height,
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
    if let Some(double) = click {
        for _ in 0..if double { 2 } else { 1 } {
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
    }
    Ok(out)
}

/// The window, walked and drawn.
fn shoot(d: &mut Detached<crate::icons_window::IconsWindow>, asked: &Asked) -> Result<()> {
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
    let mut composed = d.compose(&gpu.device, &gpu.queue, &mut images);
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
            Step::Click(column, line, double) => {
                for events in frames_of((column, line), Some(double), &mut entered, &d.ui)? {
                    d.pointer(&events);
                }
            }
        }
        d.turn();
        if d.ui.quit {
            break;
        }
        composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    }

    if asked.list {
        println!(
            "{}x{} px logical, {} columns by {} lines, scale {}",
            d.ui.width, d.ui.height, d.ui.columns, d.ui.lines, d.ui.scale
        );
        // THE CELL AND THE LINE, in logical pixels: what turns a run's line
        // and column into the pixel its box is centred on — for a reader of
        // this frame that asks WHERE a run is (`the-icon-grows-from-its-
        // centre`).
        println!(
            "cell {:.4} px, line {:.4} px",
            crate::icons::Area::cell(),
            crate::icons::Area::line()
        );
        for s in &composed.spans {
            println!(
                "run line {} column {} {:?} {:?} {:?}",
                s.line, s.column, s.size, s.color, s.text
            );
        }
        for q in &composed.quads {
            println!(
                "quad {:.1},{:.1} {:.1}x{:.1} {:?}",
                q.x, q.y, q.width, q.height, q.color
            );
        }
        for (key, a) in &composed.pictures {
            let key: String = key.chars().take(60).collect();
            println!(
                "picture {key}… {:.1},{:.1} {:.1}x{:.1}",
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn the_words_are_the_walk() {
        let asked = parse(&words(&[
            "--door",
            "icons",
            "--screen",
            "lucide",
            "--type",
            "ho",
            "--key",
            "RET",
            "--click",
            "3,4,double",
        ]))
        .expect("the words");
        assert_eq!(asked.screen, ronoctua_iconset::Collection::Lucide);
        assert_eq!(asked.steps.len(), 4);
        assert!(matches!(asked.steps[3], Step::Click(3, 4, true)));
        assert!(parse(&words(&["--door", "files"])).is_err(), "another door");
        assert!(
            parse(&words(&["--screen", "emoji"])).is_err(),
            "no such screen"
        );
    }
}
