//! The installer's TUI — milestone 8's criterion, section 7.1.
//!
//! "The TUI collects **only** disk, hostname, password, passphrase, language and
//! `resume_offset`."
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
//! **8:** no label is a literal; everything comes from `ronoctua_i18n::key`. A
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

use std::io::Stdout;
use std::time::Duration;

use anyhow::{Context as _, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use ronoctua_i18n::{Texts, key};
use ronoctua_tokens::{Color, color};

use crate::model::{Answers, Question, validate_it};
use crate::plan::{Step, steps};

/// Converts a colour token into what ratatui understands.
///
/// Node `to_linear`: the terminal receives sRGB, unlike wgpu. Passing
/// linear here would darken everything — the mirror image of the mistake that cost an afternoon
/// in the renderer, and it is worth saying which side we are on.
const fn color(c: Color) -> ratatui::style::Color {
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
    let Ok(output) = std::process::Command::new("lsblk")
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
    /// Showing the plan, waiting for confirmation.
    Reviewing,
    /// Running.
    Running,
    /// Finished. `Ok` when it worked.
    Finished(bool),
}

/// What the TUI has read so far.
///
/// Separated from the drawing on purpose: this is what the tests exercise, and the
/// drawing is what cannot be tested without a terminal.
#[derive(Debug)]
pub struct State {
    /// Where we are.
    pub screen: Screen,
    /// What has been answered.
    pub answers: Answers,
    /// What is being typed in the current field.
    pub typing: String,
    /// The current field's validation error, if any.
    pub error: Option<String>,
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
    /// Which list item is selected.
    pub choice: usize,
    /// The lines the execution printed.
    pub output: Vec<String>,
    /// Which step is running.
    pub step: Option<(usize, usize, &'static str)>,
}

impl State {
    /// It starts on the first question.
    #[must_use]
    pub fn fresh(disks: Vec<Disk>, networks: Vec<String>) -> Self {
        Self {
            screen: Screen::Asking(0),
            networks,
            answers: Answers::default(),
            typing: String::new(),
            error: None,
            confirmation: String::new(),
            disks,
            choice: 0,
            output: Vec::new(),
            step: None,
        }
    }

    /// The current screen's question.
    #[must_use]
    pub fn question_of(&self) -> Option<Question> {
        match self.screen {
            Screen::Asking(n) => Question::ALL_KEYS.get(n).copied(),
            _ => None,
        }
    }

    /// Whether the current question is answered by choosing from a list.
    #[must_use]
    pub fn is_list(&self) -> bool {
        matches!(
            self.question_of(),
            Some(Question::Disk | Question::Language | Question::Wifi)
        )
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
            // "NO WI-FI" IS THE FIRST OPTION, and not the last.
            //
            // Whoever has a cable does not want to scroll through a list of the neighbour's networks
            // to say they want none. And, with it first, the
            // default for whoever just presses Enter is the one that asks for no secret.
            Some(Question::Wifi) => std::iter::once(String::new())
                .chain(self.networks.iter().cloned())
                .collect(),
            _ => Vec::new(),
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
            // Index 0 is "no Wi-Fi", and returns an empty string — which is
            // exactly what `Question::optional` accepts and what makes the
            // password screen be skipped.
            Some(Question::Wifi) => self
                .choice
                .checked_sub(1)
                .and_then(|i| self.networks.get(i).cloned())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Advances to the next question that gets typed, or to the review.
    ///
    /// It skips the computed ones: the `resume_offset` is on section 7.1's list
    /// because the installer has to produce it, not because anyone knows it.
    fn follow(&mut self, n: usize) {
        let mut i = n + 1;
        while let Some(q) = Question::ALL_KEYS.get(i) {
            if !q.computed() && !q.skipped_one(&self.answers) {
                self.screen = Screen::Asking(i);
                self.typing.clear();
                self.choice = 0;
                self.error = None;
                return;
            }
            i += 1;
        }
        self.screen = Screen::Reviewing;
        self.error = None;
    }

    /// Goes back one question. From the first, it does not go back.
    fn go_back(&mut self) {
        let current = match self.screen {
            Screen::Asking(n) => n,
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
            self.typing = self.answers.get(*q).to_owned();
            self.error = None;
            return;
        }
    }

    /// Applies a key. Returns `true` when the TUI should execute.
    ///
    /// **Testable without a terminal**: it is the reason key reading and
    /// drawing are in functions other than this one.
    pub fn key(&mut self, k: KeyCode, texts: &Texts) -> bool {
        let languages: Vec<&str> = ronoctua_i18n::LANGUAGES.to_vec();
        // THE LAYOUTS THE ISO CARRIES, read from xkeyboard-config itself.
        //
        // A hand-written list would go stale, and the failure would be a layout
        // that exists in the system and that the installer refuses. Reading the
        // real file means the two can never disagree.
        let layouts = crate::model::available_layouts();
        let layouts: Vec<&str> = layouts.iter().map(String::as_str).collect();
        match self.screen {
            Screen::Asking(n) => {
                let Some(q) = Question::ALL_KEYS.get(n).copied() else {
                    return false;
                };
                match k {
                    KeyCode::Esc => self.go_back(),
                    KeyCode::Up if self.is_list() => self.choice = self.choice.saturating_sub(1),
                    KeyCode::Down if self.is_list() => {
                        let last = self.options().len().saturating_sub(1);
                        self.choice = (self.choice + 1).min(last);
                    }
                    KeyCode::Backspace if !self.is_list() => {
                        self.typing.pop();
                        self.error = None;
                    }
                    KeyCode::Char(c) if !self.is_list() => {
                        self.typing.push(c);
                        self.error = None;
                    }
                    KeyCode::Enter => {
                        let v = if self.is_list() {
                            self.choice_value()
                        } else {
                            self.typing.clone()
                        };
                        match validate_it(q, &v, &languages, &layouts) {
                            Ok(()) => {
                                self.answers.set(q, v);
                                self.follow(n);
                            }
                            // The message comes from `Invalid::Display`, which is
                            // the MODEL's text and not the screen's — and the only
                            // place where the TUI shows something that did not come from
                            // a key, because validation belongs to the
                            // model and not to the drawing.
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                    _ => {}
                }
                false
            }
            Screen::Reviewing => match k {
                KeyCode::Esc => {
                    self.go_back();
                    false
                }
                KeyCode::Backspace => {
                    self.confirmation.pop();
                    self.error = None;
                    false
                }
                KeyCode::Char(c) => {
                    self.confirmation.push(c);
                    self.error = None;
                    false
                }
                KeyCode::Enter => {
                    // Typing the disk's path, and not "yes". Copying the
                    // path forces one to READ the path, which is the one thing
                    // on this screen nobody can get wrong.
                    if self.confirmation.trim() == self.answers.disk {
                        self.screen = Screen::Running;
                        true
                    } else {
                        self.error = Some(texts.t(key::INST_CONFIRM_ERROR));
                        false
                    }
                }
                _ => false,
            },
            Screen::Running | Screen::Finished(_) => false,
        }
    }

    /// The steps the review shows.
    #[must_use]
    pub fn build_plan(&self) -> Vec<Step> {
        steps(&self.answers, "/mnt/etc/nixos")
    }
}

/// Draws the screen.
fn draw(f: &mut ratatui::Frame, e: &State, texts: &Texts) {
    let base = Style::default()
        .fg(color(color::TEXT_PRIMARY))
        .bg(color(color::SURFACE_BASE));
    f.render_widget(ratatui::widgets::Block::default().style(base), f.area());

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // title + label
            Constraint::Min(1),    // the field, or the list, or the output
            Constraint::Length(2), // help + keys — the usual two lines
        ])
        .split(f.area());

    let title = match e.screen {
        Screen::Asking(_) => texts.t(key::INST_TITLE),
        Screen::Reviewing => texts.t(key::INST_REVIEW),
        Screen::Running => texts.t(key::INST_RUNNING),
        Screen::Finished(true) => texts.t(key::INST_DONE),
        Screen::Finished(false) => texts.t(key::INST_FAILED),
    };
    let label = e
        .question_of()
        .map(|q| texts.t(q.label()))
        .unwrap_or_default();
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                title,
                Style::default()
                    .fg(color(color::STATE_ACCENT))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                label,
                Style::default().fg(color(color::TEXT_PRIMARY)),
            )),
        ])
        .style(base),
        areas[0],
    );

    body(f, e, texts, areas[1], base);

    let help = match e.screen {
        Screen::Asking(_) => e
            .question_of()
            .map(|q| texts.t(q.help()))
            .unwrap_or_default(),
        Screen::Reviewing => texts.t(key::INST_DESTRUCTIVE),
        _ => String::new(),
    };
    let keys = match e.screen {
        Screen::Asking(_) if e.is_list() => texts.t(key::INST_KEYS_CHOOSE),
        Screen::Asking(_) => texts.t(key::INST_KEYS_EDIT),
        Screen::Reviewing => texts.t(key::INST_KEYS_REVIEW),
        _ => texts.t(key::INST_KEYS_QUIT),
    };
    let dim = Style::default().fg(color(color::TEXT_DIM));
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                e.error.clone().unwrap_or(help),
                if e.error.is_some() {
                    Style::default().fg(color(color::STATE_ERROR))
                } else {
                    dim
                },
            )),
            Line::from(Span::styled(keys, dim)),
        ])
        .style(base),
        areas[2],
    );
}

