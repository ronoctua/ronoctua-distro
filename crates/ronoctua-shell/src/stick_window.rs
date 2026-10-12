//! The installer stick's SECOND door — a window of its own (`LEADER s z w`,
//! and the file manager's `W` on a drive's heading, which opens it on that
//! drive).
//!
//! Everything it knows lives in [`crate::stick::View`], which the footer
//! draws from too; what this holds is a window, the [`crate::stick::Feed`]
//! whose threads wake it, and the carrying out of the view's errands —
//! consent, the queue, and the root gesture, asked in the footer's sudo bar
//! like every other ([`crate::sudo_bar`]), its verdict said on this window's
//! own line.
//!
//! # The camera
//!
//! `ronoctua-shell photograph --door stick --scene <file.json>` draws this
//! window over a [`Scene`] — the readings handed in whole, no thread, no
//! udisks, no network — down the path a key and a pointer take.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Content, Ui};
use crate::stick::{Errand, Feed, Press, Readings, View};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.stick";

/// The width the cursor is first settled at, before the compositor says:
/// nothing is drawn at it, and a key settles again at the door's own width.
const OPENING_COLUMNS: u16 = 80;

/// The arguments that open the window on a drive (`--drive <id>`).
#[must_use]
pub fn window_on(drive: &str) -> Vec<String> {
    vec!["stick".to_owned(), "--drive".to_owned(), drive.to_owned()]
}

/// Opens the view in a window, on `drive` when one is named.
///
/// # Errors
///
/// When there is no Wayland compositor, or it offers no `xdg_shell`.
pub fn host_it(drive: Option<String>) -> Result<()> {
    crate::door::run(|wiring| {
        let readings = Readings::of_the_machine();
        let feed = Feed::start(wiring.wake(), readings.cache.as_deref());
        if readings.consent {
            feed.ask_latest();
        }
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let mut view = View::on(drive);
        view.settle(&readings, &zone, OPENING_COLUMNS, &wiring.texts);
        Ok(StickWindow {
            view,
            readings,
            feed: Some(feed),
            zone,
        })
    })
}

/// The window.
pub struct StickWindow {
    view: View,
    readings: Readings,
    /// `None` in the camera: nothing is read, nothing is asked.
    feed: Option<Feed>,
    zone: ronoctua_files::when::Zone,
}

impl StickWindow {
    /// Over readings handed in whole — the camera's.
    #[must_use]
    pub fn over(readings: Readings, drive: Option<String>) -> Self {
        Self {
            view: View::on(drive),
            readings,
            feed: None,
            zone: ronoctua_files::when::Zone::utc(),
        }
    }

    /// Carries out what the view asked for — on this window's own line.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        use ronoctua_i18n::key;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Speak(k) => ui.say(ui.texts.t(k)),
            Errand::Allow => match crate::consent_rows::allow(&ronoctua_stick::HOSTS, &ui.texts) {
                Ok(()) => {
                    self.readings.consent = true;
                    self.reread(ui);
                }
                Err(said) => ui.say(said),
            },
            Errand::Reread => self.reread(ui),
            Errand::Fetch(release) => {
                let said = crate::stick::fetch_said(&release, &ui.texts);
                ui.say(said);
            }
            Errand::Arm(armed) => {
                if let Some(said) = crate::stick::cannot_arm(&ui.texts) {
                    ui.say(said);
                } else if let Some(feed) = self.feed.as_ref() {
                    // ROOT, ASKED IN THE FOOTER'S SUDO BAR: this window's
                    // gesture goes down the footer's socket, and the verdict
                    // comes back to be said here.
                    let gesture =
                        crate::sudo_bar::Gate::window().gesture(crate::sudo_bar::Asker::Stick);
                    feed.arm(*armed, gesture);
                    ui.say(ui.texts.t(key::STICK_STARTING));
                }
            }
            Errand::Door(why) => ui.say(ui.texts.t(crate::security_words::refusal(why))),
        }
        ui.ask_paint();
    }

    /// GitHub asked again (when it is let), the cache read again — and the
    /// drives from a new watch when udisks did not answer — the cursor kept
    /// on what it stood on.
    fn reread(&mut self, ui: &Ui) {
        let feed = self.feed.as_mut();
        self.view
            .kept(&mut self.readings, &self.zone, ui.columns, &ui.texts, |r| {
                crate::stick::reread(r, feed);
            });
    }

    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let errand = self
            .view
            .press(press, &self.readings, &self.zone, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }
}

impl Content for StickWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::STICK_TITLE))
    }

    /// A key, by its whole notation, through the reader the footer calls too.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::stick::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&self.readings, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the footer's mark for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view
            .rows(&self.readings, &self.zone, ui.columns, &ui.texts)
            .into_iter()
            .map(|r| r.fields)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS (invariant 19).
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self
            .view
            .rows(&self.readings, &self.zone, ui.columns, &ui.texts);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        } else {
            ui.ask_paint();
        }
    }

    /// A thread answered: the drives, GitHub, the writer, the cache, the door.
    fn woken(&mut self, ui: &mut Ui) {
        let came = self.feed.as_ref().map(Feed::take).unwrap_or_default();
        for c in came {
            let errand = self
                .view
                .heard(&mut self.readings, c, &self.zone, ui.columns, &ui.texts);
            self.apply(ui, errand);
        }
        ui.ask_paint();
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        ui.ask_paint();
    }
}

