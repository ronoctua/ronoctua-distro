//! The installer's TUI — milestone 8's criterion, section 7.1.
//!
//! "The TUI collects **only** what only the person knows" — and, since the
//! person's repository became mandatory, checks three of those answers
//! against the network before the next screen: the way out, the access, the
//! passphrase. The checks are jobs ([`crate::job`]) the state asks for and a
//! thread runs; the state never waits on the network itself.
//!
//! # It decides nothing
//!
//! What the questions are and how to validate them is in [`model`](crate::model);
//! what to run is in [`build_plan`](crate::plan); how to run it is in
//! [`executor`](crate::executor). What is left for this module is reading keys and
//! drawing — and that is why its state ([`State`]) is testable with no terminal
//! at all, which is where the tests at the end of the file strike.
//!
//! # The invariants hold here too
//!
//! **8:** no label is a literal; everything comes from `crate::key`. A
//! screenshot of the installer with `[inst-disk]` written on it would be proof
//! that the invariant had fallen — and that is exactly how it was.
//!
//! **9:** no colour written by hand; all come from `ronoctua_tokens::color`.
//! The installer is the first thing the person sees of the system, and seeing another
//! palette there would be seeing another system.
//!
//! **14:** no chrome — no decorated title bar, no floating dialogue
//! box. One question per screen, the help below, the keys in the
//! footer. The same geometry as everything else: content, and two lines of state.
//!
//! **19:** everything answers the pointer too — a terminal over SSH sends
//! mouse events. A list's row is selected by a click and taken by a second
//! one, the wheel moves the list, every key in the keys line is a click
//! (`Enter`, `Esc`, `Ctrl+C`), and what the pointer rests on is lit. What the
//! drawing paints and what the pointer hits are ONE answer ([`places`]).

use std::io::Stdout;
use std::time::Duration;

use crate::key;
use anyhow::{Context as _, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::half_block;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Widget, Wrap};
use ronoctua_belongings::prefs::Auth;
use ronoctua_i18n::Texts;
use ronoctua_tokens::{Color, color};

use crate::access::{self, Probed, QUIET, Qr};
use crate::gate::Gate;
use crate::job::{Asked, Done, Job};
use crate::model;
use crate::model::{Answers, Question, sentence, validate_it};
use crate::plan::{Origin, Step, steps};
use crate::reach::Reach;
use crate::release::{Follow, Hop, Stay};

/// Converts a colour token into what ratatui understands.
///
/// Node `to_linear`: the terminal receives sRGB, unlike wgpu. Passing
/// linear here would darken everything — the mirror image of the mistake that cost an afternoon
/// in the renderer, and it is worth saying which side we are on.
fn color(c: Color) -> ratatui::style::Color {
    let [r, g, b, _] = c.to_rgba8();
    ratatui::style::Color::Rgb(r, g, b)
}

/// A candidate disk, as `lsblk` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    /// `/dev/nvme0n1`.
    pub path: String,
    /// `931,5G`.
    pub size: String,
    /// The model, when the kernel knows it.
    pub model: String,
}

impl Disk {
    /// The line that appears in the list.
    #[must_use]
    pub fn line(&self) -> String {
        if self.model.is_empty() {
            format!("{}  {}", self.path, self.size)
        } else {
            format!("{}  {}  {}", self.path, self.size, self.model)
        }
    }
}

/// Parses `lsblk`'s output into disks.
///
/// A **pure** function, and therefore testable: the alternative would be finding out on a
/// actual laptop that the parser chokes on a model with a space in its
/// name — and then the right disk does not appear in the list.
#[must_use]
pub fn disks_from(output: &str) -> Vec<Disk> {
    output
        .lines()
        .filter_map(|l| {
            let mut fields = l.split_whitespace();
            let path = fields.next()?.to_owned();
            if !path.starts_with("/dev/") {
                return None;
            }
            let size = fields.next().unwrap_or_default().to_owned();
            // The model may have a space (`Samsung SSD 980 PRO`), so it is the
            // REST of the line, and not the next field.
            let model = fields.collect::<Vec<_>>().join(" ");
            Some(Disk { path, size, model })
        })
        .collect()
}

/// Asks the kernel which disks exist.
///
/// `-d` whole disks only (not the partitions), `-e 7,11` without the ISO's own
/// `loop` devices nor the CD reader — which would appear in the list and are no use.
fn disks() -> Vec<Disk> {
    let Ok(output) = crate::executor::command("lsblk")
        .args(["-dnpo", "NAME,SIZE,MODEL", "-e", "7,11"])
        .output()
    else {
        return Vec::new();
    };
    disks_from(&String::from_utf8_lossy(&output.stdout))
}

/// The Wi-Fi networks in range, through the ISO's NetworkManager.
///
/// It returns empty when there is no radio, no NetworkManager or the bus
/// does not answer — and empty is NOT an error: the list is left with only "no Wi-Fi", which is
/// the right answer on a cabled machine. Failing the whole installation because
/// there is no radio would be the opposite of what this question exists to solve.
fn networks() -> Vec<String> {
    let Ok(g) = ronoctua_network::Manager::fresh() else {
        return Vec::new();
    };
    // It asks for the scan BEFORE reading: on a freshly booted ISO
    // NetworkManager may never have scanned, and the list would come back empty with the
    // radio working perfectly.
    let _ = g.scan();
    g.networks()
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.ssid)
        .collect()
}

/// Which screen the TUI is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// Answering question number `n` in [`Question::ALL_KEYS`].
    Asking(usize),
    /// The release's installer was found: Enter starts it, with the answers.
    Hopping(Hop),
    /// Showing the plan, waiting for confirmation.
    Reviewing,
    /// Running.
    Running,
    /// Finished. `Ok` when it worked.
    Finished(bool),
}

/// What a key leads to, beyond the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Nothing beyond the screen.
    Stay,
    /// The person confirmed: the installation runs.
    Run,
    /// The person starts the release's installer.
    Hop(Hop),
    /// The person gave up (`Ctrl+C`, or its key clicked); nothing was
    /// touched.
    GiveUp,
}

/// What the pointer did, as the TUI reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pointer {
    /// The left button went down.
    Click,
    /// It moved, or dragged.
    Moved,
    /// The wheel, away from the person.
    WheelUp,
    /// The wheel, towards the person.
    WheelDown,
}

/// What is under the pointer: one of the places [`places`] answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// A list's option, by its index.
    Row(usize),
    /// One of the keys line's hints, by its index in [`Places::hints`].
    Key(usize),
}

/// What a hint of the keys line does when clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The key it names, by the same path as the keyboard's.
    Key(KeyCode),
    /// `Ctrl+C`: give up.
    Quit,
}

/// One hint of the keys line, where it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// Its first column.
    pub x: u16,
    /// Its width, in cells.
    pub width: u16,
    /// Its text (`Enter continues`).
    pub text: String,
    /// Where its text starts in the keys line's, in bytes: what lies
    /// between two hints is drawn from that text, never from the code.
    pub at: usize,
    /// What a click does; `None` for one that names no clickable key
    /// (`↑↓`, which the list and the wheel answer), drawn inert.
    pub act: Option<Act>,
}

/// A list, where it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
    /// Its rows' area.
    pub area: Rect,
    /// The option its first row shows.
    pub offset: usize,
    /// How many options it has.
    pub count: usize,
}

/// Where everything on the screen is — what [`draw`] paints and what the
/// pointer hits, from one answer: a control is grabbed where it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The title and the label.
    pub head: Rect,
    /// The field, the list, the plan or the output.
    pub body: Rect,
    /// The help line, or the refusal: as many rows as it wraps to.
    pub help: Rect,
    /// The keys line.
    pub keys: Rect,
    /// The keys line's text, as the language writes it.
    pub key_line: String,
    /// The list, when the screen is one.
    pub list: Option<Rows>,
    /// The keys line's hints.
    pub hints: Vec<Hint>,
}

impl Places {
    /// What is under the cell `(x, y)`.
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> Option<Hit> {
        let inside = |r: Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        if inside(self.keys) {
            return self
                .hints
                .iter()
                .position(|h| h.act.is_some() && x >= h.x && x < h.x + h.width)
                .map(Hit::Key);
        }
        let rows = self.list?;
        if !inside(rows.area) {
            return None;
        }
        let i = rows.offset + usize::from(y - rows.area.y);
        (i < rows.count).then_some(Hit::Row(i))
    }
}

/// How long the access screen waits before asking the repository again for
/// a key the person has not added yet: long enough to add it, short enough
/// that "ready" appears while they look.
pub const PROBE_AGAIN_S: u64 = 5;

/// What the TUI has read so far.
///
/// Separated from the drawing on purpose: this is what the tests exercise, and the
/// drawing is what cannot be tested without a terminal. And separated from the
/// network: what the screen wants checked it ASKS ([`State::asked`]), and the
/// answer comes back through [`State::arrived`] — a thread's, or a test's.
#[derive(Debug)]
pub struct State {
    /// Where we are.
    pub screen: Screen,
    /// What has been answered.
    pub answers: Answers,
    /// What is being typed in the current field.
    pub typing: String,
    /// The current field's refusal, as a text key, if any.
    pub error: Option<&'static str>,
    /// What the screen says while it waits, or what it found — a text key.
    pub status: Option<&'static str>,
    /// What was typed in the confirmation.
    pub confirmation: String,
    /// The machine's disks.
    pub disks: Vec<Disk>,
    /// The Wi-Fi networks in range, scanned at startup.
    ///
    /// Scanned ONCE, before the first screen: the radio takes seconds to
    /// answer, and scanning when the question arrives would make the screen open empty
    /// and fill up on its own afterwards — the person would already have chosen "no Wi-Fi".
    pub networks: Vec<String>,
    /// The hosts the network question names (its answer is the consent to
    /// reach them).
    pub hosts: String,
    /// Which list item is selected.
    pub choice: usize,
    /// The lines the execution printed.
    pub output: Vec<String>,
    /// Which step is running.
    pub step: Option<(usize, usize, &'static str)>,
    /// A job the screen wants run — taken by whoever runs jobs.
    pub asked: Option<Asked>,
    /// The number of the job whose answer the screen waits for.
    pub waiting: Option<u64>,
    /// The last job's number.
    serial: u64,
    /// The deploy key's public half, made for this machine.
    pub public_key: Option<String>,
    /// What the last probe of the access found.
    pub probed: Option<Probed>,
    /// The passphrase typed once, for an empty repository: it is typed again.
    first_passphrase: Option<String>,
    /// Why the installer goes on with its own distro, when the release said
    /// to stay.
    pub stay: Option<Stay>,
    /// What the pointer rests on: lit, and never the cursor.
    pub hover: Option<Hit>,
    /// The option the list's first row showed when the screen was last
    /// drawn ([`State::drawn`]): the list scrolls only when the selection
    /// leaves the rows, so a row stays under the pointer between its two
    /// clicks.
    pub scroll: usize,
}

impl State {
    /// It starts on the first question.
    #[must_use]
    pub fn fresh(disks: Vec<Disk>, networks: Vec<String>) -> Self {
        let mut e = Self {
            screen: Screen::Asking(0),
            networks,
            hosts: String::new(),
            answers: Answers::default(),
            typing: String::new(),
            error: None,
            status: None,
            confirmation: String::new(),
            disks,
            // The FIRST screen is the language, and it opens on the default like
            // every other list. `State::fresh` cannot call `default_choice`
            // before the struct exists, so it is set right after.
            choice: 0,
            output: Vec::new(),
            step: None,
            asked: None,
            waiting: None,
            serial: 0,
            public_key: None,
            probed: None,
            first_passphrase: None,
            stay: None,
            hover: None,
            scroll: 0,
        };
        e.choice = e.default_choice();
        e
    }

