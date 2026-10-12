//! The security centre in a window of its own — the second door onto
//! `crate::security` (`LEADER s e w`).
//!
//! A window and nothing else: which rows exist and what a key does is
//! `security.rs`, and both doors ask the same view the same questions. What
//! this holds is a window, a channel from its threads (the scene, the sudo
//! door, the online checks) and the watches that say when to read the scene
//! again; what it says — a refusal, a password that was wrong — it says on
//! ITS line, because the door where the key was pressed is where it is read.
//!
//! Root is never asked here: every gesture that needs it goes down the
//! footer's socket to the SUDO BAR (`crate::sudo_bar`, `Gate::window`), which
//! asks the password, a key or a finger above the modeline, and the verdict
//! comes back to be said on this window's line. The passwordless window's
//! live count is a clock armed only while it is drawn.
//!
//! It is also photographed here (`ronoctua-shell photograph --door security`,
//! [`photographed`]), over a scene handed in whole — never this machine's.

use crate::door::{Content, Door, Ui};
use crate::security::{Came, Errand, Job, Scene, Section, View};
use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::{self, channel},
    seat::keyboard::{KeyEvent, Modifiers},
    seat::pointer::{PointerEvent, PointerEventKind},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.security";

/// Opens the security centre in a window, on `section` when one is named —
/// and checking `check` with VirusTotal at once when a file is handed in (the
/// file manager's "Check with VirusTotal", from either of its doors).
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(section: Option<Section>, check: Option<std::path::PathBuf>) -> Result<()> {
    crate::door::run(|w| {
        let (sender, came) = channel::channel::<Came>();
        w.handle
            .insert_source(came, |event, (), door: &mut Door<SecurityWindow>| {
                if let channel::Event::Msg(came) = event {
                    door.content.came(&mut door.ui, came);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the security channel: {e}"))?;
        // THE FOLDERS, WATCHED: a window opened in the footer's door, a
        // window the timer closed, a check that finished in the queue — each
        // is a file that moved, a stir on the channel, and what moves in the
        // scene is read again.
        let stir = sender.clone();
        let watches = crate::security::watches(ronoctua_xdg::state_dir(), move || {
            stir.send(Came::Stirred).is_ok()
        });
        // AND THE WATCH'S PAGE: a connection refused, sudo used — the brain's
        // stream says when its page moved, and only then is it asked again.
        hear_the_watch(sender.clone());
        // AND THE QUEUE: an open this page sent that the queue refused for
        // its passphrase is asked again HERE, the door that asked first.
        hear_the_queue(sender.clone());
        let mut me = SecurityWindow {
            view: View::at(section),
            scene: Scene::default(),
            send: Some(sender),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            _watches: watches,
            handle: Some(w.handle.clone()),
            count: None,
        };
        me.start(Job::Read);
        if let Some(path) = check {
            me.view.check_started();
            me.start(Job::Hash(path));
        }
        Ok(me)
    })
}

/// Follows the brain's stream on a thread of its own and stirs the window
/// when the watch's page moved. The first mind is the baseline; the thread
/// ends with the window (the channel closes).
fn hear_the_watch(send: channel::Sender<Came>) {
    let spawned = std::thread::Builder::new()
        .name("security-heard".into())
        .spawn(move || {
            let mut last: Option<Option<u64>> = None;
            let _ = ronoctua_brain::client::watch(|mind| {
                let now = crate::security::watch_generation(&mind);
                let moved = last.is_some_and(|l| l != now);
                last = Some(now);
                // `true` STOPS the stream: when the window is gone.
                moved && send.send(Came::Stirred).is_err()
            });
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "could not follow the brain for the security centre");
    }
}

/// Follows the transfer queue on a thread of its own and hands the window
/// every seal the queue refused — what ENDED, by the function the footer and
/// the file manager's window use (`transfers::what_ended`); the view answers
/// only the ones it sent. The queue pushes its list: no clock. The thread
/// ends with the window (the channel closes).
fn hear_the_queue(send: channel::Sender<Came>) {
    if ronoctua_transfers::wire::socket_path().is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("security-queue".into())
        .spawn(move || {
            // NONE until the first list: that one is the whole history.
            let mut before: Option<Vec<ronoctua_transfers::Job>> = None;
            let _ = ronoctua_transfers::client::watch(|jobs| {
                for report in crate::transfers::what_ended(before.as_deref(), &jobs) {
                    if let crate::files::Report::Seal(r) = report
                        && send.send(Came::Sealed(*r)).is_err()
                    {
                        return true;
                    }
                }
                before = Some(jobs);
                false
            });
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "could not follow the queue for the security centre");
    }
}

/// The words after `ronoctua-shell security`: a section's word, and
/// `--check <file>`, in any order. Pure.
///
/// # Errors
///
/// A word that is neither, or `--check` with no file.
pub fn words_of(words: &[String]) -> Result<(Option<Section>, Option<std::path::PathBuf>)> {
    let mut section = None;
    let mut check = None;
    let mut it = words.iter();
    while let Some(w) = it.next() {
        if w == "--check" {
            let file = it.next().context("`--check` with no file")?;
            check = Some(std::path::PathBuf::from(file));
        } else {
            section = Some(Section::of_word(w).with_context(|| format!("`{w}` is not a section"))?);
        }
    }
    Ok((section, check))
}

/// The window: the view, the scene it draws, and the way back from its
/// threads.
pub struct SecurityWindow {
    view: View,
    scene: Scene,
    /// `None` in a photograph, which starts no thread.
    send: Option<channel::Sender<Came>>,
    zone: ronoctua_files::when::Zone,
    _watches: Vec<ronoctua_files::notice::Notice>,
    /// The loop, for the count's clock — `None` in a photograph.
    handle: Option<calloop::LoopHandle<'static, Door<SecurityWindow>>>,
    /// The count's clock, while a window with an end is open.
    count: Option<calloop::RegistrationToken>,
}

impl SecurityWindow {
    /// The window over a scene handed in whole — what the photograph draws;
    /// the section is brought into view at its first settle.
    #[must_use]
    pub fn over(scene: Scene, section: Option<Section>) -> Self {
        Self {
            view: View::at(section),
            scene,
            send: None,
            zone: ronoctua_files::when::Zone::utc(),
            _watches: Vec::new(),
            handle: None,
            count: None,
        }
    }

    /// The passwordless window's live count: a clock that beats once a
    /// second while a window with an end is open — armed here, dropping
    /// itself when none is — painting only while the administrator's rows
    /// are on screen. The footer's twin (`Footer::security_keep_the_count`).
    fn keep_the_count(&mut self) {
        let Some(handle) = self.handle.clone() else {
            return;
        };
        if !View::counts_down(&self.scene) {
            if let Some(token) = self.count.take() {
                handle.remove(token);
            }
            return;
        }
        if self.count.is_some() {
            return;
        }
        let beat = std::time::Duration::from_secs(1);
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(beat),
            move |_, (), door: &mut Door<Self>| {
                if !View::counts_down(&door.content.scene) {
                    door.content.count = None;
                    door.ui.ask_paint();
                    return calloop::timer::TimeoutAction::Drop;
                }
                let rows = door.content.view_rows(&door.ui);
                let shown = door.ui.offset..door.ui.offset + door.ui.room_for_rows();
                if View::count_on_screen(&rows, shown) {
                    door.ui.ask_paint();
                }
                calloop::timer::TimeoutAction::ToDuration(beat)
            },
        );
        match armed {
            Ok(token) => self.count = Some(token),
            Err(e) => tracing::warn!(error = %e, "the passwordless window's count will not move"),
        }
    }

    /// Starts a job on a thread, its answer coming back down the channel.
    fn start(&self, job: Job) {
        if let Some(send) = self.send.clone() {
            crate::security::start(job, move |came| {
                let _ = send.send(came);
            });
        }
    }

    /// Reads again what moves in the scene, off the loop.
    fn refresh(&self) {
        self.start(Job::Refresh(Box::new(self.scene.clone())));
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::security::Row> {
        self.view
            .rows(&self.scene, ui.columns, &ui.texts, &self.zone)
    }

    /// What came back from a thread.
    fn came(&mut self, ui: &mut Ui, came: Came) {
        let out = self.view.came(came, &mut self.scene, &ui.texts);
        if let Some(said) = out.say {
            ui.say_for(said, crate::echo::Echo::Long);
        }
        if out.read {
            self.refresh();
        }
        self.errand(ui, out.errand);
        self.settle(ui);
        self.keep_the_count();
        ui.ask_paint();
    }

    /// The cursor on a row that acts — and, when a place was asked for (the
    /// leader's word, a check's answer the person waited for, a question),
    /// that place's first row FIRST in the list, the cursor kept on screen.
    /// A scene read again never moves the page.
    fn settle(&mut self, ui: &mut Ui) {
        let rows = self.view_rows(ui);
        if let Some(top) = self.view.settle_on(&rows, &self.scene) {
            ui.offset = top;
            ui.settle_offset(rows.len());
            let shown = ui.offset..ui.offset + ui.room_for_rows();
            if !shown.contains(&self.view.cursor()) {
                ui.follow(self.view.cursor(), rows.len());
            }
        }
    }

    /// Carries out what the view asked for.
    fn errand(&mut self, ui: &mut Ui, errand: Errand) {
        match errand {
            Errand::Nothing => {}
            Errand::Close => ui.quit = true,
            Errand::Ask {
                id,
                secret,
                initial,
            } => ui.ask(crate::line::Line::new(id, secret, initial.as_deref())),
            // ROOT, ASKED IN THE FOOTER'S SUDO BAR: this window's gesture goes
            // down the footer's socket, the verdict comes back here.
            Errand::Door(work) => {
                self.view.sent(&work);
                let gesture =
                    crate::sudo_bar::Gate::window().gesture(crate::sudo_bar::Asker::Security);
                self.start(Job::Door(work, gesture));
            }
            Errand::Register => self.start(Job::Register),
            Errand::Shields => self.start(Job::Shields),
            Errand::Hash(path) => {
                ui.say_for(
                    crate::security::checking(&ui.texts),
                    crate::echo::Echo::Brief,
                );
                self.view.check_started();
                self.start(Job::Hash(path));
            }
            Errand::Upload { path, sha256 } => {
                let said = crate::security::queue_upload(path, sha256, &ui.texts);
                ui.say_for(said, crate::echo::Echo::Long);
            }
            Errand::Email(address) => {
                ui.say_for(
                    crate::security::checking(&ui.texts),
                    crate::echo::Echo::Brief,
                );
                self.view.check_started();
                self.start(Job::Email(address));
            }
            Errand::Password(secret) => {
                ui.say_for(
                    crate::security::checking(&ui.texts),
                    crate::echo::Echo::Brief,
                );
                self.view.check_started();
                self.start(Job::Password(secret));
            }
            Errand::KeepKey(secret) => {
                let said = crate::security::keep_key(&secret, &ui.texts);
                ui.say_for(said, crate::echo::Echo::Long);
                self.refresh();
            }
            Errand::ForgetKey => {
                let said = crate::security::forget_key(&ui.texts);
                ui.say_for(said, crate::echo::Echo::Long);
                self.refresh();
            }
            // ANOTHER VIEW: the footer opens it, and this window stays.
            Errand::Command(id) => ui.tell_the_footer(id, None),
            Errand::Show(section) => self.view.open_on(section),
            Errand::Url(url) => match option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) {
                Some(opener) => ui.launch(opener, &[url], None),
                None => {
                    tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
                }
            },
            Errand::Say(said) => ui.say_for(said, crate::echo::Echo::Long),
            // A SEAL, to the queue on a thread — what it answered comes
            // back (`Came::SealHanded`) and is said here.
            Errand::Seal(doing) => self.start(Job::Seal(doing)),
            Errand::Sniff(path) => self.start(Job::Sniff(path)),
            // THE CLIPBOARD, off the loop, for the file picker over the page.
            Errand::Paste => ui.paste(),
        }
    }

    /// What one key means — from the keyboard, or `RET` from a second click.
    fn pressed(&mut self, ui: &mut Ui, press: crate::security::Press) {
        ui.message = None;
        let errand = self
            .view
            .press(press, &self.scene, ui.columns, &ui.texts, &self.zone);
        self.errand(ui, errand);
        self.settle(ui);
        ui.ask_paint();
    }
}

impl Content for SecurityWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::SECURITY_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`security::View::press_of`: the file picker's while it is up).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(|key| self.view.press_of(key))
        {
            self.pressed(ui, press);
        }
    }

    /// THE CLIPBOARD, for the file picker over the page — the footer's
    /// `PasteFor::Picker`, on the same view.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let errand = self.view.picker_pasted(text, &ui.texts);
        self.errand(ui, errand);
        self.settle(ui);
        ui.ask_paint();
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let errand = self.view.answered(id, text, &self.scene, &ui.texts);
        self.errand(ui, errand);
        ui.ask_paint();
    }

    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        self.view.unanswered(id);
        ui.ask_paint();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint_now(&self.scene, &ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::security::Press::Enter);
        } else {
            ui.ask_paint();
        }
    }
}

