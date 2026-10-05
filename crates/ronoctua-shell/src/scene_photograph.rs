//! FOUR DOORS IN A PNG over a scene handed in whole — `ronoctua-shell
//! photograph --door display|clipboard|curtain|lifeline`.
//!
//! # Why one camera for four
//!
//! Because what each needs is the same: the production door, DETACHED, over
//! a reading the words give instead of the machine's — the screens, the
//! clipboard's history, the resizer's cut, the unit that fell — walked down
//! the path a key and a pointer take (`door::Detached`) and composed by
//! `door::compose`, the function `Door::paint` calls. The lifeline is not a
//! door; its frame is [`crate::lifeline::line_of`], the function its own
//! paint calls, over the ground its paint clears to.
//!
//! # What makes the picture the same on every machine
//!
//! Nothing is read from this one: the screens are a scene of this camera's
//! (`--screens`), the history is the entries given, the cut is the sizes
//! given, and no order leaves — an order the display window would give is
//! said to have no thread, an act the clipboard's window would send stays on
//! its line, and what each would have done is printed.
//!
//! Usage: `ronoctua-shell photograph --door <door> [options]`, `--help` for
//! the list. Every word is this camera's, those before `--door` too
//! ([`taken`]).

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Content, Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door display|clipboard|curtain|lifeline [options] [steps]

  display    the screens' window (`LEADER s d w`) over a scene of screens
  clipboard  the clipboard's window (`LEADER LEADER c`) over a history given
  curtain    the resizer's bottom curtain (`LEADER / r`) over a cut given
  lifeline   the line that outlives the footer, for a unit that fell

  --screens <scene>     for display: two (side by side), stacked, dark (the
                        second off, with what is filed for it), one
                                                       (default: two)
  --entry <text>        for clipboard: an entry of the history, newest first
                        — repeatable
  --image <png>         for clipboard: an image entry — repeatable
  --pinned <n>          for clipboard: entry n is pinned (from 0)
  --screen <w>x<h>      for curtain: the screen that is cut (default: 1920x1080)
  --room <w>x<h>        for curtain: the room the curtains leave
                                                       (default: 1280x720)
  --unit <name>         for lifeline: the unit         (default: ronoctua-footer.service)
  --result <word>       for lifeline: systemd's Result= (default: core-dump)
  --asking              for lifeline: the question about sending the report is up
  --out <file>          where to write                 (default: <door>.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --width <px>          the window's width, in logical pixels
  --height <px>         the height, in logical pixels
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear (not for lifeline):

  --key <notation>      a key, as the keymap writes it: <down>, RET, S-<up>
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there";

/// Which door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Display,
    Clipboard,
    Curtain,
    Lifeline,
}

impl Which {
    fn of(word: &str) -> Option<Self> {
        Some(match word {
            "display" => Self::Display,
            "clipboard" => Self::Clipboard,
            "curtain" => Self::Curtain,
            "lifeline" => Self::Lifeline,
            _ => return None,
        })
    }

    const fn word(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Clipboard => "clipboard",
            Self::Curtain => "curtain",
            Self::Lifeline => "lifeline",
        }
    }
}

/// One step of the walk.
#[derive(Debug, Clone, PartialEq)]
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
    which: Which,
    screens: String,
    entries: Vec<ronoctua_clipboard::Entry>,
    pinned: Vec<usize>,
    screen: (u32, u32),
    room: (u32, u32),
    unit: String,
    result: String,
    asking: bool,
    out: Option<std::path::PathBuf>,
    width: Option<Measure>,
    height: Option<Measure>,
    language: String,
    theme: tokens::theme::Theme,
    text: u16,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

/// A whole number inside `range`.
pub(crate) fn number<T>(flag: &str, raw: &str, range: std::ops::RangeInclusive<T>) -> Result<T>
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

