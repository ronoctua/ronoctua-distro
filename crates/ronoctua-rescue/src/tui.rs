//! The screen: sidebar on the left, content on the right.
//!
//! # The two invariants a rescue console has to keep anyway
//!
//! **8:** not one sentence is written here. Everything the person reads comes
//! from `ronoctua_i18n::key`, and the console that would say `[rescue-reboot]`
//! on the day a key went missing is the console nobody could use.
//!
//! **9:** not one colour by hand. `to_rgba8()` and never `to_linear()` — the
//! terminal takes sRGB, which is the opposite of what the renderer takes, and
//! passing linear here would darken everything (see the installer's TUI, which
//! learned this first).
//!
//! # And the one it keeps by construction
//!
//! **14:** this is not chrome. It is a program on a terminal — a window the
//! compositor floats by a generated rule, exactly like the process monitor, or
//! a VT that is not on the desktop at all.

use std::io::{Stdout, Write as _};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use ronoctua_i18n::{Texts, key};
use ronoctua_tokens::{Color, color};

use crate::view::{Hand, Key, Pane, Step, View, logs, papers};
use crate::{Action, Own, Run, Section, Tier, action, of_section};

/// A token, as this terminal wants it.
///
/// `to_rgba8()` and NOT `to_linear()`: the terminal is sRGB. The alpha is
/// dropped — a terminal cell has no transparency to give.
const fn colour(c: Color) -> ratatui::style::Color {
    let [r, g, b, _] = c.to_rgba8();
    ratatui::style::Color::Rgb(r, g, b)
}

/// How wide the sidebar is, in cells.
///
/// Fixed and not proportional: on an 80-column VT a proportional sidebar would
/// take a quarter of a screen that has no room to spare, and the seven words in
/// it do not get longer on a wider screen.
const SIDEBAR: u16 = 18;

/// Puts the terminal back however this ends.
///
/// It is the same guard the installer has, and for the reason written there: a
/// `panic!` in the middle of drawing must not leave a terminal in raw mode. Here
/// it matters more — the terminal it would ruin may be the only one the person
/// still has.
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

/// Opens the console and runs it until the person leaves.
///
/// # Errors
///
/// Only what the terminal itself refuses. Everything a ROW does is reported on
/// the screen instead of ending the program: a rescue console that exits
/// because a unit failed to restart is a rescue console that leaves at the
/// worst moment.
pub fn run() -> Result<()> {
    let texts = Texts::from_env();
    // polkit's agent, for the VT where no desktop registered one. It has to
    // outlive the loop: dropping it takes the agent with it.
    let _agent = action::agent();

    let mut output = std::io::stdout();
    enable_raw_mode().context("raw mode")?;
    // The mouse is a bonus and never a requirement: on a VT nothing answers
    // this, and the console goes on being driven by the keyboard.
    let _ = crossterm::execute!(output, EnterAlternateScreen, EnableMouseCapture);
    let mut guard = Guard(Terminal::new(CrosstermBackend::new(output)).context("terminal")?);

    let mut view = View::fresh();
    let places = ronoctua_memory::Places::default();
    view.panes = panes_of();
    let mut state_lines = state_of(&places, &texts);

    loop {
        guard
            .0
            .draw(|f| draw(f, &view, &texts, &state_lines))
            .context("desenhar")?;

        if !event::poll(Duration::from_millis(200)).context("wait for a key")? {
            continue;
        }
        let Some(k) = read_key()? else { continue };
        match view.key(k) {
            Step::Still => {}
            Step::Draw => {}
            Step::Leave => return Ok(()),
            Step::Run(a) => {
                view.said = Some(say_of(a, &view, &places, &texts));
                view.panes = panes_of();
                state_lines = state_of(&places, &texts);
            }
            Step::Hand(h) => {
                hand_over(&mut guard, &h, &texts)?;
                view.panes = panes_of();
                state_lines = state_of(&places, &texts);
            }
        }
    }
}