    /// The release's installer, picking up where the hop left: the answers
    /// given, and the question it was about to ask.
    ///
    /// The networks in range are scanned here too, and the one chosen
    /// before the hop is always among them: going back to the network
    /// question must land on it, and a list without it would turn the
    /// carried answer into "no Wi-Fi".
    #[must_use]
    pub fn resumed(
        disks: Vec<Disk>,
        mut networks: Vec<String>,
        answers: Answers,
        at: Question,
    ) -> Self {
        if !answers.wifi_ssid.is_empty() && !networks.contains(&answers.wifi_ssid) {
            networks.insert(0, answers.wifi_ssid.clone());
        }
        let mut e = Self::fresh(disks, networks);
        e.answers = answers;
        e.stay = Some(Stay::FromRelease);
        let n = Question::ALL_KEYS
            .iter()
            .position(|q| *q == at)
            .unwrap_or_default();
        e.screen = Screen::Asking(n);
        e.choice = e.default_choice();
        e
    }

    /// The current screen's question.
    #[must_use]
    pub fn question_of(&self) -> Option<Question> {
        match self.screen {
            Screen::Asking(n) => Question::ALL_KEYS.get(n).copied(),
            _ => None,
        }
    }

    /// Whether the repository is reached with a token (HTTPS) rather than a
    /// deploy key.
    #[must_use]
    pub fn by_token(&self) -> bool {
        access::auth_of(&self.answers.belongings) == Auth::Token
    }

    /// Whether the current screen takes typed characters: not a list, not
    /// while a check runs, and not the access screen of a deploy key, which
    /// is shown and not typed.
    #[must_use]
    pub fn types(&self) -> bool {
        self.waiting.is_none()
            && !self.is_list()
            && !(self.question_of() == Some(Question::BelongingsAccess) && !self.by_token())
            && matches!(self.screen, Screen::Asking(_))
    }

    /// Whether the current question is answered by choosing from a list.
    #[must_use]
    pub fn is_list(&self) -> bool {
        // A LIST WITH NO OPTIONS IS NOT A LIST, it is a screen with no way out.
        //
        // The lists come from files that can be absent — a `base.lst` the build
        // did not point at, a `SUPPORTED` from a glibc that moved. With no
        // options the chosen value is the empty string, validation refuses it,
        // the screen never advances, and the person is trapped on a field that
        // shows nothing and accepts nothing. Falling back to a typed field
        // keeps the way out.
        if self.list_is_empty() {
            return false;
        }
        matches!(
            self.question_of(),
            Some(
                Question::Disk
                    | Question::Language
                    | Question::Wifi
                    | Question::Keyboard
                    | Question::KeyboardVariant
                    | Question::ComposeKey
                    | Question::SystemLocale
                    | Question::Timezone
                    | Question::Profile
            )
        )
    }

    /// Whether the question WOULD be a list, but has nothing to offer.
    ///
    /// Split from `is_list` so the two do not call each other: `options` asks
    /// the question directly, never through `is_list`.
    #[must_use]
    fn list_is_empty(&self) -> bool {
        matches!(
            self.question_of(),
            Some(
                Question::Disk
                    | Question::Keyboard
                    | Question::KeyboardVariant
                    | Question::ComposeKey
                    | Question::SystemLocale
                    | Question::Timezone
            )
        ) && self.options().is_empty()
    }

    /// The current question's list options.
    #[must_use]
    pub fn options(&self) -> Vec<String> {
        match self.question_of() {
            Some(Question::Disk) => self.disks.iter().map(Disk::line).collect(),
            Some(Question::Language) => ronoctua_i18n::LANGUAGES
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            // "NO WI-FI" IS THE FIRST OPTION, and not the last: whoever has a
            // cable does not want to scroll through the neighbour's networks
            // to say they want none.
            Some(Question::Wifi) => std::iter::once(String::new())
                .chain(self.networks.iter().cloned())
                .collect(),
            // THE KEYBOARD TRIO reads the same file the validation reads, so a
            // list can never offer what the next screen refuses.
            Some(Question::Keyboard) => model::available_layouts()
                .iter()
                .map(model::Choice::line)
                .collect(),
            // Filtered by the layout ALREADY ANSWERED — which is why the layout
            // comes first of the three.
            Some(Question::KeyboardVariant) => model::variants_of(&self.answers.keyboard)
                .iter()
                .map(model::Choice::line)
                .collect(),
            Some(Question::ComposeKey) => model::compose_options()
                .iter()
                .map(model::Choice::line)
                .collect(),
            Some(Question::SystemLocale) => model::available_locales(),
            Some(Question::Timezone) => model::available_timezones(),
            // THE ONLY LIST HERE THAT COMES FROM NO FILE, and so the only one
            // that cannot be empty.
            Some(Question::Profile) => model::PROFILES.iter().map(|p| (*p).to_owned()).collect(),
            _ => Vec::new(),
        }
    }

    /// WHICH OPTION IS ALREADY SELECTED when a list opens: the answer
    /// someone pressing Enter through the whole install would get, looked up
    /// by NAME (the lists come from files that move).
    #[must_use]
    fn default_choice(&self) -> usize {
        let at = |list: Vec<String>, wanted: &str| {
            list.iter().position(|o| o == wanted).unwrap_or_default()
        };
        match self.question_of() {
            Some(Question::Language) => at(
                ronoctua_i18n::LANGUAGES
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect(),
                model::DEFAULT_LANGUAGE,
            ),
            Some(Question::Keyboard) => at(
                model::available_layouts()
                    .iter()
                    .map(model::Choice::line)
                    .collect(),
                &model::available_layouts()
                    .iter()
                    .find(|c| c.name == model::DEFAULT_LAYOUT)
                    .map(model::Choice::line)
                    .unwrap_or_default(),
            ),
            Some(Question::ComposeKey) => at(
                model::compose_options()
                    .iter()
                    .map(model::Choice::line)
                    .collect(),
                &model::compose_options()
                    .iter()
                    .find(|c| c.name == model::DEFAULT_COMPOSE)
                    .map(model::Choice::line)
                    .unwrap_or_default(),
            ),
            // The rest already have the right answer first: no variant, the
            // British locale, London, no Wi-Fi, and the machine's own disk.
            _ => 0,
        }
    }

