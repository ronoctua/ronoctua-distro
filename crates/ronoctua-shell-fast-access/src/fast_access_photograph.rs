//! THE SEARCH'S WINDOW AND COLUMN IN A PNG — `ronoctua-shell photograph
//! --door fast-access`.
//!
//! A camera of its own, for the icons' reason (`ronoctua_shell_icons::icons_photograph`):
//! the player's camera walks a daemon's states, and this door has none. What
//! is the SAME is taken as it is: the production door
//! (`fast_access_window::FastAccess`), detached (`door::Detached`), walked
//! down the path a key and a pointer take, and a frame composed by
//! `door::compose`, the function `Door::paint` calls.
//!
//! # What it reads, and what it never does
//!
//! The feed answers on the camera's thread (`Feed::inline_at`): the
//! in-process sources as on a screen, the index in the folder `--index`
//! names (one `ronoctua-index build` made for it), and the icon tables from
//! `RONOCTUA_ICONSET`. It asks NO host: what the network would answer — the
//! YouTube grid — is a fixture (`--videos`), each video's picture a file. A
//! file's panels are answered on the spot (`inspection::look`), as the file
//! manager's camera answers them.
//!
//! Usage: `ronoctua-shell photograph --door fast-access [options] [steps]`,
//! `--help` for the list.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Detached, LEFT_BUTTON, Ui};
use crate::fast_access::{Answer, Filled, Hit, SectionId, Video, What};
use crate::fast_access_search::{Back, Feed, Sink};
use crate::fast_access_window::{FastAccess, Host};