/// The middle part, which changes with the screen.
fn body(f: &mut ratatui::Frame, e: &State, texts: &Texts, area: Rect, base: Style) {
    match e.screen {
        Screen::Asking(_) if e.is_list() => {
            let options = e.options();
            if options.is_empty() {
                f.render_widget(
                    Paragraph::new(texts.t(key::INST_NO_DISK))
                        .style(base.fg(color(color::STATE_ERROR))),
                    area,
                );
                return;
            }
            let items: Vec<ListItem> = options.into_iter().map(ListItem::new).collect();
            let mut st = ListState::default();
            st.select(Some(e.choice));
            f.render_stateful_widget(
                List::new(items).style(base).highlight_style(
                    Style::default()
                        .fg(color(color::TEXT_PRIMARY))
                        .bg(color(color::SURFACE_SELECTION)),
                ),
                area,
                &mut st,
            );
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
            f.render_widget(Paragraph::new(format!("{shown}_")).style(base), area);
        }
        Screen::Reviewing => {
            let mut lines: Vec<Line> = Vec::new();
            for (i, p) in e.build_plan().iter().enumerate() {
                let mark = if p.destructive { "!" } else { " " };
                let estilo = if p.destructive {
                    Style::default().fg(color(color::STATE_CRITICAL))
                } else {
                    Style::default().fg(color(color::TEXT_SECONDARY))
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark} {}. {}", i + 1, texts.t(p.label)),
                    estilo,
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("{}: {}", texts.t(key::INST_CONFIRM), e.answers.disk),
                Style::default().fg(color(color::STATE_WARNING)),
            )));
            lines.push(Line::from(Span::styled(
                format!("{}_", e.confirmation),
                Style::default().fg(color(color::TEXT_PRIMARY)),
            )));
            f.render_widget(
                Paragraph::new(lines).style(base).wrap(Wrap { trim: true }),
                area,
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
            let cabem = usize::from(area.height).saturating_sub(lines.len()).max(1);
            let start = e.output.len().saturating_sub(cabem);
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
        let _ = crossterm::execute!(self.0.backend_mut(), LeaveAlternateScreen);
        let _ = self.0.show_cursor();
    }
}