    /// The value the current list choice produces.
    #[must_use]
    fn choice_value(&self) -> String {
        match self.question_of() {
            Some(Question::Disk) => self
                .disks
                .get(self.choice)
                .map(|d| d.path.clone())
                .unwrap_or_default(),
            Some(Question::Language) => ronoctua_i18n::LANGUAGES
                .get(self.choice)
                .map(|s| (*s).to_owned())
                .unwrap_or_default(),
            // The list SHOWS `br — Portuguese (Brazil)` and the config wants
            // `br`: the name comes from the same structure that built the line.
            Some(Question::Keyboard) => model::available_layouts()
                .get(self.choice)
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            Some(Question::KeyboardVariant) => model::variants_of(&self.answers.keyboard)
                .get(self.choice)
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            Some(Question::ComposeKey) => model::compose_options()
                .get(self.choice)
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            Some(Question::SystemLocale) => model::available_locales()
                .get(self.choice)
                .cloned()
                .unwrap_or_default(),
            Some(Question::Timezone) => model::available_timezones()
                .get(self.choice)
                .cloned()
                .unwrap_or_default(),
            Some(Question::Profile) => model::PROFILES
                .get(self.choice)
                .map(|p| (*p).to_owned())
                .unwrap_or_default(),
            // Index 0 is "no Wi-Fi", and returns an empty string — which is
            // what makes the password screen be skipped.
            Some(Question::Wifi) => self
                .choice
                .checked_sub(1)
                .and_then(|i| self.networks.get(i).cloned())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Asks for `job`, and waits for its answer.
    fn ask(&mut self, job: Job, status: &'static str) {
        self.serial += 1;
        self.asked = Some(Asked {
            serial: self.serial,
            job,
        });
        self.waiting = Some(self.serial);
        self.status = Some(status);
        self.error = None;
    }

    /// Stops waiting: an answer still on its way is dropped when it comes.
    fn stop_waiting(&mut self) {
        self.asked = None;
        self.waiting = None;
    }

    /// Advances to the next question that gets typed, or to the review.
    ///
    /// It skips the computed ones: the `resume_offset` is on section 7.1's list
    /// because the installer has to produce it, not because anyone knows it.
    fn follow(&mut self, n: usize) {
        self.stop_waiting();
        self.status = None;
        self.first_passphrase = None;
        self.scroll = 0;
        let mut i = n + 1;
        while let Some(q) = Question::ALL_KEYS.get(i) {
            if !q.computed() && !q.skipped_one(&self.answers) {
                self.screen = Screen::Asking(i);
                self.typing.clear();
                self.error = None;
                // AFTER setting the screen: `default_choice` asks which question
                // is showing, and the answer is the one just set.
                self.choice = self.default_choice();
                return;
            }
            i += 1;
        }
        self.screen = Screen::Reviewing;
        self.error = None;
    }

    /// Goes back one question. From the first, it does not go back.
    fn go_back(&mut self) {
        self.stop_waiting();
        self.status = None;
        self.first_passphrase = None;
        self.scroll = 0;
        let current = match &self.screen {
            Screen::Asking(n) => *n,
            // The hop's screen is the network's answer: back to it.
            Screen::Hopping(_) => Question::ALL_KEYS
                .iter()
                .position(|q| *q == Question::BelongingsAddress)
                .unwrap_or_default(),
            Screen::Reviewing => Question::ALL_KEYS.len(),
            _ => return,
        };
        let mut i = current;
        while i > 0 {
            i -= 1;
            let Some(q) = Question::ALL_KEYS.get(i) else {
                continue;
            };
            if q.computed() || q.skipped_one(&self.answers) {
                continue;
            }
            self.screen = Screen::Asking(i);
            // A SECRET IS NOT TYPED BACK: going back to it shows an empty
            // field, as a password field does — and the access of a deploy
            // key is shown, not typed.
            self.typing = if q.secret_one() || *q == Question::BelongingsAccess {
                String::new()
            } else {
                self.answers.get(*q).to_owned()
            };
            self.error = None;
            // GOING BACK lands on what was ANSWERED, not on the default.
            self.choice = self
                .options()
                .iter()
                .position(|o| {
                    o.starts_with(self.answers.get(*q)) && !self.answers.get(*q).is_empty()
                })
                .unwrap_or_else(|| self.default_choice());
            // THE ACCESS IS ASKED AGAIN when its screen is shown again.
            if *q == Question::BelongingsAccess && !self.by_token() {
                self.ask_probe(0);
            }
            return;
        }
    }

    /// Asks the repository whether the deploy key reads and writes, after
    /// `wait_s`.
    fn ask_probe(&mut self, wait_s: u64) {
        self.ask(
            Job::Probe {
                address: self.answers.belongings.clone(),
                token: None,
                wait_s,
            },
            self.probed
                .as_ref()
                .map_or(key::INST_ACCESS_WAITING, Probed::status_key),
        );
    }

    /// The file the access is kept in, in the scratch home.
    fn access_file(&self) -> String {
        format!(
            "{}/{}",
            crate::plan::scratch_keys(),
            if self.by_token() {
                ronoctua_belongings::places::TOKEN_FILE
            } else {
                ronoctua_belongings::places::DEPLOY_KEY
            }
        )
    }

    /// An answer to a job has come back. One the screen no longer waits for
    /// is dropped.
    pub fn arrived(&mut self, serial: u64, done: Done) {
        if self.waiting != Some(serial) {
            return;
        }
        self.waiting = None;
        self.status = None;
        let Screen::Asking(n) = self.screen else {
            return;
        };
        match done {
            Done::Network(gate) if gate.open() => {
                self.ask(Job::Release, key::INST_RELEASE_LOOKING);
            }
            Done::Network(gate) => self.error = Some(gate.key()),
            Done::Release(Follow::Hop(hop)) => self.screen = Screen::Hopping(hop),
            Done::Release(Follow::Stay(stay)) => {
                self.stay = Some(stay);
                self.follow(n);
            }
            Done::Prepared(Ok(public_key)) => {
                self.public_key = public_key;
                self.probed = None;
                self.follow(n);
                if !self.by_token() {
                    self.ask_probe(0);
                }
            }
            Done::Prepared(Err(why)) | Done::Verified(Err(why)) => {
                self.error = Some(access::refusal_key(why));
                self.typing.clear();
            }
            Done::Probed(Ok(probed)) => {
                self.probed = Some(probed);
                if probed.ready() {
                    self.answers.belongings_access = self.access_file();
                    self.answers.belongings_empty = Some(probed.empty);
                    if self.by_token() {
                        self.follow(n);
                    } else {
                        self.status = Some(probed.status_key());
                    }
                } else if self.by_token() {
                    self.error = Some(probed.why.map_or(probed.status_key(), access::refusal_key));
                    self.typing.clear();
                } else {
                    self.ask_probe(PROBE_AGAIN_S);
                }
            }
            Done::Probed(Err(why)) => {
                self.error = Some(access::refusal_key(why));
                if self.by_token() {
                    self.typing.clear();
                } else {
                    // The person may be fixing it right now: ask again.
                    self.ask_probe(PROBE_AGAIN_S);
                    self.error = Some(access::refusal_key(why));
                }
            }
            Done::Verified(Ok(())) => self.follow(n),
        }
    }

    /// `Enter` on question `q` (number `n`), with `v` the value given.
    fn answer(&mut self, n: usize, q: Question, v: String, languages: &[&str], layouts: &[&str]) {
        // THE ACCESS OF A DEPLOY KEY is not typed: Enter goes on when it
        // reads and writes, and asks again now when it does not.
        if q == Question::BelongingsAccess && !self.by_token() {
            if self.probed.is_some_and(|p| p.ready()) {
                self.follow(n);
            } else {
                self.ask_probe(0);
            }
            return;
        }
        // A TOKEN TYPED is the access itself, not the file the terminal-less
        // mode names: only a blank one is refused here, and the probe judges
        // the rest.
        let checked = if q == Question::BelongingsAccess {
            if v.trim().is_empty() {
                Err(model::Invalid::Blank)
            } else {
                Ok(())
            }
        } else {
            validate_it(q, &v, languages, layouts)
        };
        if let Err(e) = checked {
            self.error = Some(e.key());
            return;
        }
        match q {
            // THE NETWORK IS A GATE: a cable is checked here, a Wi-Fi network
            // after its password.
            Question::Wifi if v.is_empty() => {
                self.answers.set(q, v);
                self.answers.wifi_password.clear();
                self.ask(
                    Job::Network {
                        ssid: String::new(),
                        password: String::new(),
                    },
                    key::INST_GATE_CHECKING,
                );
            }
            Question::WifiPassword => {
                self.answers.set(q, v);
                self.ask(
                    Job::Network {
                        ssid: self.answers.wifi_ssid.clone(),
                        password: self.answers.wifi_password.clone(),
                    },
                    key::INST_GATE_CHECKING,
                );
            }
            Question::BelongingsAddress => {
                self.answers.set(q, v.trim().to_owned());
                self.answers.belongings_access.clear();
                self.answers.belongings_empty = None;
                self.ask(
                    Job::Prepare {
                        address: self.answers.belongings.clone(),
                    },
                    key::INST_CHECKING,
                );
            }
            // A TOKEN, probed before it is taken.
            Question::BelongingsAccess => self.ask(
                Job::Probe {
                    address: self.answers.belongings.clone(),
                    token: Some(v),
                    wait_s: 0,
                },
                key::INST_CHECKING,
            ),
            // AN EMPTY REPOSITORY is MADE with this passphrase: as long as a
            // disk's, and typed twice, since nothing can prove it.
            Question::BelongingsPassphrase if self.answers.belongings_empty == Some(true) => {
                match self.first_passphrase.take() {
                    None if v.chars().count() < model::MINIMUM_PASSPHRASE => {
                        self.error = Some(key::INST_INVALID_PASSPHRASE_SHORT);
                    }
                    // The label above the field says it is the second time.
                    None => {
                        self.first_passphrase = Some(v);
                        self.typing.clear();
                    }
                    Some(first) if first == v => {
                        self.answers.set(q, v);
                        self.follow(n);
                    }
                    Some(_) => {
                        self.error = Some(key::INST_BELONGINGS_PASSPHRASE_DIFFERS);
                        self.typing.clear();
                    }
                }
            }
            // ONE THAT HAS SOMETHING IN IT: the passphrase opens its
            // `key.age`, or the installer does not go on.
            Question::BelongingsPassphrase => {
                self.answers.set(q, v.clone());
                self.ask(
                    Job::Verify {
                        address: self.answers.belongings.clone(),
                        passphrase: v,
                    },
                    key::INST_CHECKING,
                );
            }
            _ => {
                self.answers.set(q, v);
                self.follow(n);
            }
        }
    }

    /// Applies a key.
    ///
    /// **Testable without a terminal**: it is the reason key reading and
    /// drawing are in functions other than this one.
    pub fn key(&mut self, k: KeyCode) -> Next {
        let languages: Vec<&str> = ronoctua_i18n::LANGUAGES.to_vec();
        // THE LAYOUTS THE ISO CARRIES, read from xkeyboard-config itself.
        let layouts = crate::model::layout_names();
        let layouts: Vec<&str> = layouts.iter().map(String::as_str).collect();
        match self.screen.clone() {
            Screen::Asking(n) => {
                let Some(q) = Question::ALL_KEYS.get(n).copied() else {
                    return Next::Stay;
                };
                match k {
                    KeyCode::Esc => self.go_back(),
                    // WHILE A CHECK RUNS, only Esc counts: an Enter would ask
                    // twice, and typing would change what is being checked.
                    _ if self.waiting.is_some() => {}
                    KeyCode::Up if self.is_list() => self.choice = self.choice.saturating_sub(1),
                    KeyCode::Down if self.is_list() => {
                        let last = self.options().len().saturating_sub(1);
                        self.choice = (self.choice + 1).min(last);
                    }
                    KeyCode::Backspace if self.types() => {
                        self.typing.pop();
                        self.error = None;
                    }
                    KeyCode::Char(c) if self.types() => {
                        self.typing.push(c);
                        self.error = None;
                    }
                    KeyCode::Enter => {
                        let v = if self.is_list() {
                            self.choice_value()
                        } else {
                            self.typing.clone()
                        };
                        self.answer(n, q, v, &languages, &layouts);
                    }
                    _ => {}
                }
                Next::Stay
            }
            Screen::Hopping(hop) => match k {
                KeyCode::Enter => Next::Hop(hop),
                KeyCode::Esc => {
                    self.go_back();
                    Next::Stay
                }
                _ => Next::Stay,
            },
            Screen::Reviewing => match k {
                KeyCode::Esc => {
                    self.go_back();
                    Next::Stay
                }
                KeyCode::Backspace => {
                    self.confirmation.pop();
                    self.error = None;
                    Next::Stay
                }
                KeyCode::Char(c) => {
                    self.confirmation.push(c);
                    self.error = None;
                    Next::Stay
                }
                KeyCode::Enter => {
                    // Typing the disk's path, and not "yes". Copying the
                    // path forces one to READ the path, which is the one thing
                    // on this screen nobody can get wrong.
                    if self.confirmation.trim() == self.answers.disk {
                        self.screen = Screen::Running;
                        Next::Run
                    } else {
                        self.error = Some(key::INST_CONFIRM_ERROR);
                        Next::Stay
                    }
                }
                _ => Next::Stay,
            },
            Screen::Running | Screen::Finished(_) => Next::Stay,
        }
    }

    /// Applies what the pointer did at the cell `(x, y)`, read against the
    /// screen as it is drawn ([`places`]).
    ///
    /// A list's row: the first click selects it, a second one on the
    /// selected row does what `Enter` does. A hint of the keys line: its
    /// key, by the same path as the keyboard's. The wheel moves the list.
    /// Resting, the pointer lights what is under it ([`State::hover`]).
    pub fn pointer(&mut self, p: Pointer, x: u16, y: u16, places: &Places) -> Next {
        let hit = places.hit(x, y);
        match p {
            Pointer::Moved => {
                self.hover = hit;
                Next::Stay
            }
            Pointer::WheelUp if self.is_list() => self.key(KeyCode::Up),
            Pointer::WheelDown if self.is_list() => self.key(KeyCode::Down),
            Pointer::WheelUp | Pointer::WheelDown => Next::Stay,
            Pointer::Click => {
                self.hover = hit;
                match hit {
                    Some(Hit::Row(i)) if self.waiting.is_none() && self.is_list() => {
                        if i == self.choice {
                            self.key(KeyCode::Enter)
                        } else {
                            self.choice = i;
                            Next::Stay
                        }
                    }
                    Some(Hit::Key(j)) => match places.hints.get(j).and_then(|h| h.act) {
                        Some(Act::Key(k)) => self.key(k),
                        Some(Act::Quit) => Next::GiveUp,
                        None => Next::Stay,
                    },
                    _ => Next::Stay,
                }
            }
        }
    }

    /// The screen was drawn at `at`: what the list showed is kept, so the
    /// next drawing scrolls from there ([`State::scroll`]).
    pub fn drawn(&mut self, at: &Places) {
        self.scroll = at.list.map_or(0, |rows| rows.offset);
    }

    /// Where the installation takes its distro from, as the plan shows it.
    #[must_use]
    pub fn origin(&self, base: &Origin) -> Origin {
        Origin {
            aimed: self.stay.is_some_and(Stay::aims_at_own),
            ..base.clone()
        }
    }

    /// The steps the review shows.
    #[must_use]
    pub fn build_plan(&self, base: &Origin) -> Vec<Step> {
        steps(&self.answers, &self.origin(base))
    }
}

/// The label the current screen shows over its field.
fn label_of(e: &State, texts: &Texts) -> String {
    match (&e.screen, e.question_of()) {
        (Screen::Asking(_), Some(Question::BelongingsPassphrase))
            if e.first_passphrase.is_some() =>
        {
            texts.t(key::INST_BELONGINGS_PASSPHRASE_AGAIN)
        }
        (Screen::Asking(_), Some(q)) => texts.t(q.label()),
        _ => String::new(),
    }
}

/// The help line of the current question.
fn help_of(e: &State, q: Question, texts: &Texts) -> String {
    match q {
        // THE NETWORK NAMES THE HOSTS IT WILL REACH: answering is the
        // consent to reach them.
        Question::Wifi if e.hosts.is_empty() => texts.t(key::INST_NETWORK_HELP_UNNAMED),
        Question::Wifi => texts.t_with(q.help(), &[("hosts", &e.hosts)]),
        Question::BelongingsAccess if e.by_token() => texts.t(key::INST_BELONGINGS_TOKEN_HELP),
        Question::BelongingsPassphrase if e.answers.belongings_empty == Some(true) => {
            texts.t(key::INST_BELONGINGS_PASSPHRASE_NEW_HELP)
        }
        _ => texts.t(q.help()),
    }
}

/// The help line's sentence — or the refusal's — and whether it is one.
fn said_of(e: &State, texts: &Texts) -> (String, bool) {
    if let Some(k) = e.error {
        return (sentence(texts, k), true);
    }
    let help = match e.screen {
        Screen::Asking(_) => e
            .question_of()
            .map(|q| help_of(e, q, texts))
            .unwrap_or_default(),
        Screen::Reviewing => texts.t(key::INST_DESTRUCTIVE),
        _ => String::new(),
    };
    (help, false)
}

/// The keys line of the screen.
fn keys_of(e: &State, texts: &Texts) -> String {
    match e.screen {
        Screen::Asking(_) if e.is_list() => texts.t(key::INST_KEYS_CHOOSE),
        Screen::Asking(_) | Screen::Hopping(_) => texts.t(key::INST_KEYS_EDIT),
        Screen::Reviewing => texts.t(key::INST_KEYS_REVIEW),
        _ => texts.t(key::INST_KEYS_QUIT),
    }
}

/// What separates two hints of a keys line, as both `.ftl` files write it —
/// only to FIND the hints: what is drawn between them is the text's own
/// (`Hint::at`).
pub const HINT_SEPARATOR: &str = " · ";

/// What a hint does, from its first word — the key's name, which is the
/// code's and never translated (`Enter`, `Esc`, `Ctrl+C`).
#[must_use]
pub fn act_of(hint: &str) -> Option<Act> {
    match hint.split_whitespace().next()? {
        "Enter" => Some(Act::Key(KeyCode::Enter)),
        "Esc" => Some(Act::Key(KeyCode::Esc)),
        "Ctrl+C" => Some(Act::Quit),
        _ => None,
    }
}

/// The width of `text`, in cells, as ratatui draws it.
fn cells(text: &str) -> u16 {
    u16::try_from(Span::raw(text).width()).unwrap_or(u16::MAX)
}

/// The hints of a keys line drawn from the column `x`: each where the text
/// before it ends, in cells.
fn hints_of(text: &str, x: u16) -> Vec<Hint> {
    let mut start = 0;
    text.split(HINT_SEPARATOR)
        .map(|part| {
            let hint = Hint {
                x: x.saturating_add(cells(&text[..start])),
                width: cells(part),
                text: part.to_owned(),
                at: start,
                act: act_of(part),
            };
            start += part.len() + HINT_SEPARATOR.len();
            hint
        })
        .collect()
}

/// The option a list's first row shows: where it was (`scroll`), unless the
/// selection left the rows — then just far enough to show it again, as the
/// list's own widget scrolls. Pure.
fn list_offset(scroll: usize, choice: usize, height: u16) -> usize {
    let rows = usize::from(height.max(1));
    if choice < scroll {
        choice
    } else if choice >= scroll + rows {
        choice + 1 - rows
    } else {
        scroll
    }
}

/// The list on a body of `area`, when the screen is one.
fn list_rows(e: &State, area: Rect) -> Option<Rows> {
    if !matches!(e.screen, Screen::Asking(_)) || !e.is_list() {
        return None;
    }
    let count = e.options().len();
    if count == 0 {
        return None;
    }
    let (area, _) = split_status(area, e.status.is_some());
    Some(Rows {
        area,
        offset: list_offset(e.scroll, e.choice, area.height),
        count,
    })
}

/// Where everything is on a screen of `area` — the one answer the drawing
/// paints and the pointer hits.
#[must_use]
pub fn places(area: Rect, e: &State, texts: &Texts) -> Places {
    // THE HELP WRAPS, by words as it is drawn, up to a third of the
    // screen: cut at the edge, the sentence that explains a question lost
    // its end — and on an ISO's console the end is often where the reason
    // is. Counted by the wrapper that draws it (`line_count`), never by
    // characters ÷ width, which under-counts a line wrapped at its words.
    let (said, _) = said_of(e, texts);
    let rows = Paragraph::new(said)
        .wrap(Wrap { trim: true })
        .line_count(area.width.max(1));
    let most = (area.height / 3).max(1);
    let help_rows = u16::try_from(rows).unwrap_or(u16::MAX).clamp(1, most);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),             // title + label
            Constraint::Min(1),                // the field, or the list, or the output
            Constraint::Length(help_rows + 1), // help + keys — the usual two lines
        ])
        .split(area);
    let footer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(help_rows), Constraint::Length(1)])
        .split(areas[2]);
    let key_line = keys_of(e, texts);
    Places {
        head: areas[0],
        body: areas[1],
        help: footer[0],
        keys: footer[1],
        hints: hints_of(&key_line, footer[1].x),
        key_line,
        list: list_rows(e, areas[1]),
    }
}