// ─── the photograph ─────────────────────────────────────────────────────────

const USAGE: &str = "\
usage: ronoctua-shell photograph --door security [options] [steps]

  --state <file.json>   the scene the page draws (`security::Scene`, JSON)
                                                       (default: an empty one)
  --section <word>      now, firewall, admin, finger, keys, private,
                        encrypt, device, quantum, privacy, online or
                        suggest: the cursor starts there
  --out <file>          where to write                 (default: security.png)
  --columns <n>         the window's width, in cells   (default: 100)
  --lines <n>           the height, in lines           (default: 60)
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: <down>, RET, C-r
  --hover <col,line>    rests the pointer on a cell
  --click <col,line>    a press and a release there
  --never-seen <file>   VirusTotal answered that it never saw this file: the
                        upload's question, as a check's answer brings it
  --breached <address>=<names>
                        XposedOrNot's answer about an address: the breaches,
                        comma-separated (none after `=` is a clean address)
  --pwned <count>       Pwned Passwords' answer: how often the password was
                        seen in breaches
  --pick <what>         the file picker over the page: files, folder or open
                        (it opens where it was left, else in $HOME)
  --seal <path>         <path> chosen to encrypt: \"for whom\" over the page
  --sniffed <path>=<kind>
                        a .age's header read: pq, classical, passphrase, mixed
                        or not-age — the question it asks, on the line
  --registering         a security key's registration waiting for its touch
  --refused <why>       the sudo work a `RET` sent ended refused, as the door's
                        thread brings it back: a refusal's id (wrong-password,
                        cancelled, no-key, no-footer…) — said on the window's
                        line";

