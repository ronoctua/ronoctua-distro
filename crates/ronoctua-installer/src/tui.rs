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
//! (`Space`, `Enter`, `Esc`, `Ctrl+C`, named by the code: [`KEYS`]), and
//! what the pointer rests on is lit. What the drawing paints and what the
//! pointer hits are ONE answer ([`places`]).
//!
//! **What a click opens is born under the pointer** (AGENTS, "Perguntas"): a
//! screen that a click brought takes no click of its own until one SELECTED
//! a row of it ([`State::aimed`]) — the cursor's row included. Without that,
//! the second click of a double click on `full` landed on the next screen's
//! first row and took it.

use std::io::Stdout;
use std::time::Duration;

use crate::key;
use anyhow::{Context as _, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::style::Print;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::half_block;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Widget, Wrap};
use ronoctua_belongings::prefs::Auth;
use ronoctua_i18n::{Arg, Texts};
use ronoctua_tokens::{Color, color};
use ronoctua_vt::{Console, Table};

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

/// The roles the TUI paints, each in the console's slot it takes — on the
/// kernel's console, which keeps one bit of a 24-bit colour, the TUI paints
/// by slot ([`ronoctua_vt`]).
///
/// THE FIRST EIGHT are the ones a background can be, so every role a cell's
/// ground wears is there — the bands, and the QR code's two tones. Slots 0 and
/// 7 are the console's own ground and text. Every colour any screen paints has
/// a slot (`every_colour_has_a_slot`).
const SLOTS: Table = Table::new(&[
    color::SURFACE_BASE,
    color::STATE_ERROR,
    color::STATE_SUCCESS,
    color::STATE_WARNING,
    color::SURFACE_SELECTION,
    color::STATE_ACCENT,
    color::SURFACE_HOVER,
    color::TEXT_PRIMARY,
    color::TEXT_DIM,
    color::STATE_CRITICAL,
    color::TEXT_ANSWER,
    color::TEXT_SUBTLE_LIT_KEY,
    color::TEXT_HERE_ACTS,
    color::TEXT_SECONDARY,
    color::TEXT_LABEL_ACTS,
    color::TEXT_HEADING,
]);

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
    /// Saying something of the machine before any question is about it —
    /// right after the language, so in the person's (`crate::platform`).
    Stop(Stop),
    /// The release's installer was found: Enter starts it, with the answers.
    Hopping(Hop),
    /// Showing the plan, waiting for confirmation.
    Reviewing,
    /// Running.
    Running,
    /// Finished. `Ok` when it worked.
    Finished(bool),
}

/// What stops the questions, about the machine itself
/// (`crate::platform`). Each is a list whose first row — the cursor's — is
/// the way out: it appears on its own, about something the person may not
/// have chosen (AGENTS, "Perguntas").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// No TPM 2.0: what it is, why it is required, how to switch it on — and
    /// no installation. A rehearsal may go on, to see the rest.
    Tpm,
    /// Started from BIOS: what UEFI is, why it is better, how to switch it
    /// on — and BIOS only by a row that says it is the worse one.
    Bios,
}

/// What a stop says, for somebody who never heard of it: what it is, why it
/// matters, how to switch it on — step by step, in the firmware's setup —,
/// and what is left when that cannot be done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Told {
    /// What it is.
    pub what: &'static str,
    /// Why it matters.
    pub why: &'static str,
    /// The steps' heading.
    pub how: &'static str,
    /// The steps, numbered as drawn; handed the setup's keys.
    pub steps: &'static [&'static str],
    /// What is left when the steps cannot be followed.
    pub after: &'static str,
    /// What is left instead on a machine started from BIOS, when that is
    /// something else.
    pub after_in_bios: Option<&'static str>,
}

/// A stop's row, and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// Leaves the installer.
    Leave,
    /// Goes on with the rehearsal: nothing is installed.
    Rehearse,
    /// Goes on, from BIOS.
    Worse,
}

impl Way {
    /// Its row's text **key**.
    #[must_use]
    pub const fn key(self, stop: Stop) -> &'static str {
        match (stop, self) {
            (Stop::Tpm, Self::Leave) => key::INST_TPM_LEAVE,
            (Stop::Bios, Self::Leave) => key::INST_FIRMWARE_LEAVE,
            (_, Self::Rehearse) => key::INST_TPM_PREVIEW,
            (_, Self::Worse) => key::INST_FIRMWARE_WORSE,
        }
    }
}

impl Stop {
    /// Its heading's text **key**.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Tpm => key::INST_TPM,
            Self::Bios => key::INST_FIRMWARE,
        }
    }

    /// What it says, top to bottom, by text **keys** (`stop_text`).
    #[must_use]
    pub const fn told(self) -> Told {
        match self {
            Self::Tpm => Told {
                what: key::INST_TPM_WHAT,
                why: key::INST_TPM_WHY,
                how: key::INST_TPM_HOW,
                steps: &[
                    key::INST_SETUP_OPEN,
                    key::INST_TPM_FIND,
                    key::INST_SETUP_SAVE,
                    key::INST_SETUP_BOOT_AGAIN,
                ],
                after: key::INST_TPM_NONE,
                // IN BIOS MODE THE TPM MAY ONLY BE HIDDEN: UEFI first.
                after_in_bios: Some(key::INST_TPM_CSM),
            },
            Self::Bios => Told {
                what: key::INST_FIRMWARE_WHAT,
                why: key::INST_FIRMWARE_WHY,
                how: key::INST_FIRMWARE_HOW,
                steps: &[
                    key::INST_SETUP_OPEN,
                    key::INST_FIRMWARE_FIND,
                    key::INST_SETUP_SAVE,
                    key::INST_FIRMWARE_BOOT_AGAIN,
                ],
                after: key::INST_FIRMWARE_STILL,
                after_in_bios: None,
            },
        }
    }

    /// Its rows, the way out first: a machine with no TPM 2.0 installs
    /// nothing, and goes on only to rehearse.
    #[must_use]
    pub fn ways(self, rehearsal: bool) -> Vec<Way> {
        match self {
            Self::Tpm if rehearsal => vec![Way::Leave, Way::Rehearse],
            Self::Tpm => vec![Way::Leave],
            Self::Bios => vec![Way::Leave, Way::Worse],
        }
    }
}

/// The keys a firmware's setup opens with, on most machines — not this
/// system's keys, and so never a sentence's: the screens that send the
/// person to the setup name them one by one, and the language joins them
/// (`{ $f2 }, { $f10 }, { $f12 } or { $del }`).
pub const SETUP_KEYS: [(&str, &str); 4] = [
    ("f2", KEY_SETUP_F2),
    ("f10", KEY_SETUP_F10),
    ("f12", KEY_SETUP_F12),
    ("del", KEY_SETUP_DEL),
];

/// The setup's keys, one by one ([`SETUP_KEYS`]).
pub const KEY_SETUP_F2: &str = "F2";
/// See [`SETUP_KEYS`].
pub const KEY_SETUP_F10: &str = "F10";
/// See [`SETUP_KEYS`].
pub const KEY_SETUP_F12: &str = "F12";
/// See [`SETUP_KEYS`].
pub const KEY_SETUP_DEL: &str = "Del";

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
    /// The row that says what a list or the plan has above what it shows:
    /// a click shows it.
    Earlier,
    /// The row that says what a list or the plan has below what it shows:
    /// a click shows it.
    Later,
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
///
/// A list that does not fit keeps its first and last rows for saying what
/// it does not show — above and below — and its options take the rows
/// between: on a console of twenty-four rows the list of programs ended in
/// the middle with nothing saying more followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
    /// Its options' rows.
    pub area: Rect,
    /// The option its first row shows.
    pub offset: usize,
    /// How many options it has.
    pub count: usize,
    /// The row saying how many options are above, while some are.
    pub above: Option<Rect>,
    /// The row saying how many options are below, while some are.
    pub below: Option<Rect>,
}