/// Draws the screen.
fn draw(f: &mut ratatui::Frame, e: &State, texts: &Texts) {
    let base = Style::default()
        .fg(color(color::TEXT_PRIMARY))
        .bg(color(color::SURFACE_BASE));
    f.render_widget(ratatui::widgets::Block::default().style(base), f.area());
    let at = places(f.area(), e, texts);

    let title = match e.screen {
        Screen::Asking(_) | Screen::Hopping(_) => texts.t(key::INST_TITLE),
        Screen::Reviewing => texts.t(key::INST_REVIEW),
        Screen::Running => texts.t(key::INST_RUNNING),
        Screen::Finished(true) => texts.t(key::INST_DONE),
        Screen::Finished(false) => texts.t(key::INST_FAILED),
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                title,
                Style::default()
                    .fg(color(color::STATE_ACCENT))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                label_of(e, texts),
                Style::default().fg(color(color::TEXT_PRIMARY)),
            )),
        ])
        .style(base),
        at.head,
    );

    body(f, e, texts, at.body, base, at.list);

    let dim = Style::default().fg(color(color::TEXT_DIM));
    let (said, refused) = said_of(e, texts);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            said,
            if refused {
                Style::default().fg(color(color::STATE_ERROR))
            } else {
                dim
            },
        )))
        .style(base)
        .wrap(Wrap { trim: true }),
        at.help,
    );
    // EVERY HINT IS A CLICK, drawn where `places` says it is; the one the
    // pointer rests on is lit. A hint that names no clickable key stays
    // as it is, and what lies between two hints is the language's text.
    let lit = Style::default().fg(color(color::TEXT_SUBTLE_LIT_KEY));
    let mut spans = Vec::new();
    let mut drawn = 0;
    for (i, hint) in at.hints.iter().enumerate() {
        if let Some(between) = at.key_line.get(drawn..hint.at).filter(|b| !b.is_empty()) {
            spans.push(Span::styled(between.to_owned(), dim));
        }
        let style = if hint.act.is_some() && e.hover == Some(Hit::Key(i)) {
            lit
        } else {
            dim
        };
        spans.push(Span::styled(hint.text.clone(), style));
        drawn = hint.at + hint.text.len();
    }
    if let Some(rest) = at.key_line.get(drawn..).filter(|r| !r.is_empty()) {
        spans.push(Span::styled(rest.to_owned(), dim));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(base), at.keys);
}

/// The line that says what a check is doing, or what it found.
fn status_line(e: &State, texts: &Texts) -> Option<Line<'static>> {
    let k = e.status?;
    let tone = if k == key::INST_ACCESS_READY {
        color::STATE_SUCCESS
    } else {
        color::TEXT_SECONDARY
    };
    Some(Line::from(Span::styled(
        texts.t(k),
        Style::default().fg(color(tone)),
    )))
}

/// The middle part, which changes with the screen. `list`: where
/// [`places`] put the list, when the screen is one.
fn body(
    f: &mut ratatui::Frame,
    e: &State,
    texts: &Texts,
    area: Rect,
    base: Style,
    list: Option<Rows>,
) {
    match &e.screen {
        Screen::Asking(_)
            if e.question_of() == Some(Question::BelongingsAccess) && !e.by_token() =>
        {
            access_body(f, e, texts, area, base);
        }
        Screen::Asking(_) if e.is_list() => {
            let Some(rows) = list else {
                f.render_widget(
                    Paragraph::new(texts.t(key::INST_NO_DISK))
                        .style(base.fg(color(color::STATE_ERROR))),
                    area,
                );
                return;
            };
            let (_, status_area) = split_status(area, e.status.is_some());
            // "NO WI-FI" IS AN ANSWER, and it is drawn as one: its value is
            // empty, and an empty line at the top of the list read as no
            // option at all.
            let wifi = e.question_of() == Some(Question::Wifi);
            // THE ROW UNDER THE POINTER is lit, and it is not the cursor:
            // the selection keeps its own band.
            let hovering = Style::default().bg(color(color::SURFACE_HOVER));
            let items: Vec<ListItem> = e
                .options()
                .into_iter()
                .enumerate()
                .map(|(i, o)| {
                    let item = if wifi && o.is_empty() {
                        ListItem::new(texts.t(key::INST_WIFI_NONE))
                    } else {
                        ListItem::new(o)
                    };
                    if e.hover == Some(Hit::Row(i)) && i != e.choice {
                        item.style(hovering)
                    } else {
                        item
                    }
                })
                .collect();
            // THE OFFSET IS `places`'s, so the row a click hits is the row
            // drawn there.
            let mut st = ListState::default()
                .with_offset(rows.offset)
                .with_selected(Some(e.choice));
            f.render_stateful_widget(
                List::new(items).style(base).highlight_style(
                    Style::default()
                        .fg(color(color::TEXT_PRIMARY))
                        .bg(color(color::SURFACE_SELECTION)),
                ),
                rows.area,
                &mut st,
            );
            if let (Some(line), Some(at)) = (status_line(e, texts), status_area) {
                f.render_widget(Paragraph::new(line).style(base), at);
            }
        }
        Screen::Asking(_) => {
            let q = e.question_of();
            // A secret does not appear on screen — neither for whoever is looking over
            // the shoulder, nor in a screenshot sent to ask for help.
            let shown = if q.is_some_and(Question::secret_one) {
                "*".repeat(e.typing.chars().count())
            } else {
                e.typing.clone()
            };
            let mut lines = vec![Line::from(format!("{shown}_"))];
            if q == Some(Question::BelongingsAccess)
                && let Some(page) = access::token_page(&e.answers.belongings)
            {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    texts.t_with(key::INST_BELONGINGS_TOKEN_WHERE, &[("address", &page)]),
                    Style::default().fg(color(color::TEXT_SECONDARY)),
                )));
            }
            if let Some(line) = status_line(e, texts) {
                lines.push(Line::from(""));
                lines.push(line);
            }
            f.render_widget(
                Paragraph::new(lines).style(base).wrap(Wrap { trim: false }),
                area,
            );
        }
        Screen::Hopping(hop) => {
            f.render_widget(
                Paragraph::new(texts.t_with(hop.key(), &[("rev", hop.short())]))
                    .style(base)
                    .wrap(Wrap { trim: true }),
                area,
            );
        }
        Screen::Reviewing => {
            let mut lines: Vec<Line> = Vec::new();
            if let Some(said) = e.stay.and_then(Stay::key) {
                lines.push(Line::from(Span::styled(
                    texts.t(said),
                    Style::default().fg(color(color::STATE_WARNING)),
                )));
                lines.push(Line::from(""));
            }
            for (i, p) in e.build_plan(&review_origin()).iter().enumerate() {
                let mark = if p.destructive { "!" } else { " " };
                let style = if p.destructive {
                    Style::default().fg(color(color::STATE_CRITICAL))
                } else {
                    Style::default().fg(color(color::TEXT_SECONDARY))
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark} {}. {}", i + 1, texts.t(p.label)),
                    style,
                )));
            }
            // THE CONFIRMATION IS PINNED to the bottom: the plan grew to two
            // dozen steps, and on a console of thirty rows a confirmation
            // drawn after them fell off the screen — the one line the
            // person has to read.
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(area);
            f.render_widget(
                Paragraph::new(lines).style(base).wrap(Wrap { trim: false }),
                parts[0],
            );
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("{}: {}", texts.t(key::INST_CONFIRM), e.answers.disk),
                        Style::default().fg(color(color::STATE_WARNING)),
                    )),
                    Line::from(Span::styled(
                        format!("{}_", e.confirmation),
                        Style::default().fg(color(color::TEXT_PRIMARY)),
                    )),
                ])
                .style(base),
                parts[1],
            );
        }
        Screen::Running | Screen::Finished(_) => {
            let mut lines: Vec<Line> = Vec::new();
            if let Some((i, total, label)) = e.step {
                lines.push(Line::from(Span::styled(
                    format!("{}/{total}  {}", i + 1, texts.t(label)),
                    Style::default()
                        .fg(color(color::STATE_ACCENT))
                        .add_modifier(Modifier::BOLD),
                )));
            }
            // Only the end of the output: `nixos-install` prints thousands of lines,
            // and what matters is always the last one.
            let fit_in = usize::from(area.height).saturating_sub(lines.len()).max(1);
            let start = e.output.len().saturating_sub(fit_in);
            for l in &e.output[start..] {
                lines.push(Line::from(Span::styled(
                    l.clone(),
                    Style::default().fg(color(color::TEXT_SECONDARY)),
                )));
            }
            f.render_widget(Paragraph::new(lines).style(base), area);
        }
    }
}

/// The review's origin: only whether the flake is aimed matters to the
/// labels, and that comes from the state.
fn review_origin() -> Origin {
    Origin {
        distro: String::new(),
        aimed: false,
        nix: String::new(),
    }
}