/// `<w>x<h>`.
pub(crate) fn pair(flag: &str, raw: &str) -> Result<(u32, u32)> {
    let (w, h) = raw
        .split_once('x')
        .with_context(|| format!("`{flag}` wants `<w>x<h>`, not `{raw}`"))?;
    Ok((number(flag, w, 1..=16384)?, number(flag, h, 1..=16384)?))
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

/// Reads the words — all of them, `--door` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        which: Which::Display,
        screens: "two".to_owned(),
        entries: Vec::new(),
        pinned: Vec::new(),
        screen: (1920, 1080),
        room: (1280, 720),
        unit: "ronoctua-footer.service".to_owned(),
        result: "core-dump".to_owned(),
        asking: false,
        out: None,
        width: None,
        height: None,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        text: tokens::text::NORMAL,
        scale: 1,
        list: false,
        steps: Vec::new(),
    };
    let mut door = None;
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
                door = Some(
                    Which::of(&said)
                        .with_context(|| format!("`--door {said}` is not this camera's door"))?,
                );
            }
            "--screens" => me.screens = value()?,
            "--entry" => me
                .entries
                .push(ronoctua_clipboard::Entry::of_text(value()?)),
            "--image" => {
                let f = std::path::PathBuf::from(value()?);
                let bytes = std::fs::read(&f)
                    .with_context(|| format!("could not read `{}`", f.display()))?;
                me.entries
                    .push(ronoctua_clipboard::Entry::of_image(bytes, "image/png"));
            }
            "--pinned" => me.pinned.push(number("--pinned", &value()?, 0..=49)?),
            "--screen" => me.screen = pair("--screen", &value()?)?,
            "--room" => me.room = pair("--room", &value()?)?,
            "--unit" => me.unit = value()?,
            "--result" => me.result = value()?,
            "--asking" => me.asking = true,
            "--out" | "-o" => me.out = Some(std::path::PathBuf::from(value()?)),
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
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    me.which = door.context("which door? `--door display|clipboard|curtain|lifeline`")?;
    ensure!(
        me.which != Which::Lifeline || me.steps.is_empty(),
        "the lifeline takes no steps: it is drawn as it rises"
    );
    Ok(me)
}

/// `ronoctua-shell photograph --door <one of the four> …`, to its end.
pub fn taken(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let words = if line.ends_with(rest) {
        Ok(line)
    } else {
        Err(anyhow::anyhow!(
            "the words handed to the scenes' camera are not the end of the command line"
        ))
    };
    match words.and_then(|w| run(&w)) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// A screen of a scene — round numbers no panel has.
fn screen(at: &str, x: i32, y: i32, lit: bool) -> ronoctua_display::Screen {
    use ronoctua_compositor::{Logical, Mode, Transform};
    let mode = |width: u16, height: u16, refresh_rate: u32| Mode {
        width,
        height,
        refresh_rate,
    };
    ronoctua_display::Screen {
        at: at.to_owned(),
        naming: ronoctua_compositor::Naming::Description(format!("Make Model {at}")),
        description: format!("Make Model {at}"),
        make: "Make".to_owned(),
        model: "Model".to_owned(),
        serial: Some(at.to_owned()),
        millimetres: Some((520, 290)),
        modes: vec![
            mode(1920, 1080, 59_951),
            mode(1920, 1080, 143_856),
            mode(1280, 720, 60_000),
        ],
        current: Some(0),
        logical: lit.then_some(Logical {
            x,
            y,
            width: 1920,
            height: 1080,
            scale: 1.0,
            transform: Transform::Normal,
        }),
        vrr_supported: true,
        vrr_enabled: false,
        internal: at.starts_with("eDP"),
        light: None,
        wiring: ronoctua_display::connector::Wiring::default(),
        wanted: ronoctua_display::fragment::Wanted::default(),
    }
}

/// The screens of a scene.
pub(crate) fn scene(name: &str) -> Result<ronoctua_display::Snapshot> {
    let screens = match name {
        "one" => vec![screen("eDP-1", 0, 0, true)],
        "two" => vec![screen("eDP-1", 0, 0, true), screen("DP-1", 1920, 0, true)],
        "stacked" => {
            let mut top = screen("DP-1", 0, 0, true);
            top.wanted.position = Some(ronoctua_display::fragment::Spot { x: 0, y: 0 });
            let mut under = screen("eDP-1", 0, 1080, true);
            under.wanted.position = Some(ronoctua_display::fragment::Spot { x: 0, y: 1080 });
            under.wanted.depth = Some(10);
            vec![under, top]
        }
        "dark" => {
            let mut off = screen("DP-1", 0, 0, false);
            off.wanted.off = true;
            off.wanted.scale = Some(1.5);
            off.wanted.depth = Some(10);
            off.wanted.mode = Some(ronoctua_display::fragment::Chosen {
                width: 1920,
                height: 1080,
                rate: Some(143.856),
            });
            off.wanted.position = Some(ronoctua_display::fragment::Spot { x: 1920, y: 0 });
            off.wanted.transform = ronoctua_compositor::Transform::R90;
            vec![screen("eDP-1", 0, 0, true), off]
        }
        other => {
            bail!("`--screens {other}` is not a scene; the scenes are one, two, stacked and dark")
        }
    };
    Ok(ronoctua_display::Snapshot {
        focused: screens.first().map(|s| s.at.clone()),
        screens,
        compositor: true,
        settings_failed: false,
    })
}

/// Draws what the words ask for into a PNG, or prints its frame.
///
/// # Errors
///
/// A word not understood, a scene that is not one, no graphics adapter, or a
/// picture larger than the GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    let asked = parse(words)?;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(tokens::theme::Ground::Chrome);
    tokens::text::set(asked.text);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    match asked.which {
        Which::Display => {
            let snapshot = scene(&asked.screens)?;
            let window = crate::display_window::photographed(snapshot);
            walked(Detached::new(ui, window), &asked)
        }
        Which::Clipboard => {
            let mut queue = ronoctua_clipboard::Queue::of_entries(asked.entries.clone());
            for n in &asked.pinned {
                ensure!(
                    queue.toggle_pinned(*n).is_some(),
                    "`--pinned {n}` names no entry"
                );
            }
            let mirror = crate::clipboard_wire::Mirror::of_parts(
                queue.inputs().iter(),
                &ronoctua_clipcascade::Snapshot::default(),
                1,
            );
            walked(
                Detached::new(ui, crate::clipboard_window::photographed(mirror)),
                &asked,
            )
        }
        Which::Curtain => {
            let state = crate::resizer::State {
                output: "DP-1".to_owned(),
                screen_width_px: asked.screen.0,
                screen_height_px: asked.screen.1,
                width_px: asked.room.0,
                height_px: asked.room.1,
            };
            let qh = ui.conn.new_event_queue().handle();
            let curtain =
                crate::resizer::photographed(crate::resizer::Edge::Bottom, state.clone(), qh);
            // THE BOTTOM CURTAIN'S OWN SIZE, as the compositor configures it:
            // the screen's width, and what the room leaves of its height.
            let mut d = Detached::new(ui, curtain);
            let tall = crate::resizer::curtain_px(crate::resizer::Edge::Bottom, &state);
            d.lay_out_at((state.screen_width_px, tall));
            shoot(d, &asked, true)
        }
        Which::Lifeline => lifeline(ui, &asked),
    }
}