/// Runs the TUI. Returns the answers once the person confirms.
///
/// `None` when they give up — and giving up is leaving without having touched anything.
///
/// # Errors
///
/// The terminal did not accept raw mode, or reading a key failed.
pub fn collect(texts: &Texts) -> Result<Option<Answers>> {
    enable_raw_mode().context("raw mode")?;
    let mut output = std::io::stdout();
    crossterm::execute!(output, EnterAlternateScreen).context("tela alternativa")?;
    let mut g = Guard(Terminal::new(CrosstermBackend::new(output)).context("terminal")?);

    let mut e = State::fresh(disks(), networks());
    loop {
        g.0.draw(|f| draw(f, &e, texts)).context("desenhar")?;
        if !event::poll(Duration::from_millis(100)).context("poll")? {
            continue;
        }
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event::read().context("ler tecla")?
        else {
            continue;
        };
        // `Release` also arrives in a terminal with the kitty keyboard protocol.
        // Without this filter every key counts twice, and the field fills up with
        // doubled letters.
        if kind != KeyEventKind::Press {
            continue;
        }
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            return Ok(None);
        }
        if e.key(code, texts) {
            return Ok(Some(e.answers));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> Texts {
        Texts::fresh("pt-BR").expect("pt-BR tem de carregar")
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
            vec!["casa-5g".into(), "casa-2g".into()],
        )
    }

    #[test]
    fn lsblk_with_a_multi_word_model() {
        // `Samsung SSD 980 PRO` has spaces. A parser by positional field
        // would return `Samsung` and throw the rest away — and the worst part is that it
        // would work, only showing the wrong model.
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

    #[test]
    fn the_questions_are_walked_and_only_them() {
        // Milestone 8's literal criterion. If a question enters the enum and the
        // TUI does not show it, or if it shows something outside the enum, this is where
        // it appears.
        let mut e = with_disks();
        let t = texts();
        let mut views = Vec::new();
        while let Some(q) = e.question_of() {
            views.push(q);
            match q {
                Question::Disk | Question::Language => {
                    e.key(KeyCode::Enter, &t);
                }
                // The layout: typed, because the default of the list is not
                // necessarily the one this machine wants.
                Question::Keyboard => {
                    for c in "br".chars() {
                        e.key(KeyCode::Char(c), &t);
                    }
                    e.key(KeyCode::Enter, &t);
                }
                Question::Hostname => {
                    for c in "noctua-teste".chars() {
                        e.key(KeyCode::Char(c), &t);
                    }
                    e.key(KeyCode::Enter, &t);
                }
                Question::Password | Question::Passphrase => {
                    for c in "Zq7-frase-de-teste".chars() {
                        e.key(KeyCode::Char(c), &t);
                    }
                    e.key(KeyCode::Enter, &t);
                }
                // The network: Enter on the first item, which is "no Wi-Fi".
                Question::Wifi => {
                    e.key(KeyCode::Enter, &t);
                }
                Question::WifiPassword => {
                    panic!("a senha apareceu sem rede escolhida")
                }
                Question::ResumeOffset => panic!("a TUI nao pergunta o offset"),
            }
        }
        assert_eq!(e.screen, Screen::Reviewing);
        // SEVEN screens for whoever goes by cable: the six that existed plus the
        // keyboard. The offset and the Wi-Fi password are not asked here — the
        // asserts below are what hold that.
        assert_eq!(views.len(), 7, "a TUI mostrou {views:?}");
        assert!(!views.contains(&Question::ResumeOffset));
        assert!(
            !views.contains(&Question::WifiPassword),
            "quem vai de cabo nao devia ver a tela de senha"
        );
        assert_eq!(e.answers.disk, "/dev/nvme0n1");
        assert_eq!(e.answers.hostname, "noctua-teste");
        assert_eq!(e.answers.language, ronoctua_i18n::LANGUAGES[0]);
        // The typed layout reached the answers. Without this the question could
        // be walked past and the machine would still come up on the default.
        assert_eq!(e.answers.keyboard, "br");
        assert!(e.answers.wifi_ssid.is_empty(), "escolheu rede sem querer");
    }

    #[test]
    fn choosing_a_network_makes_the_password_screen_appear() {
        // The other side of the test above. Without it, `skipped_one` could be
        // always true and the password would never be asked for — the installation
        // would write a profile with no password, which fails to connect.
        let mut e = with_disks();
        let t = texts();
        let mut views = Vec::new();
        while let Some(q) = e.question_of() {
            views.push(q);
            match q {
                Question::Disk | Question::Language => {
                    e.key(KeyCode::Enter, &t);
                }
                // The layout: typed, because the default of the list is not
                // necessarily the one this machine wants.
                Question::Keyboard => {
                    for c in "br".chars() {
                        e.key(KeyCode::Char(c), &t);
                    }
                    e.key(KeyCode::Enter, &t);
                }
                // The second option is the first actual network: index 0
                // is "no Wi-Fi".
                Question::Wifi => {
                    e.key(KeyCode::Down, &t);
                    e.key(KeyCode::Enter, &t);
                }
                Question::Hostname => {
                    for c in "noctua-teste".chars() {
                        e.key(KeyCode::Char(c), &t);
                    }
                    e.key(KeyCode::Enter, &t);
                }
                Question::Password | Question::Passphrase | Question::WifiPassword => {
                    for c in "Zq7-frase-de-teste".chars() {
                        e.key(KeyCode::Char(c), &t);
                    }
                    e.key(KeyCode::Enter, &t);
                }
                Question::ResumeOffset => panic!("a TUI nao pergunta o offset"),
            }
        }
        assert_eq!(e.screen, Screen::Reviewing);
        assert!(views.contains(&Question::WifiPassword), "nao pediu a senha");
        assert_eq!(e.answers.wifi_ssid, "casa-5g");
        assert_eq!(e.answers.wifi_password, "Zq7-frase-de-teste");
    }

    #[test]
    fn an_invalid_answer_does_not_advance() {
        let mut e = with_disks();
        let t = texts();
        e.key(KeyCode::Enter, &t); // disk
        assert_eq!(e.screen, Screen::Asking(1));
        for c in "semprefixo".chars() {
            e.key(KeyCode::Char(c), &t);
        }
        e.key(KeyCode::Enter, &t);
        assert_eq!(e.screen, Screen::Asking(1), "avancou com hostname invalido");
        assert!(e.error.is_some(), "e nem disse por que");
    }

    #[test]
    fn esc_goes_back_and_brings_back_what_was_typed() {
        // Going back and losing the answer would be worse than not being able to go back: the
        // person would stop checking for fear of retyping everything.
        let mut e = with_disks();
        let t = texts();
        e.key(KeyCode::Enter, &t);
        for c in "noctua-teste".chars() {
            e.key(KeyCode::Char(c), &t);
        }
        e.key(KeyCode::Enter, &t);
        assert_eq!(e.screen, Screen::Asking(2));
        e.key(KeyCode::Esc, &t);
        assert_eq!(e.screen, Screen::Asking(1));
        assert_eq!(e.typing, "noctua-teste");
    }

    #[test]
    fn from_the_first_question_esc_does_not_leave() {
        // Leaving the first screen with Esc would erase the distinction between "go back"
        // and "give up". Giving up is Ctrl+C, and that is all.
        let mut e = with_disks();
        e.key(KeyCode::Esc, &texts());
        assert_eq!(e.screen, Screen::Asking(0));
    }

    #[test]
    fn the_confirmation_requires_the_disks_path() {
        let mut e = with_disks();
        let t = texts();
        e.answers.disk = "/dev/nvme0n1".into();
        e.screen = Screen::Reviewing;

        for c in "sim".chars() {
            e.key(KeyCode::Char(c), &t);
        }
        assert!(!e.key(KeyCode::Enter, &t), "`sim` armou a instalacao");
        assert!(e.error.is_some());
        assert_ne!(e.screen, Screen::Running);

        e.confirmation.clear();
        for c in "/dev/nvme0n1".chars() {
            e.key(KeyCode::Char(c), &t);
        }
        assert!(e.key(KeyCode::Enter, &t), "o caminho certo nao armou");
        assert_eq!(e.screen, Screen::Running);
    }

    #[test]
    fn the_confirmation_is_of_the_chosen_disk_and_not_another() {
        let mut e = with_disks();
        let t = texts();
        e.answers.disk = "/dev/sda".into();
        e.screen = Screen::Reviewing;
        for c in "/dev/nvme0n1".chars() {
            e.key(KeyCode::Char(c), &t);
        }
        assert!(!e.key(KeyCode::Enter, &t), "confirmou o disco errado");
    }

    #[test]
    fn no_label_on_screen_comes_out_as_a_key() {
        // Invariant 8 at the last link. A key missing from the `.ftl` files comes out
        // as `[inst-disk]` on screen — with no error, no warning, and visible only
        // to whoever installs. That is how it was.
        for language in ronoctua_i18n::LANGUAGES {
            let t = Texts::fresh(language).unwrap();
            let mut keys: Vec<&str> = Vec::new();
            for q in Question::ALL_KEYS {
                keys.push(q.label());
                keys.push(q.help());
            }
            for p in steps(&Answers::default(), "f") {
                keys.push(p.label);
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
            ]);
            for c in keys {
                let s = t.t(c);
                assert!(
                    !s.starts_with('['),
                    "{language}: `{c}` saiu como chave crua"
                );
            }
        }
    }

    #[test]
    fn the_list_does_not_go_past_the_ends() {
        let mut e = with_disks();
        let t = texts();
        e.key(KeyCode::Up, &t);
        assert_eq!(e.choice, 0, "subiu acima do primeiro");
        for _ in 0..10 {
            e.key(KeyCode::Down, &t);
        }
        assert_eq!(e.choice, 1, "desceu abaixo do ultimo");
    }

    #[test]
    fn with_no_disk_at_all_enter_does_not_advance() {
        // An ISO running where `lsblk` lists nothing: advancing would write an
        // empty disk and `disko` would fail much later, with a message
        // that does not point back here.
        let mut e = State::fresh(Vec::new(), Vec::new());
        e.key(KeyCode::Enter, &texts());
        assert_eq!(e.screen, Screen::Asking(0));
        assert!(e.error.is_some());
    }
}