/// One event, turned into what it MEANS.
///
/// The mouse becomes the same vocabulary as the keyboard on purpose: a click on
/// the sidebar is [`Key::Pick`], which is the key that goes straight to a
/// section. Two vocabularies would be two behaviours to keep in step, and the
/// one nobody tests is the mouse's.
fn read_key() -> Result<Option<Key>> {
    Ok(match event::read().context("ler evento")? {
        Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) => {
            // Without this every key counts twice under the kitty protocol —
            // the trap the installer's TUI documents.
            if kind != KeyEventKind::Press {
                return Ok(None);
            }
            if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
                return Ok(Some(Key::Leave));
            }
            match code {
                KeyCode::Up | KeyCode::Char('k') => Some(Key::Up),
                KeyCode::Down | KeyCode::Char('j') => Some(Key::Down),
                KeyCode::Tab | KeyCode::Left | KeyCode::Right => Some(Key::Sideways),
                KeyCode::Enter | KeyCode::Char(' ') => Some(Key::Enter),
                KeyCode::Esc | KeyCode::Char('q') => Some(Key::Leave),
                _ => None,
            }
        }
        Event::Mouse(m) => match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if m.column < SIDEBAR {
                    // Row 0 of the sidebar is the title line, so the first entry
                    // sits one below — the same arithmetic `draw` uses.
                    let at = usize::from(m.row.saturating_sub(1));
                    Section::all_of().get(at).copied().map(Key::Pick)
                } else {
                    Some(Key::Enter)
                }
            }
            MouseEventKind::ScrollDown => Some(Key::Down),
            MouseEventKind::ScrollUp => Some(Key::Up),
            _ => None,
        },
        _ => None,
    })
}

/// Draws the whole screen.
fn draw(f: &mut ratatui::Frame, view: &View, texts: &Texts, state_lines: &[String]) {
    let base = Style::default()
        .fg(colour(color::TEXT_PRIMARY))
        .bg(colour(color::SURFACE_BASE));
    f.render_widget(Block::default().style(base), f.area());

    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(SIDEBAR), Constraint::Min(20)])
        .split(f.area());
    let right = Layout::default()
        .direction(Direction::Vertical)
        // The same geometry as everything else in this system: content, and two
        // lines of state underneath.
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(split[1]);

    sidebar(f, split[0], view, texts);
    content(f, right[0], view, texts, state_lines);
    footer(f, right[1], view, texts);
}

/// The seven entries, and which one is open.
fn sidebar(f: &mut ratatui::Frame, at: Rect, view: &View, texts: &Texts) {
    let title = Line::from(Span::styled(
        texts.t(key::RESCUE_TITLE),
        Style::default()
            .fg(colour(color::STATE_ACCENT))
            .add_modifier(Modifier::BOLD),
    ));
    let rows: Vec<ListItem> = Section::all_of()
        .iter()
        .map(|s| ListItem::new(Line::from(texts.t(s.label()))))
        .collect();
    let mut state = ListState::default();
    state.select(Section::all_of().iter().position(|s| *s == view.section));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(at);
    f.render_widget(Paragraph::new(title), chunks[0]);
    f.render_stateful_widget(
        List::new(rows).highlight_style(highlight(view.on_sidebar)),
        chunks[1],
        &mut state,
    );
}

/// The colour of what is under the cursor — never an outline (invariant 15).
fn highlight(sharp: bool) -> Style {
    let bg = if sharp {
        color::SURFACE_SELECTION
    } else {
        color::SURFACE_CHROME
    };
    Style::default()
        .fg(colour(color::TEXT_PRIMARY))
        .bg(colour(bg))
}

/// Whatever the open section has to show.
fn content(f: &mut ratatui::Frame, at: Rect, view: &View, texts: &Texts, state_lines: &[String]) {
    let mut state = ListState::default();
    state.select(Some(view.row));
    let rows: Vec<ListItem> = match view.section {
        Section::State => {
            let lines: Vec<Line> = state_lines.iter().map(|l| Line::from(l.clone())).collect();
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), at);
            return;
        }
        Section::Windows if view.panes.is_empty() => {
            f.render_widget(
                Paragraph::new(texts.t(key::RESCUE_NO_WINDOWS)).wrap(Wrap { trim: false }),
                at,
            );
            return;
        }
        Section::Windows => view
            .panes
            .iter()
            .map(|p| ListItem::new(Line::from(p.name.clone())))
            .collect(),
        Section::Config => papers()
            .iter()
            .map(|p| {
                let mark = if p.origin.is_some() {
                    texts.t(key::RESCUE_READ_ONLY)
                } else {
                    String::new()
                };
                ListItem::new(Line::from(vec![
                    Span::raw(p.path),
                    Span::styled(
                        if mark.is_empty() {
                            String::new()
                        } else {
                            format!("  {mark}")
                        },
                        Style::default().fg(colour(color::TEXT_DIM)),
                    ),
                ]))
            })
            .collect(),
        Section::Log => logs()
            .iter()
            .map(|(unit, _)| {
                ListItem::new(Line::from(if unit.is_empty() {
                    texts.t(key::RESCUE_SECTION_LOG)
                } else {
                    (*unit).to_owned()
                }))
            })
            .collect(),
        _ => of_section(view.section)
            .iter()
            .map(|a| row_of(a, texts))
            .collect(),
    };
    f.render_stateful_widget(
        List::new(rows).highlight_style(highlight(!view.on_sidebar)),
        at,
        &mut state,
    );
}