/// `area` with its last line kept for the status, when there is one.
fn split_status(area: Rect, status: bool) -> (Rect, Option<Rect>) {
    if !status || area.height < 3 {
        return (area, None);
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);
    (
        rows[0],
        Some(Rect {
            y: rows[1].y + 1,
            height: 1,
            ..rows[1]
        }),
    )
}

/// The access screen of a deploy key: what to do, the key, where to add
/// it, what the repository says of it so far, and the key as a QR code
/// when the screen holds one.
fn access_body(f: &mut ratatui::Frame, e: &State, texts: &Texts, area: Rect, base: Style) {
    let secondary = Style::default().fg(color(color::TEXT_SECONDARY));
    let mut lines = vec![Line::from(texts.t(key::INST_BELONGINGS_KEY_INTRO))];
    if let Some(public) = &e.public_key {
        lines.push(Line::from(Span::styled(
            public.clone(),
            Style::default().fg(color(color::TEXT_ANSWER)),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        match access::add_key_page(&e.answers.belongings) {
            Some(page) => texts.t_with(key::INST_BELONGINGS_KEY_WHERE, &[("address", &page)]),
            None => texts.t(key::INST_BELONGINGS_KEY_WHERE_UNKNOWN),
        },
        secondary,
    )));
    // THE TEXT FIRST, as many rows as it wraps to — counted by the wrapper
    // that draws it: a key of 80 characters wraps at its words into three
    // rows where characters ÷ width says two.
    let width = area.width.max(1);
    let text = Paragraph::new(lines).style(base).wrap(Wrap { trim: false });
    let text_count = u16::try_from(text.line_count(width)).unwrap_or(u16::MAX);
    // THE STATUS ON ROWS OF ITS OWN, kept whatever the text takes: what the
    // repository says of the key is the line the person waits for.
    let status =
        status_line(e, texts).map(|l| Paragraph::new(l).style(base).wrap(Wrap { trim: false }));
    let status_rows = status.as_ref().map_or(0, |p| {
        u16::try_from(p.line_count(width)).unwrap_or(u16::MAX)
    });
    let reserved = if status_rows > 0 { status_rows + 1 } else { 0 };
    let text_rows = text_count.min(area.height.saturating_sub(reserved));
    f.render_widget(
        text,
        Rect {
            height: text_rows,
            ..area
        },
    );
    if let Some(said) = status {
        let y = area.y + text_rows + 1;
        f.render_widget(
            said,
            Rect {
                y,
                height: status_rows.min((area.y + area.height).saturating_sub(y)),
                ..area
            },
        );
    }
    let used = text_rows + reserved;
    if let Some(qr) = e.public_key.as_deref().and_then(Qr::of) {
        let below = Rect {
            y: area.y + used + 1,
            height: area.height.saturating_sub(used + 1),
            ..area
        };
        f.render_widget(QrBlock { qr: &qr }, below);
    }
}

/// A QR code drawn with half blocks: each cell two modules, the upper one
/// in the foreground and the lower in the background — so the code is
/// square on a terminal whose cells are twice as tall as wide. The glyph is
/// `ratatui`'s own (`symbols::half_block`), and the two colours tokens: the
/// light the screen's text, the dark its background, as the clipboard's QR.
struct QrBlock<'a> {
    qr: &'a Qr,
}

impl Widget for QrBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let side = self.qr.side + 2 * QUIET;
        let rows = side.div_ceil(2);
        // A CODE THAT DOES NOT FIT IS NOT DRAWN: a cut one reads as nothing
        // to a camera, and the key is on the screen as text anyway.
        if usize::from(area.width) < side || usize::from(area.height) < rows {
            return;
        }
        let light = color(color::TEXT_PRIMARY);
        let dark = color(color::SURFACE_BASE);
        let quiet = isize::try_from(QUIET).unwrap_or_default();
        for r in 0..rows {
            for c in 0..side {
                let (Ok(x), Ok(y)) = (isize::try_from(c), isize::try_from(2 * r)) else {
                    continue;
                };
                let (mx, my) = (x - quiet, y - quiet);
                let top = if self.qr.is_dark(mx, my) { dark } else { light };
                let bottom = if self.qr.is_dark(mx, my + 1) {
                    dark
                } else {
                    light
                };
                let (Ok(cx), Ok(cy)) = (u16::try_from(c), u16::try_from(r)) else {
                    continue;
                };
                if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                    cell.set_char(half_block::UPPER).set_fg(top).set_bg(bottom);
                }
            }
        }
    }
}

/// It keeps the terminal, so `Drop` gives it back as it was.
///
/// Without this, a `panic!` in the middle of drawing leaves the terminal in raw mode and
/// on the alternate screen: the person is left with a shell that does not echo what they
/// type, and the only way out is closing the window. An installer cannot be the
/// thing that breaks the terminal of whoever tried to use it.
struct Guard(Terminal<CrosstermBackend<Stdout>>);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            self.0.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.0.show_cursor();
    }
}

/// What the TUI collected.
#[derive(Debug)]
pub enum Collected {
    /// The person gave up; nothing was touched.
    GaveUp,
    /// The answers, confirmed — and why the installer stays with its own
    /// distro, when the release said so.
    Answers(Box<Answers>, Option<Stay>),
    /// The person starts the release's installer, with these answers.
    Hop(Hop, Box<Answers>),
}

/// Runs the TUI. Returns the answers once the person confirms.
///
/// `resume`: the answers a hop handed on, and where to pick up.
///
/// # Errors
///
/// The terminal did not accept raw mode, or reading a key failed.
pub fn collect(
    initial: &Texts,
    reach: &Reach,
    resume: Option<(Answers, Question)>,
) -> Result<Collected> {
    // WHAT TAKES A MOMENT, before the screen is taken: the disks, the
    // networks in range, and the hosts the network question names (`nix`
    // answers those).
    let mut e = match resume {
        Some((answers, at)) => State::resumed(disks(), networks(), answers, at),
        None => State::fresh(disks(), networks()),
    };
    e.hosts = reach.hosts().join(", ");

    enable_raw_mode().context("raw mode")?;
    let mut output = std::io::stdout();
    // THE POINTER TOO (invariant 19): a terminal over the ISO's `sshd` sends
    // mouse events, and a console without a mouse sends none.
    crossterm::execute!(output, EnterAlternateScreen, EnableMouseCapture)
        .context("alternate screen")?;
    let mut g = Guard(Terminal::new(CrosstermBackend::new(output)).context("terminal")?);
    // THE JOBS RUN ON THREADS, one at a time as the screens ask them, and
    // their answers come back through this channel: the loop below keeps
    // drawing and reading keys while the radio and the remote take seconds.
    let (tx, rx) = std::sync::mpsc::channel::<(u64, Done)>();
    // THE TEXTS FOLLOW THE ANSWER, and that is the whole point of asking the
    // language first: they are reloaded the moment the first answer exists.
    let mut chosen: Option<Texts> = None;
    let mut showing = String::new();
    loop {
        if let Some(asked) = e.asked.take() {
            let (tx, reach) = (tx.clone(), reach.clone());
            std::thread::spawn(move || {
                let done = reach.run(&asked.job);
                let _ = tx.send((asked.serial, done));
            });
        }
        while let Ok((serial, done)) = rx.try_recv() {
            e.arrived(serial, done);
        }
        if e.answers.language != showing && !e.answers.language.is_empty() {
            if let Some(t) = Texts::fresh(&e.answers.language) {
                chosen = Some(t);
            }
            showing = e.answers.language.clone();
        }
        let texts = chosen.as_ref().unwrap_or(initial);
        // WHERE EVERYTHING IS, kept before it is drawn: the list scrolls
        // from where it was, and the pointer reads this same answer.
        let size = g.0.size().context("the terminal's size")?;
        let at = places(Rect::new(0, 0, size.width, size.height), &e, texts);
        e.drawn(&at);
        g.0.draw(|f| draw(f, &e, texts)).context("draw")?;
        if !event::poll(Duration::from_millis(100)).context("poll")? {
            continue;
        }
        let next = match event::read().context("read a key")? {
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            }) => {
                // `Release` also arrives in a terminal with the kitty keyboard protocol.
                // Without this filter every key counts twice.
                if kind != KeyEventKind::Press {
                    continue;
                }
                if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
                    return Ok(Collected::GaveUp);
                }
                e.key(code)
            }
            Event::Mouse(MouseEvent {
                kind, column, row, ..
            }) => {
                let Some(p) = pointer_of(kind) else {
                    continue;
                };
                // READ AGAINST THE SCREEN AS IT IS DRAWN NOW: the same
                // `places` the drawing above painted from.
                e.pointer(p, column, row, &at)
            }
            _ => continue,
        };
        match next {
            Next::Stay => {}
            Next::Run => return Ok(Collected::Answers(Box::new(e.answers), e.stay)),
            Next::Hop(hop) => return Ok(Collected::Hop(hop, Box::new(e.answers))),
            Next::GiveUp => return Ok(Collected::GaveUp),
        }
    }
}

/// What a mouse event is to the TUI; `None` for one it does not read (a
/// button let go, the middle and right buttons).
fn pointer_of(kind: MouseEventKind) -> Option<Pointer> {
    match kind {
        MouseEventKind::Down(MouseButton::Left) => Some(Pointer::Click),
        MouseEventKind::Moved | MouseEventKind::Drag(_) => Some(Pointer::Moved),
        MouseEventKind::ScrollUp => Some(Pointer::WheelUp),
        MouseEventKind::ScrollDown => Some(Pointer::WheelDown),
        _ => None,
    }
}

/// One of the screens that check the network or the repository, drawn from
/// a fixture as the TUI draws it — `ronoctua-install --show <screen>`;
/// `None` for a name that is not one.
#[must_use]
pub fn show(screen: &str, texts: &Texts) -> Option<String> {
    let mut e = State::fresh(Vec::new(), vec!["home-5g".into(), "home-2g".into()]);
    e.hosts = "github.com, cache.nixos.org".into();
    let at = |e: &mut State, q: Question| {
        e.screen = Screen::Asking(
            Question::ALL_KEYS
                .iter()
                .position(|x| *x == q)
                .unwrap_or_default(),
        );
        e.choice = e.default_choice();
    };
    e.answers.set(Question::Disk, "/dev/nvme0n1".into());
    e.answers.set(Question::Hostname, "noctua-laptop".into());
    match screen {
        "network" => {
            at(&mut e, Question::Wifi);
            e.error = Some(Gate::NoLink.key());
        }
        "access" => {
            e.answers.set(
                Question::BelongingsAddress,
                "git@github.com:someone/belongings.git".into(),
            );
            e.public_key = Some(
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK7mX2pDqfQ1n0Ue9Wc4Vt3yRg8LhJzS5oP6bN2aYf0e ronoctua-belongings"
                    .into(),
            );
            at(&mut e, Question::BelongingsAccess);
            e.probed = Some(Probed {
                read: true,
                write: false,
                empty: false,
                why: None,
            });
            e.status = Some(key::INST_ACCESS_READS);
        }
        "token" => {
            e.answers.set(
                Question::BelongingsAddress,
                "https://github.com/someone/belongings.git".into(),
            );
            at(&mut e, Question::BelongingsAccess);
            e.typing = "github_pat_0123456789".into();
        }
        "passphrase" => {
            e.answers.set(
                Question::BelongingsAddress,
                "git@github.com:someone/belongings.git".into(),
            );
            e.answers.belongings_empty = Some(false);
            at(&mut e, Question::BelongingsPassphrase);
            e.typing = "secret".into();
            e.error = Some(access::refusal_key(
                ronoctua_belongings::Why::WrongPassphrase,
            ));
        }
        "passphrase-new" => {
            e.answers.belongings_empty = Some(true);
            at(&mut e, Question::BelongingsPassphrase);
            e.first_passphrase = Some("Qw4-a-long-passphrase".into());
            e.typing = "Qw4-a-long".into();
        }
        "hop" => {
            e.screen = Screen::Hopping(Hop {
                release: crate::release::Release {
                    revision: crate::release::Revision {
                        rev: "4f1c2a9b7e3d5f60a1b2c3d4e5f60718293a4b5c".into(),
                        last_modified_s: 0,
                    },
                    url: String::new(),
                    path: String::new(),
                },
            });
        }
        "review" => {
            e.answers.set(
                Question::BelongingsAddress,
                "git@github.com:someone/belongings.git".into(),
            );
            e.answers.belongings_empty = Some(false);
            e.stay = Some(Stay::Unreachable);
            e.screen = Screen::Reviewing;
        }
        _ => return None,
    }
    Some(rendered(&e, texts, 100, 34))
}