/// The review's plan, where it is drawn — the same shape as a list's: a plan
/// that does not fit keeps its first and last rows for saying what it does
/// not show, and SCROLLS (the keys, the wheel, a click on those rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// The rows its lines are drawn in.
    pub area: Rect,
    /// The first line drawn.
    pub offset: usize,
    /// How many lines are drawn from it.
    pub shown: usize,
    /// How many lines the plan has.
    pub count: usize,
    /// The row saying what is above, while something is.
    pub above: Option<Rect>,
    /// The row saying what is below, while something is.
    pub below: Option<Rect>,
    /// The first line it may start at for its last one to be drawn.
    pub last_offset: usize,
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
    /// The plan, when the screen is the review.
    pub plan: Option<Plan>,
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
        let on = |r: Option<Rect>| r.is_some_and(inside);
        let (above, below) = match (self.list, self.plan) {
            (Some(rows), _) => (rows.above, rows.below),
            (None, Some(plan)) => (plan.above, plan.below),
            (None, None) => return None,
        };
        if on(above) {
            return Some(Hit::Earlier);
        }
        if on(below) {
            return Some(Hit::Later);
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
    /// The row a click selected on this screen: only a click on it, while
    /// it is still the cursor's, does what `Enter` does. `None` on a screen
    /// just shown — so the click that opened it is never taken by it.
    pub aimed: Option<usize>,
    /// Whether the screen was just shown and has taken nothing yet — no key,
    /// no click. A click on a hint of the keys line then only lights it:
    /// `Enter` stands in the same columns on screen after screen, and the
    /// second click of a double click would press the next screen's.
    pub untouched: bool,
    /// The review's first line, as the plan was last drawn
    /// ([`State::drawn`]): the plan scrolls by the keys, the wheel and its
    /// two "more" rows.
    pub review_scroll: usize,
    /// How many lines the review's plan showed when last drawn, and the
    /// first line it may start at: a page, and how far it scrolls.
    pub review_page: (usize, usize),
    /// Where a resumed installation goes on once the list of programs it
    /// had to show first is answered (the question the hop handed on).
    pub resume_to: Option<usize>,
    /// The ids the hop's answer left out that this release no longer
    /// offers: said on the list, which shows them gone.
    pub dropped: Vec<String>,
    /// The machine, read before the first screen (`crate::platform`).
    pub platform: crate::platform::Platform,
    /// A rehearsal (no `--execute`): nothing is installed, and a machine
    /// with no TPM may still be looked through.
    pub rehearsal: bool,
    /// The person chose BIOS on its screen ([`Stop::Bios`]), or before a
    /// hop handed the answers on.
    pub bios_chosen: bool,
    /// The screen the stops were shown in front of, when it is not the
    /// first question after the language ([`State::stop_first`]).
    pub after_stop: Option<Screen>,
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
            aimed: None,
            untouched: true,
            review_scroll: 0,
            review_page: (0, 0),
            resume_to: None,
            dropped: Vec::new(),
            platform: crate::platform::Platform::default(),
            rehearsal: false,
            bios_chosen: false,
            after_stop: None,
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
    ///
    /// # The list of programs comes FIRST when the hop did not answer it
    ///
    /// Every hop is from an OLDER installer, and one older than the list
    /// never asked it: resuming at the repository would leave out the
    /// list's default — LibreOffice, declined and pushed — with nobody
    /// having seen it. So a `full` installation whose answers carry no list
    /// opens on the list, and "continue" goes on to where the hop left.
    /// And an answer that names a program this release no longer offers
    /// opens it too, saying so ([`State::dropped`]): the person is there to
    /// decide again, and a refusal with nothing to fix would be a dead end
    /// (without a terminal it IS a refusal — `validate_it` — because
    /// nobody is there to decide).
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
        let at_of = |q: Question| {
            Question::ALL_KEYS
                .iter()
                .position(|x| *x == q)
                .unwrap_or_default()
        };
        let n = at_of(at);
        let list = at_of(Question::LeftOut);
        let carried = e.answers.left_out.clone();
        let dropped = model::not_offered(&carried);
        if n > list
            && !Question::LeftOut.skipped_one(&e.answers)
            && (carried.is_empty() || !dropped.is_empty())
        {
            e.answers.left_out = if carried.is_empty() {
                model::DEFAULT_LEFT_OUT.to_owned()
            } else {
                model::canonical(&carried)
            };
            e.dropped = dropped;
            e.resume_to = Some(n);
            e.screen = Screen::Asking(list);
        } else {
            e.screen = Screen::Asking(n);
        }
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
        // A STOP is a list of its ways.
        if let Screen::Stop(stop) = self.screen {
            return !stop.ways(self.rehearsal).is_empty();
        }
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
                    | Question::LeftOut
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
        // A STOP'S WAYS, by their text keys: drawn in the person's language.
        if let Screen::Stop(stop) = self.screen {
            return stop
                .ways(self.rehearsal)
                .iter()
                .map(|w| w.key(stop).to_owned())
                .collect();
        }
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
            // "CONTINUE" FIRST, as "no Wi-Fi" is: an empty value, drawn with
            // its own words; then one row per program, by its name.
            Some(Question::LeftOut) => std::iter::once(String::new())
                .chain(model::OFFERED.iter().map(|o| o.name.to_owned()))
                .collect(),
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
            // THE ANSWER IS THE STATE: the marks are kept in it as they are
            // made, so `Esc`, the hop and a resume carry them for free.
            Some(Question::LeftOut) => self.answers.left_out.clone(),
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
        self.aimed = None;
        self.untouched = true;
        self.review_scroll = 0;
        self.dropped.clear();
        // A RESUMED INSTALLATION goes on where the hop left, once the list
        // it had to show first is answered.
        let mut i = match self.resume_to.take() {
            Some(target) if target > n => target,
            _ => n + 1,
        };
        while let Some(q) = Question::ALL_KEYS.get(i) {
            if !q.computed() && !q.skipped_one(&self.answers) {
                self.screen = Screen::Asking(i);
                self.typing.clear();
                self.error = None;
                self.landed_on(*q);
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
        self.aimed = None;
        self.untouched = true;
        // GOING BACK walks the questions again from there: no jump ahead.
        self.resume_to = None;
        self.dropped.clear();
        let current = match &self.screen {
            Screen::Asking(n) => *n,
            // A STOP STANDS AFTER THE LANGUAGE: back to it.
            Screen::Stop(_) => {
                Question::ALL_KEYS
                    .iter()
                    .position(|q| *q == Question::Language)
                    .unwrap_or_default()
                    + 1
            }
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
            self.landed_on(*q);
            // GOING BACK lands on what was ANSWERED, not on the default — but
            // on the list of programs, whose answer is every row's mark, on
            // its first row.
            self.choice = if *q == Question::LeftOut {
                0
            } else {
                self.options()
                    .iter()
                    .position(|o| {
                        o.starts_with(self.answers.get(*q)) && !self.answers.get(*q).is_empty()
                    })
                    .unwrap_or_else(|| self.default_choice())
            };
            // THE ACCESS IS ASKED AGAIN when its screen is shown again.
            if *q == Question::BelongingsAccess && !self.by_token() {
                self.ask_probe(0);
            }
            return;
        }
    }

    /// A page of the review's plan, in lines: what it showed, but one, so a
    /// line of the page before stays in sight.
    fn review_step(&self) -> isize {
        isize::try_from(self.review_page.0.saturating_sub(1).max(1)).unwrap_or(1)
    }

    /// The review's plan moved by `lines` (back when negative), within what
    /// it has as it was last drawn.
    fn scroll_review(&mut self, lines: isize) -> Next {
        let most = self.review_page.1;
        self.review_scroll = self.review_scroll.saturating_add_signed(lines).min(most);
        Next::Stay
    }

    /// A screen of `q` was just shown: the list of programs opens on its
    /// default marks when nothing was marked yet, and keeps them otherwise.
    fn landed_on(&mut self, q: Question) {
        if q == Question::LeftOut && self.answers.left_out.is_empty() {
            self.answers.left_out = model::DEFAULT_LEFT_OUT.to_owned();
        }
    }

    /// The program on the list's row `choice` (row 0 is "continue").
    fn offered_at(&self, choice: usize) -> Option<&'static model::Offered> {
        (self.question_of() == Some(Question::LeftOut))
            .then(|| choice.checked_sub(1).and_then(|i| model::OFFERED.get(i)))
            .flatten()
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
            // THE MACHINE IS SAID BEFORE ANY QUESTION IS ABOUT IT, and in
            // the language just chosen: a stop, when there is one.
            Question::Language => {
                self.answers.set(q, v);
                match self.stop_due() {
                    Some(stop) => self.stop_at(stop),
                    None => self.follow(n),
                }
            }
            _ => {
                self.answers.set(q, v);
                self.follow(n);
            }
        }
    }

    /// The stop this machine is due, if any: a BIOS start not chosen yet
    /// first — in BIOS mode many computers hide the TPM, and switching to
    /// UEFI is then the answer to both —, then no TPM 2.0, which nothing
    /// installs without.
    fn stop_due(&self) -> Option<Stop> {
        if !self.platform.uefi && !self.bios_chosen {
            Some(Stop::Bios)
        } else if !self.platform.tpm2 && !self.passed_tpm() {
            Some(Stop::Tpm)
        } else {
            None
        }
    }

    /// Shows the stop this machine is due before anything else, and then
    /// the screen it stood on: a release's installer the answers were handed
    /// to resumes past the language, where the stops stand — and the
    /// installer that handed them on may have had none.
    pub fn stop_first(&mut self) {
        if let Some(stop) = self.stop_due() {
            self.after_stop = Some(self.screen.clone());
            self.stop_at(stop);
        }
    }

    /// Whether the TPM's stop was passed, which only a rehearsal does.
    fn passed_tpm(&self) -> bool {
        self.rehearsal && self.screen == Screen::Stop(Stop::Tpm)
    }

    /// Shows `stop`, the cursor on its way out.
    fn stop_at(&mut self, stop: Stop) {
        self.stop_waiting();
        self.screen = Screen::Stop(stop);
        self.choice = 0;
        self.scroll = 0;
        self.aimed = None;
        self.untouched = true;
        self.typing.clear();
        self.error = None;
        self.status = None;
    }

    /// What a stop's row does: leave, or go on to what comes next — the
    /// TPM's stop after the BIOS's, then the screen the stops stood before
    /// (the first question after the language, or where a resumed
    /// installation picks up).
    fn take_way(&mut self, stop: Stop) -> Next {
        let ways = stop.ways(self.rehearsal);
        match ways.get(self.choice) {
            Some(Way::Leave) => return Next::GiveUp,
            Some(Way::Worse) => self.bios_chosen = true,
            Some(Way::Rehearse) => {}
            None => return Next::Stay,
        }
        match self.stop_due() {
            Some(next) if next != stop => self.stop_at(next),
            _ => self.past_the_stops(),
        }
        Next::Stay
    }

    /// The screen after the stops.
    fn past_the_stops(&mut self) {
        if let Some(screen) = self.after_stop.take() {
            self.screen = screen;
            self.scroll = 0;
            self.aimed = None;
            self.untouched = true;
            self.error = None;
            self.choice = self.default_choice();
            return;
        }
        let language = Question::ALL_KEYS
            .iter()
            .position(|q| *q == Question::Language)
            .unwrap_or_default();
        self.follow(language);
    }

    /// Applies a key.
    ///
    /// **Testable without a terminal**: it is the reason key reading and
    /// drawing are in functions other than this one.
    pub fn key(&mut self, k: KeyCode) -> Next {
        // A KEY TOUCHES THE SCREEN: a hint clicked after it acts at once.
        self.untouched = false;
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
                    ESC => self.go_back(),
                    // WHILE A CHECK RUNS, only Esc counts: an Enter would ask
                    // twice, and typing would change what is being checked.
                    _ if self.waiting.is_some() => {}
                    UP if self.is_list() => self.choice = self.choice.saturating_sub(1),
                    DOWN if self.is_list() => {
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
                    // A PROGRAM'S ROW IS MARKED OR UNMARKED, by Enter as by
                    // the space bar: the answer itself changes, and the
                    // screen stays. Enter on "continue" takes the answer.
                    ENTER | SPACE if self.offered_at(self.choice).is_some() => {
                        if let Some(o) = self.offered_at(self.choice) {
                            self.answers.left_out = model::toggle(&self.answers.left_out, o.id);
                            self.error = None;
                        }
                    }
                    ENTER => {
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
            Screen::Stop(stop) => match k {
                ESC => {
                    self.go_back();
                    Next::Stay
                }
                UP => {
                    self.choice = self.choice.saturating_sub(1);
                    Next::Stay
                }
                DOWN => {
                    let last = stop.ways(self.rehearsal).len().saturating_sub(1);
                    self.choice = (self.choice + 1).min(last);
                    Next::Stay
                }
                ENTER => self.take_way(stop),
                _ => Next::Stay,
            },
            Screen::Hopping(hop) => match k {
                ENTER => Next::Hop(hop),
                ESC => {
                    self.go_back();
                    Next::Stay
                }
                _ => Next::Stay,
            },
            Screen::Reviewing => match k {
                ESC => {
                    self.go_back();
                    Next::Stay
                }
                // THE PLAN SCROLLS — what is typed here is the disk's path,
                // and no arrow is part of one.
                UP => self.scroll_review(-1),
                DOWN => self.scroll_review(1),
                KeyCode::PageUp => self.scroll_review(-self.review_step()),
                KeyCode::PageDown => self.scroll_review(self.review_step()),
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
                ENTER => {
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
            Pointer::WheelUp if self.is_list() => self.key(UP),
            Pointer::WheelDown if self.is_list() => self.key(DOWN),
            // THE REVIEW'S PLAN, a line a notch.
            Pointer::WheelUp if self.screen == Screen::Reviewing => self.key(UP),
            Pointer::WheelDown if self.screen == Screen::Reviewing => self.key(DOWN),
            Pointer::WheelUp | Pointer::WheelDown => Next::Stay,
            Pointer::Click => {
                self.hover = hit;
                let untouched = std::mem::replace(&mut self.untouched, false);
                match hit {
                    // THE FIRST CLICK ON A ROW SELECTS IT — the cursor's
                    // too, on a screen just shown — and only a second one
                    // on the row it selected does what `Enter` does.
                    Some(Hit::Row(i)) if self.waiting.is_none() && self.is_list() => {
                        if i == self.choice && self.aimed == Some(i) {
                            self.key(ENTER)
                        } else {
                            self.choice = i;
                            self.aimed = Some(i);
                            Next::Stay
                        }
                    }
                    // WHAT IS ABOVE OR BELOW what a list or the plan shows,
                    // a page of it.
                    Some(Hit::Earlier) => self.page(places, false),
                    Some(Hit::Later) => self.page(places, true),
                    // A HINT ON A SCREEN JUST SHOWN only lights: the click
                    // may be the second of a double click on the screen
                    // before, whose `Enter` stood in the same columns.
                    Some(Hit::Key(_)) if untouched => Next::Stay,
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

    /// A page of a list or of the review's plan, as drawn at `places`:
    /// `later` the one below what is shown, else the one above. A list's
    /// cursor goes with it, to the page's first row.
    fn page(&mut self, places: &Places, later: bool) -> Next {
        if let Some(rows) = places.list {
            let page = usize::from(rows.area.height.max(1));
            let last = rows.count.saturating_sub(page);
            let top = if later {
                (rows.offset + page).min(last)
            } else {
                rows.offset.saturating_sub(page)
            };
            self.scroll = top;
            self.choice = top;
            self.aimed = None;
            return Next::Stay;
        }
        if self.screen == Screen::Reviewing {
            let step = self.review_step();
            return self.scroll_review(if later { step } else { -step });
        }
        Next::Stay
    }

    /// The screen was drawn at `at`: what the list showed is kept, so the
    /// next drawing scrolls from there ([`State::scroll`]) — and where the
    /// review's plan is, and how far it goes.
    pub fn drawn(&mut self, at: &Places) {
        self.scroll = at.list.map_or(0, |rows| rows.offset);
        if let Some(plan) = at.plan {
            self.review_scroll = plan.offset;
            self.review_page = (plan.shown, plan.last_offset);
        }
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
        (Screen::Stop(stop), _) => texts.t(stop.heading()),
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
        // THE KEYS THE DEFAULT PUTS TO WORK, by the options the system sets.
        Question::ComposeKey => texts.t_with(q.help(), &model::COMPOSE_HELP_KEYS),
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
    // WHAT THE HOP CARRIED AND THIS RELEASE NO LONGER OFFERS, said where
    // the person marks again.
    if e.question_of() == Some(Question::LeftOut) && !e.dropped.is_empty() {
        return (
            texts.t_with(
                key::INST_LEFT_OUT_DROPPED,
                &[("ids", &e.dropped.join(", "))],
            ),
            true,
        );
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

/// The keys line of the screen, for a line `width` cells wide.
fn keys_of(e: &State, texts: &Texts, width: u16) -> String {
    let line = |k| with_keys(texts, k, &[]);
    match e.screen {
        Screen::Asking(_) if e.question_of() == Some(Question::LeftOut) && e.is_list() => {
            // THE SPACE BAR IS TAUGHT WHERE IT FITS ([`spaced_fits`]). The
            // line that names it is the longer one, and a keys line is never
            // cut; narrower, the line names Enter alone, which marks a
            // program's row too.
            let spaced = line(key::INST_KEYS_CHECK_SPACE);
            if spaced_fits(&spaced, width) {
                spaced
            } else {
                line(key::INST_KEYS_CHECK)
            }
        }
        Screen::Asking(_) | Screen::Stop(_) if e.is_list() => line(key::INST_KEYS_CHOOSE),
        Screen::Asking(_) | Screen::Hopping(_) => line(key::INST_KEYS_EDIT),
        Screen::Reviewing => line(key::INST_KEYS_REVIEW),
        _ => line(key::INST_KEYS_QUIT),
    }
}

/// Whether the checklist's longer keys line, the one teaching the space
/// bar, is drawn on a line `width` cells wide: when it fits with a cell to
/// spare, so its last word never touches the edge. Measured as the language
/// writes it, never a column count.
fn spaced_fits(spaced: &str, width: u16) -> bool {
    cells(spaced) < width
}

/// What separates two hints of a keys line, as both `.ftl` files write it —
/// only to FIND the hints: what is drawn between them is the text's own
/// (`Hint::at`).
pub const HINT_SEPARATOR: &str = " · ";

// THE KEYS A LINE TEACHES, beside what they press — names and codes both
// the code's, never a sentence's (AGENTS, "Tecla nunca é texto no .ftl";
// `keys-are-never-text`). Written in a `.ftl`, a name goes stale the day its
// key moves, and a language may spell it so the pointer no longer reads it.
// Each key is ONE code here: `State::key` and `gives_up` answer these and no
// other, the sentences take the names as arguments (`{ $enter }`), and a
// hint's click is read from the same table (`KEYS`) — a hint is grabbed
// where it is drawn, and a key that moves without its name fails a test
// (`a_taught_keys_name_spells_its_code`).

/// The arrow that moves a list's cursor up.
pub const UP: KeyCode = KeyCode::Up;
/// The arrow that moves a list's cursor down.
pub const DOWN: KeyCode = KeyCode::Down;
/// The space bar, which marks a program's row of the checklist.
pub const SPACE: KeyCode = KeyCode::Char(' ');
/// The key that continues — and marks a program's row.
pub const ENTER: KeyCode = KeyCode::Enter;
/// The key that goes back.
pub const ESC: KeyCode = KeyCode::Esc;
/// The letter that gives up while Control is held ([`gives_up`]).
pub const QUIT: KeyCode = KeyCode::Char('c');

/// The two arrows, by name.
pub const KEY_ARROWS: &str = "↑↓";
/// The space bar, by name.
pub const KEY_SPACE: &str = "Space";
/// The key that continues, by name.
pub const KEY_ENTER: &str = "Enter";
/// The key that goes back, by name.
pub const KEY_ESC: &str = "Esc";
/// The chord that gives up, by name.
pub const KEY_QUIT: &str = "Ctrl+C";

/// A key a sentence names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taught {
    /// The argument the sentences name it by (`{ $enter }`).
    pub arg: &'static str,
    /// Its name, as the line draws it.
    pub name: &'static str,
    /// The keys the terminal reports for it.
    pub codes: &'static [KeyCode],
    /// Whether it is pressed with Control held (the chord that gives up).
    pub control: bool,
    /// Whether a click on its hint presses it — not the arrows, which the
    /// list and the wheel answer, and whose hint is drawn inert.
    pub clicks: bool,
}

impl Taught {
    /// What a click on its hint does: its key, by the keyboard's path, or
    /// giving up for the chord.
    #[must_use]
    pub const fn act(&self) -> Option<Act> {
        if !self.clicks {
            None
        } else if self.control {
            Some(Act::Quit)
        } else {
            Some(Act::Key(self.codes[0]))
        }
    }
}

/// Every key a sentence names.
pub const KEYS: [Taught; 5] = [
    Taught {
        arg: "arrows",
        name: KEY_ARROWS,
        codes: &[UP, DOWN],
        control: false,
        clicks: false,
    },
    Taught {
        arg: "space",
        name: KEY_SPACE,
        codes: &[SPACE],
        control: false,
        clicks: true,
    },
    Taught {
        arg: "enter",
        name: KEY_ENTER,
        codes: &[ENTER],
        control: false,
        clicks: true,
    },
    Taught {
        arg: "esc",
        name: KEY_ESC,
        codes: &[ESC],
        control: false,
        clicks: true,
    },
    Taught {
        arg: "quit",
        name: KEY_QUIT,
        codes: &[QUIT],
        control: true,
        clicks: true,
    },
];

/// Whether a key pressed gives up: [`QUIT`] with Control held, the chord
/// [`KEY_QUIT`] names. The event loop asks this, and only this.
#[must_use]
pub fn gives_up(code: KeyCode, modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) && code == QUIT
}

/// A sentence that names keys: every key's name handed in as its
/// argument ([`KEYS`]), with `more` beside them.
#[must_use]
pub fn with_keys(texts: &Texts, key: &str, more: &[(&str, &str)]) -> String {
    let args: Vec<(&str, &str)> = KEYS
        .iter()
        .map(|k| (k.arg, k.name))
        .chain(more.iter().copied())
        .collect();
    texts.t_with(key, &args)
}

/// What a hint does: the key named by the first of its words that is a
/// key's name ([`KEYS`]), wherever the language put it.
#[must_use]
pub fn act_of(hint: &str) -> Option<Act> {
    hint.split_whitespace()
        .find_map(|word| KEYS.iter().find(|k| k.name == word))
        .and_then(Taught::act)
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

/// `area` without its first and last rows, and those two rows — when what
/// is drawn in it (`needs` rows) does not fit and it has rows to spare.
fn kept_for_more(area: Rect, needs: usize) -> (Rect, Option<(Rect, Rect)>) {
    if needs <= usize::from(area.height) || area.height < 3 {
        return (area, None);
    }
    let row = |y| Rect {
        y,
        height: 1,
        ..area
    };
    (
        Rect {
            y: area.y + 1,
            height: area.height - 2,
            ..area
        },
        Some((row(area.y), row(area.y + area.height - 1))),
    )
}

/// The list on a body of `area`, when the screen is one.
fn list_rows(e: &State, area: Rect) -> Option<Rows> {
    if !matches!(e.screen, Screen::Asking(_) | Screen::Stop(_)) || !e.is_list() {
        return None;
    }
    let count = e.options().len();
    if count == 0 {
        return None;
    }
    let (area, _) = split_status(area, e.status.is_some());
    // A LIST THAT DOES NOT FIT says what it does not show, above and below.
    let (area, more) = kept_for_more(area, count);
    let offset = list_offset(e.scroll, e.choice, area.height);
    Some(Rows {
        area,
        offset,
        count,
        above: more.filter(|_| offset > 0).map(|(above, _)| above),
        below: more
            .filter(|_| offset + usize::from(area.height) < count)
            .map(|(_, below)| below),
    })
}

/// One line of the review, and what it is.
struct Said {
    /// The line.
    line: Line<'static>,
    /// `Some(destructive)` for a step of the plan, `None` for what is said
    /// above them.
    step: Option<bool>,
}

/// The review's lines, as they are drawn: why the installer stays with its
/// own distro, what this machine leaves out, then the steps — each marked
/// when it erases the disk.
fn review_lines(e: &State, texts: &Texts) -> Vec<Said> {
    let said = |line: Line<'static>| Said { line, step: None };
    let mut lines = Vec::new();
    if let Some(k) = e.stay.and_then(Stay::key) {
        lines.push(said(Line::from(Span::styled(
            texts.t(k),
            Style::default().fg(color(color::STATE_WARNING)),
        ))));
        lines.push(said(Line::from("")));
    }
    // WHAT THIS MACHINE LEAVES OUT, by the programs' names — and, from a
    // repository with something in it, that what it already declines stays
    // declined: the plan below says the repository is written before the
    // disk.
    let secondary = Style::default().fg(color(color::TEXT_SECONDARY));
    let mut told = false;
    if let Some(left) = left_out_review(&e.answers, texts) {
        lines.push(said(Line::from(Span::styled(left, secondary))));
        told = true;
    }
    if e.answers.profile == model::FULL_PROFILE && e.answers.belongings_empty != Some(true) {
        lines.push(said(Line::from(Span::styled(
            texts.t(key::INST_LEFT_OUT_REVIEW_KEPT),
            secondary,
        ))));
        told = true;
    }
    if told {
        lines.push(said(Line::from("")));
    }
    for (i, p) in e.build_plan(&review_origin()).iter().enumerate() {
        let mark = if p.destructive { "!" } else { " " };
        let style = if p.destructive {
            Style::default().fg(color(color::STATE_CRITICAL))
        } else {
            Style::default().fg(color(color::TEXT_SECONDARY))
        };
        lines.push(Said {
            line: Line::from(Span::styled(
                format!("{mark} {}. {}", i + 1, texts.t(p.label)),
                style,
            )),
            step: Some(p.destructive),
        });
    }
    lines
}

/// The rows `line` takes on `width` cells, wrapped by words as drawn.
fn rows_of(line: &Line<'static>, width: u16) -> usize {
    Paragraph::new(line.clone())
        .wrap(Wrap { trim: false })
        .line_count(width.max(1))
        .max(1)
}

/// The review's plan on a body of `area`, when the screen is the review:
/// above the confirmation, which keeps the last rows whatever the plan is.
fn plan_rows(e: &State, texts: &Texts, area: Rect) -> Option<Plan> {
    if e.screen != Screen::Reviewing {
        return None;
    }
    let area = review_parts(area)[0];
    let heights: Vec<usize> = review_lines(e, texts)
        .iter()
        .map(|s| rows_of(&s.line, area.width))
        .collect();
    let count = heights.len();
    let (area, more) = kept_for_more(area, heights.iter().sum());
    let room = usize::from(area.height);
    // THE FIRST LINE IT MAY START AT and still draw its last.
    let mut last_offset = count;
    let mut used = 0;
    while last_offset > 0 && used + heights[last_offset - 1] <= room {
        last_offset -= 1;
        used += heights[last_offset];
    }
    let last_offset = last_offset.min(count.saturating_sub(1));
    let offset = e.review_scroll.min(last_offset);
    let mut shown = 0;
    let mut used = 0;
    for h in &heights[offset..] {
        if used + h > room && shown > 0 {
            break;
        }
        used += h;
        shown += 1;
    }
    Some(Plan {
        area,
        offset,
        shown,
        count,
        above: more.filter(|_| offset > 0).map(|(above, _)| above),
        below: more
            .filter(|_| offset + shown < count)
            .map(|(_, below)| below),
        last_offset,
    })
}

/// The review's body: the plan, and the confirmation pinned below it — the
/// plan grew to two dozen steps, and on a console of thirty rows a
/// confirmation drawn after them fell off the screen, the one line the
/// person has to read.
fn review_parts(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area)
}

/// A stop's explanation: what it is and why in the text's tone, the steps
/// under their heading — each numbered, the first naming the setup's keys —,
/// and what is left in the quieter tone (another thing on a machine started
/// from BIOS, `bios`); `airy`, with a line of air between the parts.
fn stop_text(stop: Stop, texts: &Texts, bios: bool, airy: bool) -> Text<'static> {
    let told = stop.told();
    let primary = Style::default().fg(color(color::TEXT_PRIMARY));
    let secondary = Style::default().fg(color(color::TEXT_SECONDARY));
    let air = || airy.then(|| Line::from(""));
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::styled(texts.t(told.what), primary));
    lines.extend(air());
    lines.push(Line::styled(texts.t(told.why), primary));
    lines.extend(air());
    lines.push(Line::styled(
        texts.t(told.how),
        Style::default().fg(color(color::TEXT_HEADING)),
    ));
    for (n, step) in told.steps.iter().enumerate() {
        let number = (n + 1).to_string();
        lines.push(Line::from(vec![
            Span::styled(texts.t_with(key::INST_STEP, &[("n", &number)]), secondary),
            Span::raw(" "),
            Span::styled(texts.t_with(step, &SETUP_KEYS), primary),
        ]));
    }
    lines.extend(air());
    let after = told.after_in_bios.filter(|_| bios).unwrap_or(told.after);
    lines.push(Line::styled(texts.t(after), secondary));
    Text::from(lines)
}

/// A stop's explanation for a column `width` cells wide and at most `most`
/// rows tall, and the rows it takes: with its lines of air when they fit,
/// without them when they do not — the air goes before any word does. Read
/// by [`places`], which sizes the column, and by the drawing, handed the
/// column it sized: the same answer.
fn stop_fitted(
    stop: Stop,
    texts: &Texts,
    bios: bool,
    width: u16,
    most: u16,
) -> (Text<'static>, u16) {
    let rows = |text: &Text<'static>| {
        let n = Paragraph::new(text.clone())
            .wrap(Wrap { trim: true })
            .line_count(width.max(1));
        u16::try_from(n).unwrap_or(u16::MAX)
    };
    let airy = stop_text(stop, texts, bios, true);
    let n = rows(&airy);
    if n <= most {
        return (airy, n);
    }
    let tight = stop_text(stop, texts, bios, false);
    let n = rows(&tight);
    (tight, n.min(most))
}

/// A stop's heading and explanation as plain lines, for the terminal-less
/// installation, which says what the screen would.
#[must_use]
pub fn stop_said(stop: Stop, texts: &Texts, bios: bool) -> String {
    let mut said = texts.t(stop.heading());
    for line in stop_text(stop, texts, bios, true).lines {
        said.push('\n');
        for span in line.spans {
            said.push_str(&span.content);
        }
    }
    said
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
    // A STOP IS READ TOP TO BOTTOM: what it is, why and how — the whole
    // screen's point, never a third of it —, a line of air, then its ways.
    if let Screen::Stop(stop) = e.screen {
        let ways = u16::try_from(e.options().len()).unwrap_or(u16::MAX);
        let most = area.height.saturating_sub(2 + 1 + 1 + ways).max(1);
        let (_, help_rows) = stop_fitted(stop, texts, e.answers.bios, area.width, most);
        let help_rows = help_rows.max(1);
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),         // title + heading
                Constraint::Length(help_rows), // the explanation
                Constraint::Length(1),         // air
                Constraint::Min(1),            // the ways
                Constraint::Length(1),         // keys
            ])
            .split(area);
        let key_line = keys_of(e, texts, areas[4].width);
        return Places {
            head: areas[0],
            body: areas[3],
            help: areas[1],
            keys: areas[4],
            hints: hints_of(&key_line, areas[4].x),
            key_line,
            list: list_rows(e, areas[3]),
            plan: None,
        };
    }
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
    let key_line = keys_of(e, texts, footer[1].width);
    Places {
        head: areas[0],
        body: areas[1],
        help: footer[0],
        keys: footer[1],
        hints: hints_of(&key_line, footer[1].x),
        key_line,
        list: list_rows(e, areas[1]),
        plan: plan_rows(e, texts, areas[1]),
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
        Screen::Asking(_) | Screen::Stop(_) | Screen::Hopping(_) => texts.t(key::INST_TITLE),
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
            // THE QUESTION IS THE HEADING of what is below it, in a hue no
            // option wears: the answer's tone is the text's own warmed in
            // half the themes, and one could not be told from the other.
            Line::from(Span::styled(
                label_of(e, texts),
                Style::default().fg(color(color::TEXT_HEADING)),
            )),
        ])
        .style(base),
        at.head,
    );

    body(f, e, texts, base, &at);

    let dim = Style::default().fg(color(color::TEXT_DIM));
    let (said, refused) = said_of(e, texts);
    let help = match e.screen {
        // A STOP'S EXPLANATION IS THE SCREEN, in the text's own tones.
        Screen::Stop(stop) => {
            stop_fitted(stop, texts, e.answers.bios, at.help.width, at.help.height).0
        }
        _ => Text::from(Line::from(Span::styled(
            said,
            if refused {
                Style::default().fg(color(color::STATE_ERROR))
            } else {
                dim
            },
        ))),
    };
    f.render_widget(
        Paragraph::new(help).style(base).wrap(Wrap { trim: true }),
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
fn body(f: &mut ratatui::Frame, e: &State, texts: &Texts, base: Style, at: &Places) {
    let area = at.body;
    match &e.screen {
        Screen::Asking(_)
            if e.question_of() == Some(Question::BelongingsAccess) && !e.by_token() =>
        {
            access_body(f, e, texts, area, base);
        }
        Screen::Asking(_) | Screen::Stop(_) if e.is_list() => {
            let Some(rows) = at.list else {
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
            // THE LIST OF PROGRAMS draws its rows itself: a name and the
            // word that says what happens to it (`checklist_lines`).
            let checklist = (e.question_of() == Some(Question::LeftOut))
                .then(|| checklist_lines(&e.answers.left_out, texts, e.choice));
            let items: Vec<ListItem> = e
                .options()
                .into_iter()
                .enumerate()
                .map(|(i, o)| {
                    // AN OPTION IS AN ANSWER, and it is drawn in the tones of
                    // one (`option_tone`): the question above it is the text's,
                    // so what can be chosen is never read as what is asked.
                    let mut style = Style::default().fg(color(option_tone(i == e.choice)));
                    // THE ROW UNDER THE POINTER is lit, and it is not the
                    // cursor: the selection keeps its own band.
                    if e.hover == Some(Hit::Row(i)) && i != e.choice {
                        style = style.bg(color(color::SURFACE_HOVER));
                    }
                    let item = if let Some(line) = checklist.as_ref().and_then(|c| c.get(i)) {
                        ListItem::new(line.clone())
                    } else if matches!(e.screen, Screen::Stop(_)) {
                        // A STOP'S WAY, its key read in the person's language.
                        ListItem::new(texts.t(&o))
                    } else if wifi && o.is_empty() {
                        ListItem::new(texts.t(key::INST_WIFI_NONE))
                    } else {
                        ListItem::new(o)
                    };
                    item.style(style)
                })
                .collect();
            // THE OFFSET IS `places`'s, so the row a click hits is the row
            // drawn there. THE CURSOR'S BAND IS A GROUND ONLY: its text is
            // the row's, which already says it is the cursor's.
            let mut st = ListState::default()
                .with_offset(rows.offset)
                .with_selected(Some(e.choice));
            f.render_stateful_widget(
                List::new(items)
                    .style(base)
                    .highlight_style(Style::default().bg(color(color::SURFACE_SELECTION))),
                rows.area,
                &mut st,
            );
            // WHAT THE LIST DOES NOT SHOW, said where it is not shown.
            let below = rows
                .count
                .saturating_sub(rows.offset + usize::from(rows.area.height));
            more_row(f, e, texts, base, rows.above, rows.offset, Hit::Earlier);
            more_row(f, e, texts, base, rows.below, below, Hit::Later);
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
            // WHAT IS TYPED IS THE ANSWER, in its tone, as a chosen option is.
            let mut lines = vec![Line::from(Span::styled(
                format!("{shown}_"),
                Style::default().fg(color(option_tone(true))),
            ))];
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
                Paragraph::new(with_keys(texts, hop.key(), &[("rev", hop.short())]))
                    .style(base)
                    .wrap(Wrap { trim: true }),
                area,
            );
        }
        Screen::Reviewing => {
            // THE PLAN FROM WHERE IT IS SCROLLED, as `places` measured it,
            // and the confirmation PINNED below it (`review_parts`).
            let parts = review_parts(area);
            if let Some(plan) = at.plan {
                let all = review_lines(e, texts);
                let lines: Vec<Line> = all
                    .iter()
                    .skip(plan.offset)
                    .take(plan.shown)
                    .map(|s| s.line.clone())
                    .collect();
                f.render_widget(
                    Paragraph::new(lines).style(base).wrap(Wrap { trim: false }),
                    plan.area,
                );
                // WHAT IS BELOW, honestly: how many steps, and how many of
                // them erase the disk — the help says the marked ones do.
                let hidden = &all[(plan.offset + plan.shown).min(all.len())..];
                let steps = hidden.iter().filter(|s| s.step.is_some()).count();
                let marked = hidden.iter().filter(|s| s.step == Some(true)).count();
                more_row(f, e, texts, base, plan.above, plan.offset, Hit::Earlier);
                if let Some(row) = plan.below {
                    let said = texts.t_args(
                        key::INST_REVIEW_BELOW,
                        &[
                            (
                                "count",
                                Arg::Number(i64::try_from(steps).unwrap_or(i64::MAX)),
                            ),
                            (
                                "marked",
                                Arg::Number(i64::try_from(marked).unwrap_or(i64::MAX)),
                            ),
                        ],
                    );
                    let tone = if marked > 0 {
                        color::STATE_CRITICAL
                    } else {
                        color::TEXT_SECONDARY
                    };
                    lit_row(f, e, base, row, said, tone, Hit::Later);
                }
            }
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("{}: {}", texts.t(key::INST_CONFIRM), e.answers.disk),
                        Style::default().fg(color(color::STATE_WARNING)),
                    )),
                    Line::from(Span::styled(
                        format!("{}_", e.confirmation),
                        Style::default().fg(color(option_tone(true))),
                    )),
                ])
                .style(base),
                parts[1],
            );
        }
        // A stop is always a list of its ways (`Stop::ways`).
        Screen::Stop(_) => {}
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