/// One action, with the mark that says it will ask.
fn row_of(a: &Action, texts: &Texts) -> ListItem<'static> {
    let mut spans = vec![Span::raw(texts.t(a.label))];
    if a.tier == Tier::Password {
        spans.push(Span::styled(
            format!("  {}", texts.t(key::RESCUE_NEEDS_PASSWORD)),
            Style::default().fg(colour(color::TEXT_DIM)),
        ));
    }
    ListItem::new(Line::from(spans))
}

/// The two lines: what happened, and which keys there are.
fn footer(f: &mut ratatui::Frame, at: Rect, view: &View, texts: &Texts) {
    let said = view.pending.map_or_else(
        || view.said.clone().unwrap_or_default(),
        |_| texts.t(key::RESCUE_CONFIRM),
    );
    let tone = if view.pending.is_some() {
        color::STATE_WARNING
    } else {
        color::TEXT_SECONDARY
    };
    let lines = vec![
        Line::from(Span::styled(said, Style::default().fg(colour(tone)))),
        Line::from(Span::styled(
            texts.t(key::RESCUE_KEYS),
            Style::default().fg(colour(color::TEXT_DIM)),
        )),
    ];
    f.render_widget(Paragraph::new(lines), at);
}

/// Runs a row and turns the outcome into a sentence.
fn say_of(a: &Action, view: &View, places: &ronoctua_memory::Places, texts: &Texts) -> String {
    let outcome = match a.run {
        Run::Own(Own::CloseFocused) => close_focused(),
        Run::Own(Own::KillChosen) => kill_chosen(view, places),
        Run::Own(Own::KillBiggest) => kill_biggest(places),
        other => action::run_it(other),
    };
    match outcome {
        action::Outcome::Ran => texts.t(key::RESCUE_RAN),
        action::Outcome::Failed(said) => format!("{}: {said}", texts.t(key::RESCUE_FAILED)),
        action::Outcome::Missing(variable) => {
            format!("{}: {variable}", texts.t(key::RESCUE_FAILED))
        }
    }
}

/// Asks the compositor to close what is focused.
fn close_focused() -> action::Outcome {
    match ronoctua_niri::Niri::connect().and_then(|mut n| n.close_window()) {
        Ok(()) => action::Outcome::Ran,
        Err(e) => action::Outcome::Failed(e.to_string()),
    }
}

/// SIGTERM to the process behind the chosen window.
///
/// It goes through `ronoctua_memory::scan::stop`, and that is the point: the
/// function refuses anything outside `app.slice`, so clicking a row can never
/// reach the compositor or the session's own units. Killing niri from the
/// window list would end the session the person is trying to save.
fn kill_chosen(view: &View, places: &ronoctua_memory::Places) -> action::Outcome {
    let Some(pane) = view.panes.get(view.row) else {
        return action::Outcome::Failed(String::new());
    };
    let Some(pid) = pane.pid else {
        return action::Outcome::Failed(String::new());
    };
    let target = ronoctua_protocol::Consumer {
        kind: ronoctua_protocol::ConsumerKind::Process,
        name: pane.name.clone(),
        bytes: 0,
        hint: u64::try_from(pid).unwrap_or(0),
    };
    match ronoctua_memory::scan::stop(places, &target) {
        Ok(()) => action::Outcome::Ran,
        Err(e) => action::Outcome::Failed(e.to_string()),
    }
}

/// The same, for whatever is eating the most.
fn kill_biggest(places: &ronoctua_memory::Places) -> action::Outcome {
    let mut all = ronoctua_memory::scan::ram(places);
    all.sort_unstable_by_key(|c| std::cmp::Reverse(c.bytes));
    let Some(target) = all.first() else {
        return action::Outcome::Failed(String::new());
    };
    match ronoctua_memory::scan::stop(places, target) {
        Ok(()) => action::Outcome::Ran,
        Err(e) => action::Outcome::Failed(e.to_string()),
    }
}

/// The compositor's windows, or nothing when it does not answer.
///
/// Not answering is a legitimate state here — it is, in fact, the state this
/// whole application exists for.
fn panes_of() -> Vec<Pane> {
    let Ok(mut niri) = ronoctua_niri::Niri::connect() else {
        return Vec::new();
    };
    let Ok(windows) = niri.windows() else {
        return Vec::new();
    };
    windows
        .into_iter()
        .map(|w| Pane {
            name: w
                .title
                .filter(|t| !t.trim().is_empty())
                .or(w.app_id)
                .unwrap_or_else(|| w.id.to_string()),
            id: w.id,
            pid: w.pid,
        })
        .collect()
}