/// What was asked for.
#[derive(Debug)]
struct Asked {
    state: Option<std::path::PathBuf>,
    section: Option<Section>,
    out: std::path::PathBuf,
    columns: u16,
    lines: u16,
    language: String,
    theme: tokens::theme::Theme,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

/// One step of the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(ronoctua_keymap::Key),
    Hover(u16, u16),
    Click(u16, u16),
    NeverSeen(std::path::PathBuf),
    Breached(String, Vec<String>),
    Pwned(u64),
    Pick(crate::seal::Pick),
    Seal(std::path::PathBuf),
    Sniffed(std::path::PathBuf, ronoctua_pq::Kind),
    Registering,
    Refused(ronoctua_security::Why),
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
    let (c, l) = raw
        .split_once(',')
        .with_context(|| format!("`{flag}` wants `column,line`, not `{raw}`"))?;
    Ok((number(flag, c, 999)?, number(flag, l, 999)?))
}

/// Reads the words — all of them, `--door security` included.
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        state: None,
        section: None,
        out: std::path::PathBuf::from("security.png"),
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
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--door" => {
                let said = value()?;
                ensure!(
                    said == "security",
                    "`--door {said}` is not this camera's door"
                );
            }
            "--state" => me.state = Some(std::path::PathBuf::from(value()?)),
            "--section" => {
                let word = value()?;
                me.section = Some(
                    Section::of_word(&word)
                        .with_context(|| format!("`--section {word}` is not a section"))?,
                );
            }
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => me.columns = number("--columns", &value()?, 1000)?,
            "--lines" => me.lines = number("--lines", &value()?, 1000)?,
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme =
                    tokens::theme::by_id(&id).with_context(|| format!("unknown theme `{id}`"))?;
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
            "--never-seen" => me.steps.push(Step::NeverSeen(value()?.into())),
            "--breached" => {
                let said = value()?;
                let (address, names) = said
                    .split_once('=')
                    .with_context(|| format!("`--breached` wants `address=names`, not `{said}`"))?;
                let names = names
                    .split(',')
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
                me.steps.push(Step::Breached(address.to_owned(), names));
            }
            "--pwned" => {
                let raw = value()?;
                let count = raw
                    .trim()
                    .parse()
                    .ok()
                    .with_context(|| format!("`--pwned {raw}` is not a whole number"))?;
                me.steps.push(Step::Pwned(count));
            }
            "--pick" => {
                let what = value()?;
                me.steps.push(Step::Pick(match what.as_str() {
                    "files" => crate::seal::Pick::Files,
                    "folder" => crate::seal::Pick::Folder,
                    "open" => crate::seal::Pick::Open,
                    _ => bail!("`--pick {what}` is not files, folder or open"),
                }));
            }
            "--seal" => me.steps.push(Step::Seal(value()?.into())),
            "--sniffed" => {
                let said = value()?;
                let (path, kind) = said
                    .rsplit_once('=')
                    .with_context(|| format!("`--sniffed` wants `path=kind`, not `{said}`"))?;
                let kind: ronoctua_pq::Kind = serde_json::from_str(&format!("\"{kind}\""))
                    .with_context(|| {
                        format!("`{kind}` is not pq, classical, passphrase, mixed or not-age")
                    })?;
                me.steps.push(Step::Sniffed(path.into(), kind));
            }
            "--registering" => me.steps.push(Step::Registering),
            "--refused" => {
                let id = value()?;
                let why = ronoctua_security::Why::of_id(&id).with_context(|| {
                    let ids: Vec<&str> =
                        ronoctua_security::Why::ALL.iter().map(|w| w.id()).collect();
                    format!("`--refused {id}` is not one of {}", ids.join(", "))
                })?;
                me.steps.push(Step::Refused(why));
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    Ok(me)
}