/// The row that says how many of a list's options (or of the plan's lines)
/// are `hidden` above or below — `hit` is which, and a click on it shows
/// them; nothing when it is not there.
fn more_row(
    f: &mut ratatui::Frame,
    e: &State,
    texts: &Texts,
    base: Style,
    row: Option<Rect>,
    hidden: usize,
    hit: Hit,
) {
    let Some(row) = row else {
        return;
    };
    let k = if hit == Hit::Earlier {
        key::INST_MORE_ABOVE
    } else {
        key::INST_MORE_BELOW
    };
    let said = texts.t_args(
        k,
        &[(
            "count",
            Arg::Number(i64::try_from(hidden).unwrap_or(i64::MAX)),
        )],
    );
    lit_row(f, e, base, row, said, color::TEXT_SECONDARY, hit);
}

/// A row of its own that takes a click: in `tone`, and lit while the
/// pointer rests on it (`hit`).
fn lit_row(
    f: &mut ratatui::Frame,
    e: &State,
    base: Style,
    row: Rect,
    said: String,
    tone: Color,
    hit: Hit,
) {
    let style = if e.hover == Some(hit) {
        base.bg(color(color::SURFACE_HOVER))
    } else {
        base
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            said,
            Style::default().fg(color(tone)),
        )))
        .style(style),
        row,
    );
}