/// The screen as text, at `width × height` — what the tests look at, and
/// what `ronoctua-install --show <screen>` prints for a person to look at
/// without a disk to install on.
#[must_use]
pub fn rendered(e: &State, texts: &Texts, width: u16, height: u16) -> String {
    let Ok(mut terminal) = Terminal::new(ratatui::backend::TestBackend::new(width, height)) else {
        return String::new();
    };
    if terminal.draw(|f| draw(f, e, texts)).is_err() {
        return String::new();
    }
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(
                buffer
                    .cell((x, y))
                    .map_or(" ", ratatui::buffer::Cell::symbol),
            );
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::{Release, Revision};
    use ronoctua_belongings::Why;

    fn texts() -> Texts {
        Texts::fresh("pt-BR").expect("pt-BR has to load")
    }

    fn with_disks() -> State {
        State::fresh(
            vec![
                Disk {
                    path: "/dev/nvme0n1".into(),
                    size: "931,5G".into(),
                    model: "Samsung SSD 980 PRO".into(),
                },
                Disk {
                    path: "/dev/sda".into(),
                    size: "1,8T".into(),
                    model: String::new(),
                },
            ],
            vec!["home-5g".into(), "home-2g".into()],
        )
    }

    /// A world that answers every job at once: the way out is open, the
    /// release says stay, a key is made, the access reads and writes, the
    /// passphrase opens.
    fn kind(job: &Job) -> Done {
        match job {
            Job::Network { .. } => Done::Network(Gate::Open),
            Job::Release => Done::Release(Follow::Stay(Stay::Current)),
            Job::Prepare { address } => Done::Prepared(Ok((access::auth_of(address) == Auth::Ssh)
                .then(|| {
                    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITestKey ronoctua-belongings".to_owned()
                }))),
            Job::Probe { .. } => Done::Probed(Ok(Probed {
                read: true,
                write: true,
                empty: false,
                why: None,
            })),
            Job::Verify { .. } => Done::Verified(Ok(())),
        }
    }

    /// Answers every job the screen asked, with `world`, until it asks none.
    fn settle(e: &mut State, world: &dyn Fn(&Job) -> Done) {
        for _ in 0..16 {
            let Some(asked) = e.asked.take() else {
                return;
            };
            e.arrived(asked.serial, world(&asked.job));
        }
        panic!("the screen keeps asking");
    }

    fn type_in(e: &mut State, text: &str) {
        for c in text.chars() {
            e.key(KeyCode::Char(c));
        }
    }

    #[test]
    fn lsblk_with_a_multi_word_model() {
        // `Samsung SSD 980 PRO` has spaces. A parser by positional field
        // would return `Samsung` and throw the rest away.
        let d = disks_from(
            "/dev/nvme0n1 931,5G Samsung SSD 980 PRO\n\
             /dev/sda       1,8T\n",
        );
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].model, "Samsung SSD 980 PRO");
        assert_eq!(d[1].path, "/dev/sda");
        assert!(d[1].model.is_empty());
    }

    #[test]
    fn lsblk_ignores_what_is_not_a_device() {
        assert!(disks_from("NAME SIZE MODEL\nloop0 100M\n").is_empty());
    }

    /// Walks every screen with `world` answering the checks; what the walk
    /// saw.
    fn walk(e: &mut State, world: &dyn Fn(&Job) -> Done, wifi: bool) -> Vec<Question> {
        let mut views = Vec::new();
        // BOUNDED: a screen that refuses the answer never advances, and an
        // unbounded loop would spin instead of failing.
        let limit = Question::ALL_KEYS.len() * 4;
        while let Some(q) = e.question_of() {
            assert!(
                views.len() < limit,
                "the screen {q:?} does not advance: {views:?}"
            );
            views.push(q);
            match q {
                Question::Disk
                | Question::Language
                | Question::KeyboardVariant
                | Question::ComposeKey
                | Question::SystemLocale
                | Question::Timezone
                | Question::Profile => {
                    e.key(KeyCode::Enter);
                }
                // THE LAYOUT MOVES OFF THE DEFAULT ON PURPOSE: a list that
                // ignored the cursor would pass otherwise.
                Question::Keyboard => {
                    e.key(KeyCode::Down);
                    e.key(KeyCode::Enter);
                }
                Question::Hostname => {
                    type_in(e, "noctua-teste");
                    e.key(KeyCode::Enter);
                }
                Question::Password | Question::Passphrase | Question::WifiPassword => {
                    type_in(e, "Zq7-test-phrase");
                    e.key(KeyCode::Enter);
                }
                Question::Wifi => {
                    if wifi {
                        e.key(KeyCode::Down);
                    }
                    e.key(KeyCode::Enter);
                }
                Question::BelongingsAddress => {
                    type_in(e, "git@example.org:me/b.git");
                    e.key(KeyCode::Enter);
                }
                // A DEPLOY KEY is shown, not typed: Enter once it reads
                // and writes.
                Question::BelongingsAccess => {
                    type_in(e, "nothing typed counts here");
                    assert!(e.typing.is_empty(), "the key's screen took typing");
                    e.key(KeyCode::Enter);
                }
                Question::BelongingsPassphrase => {
                    type_in(e, "Qw4-belongings-phrase");
                    e.key(KeyCode::Enter);
                }
                Question::ResumeOffset => panic!("the TUI does not ask for the offset"),
            }
            settle(e, world);
        }
        views
    }

    #[test]
    fn the_questions_are_walked_and_only_them() {
        // Milestone 8's literal criterion. If a question enters the enum and the
        // TUI does not show it, or if it shows something outside the enum, this is where
        // it appears.
        let mut e = with_disks();
        let views = walk(&mut e, &kind, false);
        assert_eq!(e.screen, Screen::Reviewing);
        // FIFTEEN screens for whoever goes by cable: the seventeen questions
        // minus the offset, which is computed, and the Wi-Fi password, which
        // is skipped with no network.
        assert_eq!(views.len(), 15, "the TUI showed {views:?}");
        assert!(!views.contains(&Question::ResumeOffset));
        assert!(!views.contains(&Question::WifiPassword));
        assert_eq!(e.answers.disk, "/dev/nvme0n1");
        assert_eq!(e.answers.hostname, "noctua-teste");
        assert_eq!(e.answers.language, ronoctua_i18n::LANGUAGES[0]);
        assert_ne!(e.answers.keyboard, crate::model::DEFAULT_LAYOUT);
        assert!(crate::model::layout_names().contains(&e.answers.keyboard));
        assert!(e.answers.wifi_ssid.is_empty());
        assert_eq!(e.answers.belongings, "git@example.org:me/b.git");
        assert!(
            e.answers
                .belongings_access
                .ends_with("/belongings-keys/deploy")
        );
        assert_eq!(e.answers.belongings_empty, Some(false));
        assert_eq!(e.answers.belongings_passphrase, "Qw4-belongings-phrase");
        assert_eq!(e.stay, Some(Stay::Current));
    }

    #[test]
    fn choosing_a_network_makes_the_password_screen_appear() {
        // Without it, `skipped_one` could be always true and the password
        // would never be asked for.
        let mut e = with_disks();
        let views = walk(&mut e, &kind, true);
        assert_eq!(e.screen, Screen::Reviewing);
        assert!(views.contains(&Question::WifiPassword));
        assert_eq!(e.answers.wifi_ssid, "home-5g");
        assert_eq!(e.answers.wifi_password, "Zq7-test-phrase");
    }

    /// Walks to `wanted`, taking the pre-selected answer on every list before it.
    fn walk_to(e: &mut State, wanted: Question) {
        for _ in 0..Question::ALL_KEYS.len() * 2 {
            match e.question_of() {
                Some(q) if q == wanted => return,
                Some(Question::Hostname) => {
                    type_in(e, "noctua-teste");
                    e.key(KeyCode::Enter);
                }
                Some(Question::Password | Question::Passphrase) => {
                    type_in(e, "Zq7-test-phrase");
                    e.key(KeyCode::Enter);
                }
                Some(Question::BelongingsAddress) => {
                    type_in(e, "git@example.org:me/b.git");
                    e.key(KeyCode::Enter);
                }
                Some(_) => {
                    e.key(KeyCode::Enter);
                }
                None => break,
            }
            settle(e, &kind);
        }
        assert_eq!(e.question_of(), Some(wanted), "I did not reach {wanted:?}");
    }

    /// FAILS IF: the network question lets the person on with no way out,
    /// or does not say why on the screen they can fix it from.
    #[test]
    fn the_network_is_a_gate() {
        let mut e = with_disks();
        walk_to(&mut e, Question::Wifi);
        e.key(KeyCode::Enter);
        let asked = e.asked.take().expect("the gate was not asked");
        assert!(matches!(asked.job, Job::Network { ref ssid, .. } if ssid.is_empty()));
        e.arrived(asked.serial, Done::Network(Gate::NoLink));
        assert_eq!(
            e.question_of(),
            Some(Question::Wifi),
            "it went on with no network"
        );
        assert_eq!(e.error, Some(key::INST_GATE_NO_LINK));
        // While a check runs, nothing but Esc counts.
        e.key(KeyCode::Enter);
        let again = e.asked.take().unwrap();
        e.key(KeyCode::Enter);
        assert!(e.asked.is_none(), "an Enter asked twice");
        e.arrived(again.serial, Done::Network(Gate::Open));
        let release = e.asked.take().expect("the release was not asked");
        assert_eq!(release.job, Job::Release);
        e.arrived(
            release.serial,
            Done::Release(Follow::Stay(Stay::Unreachable)),
        );
        assert_eq!(e.question_of(), Some(Question::BelongingsAddress));
        assert_eq!(e.stay, Some(Stay::Unreachable));
        assert!(
            e.origin(&review_origin()).aimed,
            "an unreachable release aims at the ISO"
        );
    }

    /// FAILS IF: a newer release is not offered as a hop, or the hop leaves
    /// without the answers.
    #[test]
    fn a_newer_release_takes_over_with_the_answers() {
        let mut e = with_disks();
        walk_to(&mut e, Question::Wifi);
        e.key(KeyCode::Enter);
        settle(&mut e, &|job| match job {
            Job::Release => Done::Release(Follow::Hop(Hop {
                release: Release {
                    revision: Revision {
                        rev: "2".repeat(40),
                        last_modified_s: 2,
                    },
                    url: "github:o/r/x".into(),
                    path: "/nix/store/x-source".into(),
                },
            })),
            other => kind(other),
        });
        let Screen::Hopping(hop) = e.screen.clone() else {
            panic!("no hop: {:?}", e.screen);
        };
        let shown = rendered(&e, &texts(), 100, 20);
        assert!(shown.contains(&"2".repeat(12)), "{shown}");
        assert_eq!(e.key(KeyCode::Enter), Next::Hop(hop));
        // Esc from the hop goes back to the network.
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Wifi));
    }

    /// FAILS IF: the access screen goes on before the key both reads and
    /// writes, stops asking while the person adds it, or keeps asking
    /// after it works.
    #[test]
    fn the_access_is_probed_until_it_reads_and_writes() {
        let mut e = with_disks();
        walk_to(&mut e, Question::BelongingsAddress);
        type_in(&mut e, "git@github.com:me/belongings.git");
        e.key(KeyCode::Enter);
        let prepare = e.asked.take().expect("nothing was prepared");
        assert!(matches!(prepare.job, Job::Prepare { .. }));
        e.arrived(prepare.serial, kind(&prepare.job));
        assert_eq!(e.question_of(), Some(Question::BelongingsAccess));
        // ASKED AT ONCE, and again after a pause while the key is not there.
        let first = e.asked.take().expect("the key was not probed");
        assert!(matches!(
            first.job,
            Job::Probe {
                wait_s: 0,
                token: None,
                ..
            }
        ));
        e.arrived(first.serial, Done::Probed(Ok(Probed::default())));
        assert_eq!(e.status, Some(key::INST_ACCESS_WAITING));
        let shown = rendered(&e, &texts(), 100, 40);
        assert!(shown.contains("ssh-ed25519"), "{shown}");
        assert!(
            shown.contains("https://github.com/me/belongings/settings/keys/new"),
            "{shown}"
        );
        assert!(
            shown.contains(half_block::UPPER),
            "no QR code on a screen that holds one:\n{shown}"
        );
        let watching = e.asked.take().expect("the screen stopped asking");
        assert!(matches!(watching.job, Job::Probe { wait_s, .. } if wait_s == PROBE_AGAIN_S));
        // While it watches, Enter does not go on.
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::BelongingsAccess));
        // It reads, and does not write yet: said, and asked again.
        e.arrived(
            watching.serial,
            Done::Probed(Ok(Probed {
                read: true,
                write: false,
                empty: false,
                why: Some(Why::Refused),
            })),
        );
        assert_eq!(e.status, Some(key::INST_ACCESS_READS));
        let again = e.asked.take().expect("the screen stopped asking");
        // READS AND WRITES: said, no more asking, and Enter goes on.
        e.arrived(
            again.serial,
            Done::Probed(Ok(Probed {
                read: true,
                write: true,
                empty: true,
                why: None,
            })),
        );
        assert_eq!(e.status, Some(key::INST_ACCESS_READY));
        assert!(e.asked.is_none(), "it kept asking a key that works");
        assert_eq!(e.question_of(), Some(Question::BelongingsAccess));
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
        assert_eq!(e.answers.belongings_empty, Some(true));
        // A late answer to a job the screen left is dropped.
        e.arrived(again.serial, Done::Probed(Ok(Probed::default())));
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
    }

    /// FAILS IF: a token is taken without being probed, kept on screen, or
    /// a refused one lets the person on.
    #[test]
    fn a_token_is_typed_hidden_and_probed() {
        let mut e = with_disks();
        walk_to(&mut e, Question::BelongingsAddress);
        type_in(&mut e, "https://github.com/me/belongings.git");
        e.key(KeyCode::Enter);
        settle(&mut e, &kind);
        assert_eq!(e.question_of(), Some(Question::BelongingsAccess));
        assert!(e.by_token() && e.types());
        type_in(&mut e, "Zq7-token");
        let shown = rendered(&e, &texts(), 100, 20);
        assert!(!shown.contains("Zq7-token"), "the token is on screen");
        e.key(KeyCode::Enter);
        let asked = e.asked.take().unwrap();
        assert!(matches!(asked.job, Job::Probe { token: Some(ref t), .. } if t == "Zq7-token"));
        e.arrived(asked.serial, Done::Probed(Err(Why::Refused)));
        assert_eq!(e.question_of(), Some(Question::BelongingsAccess));
        assert_eq!(e.error, Some(access::refusal_key(Why::Refused)));
        assert!(e.typing.is_empty());
        type_in(&mut e, "Zq7-token-2");
        e.key(KeyCode::Enter);
        settle(&mut e, &kind);
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
        assert!(
            e.answers
                .belongings_access
                .ends_with("/belongings-keys/token")
        );
    }

    /// FAILS IF: a passphrase that does not open the repository is taken,
    /// or one for an EMPTY repository is taken without being typed twice
    /// alike and as long as a disk's.
    #[test]
    fn the_passphrase_opens_the_repository_or_is_typed_twice() {
        let mut e = with_disks();
        walk_to(&mut e, Question::BelongingsPassphrase);
        type_in(&mut e, "wrong one");
        e.key(KeyCode::Enter);
        let asked = e.asked.take().unwrap();
        assert!(matches!(asked.job, Job::Verify { .. }));
        e.arrived(asked.serial, Done::Verified(Err(Why::WrongPassphrase)));
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
        assert_eq!(e.error, Some(key::BELONGINGS_REFUSED_WRONG_PASSPHRASE));

        // AN EMPTY REPOSITORY.
        let mut e = with_disks();
        walk_to(&mut e, Question::BelongingsPassphrase);
        e.answers.belongings_empty = Some(true);
        type_in(&mut e, "short");
        e.key(KeyCode::Enter);
        assert_eq!(e.error, Some(key::INST_INVALID_PASSPHRASE_SHORT));
        e.typing.clear();
        type_in(&mut e, "Qw4-a-long-passphrase");
        e.key(KeyCode::Enter);
        assert!(e.asked.is_none(), "an empty repository has nothing to open");
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
        let shown = rendered(&e, &texts(), 100, 20);
        assert!(shown.contains(&texts().t(key::INST_BELONGINGS_PASSPHRASE_AGAIN)));
        type_in(&mut e, "Qw4-another-passphrase");
        e.key(KeyCode::Enter);
        assert_eq!(e.error, Some(key::INST_BELONGINGS_PASSPHRASE_DIFFERS));
        type_in(&mut e, "Qw4-a-long-passphrase");
        e.key(KeyCode::Enter);
        type_in(&mut e, "Qw4-a-long-passphrase");
        e.key(KeyCode::Enter);
        assert_eq!(e.screen, Screen::Reviewing);
        assert_eq!(e.answers.belongings_passphrase, "Qw4-a-long-passphrase");
    }

    /// FAILS IF: the release's installer does not pick up where the hop
    /// left, or asks the network again.
    #[test]
    fn the_release_installer_resumes_at_the_repository() {
        let mut answers = Answers::default();
        answers.set(Question::Language, "en-GB".into());
        answers.set(Question::Disk, "/dev/sda".into());
        let e = State::resumed(Vec::new(), Vec::new(), answers, Question::BelongingsAddress);
        assert_eq!(e.question_of(), Some(Question::BelongingsAddress));
        assert_eq!(e.answers.disk, "/dev/sda");
        assert_eq!(e.stay, Some(Stay::FromRelease));
    }

    /// FAILS IF: going back to the network question in the release's
    /// installer loses the network chosen before the hop — a list without
    /// it lands on "no Wi-Fi", and Enter installs a machine with no Wi-Fi.
    #[test]
    fn the_network_chosen_before_the_hop_is_still_offered() {
        let mut answers = Answers::default();
        answers.set(Question::Language, "en-GB".into());
        answers.set(Question::Wifi, "home-5g".into());
        answers.set(Question::WifiPassword, "Zq7-radio".into());
        // THE SCAN FOUND NOTHING this time (the radio is slow, or out of
        // range for a moment): the carried network is offered anyway.
        let mut e = State::resumed(Vec::new(), Vec::new(), answers, Question::BelongingsAddress);
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::WifiPassword));
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Wifi));
        assert_eq!(e.options()[e.choice], "home-5g");
        e.key(KeyCode::Enter);
        assert_eq!(e.answers.wifi_ssid, "home-5g");
        assert_eq!(e.question_of(), Some(Question::WifiPassword));
        // And a scan that finds it does not offer it twice.
        let mut answers = Answers::default();
        answers.set(Question::Wifi, "home-5g".into());
        let e = State::resumed(
            Vec::new(),
            vec!["home-5g".into(), "other".into()],
            answers,
            Question::BelongingsAddress,
        );
        assert_eq!(e.networks, ["home-5g", "other"]);
    }

    /// The row of `text` in a screen drawn as text, if it is there.
    fn row_of(shown: &str, text: &str) -> Option<usize> {
        shown.lines().position(|l| l.contains(text))
    }

    /// FAILS IF: on a terminal 60 to 82 columns wide the access screen cuts
    /// off what the repository says of the key, the key, or where to add it
    /// — the text wraps by words into more rows than characters ÷ width
    /// counts.
    #[test]
    fn the_access_status_is_never_cut_off() {
        let t = Texts::fresh("en-GB").unwrap();
        let mut e = with_disks();
        e.answers.set(
            Question::BelongingsAddress,
            "git@gitlab.com:a-group/a-subgroup/another/belongings-of-mine.git".into(),
        );
        // A deploy key as `keygen` prints it: 68 characters of base64.
        e.public_key = Some(format!(
            "ssh-ed25519 {} ronoctua-belongings",
            "A".repeat(68)
        ));
        e.screen = Screen::Asking(
            Question::ALL_KEYS
                .iter()
                .position(|q| *q == Question::BelongingsAccess)
                .unwrap(),
        );
        for status in [key::INST_ACCESS_WAITING, key::INST_ACCESS_READS] {
            e.status = Some(status);
            let page = access::add_key_page(&e.answers.belongings).unwrap();
            for width in 60..=82 {
                let shown = rendered(&e, &t, width, 30);
                let said = t.t(status);
                // The status's first words: the end may wrap on a narrow one.
                let start: String = said
                    .split_whitespace()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    row_of(&shown, &start).is_some(),
                    "{status} cut off at {width} columns:\n{shown}"
                );
                // THE KEY AND THE PAGE, whole: a word longer than the row is
                // broken across rows, so the rows are read joined.
                let joined: String = shown.lines().map(str::trim_end).collect();
                for whole in [&"A".repeat(68), "ronoctua-belongings", page.as_str()] {
                    assert!(
                        joined.contains(whole),
                        "`{whole}` cut off at {width} columns:\n{shown}"
                    );
                }
            }
        }
    }

    /// FAILS IF: a refusal or a help line that wraps by words loses its end
    /// below the rows counted for it.
    #[test]
    fn the_help_line_keeps_its_end() {
        let t = Texts::fresh("pt-BR").unwrap();
        let mut e = with_disks();
        walk_to(&mut e, Question::Wifi);
        e.error = Some(Gate::Unconnected.key());
        let said = t.t(Gate::Unconnected.key());
        let last = said.split_whitespace().last().unwrap().to_owned();
        for width in 30..=90 {
            let shown = rendered(&e, &t, width, 24);
            assert!(
                shown.contains(last.trim_end_matches('.')),
                "the end of the sentence was cut at {width} columns:\n{shown}"
            );
        }
    }

    /// FAILS IF: a list's row is not selected by a click and taken by a
    /// second, the click lands on another row than the one drawn there
    /// (a list scrolled down), the wheel does nothing, the row under the
    /// pointer is not lit, or a key of the keys line is not a click.
    #[test]
    fn everything_answers_the_pointer() {
        let t = texts();
        let area = Rect::new(0, 0, 100, 12);
        let mut e = with_disks();
        walk_to(&mut e, Question::Timezone);
        // DOWN THE LIST, so it scrolls: a click must hit the row drawn.
        for _ in 0..30 {
            e.key(KeyCode::Down);
        }
        let at = places(area, &e, &t);
        e.drawn(&at);
        let rows = at.list.expect("the zone is a list");
        assert!(rows.offset > 0, "the list did not scroll");
        let shown = rendered(&e, &t, area.width, area.height);
        let options = e.options();
        let wanted = rows.offset + 1;
        let y = rows.area.y + 1;
        assert!(
            shown
                .lines()
                .nth(usize::from(y))
                .unwrap()
                .contains(&options[wanted]),
            "row {y} does not show {}:\n{shown}",
            options[wanted]
        );
        // RESTING: lit, and not the cursor.
        assert_eq!(e.pointer(Pointer::Moved, 3, y, &at), Next::Stay);
        assert_eq!(e.hover, Some(Hit::Row(wanted)));
        assert_ne!(e.choice, wanted);
        // FIRST CLICK selects, the second takes it — on the same row: the
        // list did not move under the pointer.
        e.pointer(Pointer::Click, 3, y, &at);
        assert_eq!(e.choice, wanted);
        assert_eq!(e.question_of(), Some(Question::Timezone));
        let at = places(area, &e, &t);
        e.drawn(&at);
        assert_eq!(at.hit(3, y), Some(Hit::Row(wanted)), "the list moved");
        e.pointer(Pointer::Click, 3, y, &at);
        assert_eq!(e.answers.timezone, options[wanted]);
        assert_eq!(e.question_of(), Some(Question::Hostname));
        // THE KEYS LINE: `Esc` goes back, by the keyboard's own path.
        let at = places(area, &e, &t);
        let esc = at
            .hints
            .iter()
            .position(|h| h.act == Some(Act::Key(KeyCode::Esc)))
            .expect("no Esc in the keys line");
        let x = at.hints[esc].x;
        e.pointer(Pointer::Click, x, at.keys.y, &at);
        assert_eq!(e.question_of(), Some(Question::Timezone));
        // EVERY HINT IS WHERE IT IS HIT: the drawing and the pointer read
        // the same `places`.
        let at = places(area, &e, &t);
        let shown = rendered(&e, &t, area.width, area.height);
        let keys_row = shown.lines().nth(usize::from(at.keys.y)).unwrap();
        for hint in &at.hints {
            let drawn: String = keys_row
                .chars()
                .skip(usize::from(hint.x))
                .take(hint.text.chars().count())
                .collect();
            assert_eq!(drawn, hint.text, "a hint is not where it is hit");
        }
        // THE WHEEL moves the list.
        let before = e.choice;
        e.pointer(Pointer::WheelDown, 3, rows.area.y, &at);
        assert_eq!(e.choice, before + 1);
        // A HINT THAT NAMES NO CLICKABLE KEY is not hit.
        if let Some(inert) = at.hints.iter().find(|h| h.act.is_none()) {
            assert_eq!(at.hit(inert.x, at.keys.y), None);
        }
        // `Ctrl+C`, clicked, gives up — on a screen whose keys line has it.
        let mut e = with_disks();
        walk_to(&mut e, Question::Hostname);
        let at = places(area, &e, &t);
        let quit = at
            .hints
            .iter()
            .find(|h| h.act == Some(Act::Quit))
            .expect("no Ctrl+C in the keys line");
        assert_eq!(
            e.pointer(Pointer::Click, quit.x, at.keys.y, &at),
            Next::GiveUp
        );
        // THE ACCESS SCREEN of a deploy key, which has no list: Enter and
        // Esc are its clicks.
        let mut e = with_disks();
        walk_to(&mut e, Question::BelongingsAddress);
        type_in(&mut e, "git@github.com:me/belongings.git");
        e.key(KeyCode::Enter);
        settle(&mut e, &kind);
        assert_eq!(e.question_of(), Some(Question::BelongingsAccess));
        let at = places(Rect::new(0, 0, 100, 40), &e, &t);
        let enter = at
            .hints
            .iter()
            .find(|h| h.act == Some(Act::Key(KeyCode::Enter)))
            .expect("no Enter in the keys line");
        e.pointer(Pointer::Click, enter.x, at.keys.y, &at);
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
    }

    /// FAILS IF: a language's keys line stops naming its keys the way the
    /// pointer reads them — the clicks would fall on nothing.
    #[test]
    fn every_keys_line_names_its_keys() {
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            for k in [
                key::INST_KEYS_EDIT,
                key::INST_KEYS_CHOOSE,
                key::INST_KEYS_REVIEW,
                key::INST_KEYS_QUIT,
            ] {
                let hints = hints_of(&t.t(k), 0);
                let acts: Vec<Option<Act>> = hints.iter().map(|h| h.act).collect();
                assert!(
                    acts.iter().any(Option::is_some),
                    "{language}: `{k}` has no key a click can press: {hints:?}"
                );
                for h in &hints {
                    let first = h.text.split_whitespace().next().unwrap_or_default();
                    assert!(
                        h.act.is_some() || first == "↑↓",
                        "{language}: `{k}` names a key the pointer does not know: {first}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_invalid_answer_does_not_advance() {
        let mut e = with_disks();
        walk_to(&mut e, Question::Hostname);
        let stayed = e.screen.clone();
        type_in(&mut e, "semprefixo");
        e.key(KeyCode::Enter);
        assert_eq!(e.screen, stayed, "it moved on with an invalid hostname");
        assert_eq!(e.error, Some(key::INST_INVALID_HOSTNAME_PREFIX));
    }

    #[test]
    fn esc_goes_back_and_brings_back_what_was_typed() {
        let mut e = with_disks();
        walk_to(&mut e, Question::Hostname);
        let hostname = e.screen.clone();
        type_in(&mut e, "noctua-teste");
        e.key(KeyCode::Enter);
        assert_ne!(e.screen, hostname);
        e.key(KeyCode::Esc);
        assert_eq!(e.screen, hostname);
        assert_eq!(e.typing, "noctua-teste");
        // A SECRET IS NOT TYPED BACK.
        walk_to(&mut e, Question::Passphrase);
        type_in(&mut e, "Zq7-test-phrase");
        e.key(KeyCode::Enter);
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Passphrase));
        assert!(e.typing.is_empty(), "a secret came back into the field");
    }

    #[test]
    fn from_the_first_question_esc_does_not_leave() {
        let mut e = with_disks();
        e.key(KeyCode::Esc);
        assert_eq!(e.screen, Screen::Asking(0));
    }

    #[test]
    fn the_confirmation_requires_the_disks_path() {
        let mut e = with_disks();
        e.answers.disk = "/dev/nvme0n1".into();
        e.screen = Screen::Reviewing;
        type_in(&mut e, "sim");
        assert_ne!(
            e.key(KeyCode::Enter),
            Next::Run,
            "`yes` armed the installation"
        );
        assert!(e.error.is_some());
        assert_ne!(e.screen, Screen::Running);
        e.confirmation.clear();
        type_in(&mut e, "/dev/nvme0n1");
        assert_eq!(e.key(KeyCode::Enter), Next::Run);
        assert_eq!(e.screen, Screen::Running);
    }

    #[test]
    fn the_confirmation_is_of_the_chosen_disk_and_not_another() {
        let mut e = with_disks();
        e.answers.disk = "/dev/sda".into();
        e.screen = Screen::Reviewing;
        type_in(&mut e, "/dev/nvme0n1");
        assert_ne!(
            e.key(KeyCode::Enter),
            Next::Run,
            "it confirmed the wrong disk"
        );
    }

    #[test]
    fn no_label_on_screen_comes_out_as_a_key() {
        // Invariant 8 at the last link: a key missing from the `.ftl` files
        // comes out as `[inst-disk]` on screen.
        let mut answers = Answers::default();
        answers.set(Question::BelongingsAddress, "git@x.org:a/b.git".into());
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            let mut keys: Vec<&str> = Vec::new();
            for q in Question::ALL_KEYS {
                keys.push(q.label());
                keys.push(q.help());
            }
            for aimed in [false, true] {
                for empty in [false, true] {
                    answers.belongings_empty = Some(empty);
                    let o = Origin {
                        aimed,
                        ..review_origin()
                    };
                    for p in steps(&answers, &o) {
                        keys.push(p.label);
                    }
                }
            }
            for g in [
                Gate::Open,
                Gate::NoDaemon,
                Gate::Off,
                Gate::NoLink,
                Gate::Unconnected,
                Gate::Portal,
            ] {
                keys.push(g.key());
            }
            for s in [
                Stay::Current,
                Stay::FromRelease,
                Stay::Ahead,
                Stay::Development,
                Stay::Unreachable,
            ] {
                keys.extend(s.key());
            }
            for why in Why::ALL {
                keys.push(access::refusal_key(why));
            }
            keys.extend([
                key::INST_TITLE,
                key::INST_REVIEW,
                key::INST_RUNNING,
                key::INST_DONE,
                key::INST_FAILED,
                key::INST_DRYRUN,
                key::INST_DESTRUCTIVE,
                key::INST_CONFIRM,
                key::INST_CONFIRM_ERROR,
                key::INST_NO_DISK,
                key::INST_KEYS_EDIT,
                key::INST_KEYS_CHOOSE,
                key::INST_KEYS_REVIEW,
                key::INST_KEYS_QUIT,
                key::INST_CHECKING,
                key::INST_GATE_CHECKING,
                key::INST_RELEASE_LOOKING,
                key::INST_RELEASE_NEWER,
                key::INST_ACCESS_WAITING,
                key::INST_ACCESS_READS,
                key::INST_ACCESS_READY,
                key::INST_BELONGINGS_KEY_INTRO,
                key::INST_BELONGINGS_KEY_WHERE,
                key::INST_BELONGINGS_KEY_WHERE_UNKNOWN,
                key::INST_BELONGINGS_TOKEN_WHERE,
                key::INST_BELONGINGS_TOKEN_HELP,
                key::INST_BELONGINGS_PASSPHRASE_NEW_HELP,
                key::INST_BELONGINGS_PASSPHRASE_AGAIN,
                key::INST_BELONGINGS_PASSPHRASE_DIFFERS,
                key::INST_NETWORK_HELP_UNNAMED,
                key::INST_STEP_SKIPPED,
            ]);
            for c in keys {
                let s = t.t(c);
                assert!(
                    !s.starts_with('['),
                    "{language}: `{c}` came out as a raw key"
                );
            }
        }
    }

    #[test]
    fn the_list_does_not_go_past_the_ends() {
        let mut e = with_disks();
        e.key(KeyCode::Up);
        assert_eq!(e.choice, 0, "it went above the first one");
        for _ in 0..10 {
            e.key(KeyCode::Down);
        }
        assert_eq!(e.choice, 1, "it went below the last one");
    }

    #[test]
    fn with_no_disk_at_all_enter_does_not_advance() {
        // An ISO running where `lsblk` lists nothing: advancing would write an
        // empty disk and `disko` would fail much later.
        let mut e = State::fresh(Vec::new(), Vec::new());
        walk_to(&mut e, Question::Disk);
        let disk = e.screen.clone();
        e.key(KeyCode::Enter);
        assert_eq!(e.screen, disk, "it moved on with no disk chosen");
        assert!(e.error.is_some());
    }

    /// FAILS IF: a QR code is drawn cut — a camera reads nothing in one —
    /// or drawn where the area does not hold it.
    #[test]
    fn a_qr_is_drawn_whole_or_not_at_all() {
        let qr =
            Qr::of("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITestKey ronoctua-belongings").unwrap();
        let side = u16::try_from(qr.side + 2 * QUIET).unwrap();
        let rows = side.div_ceil(2);
        let draw_in = |w: u16, h: u16| {
            let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
            QrBlock { qr: &qr }.render(Rect::new(0, 0, w, h), &mut buf);
            buf.content()
                .iter()
                .filter(|c| c.symbol() == half_block::UPPER.to_string())
                .count()
        };
        assert_eq!(draw_in(side, rows), usize::from(side) * usize::from(rows));
        assert_eq!(draw_in(side - 1, rows), 0, "a cut code was drawn");
        assert_eq!(draw_in(side, rows - 1), 0, "a cut code was drawn");
        // AND WHAT IS DRAWN IS THE CODE: every module's colour, upper half
        // and lower, is the module's — the quiet zone light around it.
        let mut buf = Buffer::empty(Rect::new(0, 0, side, rows));
        QrBlock { qr: &qr }.render(Rect::new(0, 0, side, rows), &mut buf);
        let (light, dark) = (color(color::TEXT_PRIMARY), color(color::SURFACE_BASE));
        assert_ne!(light, dark, "the code would be one colour");
        let quiet = isize::try_from(QUIET).unwrap();
        for y in 0..usize::from(side) {
            for x in 0..usize::from(side) {
                let cell = buf
                    .cell((u16::try_from(x).unwrap(), u16::try_from(y / 2).unwrap()))
                    .unwrap();
                let painted = if y % 2 == 0 { cell.fg } else { cell.bg };
                let (mx, my) = (
                    isize::try_from(x).unwrap() - quiet,
                    isize::try_from(y).unwrap() - quiet,
                );
                let want = if qr.is_dark(mx, my) { dark } else { light };
                assert_eq!(painted, want, "module ({mx}, {my})");
            }
        }
    }
}
