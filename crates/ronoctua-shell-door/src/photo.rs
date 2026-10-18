//! THE CAMERA'S DARKROOM: a frame printed or developed into a PNG, the
//! numbers and pairs every camera's words carry, the screens a scene plants,
//! and the footer's room a view reads the machine's absolute places under.
//!
//! # Why a module of its own
//!
//! Each window's camera lives beside its door (`photograph --door …`), and
//! they all develop a frame the same way: that shared half was inside the
//! four-door scenes' camera and the footer's, so every camera named another
//! camera to print a picture. What is the same for every camera is here.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;

/// A DOOR'S CAMERA OF ITS OWN, by the word `photograph --door <door>` names
/// it with: it reads the rest of the words and ends the process.
///
/// Each crate with a camera says its cameras in one table (`pub const
/// CAMERAS`), beside its doors, and the binary's camera looks the word up in
/// every table it lists — so a camera is written in the crate whose door it
/// photographs, and a door added there recompiles that crate and the binary.
#[derive(Clone, Copy)]
pub struct Camera {
    /// Its word after `--door`.
    pub door: &'static str,
    /// Takes the picture over the rest of the words, and ends the process.
    pub take: fn(&[String]) -> !,
}

impl std::fmt::Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("door", &self.door)
            .finish_non_exhaustive()
    }
}

/// The camera `door` names in `tables`, the first that does.
#[must_use]
pub fn camera_named(tables: &[&[Camera]], door: &str) -> Option<Camera> {
    tables
        .iter()
        .flat_map(|t| t.iter())
        .find(|c| c.door == door)
        .copied()
}

/// A whole number inside `range`.
pub fn number<T>(flag: &str, raw: &str, range: std::ops::RangeInclusive<T>) -> Result<T>
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
pub fn pair(flag: &str, raw: &str) -> Result<(u32, u32)> {
    let (w, h) = raw
        .split_once('x')
        .with_context(|| format!("`{flag}` wants `<w>x<h>`, not `{raw}`"))?;
    Ok((number(flag, w, 1..=16384)?, number(flag, h, 1..=16384)?))
}

/// A screen of a scene — round numbers no panel has.
pub fn screen(at: &str, x: i32, y: i32, lit: bool) -> ronoctua_display::Screen {
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
pub fn scene(name: &str) -> Result<ronoctua_display::Snapshot> {
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
        settings_repaired: false,
    })
}

/// Where and how a frame is developed — this camera's and the footer's
/// (`ronoctua_shell_footer::footer_photograph`).
pub struct Film<'a> {
    /// Print the frame instead of drawing it.
    pub list: bool,
    /// Physical pixels per logical one.
    pub by: u16,
    /// The language the text is shaped for.
    pub language: &'a str,
    /// The PNG.
    pub out: std::path::PathBuf,
}