/// The tone of an option, which is the value of a row that acts — the
/// shell's `chrome::Kind::Setting`: an answer's, and the accent's under the
/// cursor (`text.here.acts`).
fn option_tone(here: bool) -> Color {
    if here {
        color::TEXT_HERE_ACTS
    } else {
        color::TEXT_ANSWER
    }
}

/// The list of programs' rows, as drawn: "continue", then each program's
/// name — padded to the longest, so the words stand in one column — and the
/// word that says what happens to it. NO GLYPH: the installer runs on the
/// console, whose font (PSF) has none of the icons. The tone says it too: a
/// program kept is a row that acts, its name a label's and its word an
/// answer's; one left out all dim. Under the cursor (`here`, the row's
/// index) the name takes the accent and the word the cursor's answer, as
/// every row of the shell does (`chrome::Kind::tones`).
fn checklist_lines(answer: &str, texts: &Texts, here: usize) -> Vec<Line<'static>> {
    let widest = model::OFFERED
        .iter()
        .map(|o| cells(o.name))
        .max()
        .unwrap_or_default();
    let (kept, out) = (
        texts.t(key::INST_LEFT_OUT_KEPT),
        texts.t(key::INST_LEFT_OUT_OUT),
    );
    let mut lines = vec![Line::from(texts.t(key::INST_LEFT_OUT_CONTINUE))];
    for (n, o) in model::OFFERED.iter().enumerate() {
        let left = model::is_left_out(answer, o.id);
        let (name, word) = if n + 1 == here {
            (color::STATE_ACCENT, option_tone(true))
        } else if left {
            (color::TEXT_DIM, color::TEXT_DIM)
        } else {
            (color::TEXT_LABEL_ACTS, option_tone(false))
        };
        let (name, word) = (
            Style::default().fg(color(name)),
            Style::default().fg(color(word)),
        );
        let gap = usize::from(widest.saturating_sub(cells(o.name)))
            + usize::from(ronoctua_tokens::space::INSTALLER_CHECKLISTGAP);
        lines.push(Line::from(vec![
            Span::styled(format!("{}{}", o.name, " ".repeat(gap)), name),
            Span::styled(if left { out.clone() } else { kept.clone() }, word),
        ]));
    }
    lines
}