const USAGE: &str = "\
usage: ronoctua-shell photograph --door fast-access [options] [steps]

  --column <size>       in the column, at large, medium or small (default: a window)
  --index <dir>         the index to read (built with `ronoctua-index build`)
  --videos <file>       YouTube's answer, a JSON list of
                        {id, title, channel, duration_s, views, short, picture}
  --settings <file>     the search's settings (a fast-access.json)
  --apps                the launcher's applications of this machine
  --out <file>          where to write                 (default: fast-access.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --width <px>          the window's width, in logical pixels
  --height <px>         the height, in logical pixels
                        (one of each pair; default: the door's own size)
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --page <id>           chrome, backdrop, near or absolute (default: chrome)
  --text <percent>      text size, in percent          (default: 100)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-w, TAB
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

/// One video of the fixture.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureVideo {
    id: String,
    title: String,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    duration_s: Option<f64>,
    #[serde(default)]
    views: Option<u64>,
    #[serde(default)]
    short: bool,
    #[serde(default)]
    picture: Option<PathBuf>,
}

/// What was asked for.
#[derive(Debug)]
struct Asked {
    column: Option<crate::column::Size>,
    index: Option<PathBuf>,
    videos: Option<Vec<FixtureVideo>>,
    settings: crate::fast_access::Settings,
    apps: bool,
    out: PathBuf,
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

/// Reads the words — all of them, `--door fast-access` included.
#[allow(clippy::too_many_lines)]
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        column: None,
        index: None,
        videos: None,
        settings: crate::fast_access::Settings::default(),
        apps: false,
        out: PathBuf::from("fast-access.png"),
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
                ensure!(
                    said == "fast-access",
                    "`--door {said}` is not this camera's door"
                );
            }
            "--column" => {
                let said = value()?;
                let size = crate::column::Size::named(&said)
                    .filter(|s| *s != crate::column::Size::Bar)
                    .with_context(|| {
                        format!("`{said}` is not a size the search offers; the sizes are large, medium and small")
                    })?;
                me.column = Some(size);
            }
            "--index" => me.index = Some(PathBuf::from(value()?)),
            "--videos" => {
                let file = value()?;
                let text = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.videos = Some(
                    serde_json::from_str(&text)
                        .with_context(|| format!("`{file}` is not a list of videos"))?,
                );
            }
            "--settings" => {
                let file = value()?;
                let text = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.settings = serde_json::from_str(&text)
                    .with_context(|| format!("`{file}` is not the search's settings"))?;
            }
            "--apps" => me.apps = true,
            "--out" | "-o" => me.out = PathBuf::from(value()?),
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

/// `ronoctua-shell photograph --door fast-access …`, to its end: the process
/// leaves with the answer.
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

/// The fixture's videos as the YouTube section's hits.
fn video_hits(videos: &[FixtureVideo]) -> Vec<Hit> {
    videos
        .iter()
        .map(|v| Hit {
            id: format!("yt:{}", v.id),
            title: v.title.clone(),
            note: v.channel.clone(),
            what: What::Video(Box::new(Video {
                id: v.id.clone(),
                url: format!("https://www.youtube.com/watch?v={}", v.id),
                channel: v.channel.clone(),
                duration_s: v.duration_s,
                views: v.views,
                short: v.short,
                local: false,
                thumb_file: v.picture.clone(),
                thumb: None,
            })),
        })
        .collect()
}

/// Draws the door the words ask for into a PNG, or prints its frame.
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
    crate::when::prefs::set(crate::when::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    // THE ANSWERS QUEUE, and the camera hands them in after each step: an
    // inline feed answers inside the call that asked, while the view that
    // asked is still borrowed.
    let queue: Arc<Mutex<Vec<Back>>> = Arc::default();
    let sink: Sink = {
        let queue = Arc::clone(&queue);
        Arc::new(move |b| {
            queue.lock().unwrap_or_else(PoisonError::into_inner).push(b);
        })
    };
    let feed = Feed::inline_at(Arc::clone(&sink), asked.index.clone());
    if asked.apps {
        feed.apps(ui.texts.language());
    }
    let view = crate::fast_access::View::new(asked.settings.clone(), &ui.texts);
    let search = crate::fast_access_window::Search::new(
        view,
        Some(feed),
        crate::fast_access_window::ground()?,
        crate::inspection::Threads::none(),
    );
    match asked.column {
        None => {
            let me = FastAccess::over(search, false, false);
            shoot(Detached::new(ui, me), &asked, &queue)
        }
        Some(size) => {
            let me = FastAccess::over(search, false, true);
            let column = crate::column::Column::detached(me, size, &ui, None)?;
            shoot(Detached::new(ui, column), &asked, &queue)
        }
    }
}

/// The size the door is laid out at.
fn size_of<C: Host>(d: &Detached<C>, asked: &Asked) -> (u32, u32) {
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

/// Everything the threads would have answered by now, handed in: the
/// queue's answers, the fixture's videos for a YouTube search, the videos'
/// pictures, and the file panels'. Rounds until nothing is left — a bounded
/// number: an answer that asks again is asked once more, not for ever.
fn settle<C: Host>(d: &mut Detached<C>, asked: &Asked, queue: &Arc<Mutex<Vec<Back>>>) {
    /// The rounds a settle takes at most.
    const ROUNDS: usize = 8;
    for _ in 0..ROUNDS {
        let backs = std::mem::take(&mut *queue.lock().unwrap_or_else(PoisonError::into_inner));
        let mut moved = !backs.is_empty();
        for b in backs {
            C::tenant(&mut d.content).took(&mut d.ui, b);
        }
        let me = C::tenant(&mut d.content);
        if let Some(videos) = asked.videos.as_ref()
            && me.search.view.query().scope == crate::fast_access::Scope::YouTube
            && !me.search.view.hits().any(|h| {
                videos
                    .first()
                    .is_some_and(|v| h.id == format!("yt:{}", v.id))
            })
        {
            let answer = Answer {
                generation: me.search.view.generation(),
                section: SectionId::YouTube,
                filled: Filled::Hits(video_hits(videos)),
            };
            moved |= me.search.view.arrived(answer);
        }
        let texts = std::rc::Rc::clone(&d.ui.texts);
        me.search.look_again(&texts);
        moved |= me.search.inspect_now(&texts);
        if !moved
            && queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_empty()
        {
            return;
        }
        d.ui.ask_paint();
    }
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
        "the cell {},{} is outside the door, which is {}x{} px",
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

/// The door, walked and drawn.
#[allow(clippy::too_many_lines)]
fn shoot<C: Host>(mut d: Detached<C>, asked: &Asked, queue: &Arc<Mutex<Vec<Back>>>) -> Result<()> {
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

    let size = size_of(&d, asked);
    d.lay_out_at(size);
    d.turn();
    settle(&mut d, asked, queue);
    // EVERY STEP'S FRAME IS COMPOSED, as a door paints after each: a picture
    // goes up to the GPU on the frame that first draws it.
    let _ = d.compose(&gpu.device, &gpu.queue, &mut images);
    let mut entered = false;
    for step in &asked.steps {
        ensure!(
            !d.ui.quit,
            "the door closed at the step before `{step:?}`, and a step after it reaches nothing"
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
        settle(&mut d, asked, queue);
        if d.ui.quit {
            break;
        }
        let _ = d.compose(&gpu.device, &gpu.queue, &mut images);
    }
    // AND ONCE MORE after the last answers: a picture decoded during the
    // last frame is drawn by the next.
    settle(&mut d, asked, queue);
    let composed = d.compose(&gpu.device, &gpu.queue, &mut images);

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
         for a smaller door or a smaller --scale",
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