/// `ronoctua-shell photograph --door security …`, to its end: the whole
/// command line is this camera's (the player's camera handed over the rest
/// at `--door security`).
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

/// Where the middle of a cell is, in logical pixels.
fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    (
        (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
        (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
    )
}

/// Draws the window the words ask for into a PNG, or prints its frame.
fn shoot(asked: &Asked) -> Result<()> {
    use crate::door::Detached;
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(tokens::theme::Ground::Chrome);
    tokens::text::set(tokens::text::NORMAL);
    crate::locale::prefs::set(crate::locale::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let scene: Scene = match &asked.state {
        Some(file) => {
            let raw = std::fs::read_to_string(file)
                .with_context(|| format!("could not read `{}`", file.display()))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("`{}` is not a security scene", file.display()))?
        }
        None => Scene::default(),
    };
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let mut d = Detached::new(ui, SecurityWindow::over(scene, asked.section));
    let px = tokens::text::size();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size = (
        (f32::from(asked.columns + 2 * d.content.margin()) * tokens::metrics::cell_width(px)).ceil()
            as u32,
        (f32::from(asked.lines) * tokens::metrics::line_height(px)).ceil() as u32,
    );
    d.lay_out_at(size);
    d.turn();
    // THE SECTION ASKED FOR, IN VIEW: the page opens scrolled to it, as a
    // scene arriving in the running window does.
    d.content.settle(&mut d.ui);
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
    let mut entered = false;
    for step in &asked.steps {
        match *step {
            Step::NeverSeen(ref path) => {
                let came = Came::Hashed {
                    path: path.clone(),
                    result: Ok((
                        "0".repeat(64),
                        ronoctua_lookup::virustotal::Checked::Unknown,
                    )),
                };
                d.content.came(&mut d.ui, came);
            }
            Step::Breached(ref address, ref names) => {
                let came = Came::Email {
                    address: address.clone(),
                    result: Ok(names.clone()),
                };
                d.content.came(&mut d.ui, came);
            }
            Step::Pwned(count) => d.content.came(&mut d.ui, Came::Password(Ok(count))),
            // AS THE PAGE DOES AFTER A KEY: the cursor settled on a row that
            // acts, and the place asked for brought into view.
            Step::Pick(pick) => {
                d.content.view.pick(pick);
                d.content.settle(&mut d.ui);
            }
            Step::Seal(ref path) => {
                d.content.view.seal_these(vec![path.clone()]);
                d.content.settle(&mut d.ui);
            }
            Step::Sniffed(ref path, kind) => {
                let came = Came::Sniffed {
                    path: path.clone(),
                    kind: Some(kind),
                };
                d.content.came(&mut d.ui, came);
            }
            Step::Registering => {
                d.content.view.registering_shown();
                d.content.settle(&mut d.ui);
            }
            // THE DOOR'S THREAD'S ANSWER, as it comes back: refused.
            Step::Refused(why) => {
                let work = d.content.view.in_flight().with_context(|| {
                    format!(
                        "`--refused {}` answers a sudo work, and no `RET` before it sent one",
                        why.id()
                    )
                })?;
                d.content.came(
                    &mut d.ui,
                    Came::Door {
                        work,
                        result: Err(why),
                    },
                );
            }
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
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the camera stops taking its own words, or takes another
    /// door's.
    #[test]
    fn the_camera_reads_its_words() {
        let words: Vec<String> = [
            "--door",
            "security",
            "--section",
            "finger",
            "--key",
            "<down>",
            "--click",
            "3,4",
            "--columns",
            "90",
            "--breached",
            "a@example.org=Adobe, LinkedIn",
            "--pwned",
            "3",
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
        let asked = parse(&words).expect("the words");
        assert_eq!(asked.section, Some(Section::Finger));
        assert_eq!(asked.columns, 90);
        assert_eq!(asked.steps.len(), 4);
        assert_eq!(
            asked.steps[2],
            Step::Breached(
                "a@example.org".into(),
                vec!["Adobe".into(), "LinkedIn".into()]
            )
        );
        assert_eq!(asked.steps[3], Step::Pwned(3));
        let refused = parse(&["--refused".into(), "no-footer".into()]).expect("a refusal");
        assert_eq!(
            refused.steps,
            [Step::Refused(ronoctua_security::Why::NoFooter)]
        );
        assert!(parse(&["--refused".into(), "nope".into()]).is_err());
        assert!(parse(&["--door".into(), "files".into()]).is_err());
        assert!(parse(&["--section".into(), "nowhere".into()]).is_err());
    }

    /// FAILS IF: the window stops reading the file manager's `--check`, or a
    /// word that is no section is taken for one.
    #[test]
    fn the_window_takes_a_section_and_a_file() {
        let words = |w: &[&str]| w.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        // THE FILE MANAGER'S WORDS, as both its doors write them.
        let said = crate::security::window_checking(std::path::Path::new("/home/p/a.exe"));
        assert_eq!(said.first().map(String::as_str), Some("security"));
        let (section, check) = words_of(&said[1..]).expect("the words");
        assert_eq!(section, Some(Section::Online));
        assert_eq!(check, Some(std::path::PathBuf::from("/home/p/a.exe")));
        assert_eq!(words_of(&[]).expect("none"), (None, None));
        assert!(words_of(&words(&["nowhere"])).is_err());
        assert!(words_of(&words(&["--check"])).is_err());
    }

    /// FAILS IF: the window opens on a section without bringing its title to
    /// the top — a section of facts only (Network privacy) included — or a
    /// check's answer lands below the edge.
    #[test]
    fn the_window_brings_the_place_asked_for_into_view() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut ui = Ui::for_tests(texts);
        let scene = Scene {
            read: true,
            settings: Some(ronoctua_security::firewall::Settings::default()),
            ..Scene::default()
        };
        let mut w = SecurityWindow::over(scene.clone(), Some(Section::Privacy));
        w.settle(&mut ui);
        let rows = w.view_rows(&ui);
        assert!(
            rows.len() > ui.room_for_rows(),
            "the page fits: nothing to prove"
        );
        assert_eq!(rows[ui.offset].section, Section::Privacy);
        let cursor = w.view.cursor();
        assert!(
            (ui.offset..ui.offset + ui.room_for_rows()).contains(&cursor),
            "the cursor is off screen"
        );
        let mut w = SecurityWindow::over(scene, None);
        w.settle(&mut ui);
        w.came(&mut ui, Came::Password(Ok(3)));
        let rows = w.view_rows(&ui);
        let shown: Vec<String> = rows[ui.offset..(ui.offset + ui.room_for_rows()).min(rows.len())]
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        let answer = ui.texts.t_args(
            ronoctua_i18n::key::SECURITY_ONLINE_PASSWORD_FOUND,
            &[("count", ronoctua_i18n::Arg::Number(3))],
        );
        let first_words: String = answer.chars().take(20).collect();
        assert!(
            shown.iter().any(|t| t.contains(first_words.trim())),
            "the answer is not on screen: {shown:?}"
        );
    }

    /// FAILS IF: the window asks the password itself again — a work refused
    /// for want of one is a sentence on ITS line, never a question there —
    /// or a refusal of the door stops being said here.
    #[test]
    fn the_window_never_asks_root_and_says_in_its_own_door() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut ui = Ui::for_tests(texts);
        let mut w = SecurityWindow::over(
            Scene {
                read: true,
                ..Scene::default()
            },
            None,
        );
        for why in [
            ronoctua_security::Why::NeedsPassword,
            ronoctua_security::Why::WrongPassword,
            ronoctua_security::Why::NoFooter,
        ] {
            w.came(
                &mut ui,
                Came::Door {
                    work: crate::security::Work::RelaxEnd,
                    result: Err(why),
                },
            );
            assert!(
                ui.asking.is_none(),
                "{why:?}: a question on the window's line"
            );
            assert_eq!(
                ui.message,
                Some(ui.texts.t(crate::security_words::refusal(why)))
            );
        }
    }
}