/// The review's line of the programs left out, by name — none when nothing
/// is (or the installation is not `full`).
fn left_out_review(r: &Answers, texts: &Texts) -> Option<String> {
    let names: Vec<&str> = r
        .left_out_ids()
        .into_iter()
        .filter_map(|id| model::OFFERED.iter().find(|o| o.id == id).map(|o| o.name))
        .collect();
    (!names.is_empty())
        .then(|| texts.t_with(key::INST_LEFT_OUT_REVIEW, &[("names", &names.join(", "))]))
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
///
/// And on the console, its own palette: the slots the TUI wrote are the
/// installer's, not the shell's that comes after it.
struct Guard(Terminal<CrosstermBackend<Stdout>>, Option<Console>);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            self.0.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        if self.1.is_some() {
            let _ = crossterm::execute!(self.0.backend_mut(), Print(Console::RESET));
        }
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
/// `resume`: the answers a hop handed on, and where to pick up. `rehearsal`:
/// no `--execute` — nothing is installed, so a machine with no TPM may still
/// be looked through.
///
/// # Errors
///
/// The terminal did not accept raw mode, or reading a key failed.
pub fn collect(
    initial: &Texts,
    reach: &Reach,
    resume: Option<(Answers, Question)>,
    rehearsal: bool,
) -> Result<Collected> {
    // WHAT TAKES A MOMENT, before the screen is taken: the disks, the
    // networks in range, and the hosts the network question names (`nix`
    // answers those).
    let resumed = resume.is_some();
    let mut e = match resume {
        Some((answers, at)) => State::resumed(disks(), networks(), answers, at),
        None => State::fresh(disks(), networks()),
    };
    e.hosts = reach.hosts().join(", ");
    // THE MACHINE, before the first screen: the stops after the language,
    // and the firmware the plan installs for — BIOS already chosen when the
    // answers a hop handed on say so.
    e.platform = crate::platform::Platform::here();
    e.rehearsal = rehearsal;
    e.bios_chosen = e.answers.bios;
    e.answers.bios = !e.platform.uefi;
    if resumed {
        e.stop_first();
    }

    enable_raw_mode().context("raw mode")?;
    let mut output = std::io::stdout();
    // THE POINTER TOO (invariant 19): a terminal over the ISO's `sshd` sends
    // mouse events, and a console without a mouse sends none.
    crossterm::execute!(output, EnterAlternateScreen, EnableMouseCapture)
        .context("alternate screen")?;
    // ON THE KERNEL'S CONSOLE, the palette's tones in its slots before the
    // first frame, which paints by slot.
    let console = Console::of_term(SLOTS);
    if let Some(c) = &console {
        crossterm::execute!(output, Print(c.palette())).context("the console's palette")?;
    }
    let mut g = Guard(
        Terminal::new(CrosstermBackend::new(output)).context("terminal")?,
        console,
    );
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
        g.0.draw(|f| {
            draw(f, &e, texts);
            if let Some(c) = &g.1 {
                c.paint(f.buffer_mut());
            }
        })
        .context("draw")?;
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
                if gives_up(code, modifiers) {
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

/// How `--show` gives a screen back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// Its text alone ([`rendered`]) — for a file, and what the tests read.
    Text,
    /// Its text with its colours ([`painted`]) — for a terminal, which
    /// paints it as the TUI does.
    Painted,
    /// The same in the console's slots ([`Console`]) — for the kernel's
    /// console, as the TUI paints it there. The palette is written before
    /// the screen and left: given back, it would recolour what was printed.
    Console,
}

/// One of the screens that check the network or the repository (or the list
/// of programs, or the Compose question), drawn from a fixture as the TUI draws it, `width × height`
/// cells — `ronoctua-install --show <screen>`; `None` for a name that is not
/// one.
#[must_use]
pub fn show(
    screen: &str,
    texts: &Texts,
    (width, height): (u16, u16),
    how: Shown,
) -> Option<String> {
    let e = shown(screen)?;
    Some(match how {
        Shown::Text => rendered(&e, texts, width, height),
        Shown::Painted => painted(&e, texts, width, height, None),
        Shown::Console => painted(&e, texts, width, height, Some(&Console::resolved(SLOTS))),
    })
}

/// The fixture of the screen [`show`] draws.
fn shown(screen: &str) -> Option<State> {
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
            e.answers.set(Question::Profile, model::FULL_PROFILE.into());
            e.answers
                .set(Question::LeftOut, model::DEFAULT_LEFT_OUT.into());
            e.stay = Some(Stay::Unreachable);
            e.screen = Screen::Reviewing;
        }
        // THE COMPOSE QUESTION, whose help names the keys the default puts
        // to work.
        "compose" => at(&mut e, Question::ComposeKey),
        // THE TWO STOPS: a machine with no TPM 2.0 (a rehearsal, so its two
        // ways show), and one started from BIOS.
        "tpm" => {
            e.platform.tpm2 = false;
            e.rehearsal = true;
            e.screen = Screen::Stop(Stop::Tpm);
        }
        "bios" => {
            e.platform.uefi = false;
            e.answers.bios = true;
            e.screen = Screen::Stop(Stop::Bios);
        }
        // AND THE TPM'S, on a machine started from BIOS, chosen: its note
        // says the TPM may only be hidden.
        "tpm-bios" => {
            e.platform = crate::platform::Platform {
                uefi: false,
                tpm2: false,
            };
            e.answers.bios = true;
            e.bios_chosen = true;
            e.rehearsal = true;
            e.screen = Screen::Stop(Stop::Tpm);
        }
        // THE LIST OF PROGRAMS, with one more unmarked than the default (the
        // first marked one) and the cursor on the row below it, so a dim
        // row and the cursor's are both on screen.
        "left-out" => {
            e.answers.set(Question::Profile, model::FULL_PROFILE.into());
            at(&mut e, Question::LeftOut);
            e.answers.left_out = model::DEFAULT_LEFT_OUT.into();
            let unmarked = model::OFFERED
                .iter()
                .position(|o| o.checked)
                .unwrap_or_default();
            if let Some(o) = model::OFFERED.get(unmarked) {
                e.answers.left_out = model::toggle(&e.answers.left_out, o.id);
            }
            e.choice = (unmarked + 2).min(model::OFFERED.len());
        }
        _ => return None,
    }
    Some(e)
}

/// The screen drawn into a buffer of `width × height` cells, as the TUI
/// draws it.
fn drawn(e: &State, texts: &Texts, width: u16, height: u16) -> Option<Buffer> {
    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height)).ok()?;
    terminal.draw(|f| draw(f, e, texts)).ok()?;
    Some(terminal.backend().buffer().clone())
}