/// The size the window is laid out at.
fn size_of<C: Content>(d: &Detached<C>, asked: &Asked) -> (u32, u32) {
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

/// A window laid out at the size asked, walked, and drawn.
fn walked<C: Content>(mut d: Detached<C>, asked: &Asked) -> Result<()> {
    let size = size_of(&d, asked);
    d.lay_out_at(size);
    shoot(d, asked, false)
}

/// Where the middle of a cell is, in logical pixels.
fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    (
        (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
        (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
    )
}

/// The pointer's frames for one step.
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

/// The door, walked and drawn.
fn shoot<C: Content>(mut d: Detached<C>, asked: &Asked, laid: bool) -> Result<()> {
    let _ = laid;
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let format = ronoctua_render::offscreen::FORMAT;
    let mut images = ronoctua_render::image::Images::new(&gpu.device, format)?;
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
        composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    }
    for order in d.ui.orders() {
        println!(
            "would have told the footer: {}",
            order.split(' ').take(3).collect::<Vec<_>>().join(" ")
        );
    }
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    let logical = (d.ui.width, d.ui.height);
    draw(
        &mut gpu,
        &mut images,
        &composed,
        logical,
        &asked.film(),
        d.ui.scale,
    )
}

/// The line that outlives the footer, as it rises: [`crate::lifeline::line_of`]
/// over the ground its paint clears to.
fn lifeline(mut ui: Ui, asked: &Asked) -> Result<()> {
    use ronoctua_i18n::key;
    let texts = std::rc::Rc::clone(&ui.texts);
    let height = tokens::metrics::line_height(tokens::text::size()) * 2.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size = (
        match asked.width {
            Some(Measure::Px(w)) => w,
            Some(Measure::Cells(n)) => {
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
    let said = texts.t(if asked.asking {
        key::LIFELINE_AI_SHOWN
    } else {
        key::LIFELINE_FELL
    });
    let evidence = texts.t_with(
        key::LIFELINE_WHY,
        &[
            ("unit", asked.unit.as_str()),
            ("why", asked.result.as_str()),
        ],
    );
    let (spans, _) = crate::lifeline::line_of(&crate::lifeline::Line {
        said: &said,
        evidence: &evidence,
        offered: if asked.asking {
            &crate::lifeline::ASKED
        } else {
            &crate::lifeline::OFFERED
        },
        asking: asked.asking,
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
    draw(
        &mut gpu,
        &mut images,
        &composed,
        size,
        &asked.film(),
        ui.scale,
    )
}

/// Where and how a frame is developed — this camera's and the footer's
/// (`crate::footer_photograph`).
pub(crate) struct Film<'a> {
    /// Print the frame instead of drawing it.
    pub list: bool,
    /// Physical pixels per logical one.
    pub by: u16,
    /// The language the text is shaped for.
    pub language: &'a str,
    /// The PNG.
    pub out: std::path::PathBuf,
}

impl Asked {
    fn film(&self) -> Film<'_> {
        Film {
            list: self.list,
            by: self.scale,
            language: &self.language,
            out: self
                .out
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from(format!("{}.png", self.which.word()))),
        }
    }
}

/// Prints the frame, or draws it into the PNG asked for.
pub(crate) fn draw(
    gpu: &mut ronoctua_render::offscreen::Offscreen,
    images: &mut ronoctua_render::image::Images,
    composed: &crate::door::Composed,
    logical: (u32, u32),
    film: &Film<'_>,
    scale: i32,
) -> Result<()> {
    if film.list {
        println!("{}x{} px logical, scale {scale}", logical.0, logical.1);
        for s in &composed.spans {
            println!(
                "run line {} column {} {:?} {:?} {:?}",
                s.line, s.column, s.size, s.color, s.text
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
    let format = ronoctua_render::offscreen::FORMAT;
    let by = u32::from(film.by);
    let physical = (logical.0.saturating_mul(by), logical.1.saturating_mul(by));
    gpu.resize(physical.0, physical.1);
    ensure!(
        (gpu.width, gpu.height) == physical,
        "the picture asked for is {}x{} px and this GPU takes at most {} on either side",
        physical.0,
        physical.1,
        gpu.ceiling,
    );
    let mut text = ronoctua_render::Text::new(
        &gpu.device,
        &gpu.queue,
        format,
        scale,
        tokens::text::size(),
        film.language,
    )?;
    let mut quads = ronoctua_render::quad::Quads::new(&gpu.device, format)?;
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
    let (drawn, rects, pictures) = (&text, &quads, &*images);
    gpu.frame(tokens::color::SURFACE_CHROME.to_linear(), |pass| {
        composed.draw(pass, drawn, rects, pictures)
    })?;
    let rgba = gpu.read_rgba()?;
    let out = &film.out;
    ronoctua_render::offscreen::write_png(out, &rgba, gpu.width, gpu.height)?;
    println!(
        "{}: {}x{} px, {} runs, language {}",
        out.display(),
        gpu.width,
        gpu.height,
        composed.spans.len(),
        film.language
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// FAILS IF: a word of one of the four doors is lost, the door is taken
    /// for another, or the lifeline is given a walk it cannot take.
    #[test]
    fn the_words_are_the_walk_and_the_door() {
        let asked = parse(&words(&[
            "--door",
            "display",
            "--screens",
            "dark",
            "--key",
            "S-<up>",
            "--columns",
            "100",
        ]))
        .expect("the words");
        assert_eq!(asked.which, Which::Display);
        assert_eq!(asked.steps.len(), 1);
        assert!(scene("dark").is_ok() && scene("nowhere").is_err());
        assert!(parse(&words(&["--door", "files"])).is_err());
        assert!(parse(&words(&["--door", "lifeline", "--key", "a"])).is_err());
        let clip = parse(&words(&[
            "--door",
            "clipboard",
            "--entry",
            "one",
            "--pinned",
            "0",
        ]))
        .expect("the words");
        assert_eq!(clip.entries.len(), 1);
    }
}