/// Prints the frame, or draws it into the PNG asked for.
pub fn draw(
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

/// The variable that says the camera is already in its room.
pub const IN_THE_ROOM: &str = "RONOCTUA_FOOTER_CAMERA_ROOM";

/// The room's folder that stands for the machine directory.
pub const MACHINE_DIR: &str = "machine";

/// The room's folder that stands for the folder of the machine's record.
pub const MACHINE_STATE: &str = "machine-state";

/// The room this camera runs the footer in, while it does — `None`
/// everywhere else. A view that reads an ABSOLUTE place of the machine (the
/// update's records, its root half's report under `/run`) reads it under
/// the room instead, so a photograph draws what was planted there and never
/// this machine's: a room of XDG directories alone could not plant those.
pub fn room() -> Option<std::path::PathBuf> {
    std::env::var_os(IN_THE_ROOM).map(std::path::PathBuf::from)
}

/// THE SCENES' CAMERA: a door DETACHED over a scene handed in whole, walked
/// down the paths a key and a pointer take (`door::Detached`), composed by
/// `door::compose` and developed — the walk every scene's camera shares.
///
/// Each door's camera is beside its door and says only what is its own: its
/// words and the door it builds over them ([`Scene`]) — the screens'
/// window (`display_window`), the clipboard's (`clipboard_window`), the
/// resizer's curtain (`resizer`) and the lifeline (`lifeline`). Nothing is
/// read from the machine: what a door shows is what the words planted, and
/// no order leaves — what it would have done is printed.
pub mod scenes {
    use anyhow::{Context as _, Result, bail, ensure};
    use ronoctua_tokens as tokens;
    use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

    use super::{Film, draw, number};
    use crate::door::{Content, Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};

    /// The words every scene's camera takes, after its own.
    const OPTIONS: &str = "\
  --out <file>          where to write                 (default: <door>.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --width <px>          the window's width, in logical pixels
  --height <px>         the height, in logical pixels
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it";

    /// The steps, for a camera whose door takes them.
    const STEPS: &str = "\
Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, S-<up>
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there";

    /// One door's camera: its words, and the door it builds over them.
    pub trait Scene {
        /// The door, as `--door` names it — and the picture's name.
        const DOOR: &'static str;
        /// What the door is, one line of `--help`.
        const IS: &'static str;
        /// This camera's own words, as `--help` lists them.
        const WORDS: &'static str;
        /// Whether the door takes steps: the lifeline is drawn as it rises.
        const WALKS: bool = true;

        /// One of this camera's own words — `Ok(false)` when it is none of
        /// them; `value` takes the word's value.
        ///
        /// # Errors
        ///
        /// A value that is not what the word wants.
        fn word(&mut self, flag: &str, value: &mut dyn FnMut() -> Result<String>) -> Result<bool>;

        /// Builds the door over its scene on `ui`, walks it and draws it.
        ///
        /// # Errors
        ///
        /// A scene that is not one, a cell outside the door, no graphics
        /// adapter, or a picture larger than the GPU takes.
        fn shoot(self, ui: Ui, asked: &Asked) -> Result<()>;
    }

    /// One step of the walk.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Step {
        /// A key.
        Key(ronoctua_keymap::Key),
        /// The pointer, resting on a cell.
        Hover(u16, u16),
        /// A press and a release on a cell: the button, and whether twice.
        Click(u16, u16, u32, bool),
    }

    /// A length the person gave.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Measure {
        /// In cells (or lines).
        Cells(u16),
        /// In logical pixels.
        Px(u32),
    }

    /// What every scene's camera was asked, besides its own words.
    #[derive(Debug)]
    pub struct Asked {
        /// The door, as `--door` named it.
        pub door: &'static str,
        /// Where to write.
        pub out: Option<std::path::PathBuf>,
        /// The width asked, if one was.
        pub width: Option<Measure>,
        /// The height asked, if one was.
        pub height: Option<Measure>,
        /// The language.
        pub language: String,
        /// The theme.
        pub theme: tokens::theme::Theme,
        /// The text's size, in percent.
        pub text: u16,
        /// Physical pixels per logical one.
        pub scale: u16,
        /// Print the frame instead of drawing it.
        pub list: bool,
        /// The walk.
        pub steps: Vec<Step>,
    }

    impl Asked {
        /// Where and how the frame is developed.
        #[must_use]
        pub fn film(&self) -> Film<'_> {
            Film {
                list: self.list,
                by: self.scale,
                language: &self.language,
                out: self
                    .out
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(format!("{}.png", self.door))),
            }
        }
    }

    /// The camera's `--help`: its door, its words, the shared ones.
    fn usage<S: Scene>() -> String {
        let steps = if S::WALKS { " [steps]" } else { "" };
        let mut out = format!(
            "usage: ronoctua-shell photograph --door {door} [options]{steps}\n\n  {door:<10} {is}\n\n{words}\n{OPTIONS}",
            door = S::DOOR,
            is = S::IS,
            words = S::WORDS,
        );
        if S::WALKS {
            out.push_str("\n\n");
            out.push_str(STEPS);
        }
        out
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

    /// Reads the words — all of them, `--door` included: the shared ones
    /// here, the camera's own through [`Scene::word`].
    ///
    /// # Errors
    ///
    /// A word not understood, or a value that is not what its word wants.
    pub fn parse<S: Scene>(words: &[String], scene: &mut S) -> Result<Asked> {
        let mut me = Asked {
            door: S::DOOR,
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
                    println!("{}", usage::<S>());
                    std::process::exit(0);
                }
                "--door" => {
                    let said = value()?;
                    ensure!(said == S::DOOR, "`--door {said}` is not this camera's door");
                    door = true;
                }
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
                other => {
                    if !scene.word(other, &mut value)? {
                        bail!("unknown option `{other}`.\n\n{}", usage::<S>());
                    }
                }
            }
        }
        ensure!(door, "which door? `--door {}`", S::DOOR);
        ensure!(
            S::WALKS || me.steps.is_empty(),
            "the {} takes no steps: it is drawn as it rises",
            S::DOOR
        );
        Ok(me)
    }

    /// `ronoctua-shell photograph --door <S::DOOR> …`, to its end.
    pub fn taken<S: Scene>(rest: &[String], scene: S) -> ! {
        let line: Vec<String> = std::env::args().skip(2).collect();
        let words = if line.ends_with(rest) {
            Ok(line)
        } else {
            Err(anyhow::anyhow!(
                "the words handed to the scenes' camera are not the end of the command line"
            ))
        };
        match words.and_then(|w| run(&w, scene)) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Error: {e:?}");
                std::process::exit(1);
            }
        }
    }

    /// Draws what the words ask for into a PNG, or prints its frame.
    ///
    /// # Errors
    ///
    /// A word not understood, a scene that is not one, no graphics adapter,
    /// or a picture larger than the GPU takes.
    pub fn run<S: Scene>(words: &[String], mut scene: S) -> Result<()> {
        let asked = parse(words, &mut scene)?;
        tokens::theme::set(asked.theme);
        tokens::theme::set_ground(tokens::theme::Ground::Chrome);
        tokens::text::set(asked.text);
        crate::when::prefs::set(crate::when::Choice::default());
        let texts = ronoctua_i18n::Texts::fresh(&asked.language)
            .with_context(|| format!("the language `{}` does not exist", asked.language))?;
        let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
        scene.shoot(ui, &asked)
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
    ///
    /// # Errors
    ///
    /// As [`shoot`].
    pub fn walked<C: Content>(mut d: Detached<C>, asked: &Asked) -> Result<()> {
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
    ///
    /// # Errors
    ///
    /// A cell outside the door, a step after it closed, no graphics adapter,
    /// or a picture larger than the GPU takes.
    pub fn shoot<C: Content>(mut d: Detached<C>, asked: &Asked, laid: bool) -> Result<()> {
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
}

/// THE PLANTED MACHINES' CAMERA: a window that reads a daemon a photograph
/// must not ask (NetworkManager, BlueZ, timedated, the studio) built over a
/// planted reading by its own `photographed(scene)`, then walked and drawn
/// as every camera here does it.
///
/// Each window's camera is beside it (`network_window::camera`, …) and
/// hands this its door's word, its scenes and its builder; what is drawn is
/// the same on every machine because nothing of this one is read.
pub mod planted {
    use anyhow::{Context as _, Result, bail, ensure};
    use ronoctua_tokens as tokens;
    use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

    use crate::door::{Content, Detached, Ui};

    /// The usage, printed by `--help`, for `door` and its `scenes`.
    fn usage(door: &str, scenes: &str) -> String {
        format!(
            "\
usage: ronoctua-shell photograph --door {door}
         --scene <scene> [--out file.png] [--columns N] [--lines N]
         [--language en-GB|pt-BR] [--theme id] [--scale N] [--list]
         [--key notation]... [--type text]... [--hover column,line]...

scenes: {door} {scenes}"
        )
    }

    /// One thing done to the window before the picture.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Step {
        /// A key, by its notation.
        Key(ronoctua_keymap::Key),
        /// Text, typed a character at a time.
        Type(String),
        /// The pointer, resting on a cell.
        Hover(u16, u16),
    }

    /// What the words asked for.
    #[derive(Debug, Clone)]
    pub struct Asked {
        /// The door, as `--door` named it.
        pub door: String,
        /// The planted scene.
        pub scene: String,
        /// Where to write.
        pub out: std::path::PathBuf,
        /// The width, in cells.
        pub columns: u16,
        /// The height, in lines.
        pub lines: u16,
        /// The language.
        pub language: String,
        /// The theme.
        pub theme: tokens::theme::Theme,
        /// Physical pixels per logical one.
        pub scale: u16,
        /// Print the frame instead of drawing it.
        pub list: bool,
        /// The walk.
        pub steps: Vec<Step>,
    }

    /// `ronoctua-shell photograph --door <door> …`, to its end: the rest of
    /// the command line is this camera's, and `shoot_door` plants the window
    /// over the scene the words name and draws it ([`shoot`]).
    pub fn taken(
        rest: &[String],
        door: &str,
        scenes: &str,
        shoot_door: impl FnOnce(&Asked) -> Result<()>,
    ) -> ! {
        let line: Vec<String> = std::env::args().skip(2).collect();
        let words = if line.ends_with(rest) {
            line
        } else {
            rest.to_vec()
        };
        match parse(&words, &usage(door, scenes)).and_then(|asked| {
            tokens::theme::set(asked.theme);
            tokens::theme::set_ground(tokens::theme::Ground::Chrome);
            tokens::text::set(tokens::text::NORMAL);
            crate::when::prefs::set(crate::when::Choice::default());
            shoot_door(&asked)
        }) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Error: {e:?}");
                std::process::exit(1);
            }
        }
    }

    fn number(flag: &str, raw: &str, most: u16) -> Result<u16> {
        let n: u16 = raw
            .parse()
            .ok()
            .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
        ensure!(
            (1..=most).contains(&n),
            "`{flag} {n}` is out of range; the range is 1..={most}"
        );
        Ok(n)
    }

    /// The words, `--door` included.
    ///
    /// # Errors
    ///
    /// A word not understood, a value that is not what its word wants, or
    /// no `--scene`.
    pub fn parse(words: &[String], usage: &str) -> Result<Asked> {
        let mut me = Asked {
            door: String::new(),
            scene: String::new(),
            out: std::path::PathBuf::from("system.png"),
            columns: 100,
            lines: 60,
            language: "pt-BR".to_owned(),
            theme: tokens::theme::default_one(),
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
                    println!("{usage}");
                    std::process::exit(0);
                }
                "--door" => me.door = value()?,
                "--scene" => me.scene = value()?,
                "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
                "--columns" => me.columns = number("--columns", &value()?, 1000)?,
                "--lines" => me.lines = number("--lines", &value()?, 1000)?,
                "--language" => me.language = value()?,
                "--theme" => {
                    let id = value()?;
                    me.theme = tokens::theme::by_id(&id)
                        .with_context(|| format!("unknown theme `{id}`"))?;
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
                "--type" => me.steps.push(Step::Type(value()?)),
                "--hover" => {
                    let raw = value()?;
                    let (c, l) = raw
                        .split_once(',')
                        .with_context(|| format!("`--hover` wants `column,line`, not `{raw}`"))?;
                    me.steps.push(Step::Hover(
                        number("--hover", c, 999)?,
                        number("--hover", l, 999)?,
                    ));
                }
                other => bail!("unknown option `{other}`.\n\n{usage}"),
            }
        }
        ensure!(!me.scene.is_empty(), "`--scene` is missing.\n\n{usage}");
        Ok(me)
    }

    /// Where the middle of a cell is, in logical pixels.
    fn middle_of(column: u16, line: u16) -> (f64, f64) {
        let px = tokens::text::size();
        (
            (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
            (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
        )
    }

    /// One window, its steps, one frame, the PNG.
    ///
    /// # Errors
    ///
    /// No graphics adapter, or a picture larger than the GPU takes.
    pub fn shoot<C: Content>(
        asked: &Asked,
        content: C,
        line: Option<crate::line::Line>,
    ) -> Result<()> {
        let texts = ronoctua_i18n::Texts::fresh(&asked.language)
            .with_context(|| format!("the language `{}` does not exist", asked.language))?;
        let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
        let mut d = Detached::new(ui, content);
        let px = tokens::text::size();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = (
            (f32::from(asked.columns + 2 * d.content.margin()) * tokens::metrics::cell_width(px))
                .ceil() as u32,
            (f32::from(asked.lines) * tokens::metrics::line_height(px)).ceil() as u32,
        );
        d.lay_out_at(size);
        if let Some(line) = line {
            d.ui.ask(line);
        }
        d.turn();
        let mut entered = false;
        for step in &asked.steps {
            match step {
                Step::Key(key) => d.press(*key),
                Step::Type(text) => {
                    for c in text.chars() {
                        d.press(ronoctua_keymap::Key::simple(if c == ' ' {
                            ronoctua_keymap::Sym::Space
                        } else {
                            ronoctua_keymap::Sym::Char(c)
                        }));
                    }
                }
                Step::Hover(c, l) => {
                    let kind = if std::mem::replace(&mut entered, true) {
                        PointerEventKind::Motion { time: 0 }
                    } else {
                        PointerEventKind::Enter { serial: 0 }
                    };
                    d.pointer(&[PointerEvent {
                        surface: d.ui.surface.clone(),
                        position: middle_of(*c, *l),
                        kind,
                    }]);
                }
            }
            d.turn();
            if d.ui.quit {
                break;
            }
        }
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
        let composed = d.compose(&gpu.device, &gpu.queue, &mut images);
        if asked.list {
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
        gpu.resize(logical.0 * by, logical.1 * by);
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
            d.ui.texts.language()
        );
        if let Some(m) = d.ui.message.as_ref() {
            println!("the message line says: {m}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a scene of screens stops planting, or one that is not a
    /// scene plants something.
    #[test]
    fn the_screens_are_the_four_scenes() {
        for name in ["one", "two", "stacked", "dark"] {
            assert!(scene(name).is_ok(), "{name}");
        }
        assert!(scene("nowhere").is_err());
    }

    /// FAILS IF: the planted machines' camera stops taking its words, or
    /// takes a door's scene without the scene.
    #[test]
    fn the_planted_camera_reads_its_words() {
        let words: Vec<String> = [
            "--door",
            "network",
            "--scene",
            "lan",
            "--key",
            "<down>",
            "--type",
            "Lis",
            "--hover",
            "3,4",
            "--columns",
            "90",
            "--language",
            "en-GB",
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
        let asked = planted::parse(&words, "usage").expect("the words parse");
        assert_eq!(
            (asked.door.as_str(), asked.scene.as_str()),
            ("network", "lan")
        );
        assert_eq!(asked.columns, 90);
        assert_eq!(asked.steps.len(), 3);
        assert!(planted::parse(&["--door".to_owned(), "network".to_owned()], "usage").is_err());
    }
}