/// The diagnosis, as lines of data.
///
/// It is what `nix run .#diagnostics` prints, on the screen of whoever cannot
/// reach a shell to run it. Units first, because "is it standing?" is the
/// question every other one depends on.
fn state_of(places: &ronoctua_memory::Places, texts: &Texts) -> Vec<String> {
    let mut out = Vec::new();
    for (unit, user) in logs() {
        if unit.is_empty() {
            continue;
        }
        out.push(format!("{unit}: {}", active(unit, *user)));
    }
    out.push(format!(
        "niri: {}",
        if ronoctua_niri::Niri::connect().is_ok() {
            texts.t(key::RESCUE_RAN)
        } else {
            texts.t(key::RESCUE_FAILED)
        }
    ));
    let survey = ronoctua_memory::survey(places);
    for r in survey.readings {
        out.push(format!("{:?}: {}", r.of, human(r.bytes)));
    }
    if let Some((total, free)) = ronoctua_memory::machine::filesystem("/") {
        out.push(format!("/: {} / {}", human(free), human(total)));
    }
    out
}

/// Whether a unit is standing, in one word of data.
fn active(unit: &str, user: bool) -> String {
    let Some(path) = option_env!("RONOCTUA_SYSTEMCTL") else {
        return String::new();
    };
    let mut c = std::process::Command::new(path);
    if user {
        c.arg("--user");
    }
    c.args(["is-active", unit]);
    c.output().map_or_else(
        |e| e.to_string(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
    )
}

/// Bytes, as a person reads them.
fn human(bytes: u64) -> String {
    #[expect(
        clippy::cast_precision_loss,
        reason = "one decimal place on a rescue screen"
    )]
    let n = bytes as f64;
    if n >= 1_073_741_824.0 {
        format!("{:.1} GiB", n / 1_073_741_824.0)
    } else if n >= 1_048_576.0 {
        format!("{:.0} MiB", n / 1_048_576.0)
    } else {
        format!("{bytes} B")
    }
}

/// Leaves the screen, lets something else have the terminal, and takes it back.
///
/// The alternate screen going away is what makes a full-screen sub-shell
/// possible without a PTY of our own, without a VT parser and without a second
/// way of drawing a terminal — which is exactly the machinery a rescue console
/// should not be carrying.
fn hand_over(guard: &mut Guard, h: &Hand, texts: &Texts) -> Result<()> {
    let _ = disable_raw_mode();
    let _ = crossterm::execute!(
        guard.0.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    );

    match h {
        Hand::Shell => match action::shell() {
            Ok(mut c) => {
                println!("{}", texts.t(key::RESCUE_BACK));
                let _ = c.status();
            }
            Err(variable) => println!("{}: {variable}", texts.t(key::RESCUE_FAILED)),
        },
        Hand::Journal(unit) => show_journal(unit),
        Hand::Processes => show_processes(),
        Hand::Read(path) => show_file(path, texts),
        Hand::Edit(path) => crate::edit::edit_it(path, texts),
    }

    enable_raw_mode().context("raw mode back")?;
    let _ = crossterm::execute!(
        guard.0.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    );
    guard.0.clear().context("clear")?;
    Ok(())
}

/// The journal of a unit, in the pager the journal brings with it.
fn show_journal(unit: &str) {
    let Some(path) = option_env!("RONOCTUA_JOURNALCTL") else {
        return;
    };
    let mut c = std::process::Command::new(path);
    // `-e` opens at the END: what matters in a log one opened during a failure
    // is the last thing that happened, and scrolling there by hand is the first
    // thing everybody does.
    c.arg("-e");
    if !unit.is_empty() {
        // The footer and niri are USER units; the whole machine's journal has
        // no unit at all.
        if unit.starts_with("ronoctua-") || unit.starts_with("niri") {
            c.arg("--user");
        }
        c.args(["-u", unit]);
    }
    let _ = c.status();
}

/// The process monitor, which is already a decision of this distro.
fn show_processes() {
    let Some(path) = option_env!("RONOCTUA_BTOP") else {
        return;
    };
    let _ = std::process::Command::new(path).status();
}

/// A file, read and never written.
fn show_file(path: &Path, texts: &Texts) {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let origin = papers()
                .iter()
                .find(|p| Path::new(p.path) == path)
                .and_then(|p| p.origin);
            if let Some(origin) = origin {
                println!(
                    "{}",
                    texts.t_with(key::RESCUE_GENERATED_BY, &[("source", origin)])
                );
            }
            print!("{text}");
        }
        Err(e) => println!("{}: {e}", texts.t(key::RESCUE_FAILED)),
    }
    let _ = std::io::stdout().flush();
    wait_for_a_key();
}

/// Holds the screen until a key, so a file does not scroll past and vanish.
fn wait_for_a_key() {
    let _ = enable_raw_mode();
    loop {
        if matches!(event::read(), Ok(Event::Key(k)) if k.kind == KeyEventKind::Press) {
            break;
        }
    }
    let _ = disable_raw_mode();
}