// --- the camera ---------------------------------------------------------

const USAGE: &str = "\
usage: ronoctua-shell photograph --door stick [--scene <file.json>] [options] [steps]

  --scene <file>        the readings, whole (crate::stick_window::Scene as
                        JSON): consent, latest, cached, drives, progress
  --drive <id>          the drive the window is opened on (the sidebar's W)
  --out <file>          where to write                 (default: stick.png)
  --columns <n>         the window's width, in cells    (default: 90)
  --lines <n>           the height, in lines           (default: 30)
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-r
  --hover <col,line>    rests the pointer on a cell, counted from 0 (line 0
                        is the header and its keys)
  --click <col,line>    a press and a release there";

/// The variable a photograph of the FOOTER names its stick scene with —
/// read only by a footer that is a photograph's (`Screen::photographing`).
pub const SCENE_VARIABLE: &str = "RONOCTUA_PHOTOGRAPH_STICK_SCENE";

/// A photograph's footer's readings: the [`Scene`] the file
/// [`SCENE_VARIABLE`] names, as this window's camera takes it (`--scene`);
/// `None` when it names none. A file that does not read is a refusal ON THE
/// PICTURE (udisks down, the release failed), the reason in the log — never
/// an empty scene that would pass for one.
#[must_use]
pub fn readings_of_the_footer_camera() -> Option<Readings> {
    let file = std::path::PathBuf::from(std::env::var_os(SCENE_VARIABLE)?);
    let read = std::fs::read_to_string(&file)
        .map_err(anyhow::Error::from)
        .and_then(|raw| serde_json::from_str::<Scene>(&raw).map_err(anyhow::Error::from))
        .and_then(|scene| scene.readings());
    match read {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::error!(file = %file.display(), error = %format!("{e:#}"), "the footer camera's stick scene");
            Scene {
                consent: true,
                refused: Some(ronoctua_stick::Why::Failed.id().to_owned()),
                disks_down: true,
                ..Scene::default()
            }
            .readings()
            .ok()
        }
    }
}

/// The readings a picture is taken over — a JSON file.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Scene {
    /// The three hosts are let.
    #[serde(default)]
    pub consent: bool,
    /// The release GitHub answered.
    #[serde(default)]
    pub release: Option<ronoctua_stick::Release>,
    /// Or the refusal it answered (`offline`, `no-release`…).
    #[serde(default)]
    pub refused: Option<String>,
    /// The image is in the cache, verified.
    #[serde(default)]
    pub cached: bool,
    /// The drives, judged; absent is "still reading".
    #[serde(default)]
    pub drives: Option<Vec<SceneDrive>>,
    /// udisks did not answer.
    #[serde(default)]
    pub disks_down: bool,
    /// What the writer last said.
    #[serde(default)]
    pub progress: Option<ronoctua_stick::Progress>,
}

/// A drive of a scene.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SceneDrive {
    /// The disk border's id.
    pub drive: String,
    /// What it is called.
    pub label: String,
    /// Its size.
    pub size_bytes: u64,
    /// Why it may not be written (`mounted`, `system`…), when it may not.
    #[serde(default)]
    pub unfit: Option<String>,
}

