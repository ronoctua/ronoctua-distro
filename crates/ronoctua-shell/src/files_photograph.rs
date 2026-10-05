//! THE FILE MANAGER'S WINDOW IN A PNG — `ronoctua-shell photograph --door
//! files`.
//!
//! # Why it is a second camera and not a door of the first
//!
//! `crate::photograph` is the player's: every step of its walk is a state the
//! player's daemon would say, and its door is a player's (`Photographed`
//! hands one back). The manager has no daemon — what fills its two panels is
//! the inspector, a thread of the door's own — so what differs is exactly
//! what that module is made of. What is the SAME is taken as it is: the
//! production window, detached (`files_window::Files::detached`), driven
//! through `door::Detached` down the path a key and a pointer take, and a
//! frame composed by `door::compose`, the function `Door::paint` calls.
//!
//! # What makes the picture the same on every machine
//!
//! The folder is the one given (`--dir`), and nothing else of this machine's
//! is read: no roster (owners are numbers), UTC for every date, no catalogue
//! of programs, no trash, and a sidebar whose places are the folders in
//! `--home` — none at all without it. The inspector's answer is asked on the
//! spot rather than on its thread (`Files::inspect_now`), so a frame is never
//! taken before the preview it shows has arrived.
//!
//! Usage: `ronoctua-shell photograph --door files --dir <folder> [options]`,
//! `--help` for the list. Every word is this camera's, those before `--door
//! files` too ([`taken`]); but the player's camera reads the words before it
//! first, so only a word both cameras know (`--out`, `--language`, `--theme`,
//! the lengths…) may stand there — one only this camera knows (`--dir`) is
//! refused by the player's. `--door files` first is the spelling that always
//! works.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door files --dir <folder> [options] [steps]

  --dir <folder>        the folder the manager opens        (required)
  --home <folder>       the sidebar's places are the folders in it
                                                       (default: none)
  --out <file>          where to write                 (default: files.png)
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
  --luks <label>        a stick holding a LOCKED encrypted partition
  --luks-open <label>   a stick whose encrypted partition is open and mounted
                        (the sidebar's devices, in place of a bus)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-i
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there

The panels' answer is asked for after every step, and the frame is drawn
once it has arrived.";

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
    dir: std::path::PathBuf,
    home: Option<std::path::PathBuf>,
    out: std::path::PathBuf,
    width: Option<Measure>,
    height: Option<Measure>,
    language: String,
    theme: tokens::theme::Theme,
    page: tokens::theme::Ground,
    text: u16,
    scale: u16,
    list: bool,
    /// Encrypted sticks for the sidebar: the label, and whether it is open.
    luks: Vec<(String, bool)>,
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

/// Reads the words — all of them, `--door files` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut dir = None;
    let mut me = Asked {
        dir: std::path::PathBuf::new(),
        home: None,
        out: std::path::PathBuf::from("files.png"),
        width: None,
        height: None,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        page: tokens::theme::Ground::Chrome,
        text: tokens::text::NORMAL,
        scale: 1,
        list: false,
        luks: Vec::new(),
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
                ensure!(said == "files", "`--door {said}` is not this camera's door");
            }
            "--dir" => {
                let d = std::path::PathBuf::from(value()?);
                ensure!(d.is_dir(), "`--dir {}` is not a folder", d.display());
                dir = Some(d);
            }
            "--home" => {
                let d = std::path::PathBuf::from(value()?);
                ensure!(d.is_dir(), "`--home {}` is not a folder", d.display());
                me.home = Some(d);
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
            "--luks" => me.luks.push((value()?, false)),
            "--luks-open" => me.luks.push((value()?, true)),
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
    me.dir = dir.context("the manager opens a folder: give it with `--dir <folder>`")?;
    Ok(me)
}

/// `ronoctua-shell photograph --door files …`, to its end: the process leaves
/// with the answer, because the player's camera that handed the words over
/// has nothing to do with them.
///
/// # Every word, and not only the ones after `--door files`
///
/// The player's camera reads the words in order and hands over `rest`, what
/// it had not read yet, when it reaches `--door files` — having taken, and
/// kept to itself, every word before it. An `--out`, a `--language` or a
/// `--theme` given there was dropped without a word, and the photograph
/// written where nobody asked, in a language nobody asked. So this camera
/// reads the COMMAND LINE whole ([`words_of`]), the words the player's camera
/// was handed; a word before `--door files` that only the player's camera
/// knows is refused here, as every word this camera does not know is.
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
        "the words handed to the file manager's camera are not the end of the command line"
    );
    Ok(line.to_vec())
}

/// Draws the manager the words ask for into a PNG, or prints its frame.
///
/// # Errors
///
/// A word not understood, a folder that is not one, no graphics adapter, or a
/// picture larger than the GPU takes.
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
    // THE PLACES ARE THE FOLDERS UNDER `--home`, by name, as the session's
    // `RONOCTUA_FILES_SIDEBAR` names places under the person's home — never
    // that variable itself, which is this machine's.
    let places: Vec<String> = asked.home.as_deref().map_or_else(Vec::new, |home| {
        let mut names: Vec<String> = std::fs::read_dir(home)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    });
    let side = ronoctua_files::sidebar::Sidebar::of(
        &places,
        asked.home.as_deref(),
        None,
        ronoctua_files::sidebar::Preferences::fresh(),
        None,
    );
    let mut files = crate::files_window::Files::detached(&asked.dir, side, ui.texts.clone())?;
    if !asked.luks.is_empty() {
        files.racked_now(&sticks(&asked.luks));
    }
    let mut d = Detached::new(ui, files);
    shoot(&mut d, &asked)
}