/// The screen as a terminal paints it: its text, and each run of cells in
/// its colours (24-bit SGR) and weight — what `ronoctua-install --show`
/// prints to a terminal, so a screen is looked at in its tones and not only
/// its words. On the kernel's `console`, by slot, after the palette.
#[must_use]
pub fn painted(
    e: &State,
    texts: &Texts,
    width: u16,
    height: u16,
    console: Option<&Console>,
) -> String {
    let Some(mut buffer) = drawn(e, texts, width, height) else {
        return String::new();
    };
    let sgr = |c: ratatui::style::Color, ground: u8| match c {
        ratatui::style::Color::Rgb(r, g, b) => format!("{ground}8;2;{r};{g};{b}"),
        ratatui::style::Color::Indexed(n) => format!("{ground}8;5;{n}"),
        _ => format!("{ground}9"),
    };
    let mut out = String::new();
    if let Some(c) = console {
        c.paint(&mut buffer);
        out.push_str(&c.palette());
    }
    for y in 0..height {
        let mut last = None;
        for x in 0..width {
            let Some(cell) = buffer.cell((x, y)) else {
                continue;
            };
            let bold = cell.modifier.contains(Modifier::BOLD);
            if last != Some((cell.fg, cell.bg, bold)) {
                let _ = std::fmt::Write::write_fmt(
                    &mut out,
                    format_args!(
                        "\x1b[0;{}{};{}m",
                        if bold { "1;" } else { "" },
                        sgr(cell.fg, 3),
                        sgr(cell.bg, 4)
                    ),
                );
                last = Some((cell.fg, cell.bg, bold));
            }
            out.push_str(cell.symbol());
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

/// The screen as text, at `width × height` — what the tests look at, and
/// what `ronoctua-install --show <screen>` prints for a person to look at
/// without a disk to install on.
#[must_use]
pub fn rendered(e: &State, texts: &Texts, width: u16, height: u16) -> String {
    let Some(buffer) = drawn(e, texts, width, height) else {
        return String::new();
    };
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
                // THE LIST OF PROGRAMS opens on "continue", with the
                // default marks: Enter takes them as they are.
                Question::LeftOut => {
                    assert_eq!(e.choice, 0, "the list of programs opened off its first row");
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
        // SIXTEEN screens for whoever goes by cable into a `full`
        // installation: the eighteen questions minus the offset, which is
        // computed, and the Wi-Fi password, which is skipped with no network.
        assert_eq!(views.len(), 16, "the TUI showed {views:?}");
        assert!(!views.contains(&Question::ResumeOffset));
        assert!(!views.contains(&Question::WifiPassword));
        assert!(views.contains(&Question::LeftOut));
        assert_eq!(e.answers.profile, crate::model::FULL_PROFILE);
        assert_eq!(e.answers.left_out, crate::model::DEFAULT_LEFT_OUT);
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

    /// FAILS IF: a `minimal` installation is shown the list of programs it
    /// does not carry — going forward or coming back.
    #[test]
    fn a_minimal_installation_is_not_shown_the_programs() {
        let mut e = with_disks();
        walk_to(&mut e, Question::Profile);
        e.key(KeyCode::Down);
        e.key(KeyCode::Enter);
        assert_eq!(e.answers.profile, "minimal");
        assert_eq!(e.question_of(), Some(Question::Keyboard));
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Profile));
        e.key(KeyCode::Down);
        e.key(KeyCode::Enter);
        let views = walk(&mut e, &kind, false);
        assert!(!views.contains(&Question::LeftOut), "{views:?}");
        assert_eq!(e.screen, Screen::Reviewing);
        assert!(e.answers.left_out_ids().is_empty());
        assert!(
            e.build_plan(&review_origin())
                .iter()
                .all(|s| s.label != key::INST_STEP_LEFT_OUT)
        );
    }

    /// FAILS IF: on the list of programs Enter or the space bar on a
    /// program's row does not mark and unmark it (or leaves the screen),
    /// the space bar on "continue" does anything, Enter there does not take
    /// the marks; a click takes a row it did not first select; the marks
    /// are lost going back; a row's name, its word or the keys line is cut
    /// at 60 to 100 columns in either language; or a program left out is
    /// not drawn dim, and one kept in the text's tones.
    #[test]
    fn the_checklist_toggles_by_key_and_pointer() {
        use crate::model::{DEFAULT_LEFT_OUT, OFFERED, is_left_out};
        let mut e = with_disks();
        walk_to(&mut e, Question::LeftOut);
        assert_eq!(
            (e.choice, e.answers.left_out.as_str()),
            (0, DEFAULT_LEFT_OUT)
        );
        // THE SPACE BAR ON "CONTINUE" DOES NOTHING; Enter there would go on.
        e.key(KeyCode::Char(' '));
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        assert_eq!(e.answers.left_out, DEFAULT_LEFT_OUT);
        // ENTER AND SPACE mark and unmark a program's row, and stay.
        let first = OFFERED[0].id;
        e.key(KeyCode::Down);
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        assert!(is_left_out(&e.answers.left_out, first));
        e.key(KeyCode::Char(' '));
        assert!(!is_left_out(&e.answers.left_out, first));
        e.key(KeyCode::Char(' '));
        assert!(is_left_out(&e.answers.left_out, first));
        // THE POINTER: the first click on a row selects it, the second
        // marks it — the third unmarks.
        let t = texts();
        let area = Rect::new(0, 0, 100, 34);
        let at = places(area, &e, &t);
        e.drawn(&at);
        let rows = at.list.expect("the list of programs is a list");
        let second = OFFERED[1].id;
        let y = rows.area.y + 2 - u16::try_from(rows.offset).unwrap();
        assert_eq!(at.hit(3, y), Some(Hit::Row(2)));
        let before = e.answers.left_out.clone();
        e.pointer(Pointer::Click, 3, y, &at);
        assert_eq!(
            (e.choice, &e.answers.left_out),
            (2, &before),
            "the first click took the row"
        );
        e.pointer(Pointer::Click, 3, y, &at);
        assert!(is_left_out(&e.answers.left_out, second));
        e.pointer(Pointer::Click, 3, y, &at);
        assert!(!is_left_out(&e.answers.left_out, second));
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        // THE MARKS SURVIVE GOING BACK, and coming again.
        let marked = e.answers.left_out.clone();
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Profile));
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        assert_eq!((e.choice, &e.answers.left_out), (0, &marked));
        // NEVER CUT, in either language, 60 to 100 columns: each program's
        // name with its word, the keys line whole, the help's end.
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            let (kept, out) = (t.t(key::INST_LEFT_OUT_KEPT), t.t(key::INST_LEFT_OUT_OUT));
            // THE KEYS LINE THE WIDTH TAKES: the one teaching the space bar
            // where it fits, whole either way.
            let spaced = with_keys(&t, key::INST_KEYS_CHECK_SPACE, &[]);
            let plain = with_keys(&t, key::INST_KEYS_CHECK, &[]);
            let help = t.t(key::INST_LEFT_OUT_HELP);
            let end = help
                .split_whitespace()
                .last()
                .unwrap()
                .trim_end_matches('.')
                .to_owned();
            for width in 60..=100 {
                let shown = rendered(&e, &t, width, 34);
                let keys = if spaced_fits(&spaced, width) {
                    &spaced
                } else {
                    &plain
                };
                for o in OFFERED {
                    let word = if is_left_out(&e.answers.left_out, o.id) {
                        &out
                    } else {
                        &kept
                    };
                    let row = shown
                        .lines()
                        .find(|l| l.trim_start().starts_with(o.name))
                        .unwrap_or_else(|| {
                            panic!("{language}, {width}: `{}` is not drawn:\n{shown}", o.name)
                        });
                    assert!(
                        row.trim_end().ends_with(word.as_str()),
                        "{language}, {width}: `{}` lost its word `{word}`:\n{shown}",
                        o.name
                    );
                }
                assert!(
                    shown.contains(&t.t(key::INST_LEFT_OUT_CONTINUE)),
                    "{language}, {width}"
                );
                assert!(
                    shown.lines().any(|l| l.trim_end() == keys.as_str()),
                    "{language}, {width}: the keys line is cut:\n{shown}"
                );
                assert!(
                    shown.contains(&end),
                    "{language}, {width}: the help lost its end:\n{shown}"
                );
            }
        }
        // THE TONE SAYS IT TOO, away from the cursor: a program left out
        // all dim, one kept a row that acts — its name a label's, its word
        // an answer's; under the cursor, the name the accent and the word
        // the cursor's answer.
        let buf = drawn(&e, &t, 100, 34).unwrap();
        let at = places(area, &e, &t);
        let rows = at.list.unwrap();
        let tone = |i: usize, x: u16| {
            let y = rows.area.y + u16::try_from(i - rows.offset).unwrap();
            buf.cell((rows.area.x + x, y)).unwrap().fg
        };
        let widest = OFFERED.iter().map(|o| cells(o.name)).max().unwrap();
        let word_x = widest + ronoctua_tokens::space::INSTALLER_CHECKLISTGAP;
        for (n, o) in OFFERED.iter().enumerate() {
            let i = n + 1;
            if i == e.choice {
                continue;
            }
            if is_left_out(&e.answers.left_out, o.id) {
                assert_eq!(tone(i, 0), color(color::TEXT_DIM), "{}", o.id);
                assert_eq!(tone(i, word_x), color(color::TEXT_DIM), "{}", o.id);
            } else {
                assert_eq!(tone(i, 0), color(color::TEXT_LABEL_ACTS), "{}", o.id);
                assert_eq!(tone(i, word_x), color(color::TEXT_ANSWER), "{}", o.id);
            }
        }
        // UNDER THE CURSOR, on a program kept and on one left out alike.
        let resting = e.choice;
        for o in [
            OFFERED
                .iter()
                .position(|o| is_left_out(&e.answers.left_out, o.id)),
            OFFERED
                .iter()
                .position(|o| !is_left_out(&e.answers.left_out, o.id)),
        ] {
            e.choice = o.expect("the default keeps one program and leaves one out") + 1;
            let buf = drawn(&e, &t, 100, 34).unwrap();
            let rows = places(area, &e, &t).list.unwrap();
            let y = rows.area.y + u16::try_from(e.choice - rows.offset).unwrap();
            let fg = |x: u16| buf.cell((rows.area.x + x, y)).unwrap().fg;
            assert_eq!(fg(0), color(color::STATE_ACCENT), "row {}", e.choice);
            assert_eq!(fg(word_x), color(color::TEXT_HERE_ACTS), "row {}", e.choice);
        }
        e.choice = resting;
        assert_ne!(
            color(color::TEXT_DIM),
            color(color::TEXT_LABEL_ACTS),
            "the tones would say nothing"
        );
        // ENTER ON "CONTINUE" TAKES THE MARKS and goes on.
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::Keyboard));
        assert_eq!(e.answers.left_out, marked);
        // AND THE REVIEW NAMES WHAT IS LEFT OUT, by name.
        let mut r = with_disks();
        r.answers.profile = crate::model::FULL_PROFILE.into();
        r.answers.left_out = "blender,libreoffice".into();
        r.answers.disk = "/dev/nvme0n1".into();
        r.screen = Screen::Reviewing;
        let shown = rendered(&r, &t, 100, 60);
        let said = t.t_with(
            key::INST_LEFT_OUT_REVIEW,
            &[("names", "Blender, LibreOffice")],
        );
        assert!(shown.contains(&said), "{shown}");
        assert!(
            shown.contains(&t.t(key::INST_STEP_PLAIN_DECLINING)),
            "{shown}"
        );
    }

    /// A review of a `full` installation leaving programs out of a
    /// repository with something in it, the release unreachable: the
    /// longest the plan gets.
    fn reviewing() -> State {
        let mut e = with_disks();
        e.answers.disk = "/dev/nvme0n1".into();
        e.answers.hostname = "noctua-teste".into();
        e.answers.belongings = "git@github.com:someone/belongings.git".into();
        e.answers.belongings_empty = Some(false);
        e.answers.profile = crate::model::FULL_PROFILE.into();
        e.answers.left_out = "blender,libreoffice".into();
        e.stay = Some(Stay::Unreachable);
        e.screen = Screen::Reviewing;
        e
    }

    /// The text drawn on the row `y` of a screen drawn as text.
    fn row_at(shown: &str, y: u16) -> String {
        shown
            .lines()
            .nth(usize::from(y))
            .unwrap_or_default()
            .trim_end()
            .to_owned()
    }

    /// FAILS IF: on the ISO's console (19 × 32 cells: 71 × 24 at 1366 × 768,
    /// 101 × 33 at 1920 × 1080) the review's plan is cut with nothing
    /// saying so — or says how many steps are below without how many of
    /// them erase the disk — does not scroll by the keys, the wheel and its
    /// two rows to its last step and back, loses the confirmation, or paints
    /// its rows elsewhere than the pointer takes them.
    #[test]
    fn the_review_scrolls_and_says_what_it_does_not_show() {
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            for (width, height) in [(71, 24), (101, 33)] {
                let area = Rect::new(0, 0, width, height);
                let mut e = reviewing();
                let at = places(area, &e, &t);
                e.drawn(&at);
                let plan = at.plan.expect("the review has a plan");
                assert!(
                    plan.shown < plan.count,
                    "{language} {width}×{height}: it fits, and proves nothing"
                );
                assert_eq!((plan.offset, plan.above), (0, None));
                let below = plan.below.expect("nothing says more is below");
                let all = review_lines(&e, &t);
                let hidden = &all[plan.shown..];
                let marked = hidden.iter().filter(|l| l.step == Some(true)).count();
                assert!(
                    marked > 0,
                    "{language} {width}×{height}: every step that erases is in sight"
                );
                let said = t.t_args(
                    key::INST_REVIEW_BELOW,
                    &[
                        (
                            "count",
                            Arg::Number(
                                i64::try_from(hidden.iter().filter(|l| l.step.is_some()).count())
                                    .unwrap(),
                            ),
                        ),
                        ("marked", Arg::Number(i64::try_from(marked).unwrap())),
                    ],
                );
                let shown = rendered(&e, &t, width, height);
                assert_eq!(
                    row_at(&shown, below.y),
                    said,
                    "{language} {width}×{height}:\n{shown}"
                );
                assert_eq!(at.hit(1, below.y), Some(Hit::Later));
                assert!(
                    shown.contains(&t.t(key::INST_CONFIRM)),
                    "the confirmation fell off:\n{shown}"
                );
                // DOWN TO THE LAST STEP, by the keys — every step drawn once
                // on the way, the first that erases among them.
                let steps = all.iter().filter(|l| l.step.is_some()).count();
                let mut seen = std::collections::BTreeSet::new();
                for _ in 0..all.len() {
                    let at = places(area, &e, &t);
                    e.drawn(&at);
                    let shown = rendered(&e, &t, width, height);
                    for n in 1..=steps {
                        if shown.lines().any(|l| {
                            l.trim_start_matches(['!', ' '])
                                .starts_with(&format!("{n}. "))
                        }) {
                            seen.insert(n);
                        }
                    }
                    e.key(KeyCode::Down);
                }
                assert_eq!(
                    seen.len(),
                    steps,
                    "{language} {width}×{height}: steps never reached: {seen:?}"
                );
                let at = places(area, &e, &t);
                e.drawn(&at);
                let plan = at.plan.unwrap();
                assert_eq!(
                    plan.offset + plan.shown,
                    plan.count,
                    "it did not reach the end"
                );
                assert!(plan.below.is_none());
                let above = plan.above.expect("nothing says more is above");
                assert_eq!(at.hit(1, above.y), Some(Hit::Earlier));
                // A CLICK ON "ABOVE" pages back, the wheel a line, PgUp to the top.
                let before = e.review_scroll;
                e.pointer(Pointer::Click, 1, above.y, &at);
                assert!(e.review_scroll < before, "the click on `above` did nothing");
                let at = places(area, &e, &t);
                e.drawn(&at);
                let before = e.review_scroll;
                e.pointer(Pointer::WheelDown, 1, at.plan.unwrap().area.y, &at);
                assert_eq!(
                    e.review_scroll,
                    before + 1,
                    "the wheel did not scroll the plan"
                );
                e.pointer(Pointer::WheelUp, 1, at.plan.unwrap().area.y, &at);
                assert_eq!(e.review_scroll, before);
                for _ in 0..all.len() {
                    e.key(KeyCode::PageUp);
                }
                assert_eq!(e.review_scroll, 0);
                // AND TYPING STILL TYPES the disk's path.
                type_in(&mut e, "/dev/nvme0n1");
                assert_eq!(e.confirmation, "/dev/nvme0n1");
            }
        }
    }

    /// FAILS IF: the list of programs on a console of 71 × 24 ends in the
    /// middle with nothing saying more follows, or the rows that say so do
    /// not bring the rest into sight when clicked.
    #[test]
    fn the_checklist_says_what_it_does_not_show() {
        use crate::model::OFFERED;
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            let area = Rect::new(0, 0, 71, 24);
            let mut e = with_disks();
            e.answers.language = language.to_owned();
            walk_to(&mut e, Question::LeftOut);
            let at = places(area, &e, &t);
            e.drawn(&at);
            let rows = at.list.unwrap();
            let below = rows
                .below
                .expect("the list of programs fits 71 × 24, and proves nothing");
            let hidden = rows.count - rows.offset - usize::from(rows.area.height);
            let shown = rendered(&e, &t, 71, 24);
            assert_eq!(
                row_at(&shown, below.y),
                t.t_args(
                    key::INST_MORE_BELOW,
                    &[("count", Arg::Number(i64::try_from(hidden).unwrap()))]
                ),
                "{language}:\n{shown}"
            );
            // A CLICK ON IT shows the page below — to the last program.
            e.pointer(Pointer::Click, 1, below.y, &at);
            let at = places(area, &e, &t);
            e.drawn(&at);
            let rows = at.list.unwrap();
            assert!(rows.offset > 0 && rows.below.is_none(), "{rows:?}");
            let shown = rendered(&e, &t, 71, 24);
            assert!(shown.contains(OFFERED[OFFERED.len() - 1].name), "{shown}");
            assert_eq!(
                e.question_of(),
                Some(Question::LeftOut),
                "a page took a row"
            );
            let above = rows.above.expect("nothing says more is above");
            e.pointer(Pointer::Click, 1, above.y, &at);
            let at = places(area, &e, &t);
            e.drawn(&at);
            assert_eq!(at.list.unwrap().offset, 0);
        }
    }

    /// FAILS IF: a double click on `Enter` in the keys line goes past the
    /// list of programs: the second click lands on the list's own keys
    /// line, on whatever hint stands in those columns (`Enter`, or the
    /// space bar where the line teaches it), and must only light it.
    #[test]
    fn a_double_click_on_enter_does_not_skip_the_list() {
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            for width in [60, 100] {
                let area = Rect::new(0, 0, width, 34);
                let mut e = with_disks();
                walk_to(&mut e, Question::Profile);
                let at = places(area, &e, &t);
                e.drawn(&at);
                // THE PERSON TOUCHED THE SCREEN first: `full`, by its row.
                let rows = at.list.unwrap();
                e.pointer(Pointer::Click, 2, rows.area.y, &at);
                let enter = |at: &Places| {
                    at.hints
                        .iter()
                        .find(|h| h.act == Some(Act::Key(KeyCode::Enter)))
                        .map(|h| h.x)
                        .expect("no Enter in the keys line")
                };
                let x = enter(&at);
                e.pointer(Pointer::Click, x, at.keys.y, &at);
                assert_eq!(e.question_of(), Some(Question::LeftOut));
                let at = places(area, &e, &t);
                e.drawn(&at);
                assert!(
                    matches!(at.hit(x, at.keys.y), Some(Hit::Key(_))),
                    "{language}, {width}: no hint where the Enter before stood; the test asks nothing"
                );
                let marks = e.answers.left_out.clone();
                e.pointer(Pointer::Click, x, at.keys.y, &at);
                assert_eq!(
                    (e.question_of(), &e.answers.left_out),
                    (Some(Question::LeftOut), &marks),
                    "{language}, {width}: the second click went past the list, or marked it"
                );
                // A THIRD, deliberate, on the list's own `Enter`, takes the
                // list as it is.
                e.pointer(Pointer::Click, enter(&at), at.keys.y, &at);
                assert_eq!(
                    e.question_of(),
                    Some(Question::Keyboard),
                    "{language}, {width}"
                );
            }
        }
    }

    /// FAILS IF: the click that opened a screen — the second of a double
    /// click — is taken by the screen it opened: a double click on `full`
    /// would go past the list of programs.
    #[test]
    fn a_freshly_shown_screens_cursor_row_is_not_taken_by_a_single_click() {
        let t = texts();
        let area = Rect::new(0, 0, 100, 34);
        let mut e = with_disks();
        walk_to(&mut e, Question::Profile);
        let click_cursor = |e: &mut State| {
            let at = places(area, e, &t);
            e.drawn(&at);
            let rows = at.list.unwrap();
            let y = rows.area.y + u16::try_from(e.choice - rows.offset).unwrap();
            e.pointer(Pointer::Click, 2, y, &at)
        };
        // ON THE CURSOR'S ROW of a screen just shown: selected, not taken.
        click_cursor(&mut e);
        assert_eq!(e.question_of(), Some(Question::Profile));
        click_cursor(&mut e);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        // THE NEXT SCREEN does not take the click that would have been the
        // double click's second.
        click_cursor(&mut e);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        assert_eq!(e.answers.left_out, crate::model::DEFAULT_LEFT_OUT);
        click_cursor(&mut e);
        assert_eq!(e.question_of(), Some(Question::Keyboard));
        // GOING BACK is a screen shown again: the same rule.
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        click_cursor(&mut e);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
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
        answers.set(Question::Profile, "minimal".into());
        let e = State::resumed(Vec::new(), Vec::new(), answers, Question::BelongingsAddress);
        assert_eq!(e.question_of(), Some(Question::BelongingsAddress));
        assert_eq!(e.answers.disk, "/dev/sda");
        assert_eq!(e.stay, Some(Stay::FromRelease));
        assert!(e.answers.left_out.is_empty() && e.resume_to.is_none());
    }

    /// FAILS IF: a `full` installation hopping from an installer that never
    /// asked the list of programs (every hop is from an older one) resumes
    /// past it — the default would be declined and pushed with nobody
    /// having seen it — or the list, once answered, does not go on to where
    /// the hop left; or an id the hop carried that this release no longer
    /// offers vanishes without a word; or one that carried a list it can
    /// read is asked it again.
    #[test]
    fn a_hop_without_the_list_shows_it_before_resuming() {
        let full = |left_out: &str| {
            let mut answers = Answers::default();
            answers.set(Question::Language, "en-GB".into());
            answers.set(Question::Disk, "/dev/sda".into());
            answers.set(Question::Profile, crate::model::FULL_PROFILE.into());
            answers.set(Question::LeftOut, left_out.into());
            State::resumed(Vec::new(), Vec::new(), answers, Question::BelongingsAddress)
        };
        // NO ANSWER: the list first, on its default marks and first row.
        let mut e = full("");
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        assert_eq!(
            (e.choice, e.answers.left_out.as_str()),
            (0, crate::model::DEFAULT_LEFT_OUT)
        );
        e.key(KeyCode::Down);
        e.key(KeyCode::Enter);
        e.key(KeyCode::Up);
        let marked = e.answers.left_out.clone();
        e.key(KeyCode::Enter);
        assert_eq!(
            e.question_of(),
            Some(Question::BelongingsAddress),
            "it did not go on to where the hop left"
        );
        assert_eq!(e.answers.left_out, marked);
        // AN ANSWER IT CAN READ: no list again.
        let e = full(crate::model::LEFT_OUT_NONE);
        assert_eq!(e.question_of(), Some(Question::BelongingsAddress));
        // AN ID THIS RELEASE NO LONGER OFFERS: the list, saying so.
        let mut e = full("no-such-program,blender");
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        assert_eq!(e.dropped, ["no-such-program"]);
        assert_eq!(e.answers.left_out, "blender");
        let t = Texts::fresh("en-GB").unwrap();
        let shown = rendered(&e, &t, 100, 34);
        let joined: String = shown
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("no-such-program"), "{shown}");
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::BelongingsAddress));
        assert!(e.dropped.is_empty());
        // GOING BACK from the list walks the questions again, no jump ahead.
        let mut e = full("");
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Profile));
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::LeftOut));
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::Keyboard));
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
        // THE KEYS LINE: `Esc` goes back, by the keyboard's own path — on a
        // screen just shown, the first click only lights it.
        let at = places(area, &e, &t);
        let esc = at
            .hints
            .iter()
            .position(|h| h.act == Some(Act::Key(KeyCode::Esc)))
            .expect("no Esc in the keys line");
        let x = at.hints[esc].x;
        e.pointer(Pointer::Click, x, at.keys.y, &at);
        assert_eq!(e.question_of(), Some(Question::Hostname));
        assert_eq!(e.hover, Some(Hit::Key(esc)));
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
            Next::Stay,
            "a hint of a screen just shown took the first click"
        );
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
        e.pointer(Pointer::Click, enter.x, at.keys.y, &at);
        assert_eq!(e.question_of(), Some(Question::BelongingsPassphrase));
    }

    /// FAILS IF: a language's keys line stops naming its keys the way the
    /// pointer reads them — the clicks would fall on nothing —, a hint
    /// names two keys or none, its click is not its key's, or a line names
    /// a key other than the ones its screen answers.
    #[test]
    fn every_keys_line_names_its_keys() {
        let named = |line: &str| -> Vec<&str> {
            KEYS.iter()
                .filter(|k| line.split_whitespace().any(|w| w == k.name))
                .map(|k| k.name)
                .collect()
        };
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            for (k, keys) in [
                (key::INST_KEYS_EDIT, vec![KEY_ENTER, KEY_ESC, KEY_QUIT]),
                (key::INST_KEYS_CHOOSE, vec![KEY_ARROWS, KEY_ENTER, KEY_ESC]),
                (key::INST_KEYS_CHECK, vec![KEY_ARROWS, KEY_ENTER, KEY_ESC]),
                (
                    key::INST_KEYS_CHECK_SPACE,
                    vec![KEY_ARROWS, KEY_SPACE, KEY_ENTER, KEY_ESC],
                ),
                (key::INST_KEYS_REVIEW, vec![KEY_ENTER, KEY_ESC, KEY_QUIT]),
                (key::INST_KEYS_QUIT, vec![KEY_QUIT]),
            ] {
                let line = with_keys(&t, k, &[]);
                assert_eq!(named(&line), keys, "{language}: `{k}` says {line}");
                let hints = hints_of(&line, 0);
                assert!(
                    hints.iter().any(|h| h.act.is_some()),
                    "{language}: `{k}` has no key a click can press: {hints:?}"
                );
                for h in &hints {
                    let own = named(&h.text);
                    assert_eq!(
                        own.len(),
                        1,
                        "{language}: `{k}` has a hint naming {own:?}: {}",
                        h.text
                    );
                    let act = KEYS.iter().find(|k| k.name == own[0]).unwrap().act();
                    assert_eq!(h.act, act, "{language}: `{}` clicks another key", h.text);
                }
            }
            // AND THE HOP'S SENTENCE names the key that starts the newer one.
            let hop = with_keys(&t, key::INST_RELEASE_NEWER, &[("rev", "4f1c2a9")]);
            assert_eq!(named(&hop), vec![KEY_ENTER], "{language}: {hop}");
        }
    }

    /// FAILS IF: the checklist's keys line does not teach the space bar
    /// where it fits with a cell to spare — from 61 columns in pt-BR and 67
    /// in en-GB, at 100 in both —, teaches it where it would touch the edge,
    /// or a click on the space bar's hint does not mark the cursor's row as
    /// the key does. The widths are written out: the rule is checked against
    /// the screen, never against itself.
    #[test]
    fn the_space_bar_is_taught_where_it_fits() {
        use crate::model::{OFFERED, is_left_out};
        for (language, plain_at, spaced_at) in [("pt-BR", 60, 61), ("en-GB", 66, 67)] {
            let t = Texts::fresh(language).unwrap();
            let spaced = with_keys(&t, key::INST_KEYS_CHECK_SPACE, &[]);
            let plain = with_keys(&t, key::INST_KEYS_CHECK, &[]);
            let mut e = with_disks();
            walk_to(&mut e, Question::LeftOut);
            e.key(DOWN);
            for (width, want) in [
                (60, &plain),
                (plain_at, &plain),
                (spaced_at, &spaced),
                (100, &spaced),
            ] {
                let at = places(Rect::new(0, 0, width, 34), &e, &t);
                assert_eq!(&at.key_line, want, "{language}, {width}");
            }
            // THE CLICK on `Space` marks the cursor's row (a key was pressed
            // on this screen: the click is not the second of a double one).
            let at = places(Rect::new(0, 0, 100, 34), &e, &t);
            let space = at
                .hints
                .iter()
                .find(|h| h.act == Some(Act::Key(SPACE)))
                .expect("no Space in the keys line");
            let row = OFFERED[0].id;
            let before = is_left_out(&e.answers.left_out, row);
            e.pointer(Pointer::Click, space.x, at.keys.y, &at);
            assert_ne!(
                is_left_out(&e.answers.left_out, row),
                before,
                "{language}: the click on Space did not mark the row"
            );
            assert_eq!(e.question_of(), Some(Question::LeftOut));
        }
        assert_eq!(
            ronoctua_i18n::LANGUAGES.len(),
            2,
            "a language whose widths this test does not write out"
        );
    }

    /// FAILS IF: the Compose question's help names a key in words, or not
    /// the keys the system's options put to work, in either language.
    #[test]
    fn the_compose_help_names_the_keys_the_options_set() {
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            let mut e = with_disks();
            walk_to(&mut e, Question::ComposeKey);
            let help = help_of(&e, Question::ComposeKey, &t);
            for (_, key) in model::COMPOSE_HELP_KEYS {
                assert!(
                    help.contains(key),
                    "{language}: the help does not name {key}: {help}"
                );
            }
            assert!(
                !help.contains('{') && !help.contains('$'),
                "{language}: an argument was not handed in: {help}"
            );
        }
    }

    /// FAILS IF: a taught key's code moves and its name stays, or the other
    /// way round: the line would teach a key the screen does not answer.
    #[test]
    fn a_taught_keys_name_spells_its_code() {
        for k in KEYS {
            let spelled: String = k
                .codes
                .iter()
                .map(|c| match (k.control, c) {
                    (true, KeyCode::Char(l)) => format!("Ctrl+{}", l.to_ascii_uppercase()),
                    (true, other) => format!("Ctrl+{other}"),
                    (false, KeyCode::Up) => "↑".to_owned(),
                    (false, KeyCode::Down) => "↓".to_owned(),
                    (false, other) => other.to_string(),
                })
                .collect();
            assert_eq!(
                spelled, k.name,
                "`{}` is named {} and pressed as {:?}",
                k.arg, k.name, k.codes
            );
        }
    }

    /// FAILS IF: a key a line teaches does not do what the line says, by the
    /// code the line's table names — the arrows move the cursor, Enter
    /// continues (and starts the newer installer, and runs the review),
    /// Space marks a program, Esc goes back, Ctrl+C gives up. A binding that
    /// moves away from the taught key leaves the key doing nothing here.
    #[test]
    fn every_taught_key_does_what_its_line_says() {
        use crate::model::{OFFERED, is_left_out};
        let taught = |arg: &str| *KEYS.iter().find(|k| k.arg == arg).unwrap();
        let &[up, down] = taught("arrows").codes else {
            panic!("the arrows are two keys")
        };
        let [space, enter, esc, quit] =
            ["space", "enter", "esc", "quit"].map(|a| taught(a).codes[0]);
        let mut e = with_disks();
        walk_to(&mut e, Question::Disk);
        let disk = e.screen.clone();
        e.key(down);
        assert_eq!(e.choice, 1, "the down arrow did not move the cursor");
        e.key(up);
        assert_eq!(e.choice, 0, "the up arrow did not move the cursor");
        e.key(enter);
        assert_ne!(e.screen, disk, "Enter did not continue");
        e.key(esc);
        assert_eq!(e.screen, disk, "Esc did not go back");
        // SPACE marks a program's row.
        walk_to(&mut e, Question::LeftOut);
        e.key(down);
        let before = is_left_out(&e.answers.left_out, OFFERED[0].id);
        e.key(space);
        assert_ne!(
            is_left_out(&e.answers.left_out, OFFERED[0].id),
            before,
            "Space did not mark the row"
        );
        // THE HOP: Enter starts the newer installer, Esc goes back.
        let hop = Hop {
            release: Release {
                revision: Revision {
                    rev: "4f1c2a9b7e3d5f60a1b2c3d4e5f60718293a4b5c".into(),
                    last_modified_s: 0,
                },
                url: String::new(),
                path: String::new(),
            },
        };
        e.screen = Screen::Hopping(hop.clone());
        assert_eq!(e.key(enter), Next::Hop(hop), "Enter did not hop");
        e.key(esc);
        assert!(
            matches!(e.screen, Screen::Asking(_)),
            "Esc did not go back from the hop"
        );
        // THE REVIEW: Enter runs the confirmed disk, Esc goes back.
        let mut e = with_disks();
        e.answers.disk = "/dev/nvme0n1".into();
        e.screen = Screen::Reviewing;
        e.key(esc);
        assert!(
            matches!(e.screen, Screen::Asking(_)),
            "Esc did not go back from the review"
        );
        e.screen = Screen::Reviewing;
        type_in(&mut e, "/dev/nvme0n1");
        assert_eq!(e.key(enter), Next::Run, "Enter did not run the review");
        // THE CHORD gives up with Control held, and only with it.
        assert!(gives_up(quit, KeyModifiers::CONTROL));
        assert!(
            !gives_up(quit, KeyModifiers::NONE),
            "the letter alone gave up"
        );
        assert!(!gives_up(enter, KeyModifiers::CONTROL));
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
                    // AND A `full` INSTALLATION LEAVING PROGRAMS OUT, whose
                    // plan has steps of its own in both shapes.
                    for (profile, left_out) in [("minimal", ""), ("full", "blender")] {
                        answers.belongings_empty = Some(empty);
                        answers.profile = profile.into();
                        answers.left_out = left_out.into();
                        let o = Origin {
                            aimed,
                            ..review_origin()
                        };
                        for p in steps(&answers, &o) {
                            keys.push(p.label);
                        }
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
                key::INST_KEYS_CHECK,
                key::INST_KEYS_CHECK_SPACE,
                key::INST_LEFT_OUT_CONTINUE,
                key::INST_LEFT_OUT_KEPT,
                key::INST_LEFT_OUT_OUT,
                key::INST_LEFT_OUT_REVIEW,
                key::INST_INVALID_LEFT_OUT,
                key::INST_KEPT_EVERYWHERE,
                key::INST_LEFT_OUT_REVIEW_KEPT,
                key::INST_LEFT_OUT_DROPPED,
                key::INST_LEFT_OUT_NOT_APPLIED,
                key::INST_MORE_ABOVE,
                key::INST_MORE_BELOW,
                key::INST_REVIEW_BELOW,
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

    /// Every screen the TUI draws, to `visit`: each question as the walk
    /// reaches it — with the pointer on a row, on a key and on the rows that
    /// say what a list does not show, and refusing —, the review, the run
    /// and its end, and each of `--show`'s.
    fn every_screen(visit: &mut dyn FnMut(&State)) -> usize {
        let mut seen = 0;
        let mut e = with_disks();
        let limit = Question::ALL_KEYS.len() * 4;
        loop {
            for hover in [
                None,
                Some(Hit::Row(1)),
                Some(Hit::Key(0)),
                Some(Hit::Earlier),
                Some(Hit::Later),
            ] {
                e.hover = hover;
                visit(&e);
                seen += 1;
            }
            e.hover = None;
            e.error = Some(Gate::NoLink.key());
            visit(&e);
            e.error = None;
            let Some(q) = e.question_of() else {
                break;
            };
            walk_one(&mut e);
            settle(&mut e, &kind);
            assert!(
                seen < limit * 6 && e.question_of() != Some(q),
                "the screen {q:?} does not advance"
            );
        }
        assert_eq!(e.screen, Screen::Reviewing);
        e.screen = Screen::Running;
        e.step = Some((0, 3, key::INST_RUNNING));
        e.output = vec!["a line of output".into()];
        visit(&e);
        for ok in [true, false] {
            e.screen = Screen::Finished(ok);
            visit(&e);
        }
        for name in [
            "network",
            "access",
            "token",
            "passphrase",
            "passphrase-new",
            "hop",
            "review",
            "left-out",
            "compose",
            "tpm",
            "tpm-bios",
            "bios",
        ] {
            visit(&shown(name).unwrap_or_else(|| panic!("no screen `{name}`")));
            seen += 1;
        }
        seen
    }

    /// One answer on the screen `e` is on, the walk's.
    fn walk_one(e: &mut State) {
        match e.question_of() {
            Some(Question::Hostname) => type_in(e, "noctua-teste"),
            Some(Question::Password | Question::Passphrase | Question::WifiPassword) => {
                type_in(e, "Zq7-test-phrase");
            }
            Some(Question::BelongingsAddress) => type_in(e, "git@example.org:me/b.git"),
            Some(Question::BelongingsPassphrase) => type_in(e, "Qw4-belongings-phrase"),
            _ => {}
        }
        e.key(KeyCode::Enter);
    }

    /// FAILS IF: a screen paints a colour the console has no slot for — it
    /// would come out the nearest slot's, which is another token's — or a
    /// ground outside the eight slots a background can be, which the
    /// console would draw in another slot's colour.
    #[test]
    fn every_colour_has_a_slot() {
        let t = texts();
        let console = Console::resolved(SLOTS);
        let seen = every_screen(&mut |e| {
            for (w, h) in [(100, 34), (60, 24)] {
                let off = console.unslotted(&drawn(e, &t, w, h).unwrap());
                assert!(off.is_empty(), "{:?}, {w}×{h}: {off:#?}", e.screen);
            }
        });
        assert!(seen > 50, "the walk saw {seen} screens");
    }

    /// FAILS IF: on the kernel's console the cursor's row has the ground of
    /// the rows around it, its option the tone of the others, or the
    /// question the tone of its options — the three things the ISO's console
    /// lost when it was handed 24-bit colours — or a cell keeps a 24-bit
    /// colour or the bold that turns one slot into another.
    #[test]
    fn the_console_tells_the_cursor_and_the_question_apart() {
        let t = texts();
        let console = Console::resolved(SLOTS);
        let mut e = with_disks();
        e.screen = Screen::Asking(
            Question::ALL_KEYS
                .iter()
                .position(|q| *q == Question::ComposeKey)
                .unwrap(),
        );
        e.choice = 2;
        let at = places(Rect::new(0, 0, 100, 34), &e, &t);
        let rows = at.list.unwrap();
        let mut buf = drawn(&e, &t, 100, 34).unwrap();
        console.paint(&mut buf);
        for cell in &buf.content {
            for c in [cell.fg, cell.bg] {
                assert!(
                    matches!(c, ratatui::style::Color::Indexed(n) if usize::from(n) < ronoctua_vt::SLOTS),
                    "{c:?} is not a slot"
                );
            }
            assert!(!cell.modifier.contains(Modifier::BOLD), "a bold cell");
        }
        let cell = |i: usize| {
            let y = rows.area.y + u16::try_from(i - rows.offset).unwrap();
            buf.cell((rows.area.x, y)).unwrap().clone()
        };
        let (here, other) = (cell(e.choice), cell(e.choice + 1));
        assert_ne!(here.bg, other.bg, "the cursor's row has no band");
        assert_ne!(
            here.fg, other.fg,
            "the cursor's option is in the others' tone"
        );
        let question = buf.cell((at.head.x, at.head.y + 1)).unwrap();
        assert_ne!(
            question.fg, other.fg,
            "the question is in its options' tone"
        );
        assert_ne!(question.fg, here.fg, "the question is in the cursor's tone");
    }

    /// The state on the language's screen, on this machine.
    fn on_a(platform: crate::platform::Platform, rehearsal: bool) -> State {
        let mut e = with_disks();
        e.platform = platform;
        e.rehearsal = rehearsal;
        e.answers.bios = !platform.uefi;
        e
    }

    /// FAILS IF: a machine with no TPM 2.0 goes past the language to any
    /// question — the installation would seal its secrets into nothing —, its
    /// stop opens off its way out or offers an installation another way,
    /// Enter there does not leave, Esc does not go back to the language, or a
    /// rehearsal cannot look through the rest.
    #[test]
    fn a_machine_with_no_tpm_installs_nothing() {
        use crate::platform::Platform;
        let none = Platform {
            uefi: true,
            tpm2: false,
        };
        let mut e = on_a(none, false);
        assert_eq!(e.question_of(), Some(Question::Language));
        e.key(KeyCode::Enter);
        assert_eq!(e.screen, Screen::Stop(Stop::Tpm));
        assert_eq!(e.options().len(), 1, "an installation has one way: out");
        e.key(KeyCode::Down);
        assert_eq!(e.choice, 0);
        assert_eq!(e.key(KeyCode::Enter), Next::GiveUp);
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Language));
        // A REHEARSAL may look through the rest — after the stop.
        let mut r = on_a(none, true);
        r.key(KeyCode::Enter);
        assert_eq!(r.screen, Screen::Stop(Stop::Tpm));
        assert_eq!(r.choice, 0, "the cursor is born on the way out");
        r.key(KeyCode::Down);
        assert_eq!(r.key(KeyCode::Enter), Next::Stay);
        assert_eq!(r.question_of(), Some(Question::Disk));
    }

    /// FAILS IF: a machine started from BIOS goes on without its stop, the
    /// stop's cursor is born anywhere but on the way out, the worse way does
    /// not go on (BIOS installs), or the choice is asked again on the way
    /// back — and, with no TPM either, a rehearsal skips the BIOS's stop.
    #[test]
    fn bios_goes_on_only_by_the_worse_way() {
        use crate::platform::Platform;
        let bios = Platform {
            uefi: false,
            tpm2: true,
        };
        let mut e = on_a(bios, false);
        e.key(KeyCode::Enter);
        assert_eq!(e.screen, Screen::Stop(Stop::Bios));
        assert_eq!(e.choice, 0, "the cursor is born on the way out");
        assert_eq!(e.key(KeyCode::Enter), Next::GiveUp, "the first row leaves");
        e.key(KeyCode::Down);
        assert_eq!(e.key(KeyCode::Enter), Next::Stay);
        assert!(e.bios_chosen && e.answers.bios);
        assert_eq!(e.question_of(), Some(Question::Disk));
        e.key(KeyCode::Esc);
        assert_eq!(e.question_of(), Some(Question::Language));
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(Question::Disk), "chosen once");
        let mut both = on_a(
            Platform {
                uefi: false,
                tpm2: false,
            },
            true,
        );
        // BIOS FIRST: in BIOS mode the TPM is often hidden, and UEFI is the
        // answer to both; the TPM's stop comes after the worse way.
        both.key(KeyCode::Enter);
        assert_eq!(both.screen, Screen::Stop(Stop::Bios));
        both.key(KeyCode::Down);
        both.key(KeyCode::Enter);
        assert_eq!(both.screen, Screen::Stop(Stop::Tpm));
    }

    /// FAILS IF: an installation a hop resumed — past the language, where
    /// the stops stand — goes on without them on a machine with no TPM or
    /// started from BIOS, does not come back to where it resumed once they
    /// are passed, or asks the BIOS again of whoever chose it before the hop.
    #[test]
    fn a_resumed_installation_stops_too() {
        use crate::platform::Platform;
        let mut handed = Answers::default();
        handed.set(Question::Language, "pt-BR".into());
        let at = Question::BelongingsAddress;
        let resumed = |platform: Platform, answers: &Answers, rehearsal: bool| {
            let mut e = State::resumed(Vec::new(), Vec::new(), answers.clone(), at);
            e.platform = platform;
            e.rehearsal = rehearsal;
            e.bios_chosen = e.answers.bios;
            e.answers.bios = !platform.uefi;
            e.stop_first();
            e
        };
        let bios = Platform {
            uefi: false,
            tpm2: true,
        };
        let mut e = resumed(bios, &handed, false);
        assert_eq!(e.screen, Screen::Stop(Stop::Bios));
        e.key(KeyCode::Down);
        e.key(KeyCode::Enter);
        assert_eq!(e.question_of(), Some(at), "back where it resumed");
        // CHOSEN BEFORE THE HOP: not asked again.
        let mut chosen = handed.clone();
        chosen.bios = true;
        let e = resumed(bios, &chosen, false);
        assert_eq!(e.question_of(), Some(at));
        // NO TPM: stopped, and only a rehearsal goes on.
        let none = Platform {
            uefi: true,
            tpm2: false,
        };
        let mut e = resumed(none, &handed, false);
        assert_eq!(e.screen, Screen::Stop(Stop::Tpm));
        assert_eq!(e.key(KeyCode::Enter), Next::GiveUp);
        let mut r = resumed(none, &handed, true);
        r.key(KeyCode::Down);
        r.key(KeyCode::Enter);
        assert_eq!(r.question_of(), Some(at));
        // A machine with nothing to say resumes where it was handed.
        let e = resumed(Platform::default(), &handed, false);
        assert_eq!(e.question_of(), Some(at));
    }

    /// FAILS IF: a stop's screen cuts its explanation — what the TPM or UEFI
    /// is, why it matters and how to switch it on, which is the screen's whole
    /// point —, loses its heading or a way, on a console as narrow as 60 × 24
    /// or as wide as 100 × 34, in any language; or a click takes a way it did
    /// not first select.
    #[test]
    fn a_stop_is_drawn_whole_and_answers_the_pointer() {
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            for name in ["tpm", "tpm-bios", "bios"] {
                let e = shown(name).unwrap();
                let Screen::Stop(stop) = e.screen else {
                    panic!("`{name}` is no stop");
                };
                let told = stop.told();
                let after = told
                    .after_in_bios
                    .filter(|_| e.answers.bios)
                    .unwrap_or(told.after);
                let parts = [told.what, told.why, told.how, after]
                    .iter()
                    .chain(told.steps)
                    .map(|k| t.t_with(k, &SETUP_KEYS))
                    .collect::<Vec<_>>();
                for (w, h) in [(60, 24), (100, 34)] {
                    let text = rendered(&e, &t, w, h);
                    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    for part in &parts {
                        let words: Vec<&str> = part.split_whitespace().collect();
                        let end = words[words.len().saturating_sub(3)..].join(" ");
                        assert!(
                            flat.contains(&end),
                            "{language} {name} {w}×{h}: lost `{end}`:\n{text}"
                        );
                    }
                    assert!(
                        text.contains(&t.t(stop.heading())),
                        "{language} {name} {w}×{h}:\n{text}"
                    );
                    for way in stop.ways(e.rehearsal) {
                        let row = t.t(way.key(stop));
                        assert!(
                            text.contains(&row),
                            "{language} {name} {w}×{h}: no `{row}`:\n{text}"
                        );
                    }
                }
            }
        }
        let t = texts();
        let mut e = shown("bios").unwrap();
        let at = places(Rect::new(0, 0, 100, 34), &e, &t);
        let rows = at.list.expect("a stop is a list");
        let (x, y) = (rows.area.x + 1, rows.area.y + 1);
        assert_eq!(e.pointer(Pointer::Click, x, y, &at), Next::Stay);
        assert_eq!(e.choice, 1);
        assert!(!e.bios_chosen, "the first click only selects");
        e.pointer(Pointer::Click, x, y, &at);
        assert!(e.bios_chosen, "the second takes it");
    }
}