impl Scene {
    /// The readings it describes.
    ///
    /// # Errors
    ///
    /// A refusal or an unfit reason that is not one.
    pub fn readings(&self) -> Result<Readings> {
        let latest = match (&self.release, &self.refused) {
            (Some(r), _) => crate::stick::Latest::Got(Box::new(r.clone())),
            (None, Some(id)) => crate::stick::Latest::Refused(
                ronoctua_stick::Why::of_id(id)
                    .with_context(|| format!("`{id}` is not a refusal"))?,
            ),
            (None, None) if self.consent => crate::stick::Latest::Asking,
            (None, None) => crate::stick::Latest::Unasked,
        };
        let drives = match &self.drives {
            None => None,
            Some(list) => Some(
                list.iter()
                    .map(|d| {
                        let unfit = match d.unfit.as_deref() {
                            None => None,
                            Some(id) => Some(
                                ronoctua_stick::Unfit::ALL
                                    .into_iter()
                                    .find(|u| u.id() == id)
                                    .with_context(|| format!("`{id}` is not an unfit reason"))?,
                            ),
                        };
                        Ok(ronoctua_stick::Judged {
                            target: ronoctua_stick::Target {
                                drive: d.drive.clone(),
                                by_id: format!("/dev/disk/by-id/usb-{}", d.drive.replace('/', "_"))
                                    .into(),
                                serial: None,
                                size_bytes: d.size_bytes,
                                label: d.label.clone(),
                            },
                            unfit,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
        };
        Ok(Readings {
            consent: self.consent,
            latest,
            cached: self.cached.then(|| "0".repeat(64)),
            drives,
            disks_down: self.disks_down,
            progress: self.progress,
            cache: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(ronoctua_keymap::Key),
    Hover(u16, u16),
    Click(u16, u16),
}

#[derive(Debug)]
struct Asked {
    scene: Option<std::path::PathBuf>,
    drive: Option<String>,
    out: std::path::PathBuf,
    columns: u16,
    lines: u16,
    language: String,
    theme: tokens::theme::Theme,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

fn number(flag: &str, raw: &str, range: std::ops::RangeInclusive<u16>) -> Result<u16> {
    let n: u16 = raw
        .trim()
        .parse()
        .ok()
        .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
    ensure!(
        range.contains(&n),
        "`{flag} {n}` is out of range {}..={}",
        range.start(),
        range.end()
    );
    Ok(n)
}

/// A cell, counted from 0 as the files camera counts it: line 0 is the
/// header, where the hint's keys are hovered and clicked.
fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16)> {
    let (c, l) = raw
        .split_once(',')
        .with_context(|| format!("`{flag}` wants `column,line`, not `{raw}`"))?;
    Ok((number(flag, c, 0..=999)?, number(flag, l, 0..=999)?))
}

fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        scene: None,
        drive: None,
        out: std::path::PathBuf::from("stick.png"),
        columns: 90,
        lines: 30,
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
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--door" => {
                let said = value()?;
                ensure!(said == "stick", "`--door {said}` is not this camera's door");
            }
            "--scene" => me.scene = Some(value()?.into()),
            "--drive" => me.drive = Some(value()?),
            "--out" | "-o" => me.out = value()?.into(),
            "--columns" => me.columns = number("--columns", &value()?, 1..=1000)?,
            "--lines" => me.lines = number("--lines", &value()?, 1..=1000)?,
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme =
                    tokens::theme::by_id(&id).with_context(|| format!("unknown theme `{id}`"))?;
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
    Ok(me)
}

/// `ronoctua-shell photograph --door stick …`, to its end.
pub fn photographed(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let words = if line.ends_with(rest) {
        line
    } else {
        rest.to_vec()
    };
    match parse(&words).and_then(|asked| shoot(&asked)) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    (
        (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
        (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
    )
}

fn shoot(asked: &Asked) -> Result<()> {
    use crate::door::Detached;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(tokens::theme::Ground::Chrome);
    tokens::text::set(tokens::text::NORMAL);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let scene: Scene = match &asked.scene {
        Some(file) => {
            let raw = std::fs::read_to_string(file)
                .with_context(|| format!("could not read `{}`", file.display()))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("`{}` is not a stick scene", file.display()))?
        }
        None => Scene::default(),
    };
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let mut d = Detached::new(
        ui,
        StickWindow::over(scene.readings()?, asked.drive.clone()),
    );
    let px = tokens::text::size();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size = (
        (f32::from(asked.columns + 2 * d.content.margin()) * tokens::metrics::cell_width(px)).ceil()
            as u32,
        (f32::from(asked.lines) * tokens::metrics::line_height(px)).ceil() as u32,
    );
    d.lay_out_at(size);
    d.turn();
    // THE DRIVE ASKED FOR, under the cursor — as the first reading of the
    // drives puts it in the running window.
    {
        let c = &mut d.content;
        c.view
            .settle(&c.readings, &c.zone, d.ui.columns, &d.ui.texts);
    }
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let format = ronoctua_render::offscreen::FORMAT;
    let mut images = ronoctua_render::image::Images::new(&gpu.device, format)?;
    // A FRAME BEFORE EVERY STEP: a key of the header is grabbed where the
    // last paint drew it, so a pointer that moves before anything was
    // composed finds no key under it — and the photograph would lie.
    let mut composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    let mut entered = false;
    for step in &asked.steps {
        match *step {
            Step::Key(key) => d.press(key),
            Step::Hover(c, l) | Step::Click(c, l) => {
                let event = |kind| {
                    vec![PointerEvent {
                        surface: d.ui.surface.clone(),
                        position: middle_of(c, l),
                        kind,
                    }]
                };
                let first = if std::mem::replace(&mut entered, true) {
                    PointerEventKind::Motion { time: 0 }
                } else {
                    PointerEventKind::Enter { serial: 0 }
                };
                let mut frames = vec![event(first)];
                if matches!(step, Step::Click(..)) {
                    for kind in [
                        PointerEventKind::Press {
                            time: 0,
                            button: crate::door::LEFT_BUTTON,
                            serial: 0,
                        },
                        PointerEventKind::Release {
                            time: 0,
                            button: crate::door::LEFT_BUTTON,
                            serial: 0,
                        },
                    ] {
                        frames.push(event(kind));
                    }
                }
                for events in frames {
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
    let film = crate::scene_photograph::Film {
        list: asked.list,
        by: asked.scale,
        language: &asked.language,
        out: asked.out.clone(),
    };
    crate::scene_photograph::draw(
        &mut gpu,
        &mut images,
        &composed,
        (d.ui.width, d.ui.height),
        &film,
        i32::from(asked.scale),
    )?;
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    Ok(())
}