/// The disks `--luks` and `--luks-open` ask for: a removable stick each, its
/// one partition an encrypted container — locked, or open with what came out
/// of it mounted. The object paths are made up and name no machine
/// (invariant 13); they are what the disk border would key the rows by.
fn sticks(asked: &[(String, bool)]) -> ronoctua_disks::Rack {
    use ronoctua_disks::{Bus, Drive, Holding, Rack, Volume};
    let mut rack = Rack::default();
    for (n, (label, open)) in asked.iter().enumerate() {
        let drive = format!("/drive/stick{n}");
        let node = format!(
            "/dev/sd{}",
            char::from(b'b' + u8::try_from(n % 20).unwrap_or(0))
        );
        rack.drives.push(Drive {
            at: drive.clone(),
            vendor: "Kingston".to_owned(),
            model: "DataTraveler".to_owned(),
            bytes: 32_000_000_000,
            bus: Bus::Usb,
            removable: true,
            ejectable: true,
            can_power_off: true,
            sort_key: format!("01hotplug/{n}"),
        });
        let container = format!("/block/stick{n}p1");
        let clear = format!("/block/dm-{n}");
        rack.volumes.push(Volume {
            at: container.clone(),
            node: format!("{node}1"),
            label: label.clone(),
            id_type: "crypto_LUKS".to_owned(),
            bytes: 31_999_000_000,
            drive: Some(drive),
            cleartext: open.then(|| clear.clone()),
            holds: Holding {
                encrypted: true,
                partition: true,
                ..Holding::default()
            },
            ..Volume::default()
        });
        if *open {
            rack.volumes.push(Volume {
                at: clear,
                node: format!("/dev/dm-{n}"),
                label: label.clone(),
                id_type: "ext4".to_owned(),
                bytes: 31_980_000_000,
                mounted_at: vec![format!("/run/media/somebody/{label}")],
                behind: Some(container),
                holds: Holding {
                    filesystem: true,
                    ..Holding::default()
                },
                ..Volume::default()
            });
        }
    }
    rack
}

/// The size the window is laid out at.
fn size_of(d: &Detached<crate::files_window::Files>, asked: &Asked) -> (u32, u32) {
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
/// for as long as the panels asked a question whose answer changes what is
/// drawn — never more than a handful of times, since an answer is asked once.
fn settled(
    d: &mut Detached<crate::files_window::Files>,
    gpu: &ronoctua_render::offscreen::Offscreen,
    images: &mut ronoctua_render::image::Images,
) -> crate::door::Composed {
    let mut composed = d.compose(&gpu.device, &gpu.queue, images);
    for _ in 0..4 {
        if !d.content.inspect_now(&mut d.ui) {
            break;
        }
        d.turn();
        composed = d.compose(&gpu.device, &gpu.queue, images);
    }
    composed
}

/// The window, walked and drawn.
fn shoot(d: &mut Detached<crate::files_window::Files>, asked: &Asked) -> Result<()> {
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
    // THE ANSWER FIRST, THEN THE FRAME: a window on a screen draws before the
    // inspector answers and again when it does, and the second frame is the
    // one a person looks at.
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
    fn the_words_are_the_walk_and_a_folder_is_required() {
        let here = std::env::temp_dir();
        let dir = here.to_string_lossy().into_owned();
        let asked = parse(&words(&[
            "--door",
            "files",
            "--dir",
            &dir,
            "--key",
            "<down>",
            "--click",
            "3,4,double",
            "--columns",
            "120",
        ]))
        .expect("the words");
        assert_eq!(asked.dir, here);
        assert_eq!(asked.width, Some(Measure::Cells(120)));
        assert_eq!(asked.steps.len(), 2);
        assert!(matches!(asked.steps[1], Step::Click(3, 4, _, true)));
        assert!(
            parse(&words(&["--door", "files"])).is_err(),
            "no folder was refused"
        );
        assert!(
            parse(&words(&["--door", "window", "--dir", &dir])).is_err(),
            "another door was taken"
        );
        assert!(
            parse(&words(&["--dir", &dir, "--columns", "9", "--width", "90"])).is_err(),
            "one length given twice was taken"
        );
    }

    #[test]
    fn the_words_before_the_door_are_read_and_never_dropped() {
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        // WHAT THE PLAYER'S CAMERA HANDS OVER is what follows `--door files`;
        // what this one reads is the line.
        let line = words(&[
            "--language",
            "en-GB",
            "--out",
            "asked.png",
            "--door",
            "files",
            "--dir",
            &dir,
        ]);
        let asked =
            parse(&words_of(&line, &line[6..]).expect("the line ends with what was handed"))
                .expect("the words");
        assert_eq!(
            asked.out,
            std::path::PathBuf::from("asked.png"),
            "the --out before the door was dropped"
        );
        assert_eq!(
            asked.language, "en-GB",
            "the --language before the door was dropped"
        );
        // A WORD ONLY THE PLAYER'S CAMERA KNOWS, before the door, is refused
        // here — never taken as said and never dropped.
        let line = words(&["--size", "medium", "--door", "files", "--dir", &dir]);
        assert!(
            parse(&words_of(&line, &line[4..]).expect("the line")).is_err(),
            "a word of the player's camera's was dropped"
        );
        // AND WORDS THAT ARE NOT HOW THE LINE ENDS are nobody's to photograph.
        assert!(
            words_of(&line, &words(&["--dir", "elsewhere"])).is_err(),
            "words that did not come from the line were read as the line"
        );
    }
}
