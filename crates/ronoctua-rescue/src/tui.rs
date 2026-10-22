//! The screen: sidebar on the left, content on the right.
//!
//! # The two invariants a rescue console has to keep anyway
//!
//! **8:** not one sentence is written here. Everything the person reads comes
//! from `crate::key`, and the console that would say `[rescue-reboot]`
//! on the day a key went missing is the console nobody could use.
//!
//! **9:** not one colour by hand. `to_rgba8()` and never `to_linear()` — the
//! terminal takes sRGB, which is the opposite of what the renderer takes, and
//! passing linear here would darken everything (see the installer's TUI, which
//! learned this first). And on VT 8, by slot ([`SLOTS`]): the kernel's console
//! keeps one bit of a 24-bit colour, and the cursor's band came out the black
//! of the ground.
//!
//! # And the one it keeps by construction
//!
//! **14:** this is not chrome. It is a program on a terminal — a window the
//! compositor floats by a generated rule, exactly like the process monitor, or
//! a VT that is not on the desktop at all.

use std::io::{Stdout, Write as _};
use std::path::Path;
use std::time::Duration;

use crate::key;
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
use ronoctua_i18n::Texts;
use ronoctua_tokens::{Color, color};
use ronoctua_vt::{Console, Table};

use crate::view::{Hand, Key, Pane, Step, View, logs, papers};
use crate::{Action, Own, Run, Section, Tier, action, of_section};

/// A token, as this terminal wants it.
///
/// `to_rgba8()` and NOT `to_linear()`: the terminal is sRGB. The alpha is
/// dropped — a terminal cell has no transparency to give.
fn colour(c: Color) -> ratatui::style::Color {
    let [r, g, b, _] = c.to_rgba8();
    ratatui::style::Color::Rgb(r, g, b)
}

/// The roles the console paints, each in the slot it takes on the kernel's
/// console ([`ronoctua_vt`]): the three grounds among the first eight, the
/// ground in 0 and the text in 7 — the console's own, so the shell a row hands
/// the screen to is in the palette's tones until the reset. Every colour any
/// screen paints has a slot (`every_colour_has_a_slot`); the editor's too.
pub(crate) const SLOTS: Table = Table::new(&[
    color::SURFACE_BASE,
    color::STATE_WARNING,
    color::TEXT_SECONDARY,
    color::STATE_ACCENT,
    color::SURFACE_SELECTION,
    color::TEXT_HERE_ACTS,
    color::SURFACE_CHROME,
    color::TEXT_PRIMARY,
    color::TEXT_DIM,
]);

/// How wide the sidebar is, in cells.
///
/// Fixed and not proportional: on an 80-column VT a proportional sidebar would
/// take a quarter of a screen that has no room to spare, and the seven words in
/// it do not get longer on a wider screen.
const SIDEBAR: u16 = 18;

/// The cells text keeps from the edges of its column: the shell's
/// `chrome::INSET` (`space.content.inset`), as the installer's. The font's
/// ink fills its whole advance, and a letter on the screen's edge — or on
/// the sidebar's, against the content — looked cut.
const INSET: u16 = ronoctua_tokens::space::CONTENT_INSET;

/// `area` without the inset on either side, where text is drawn; the
/// cursor's band still runs across the whole column ([`banded`]). A column
/// too narrow for it keeps one cell of text.
pub(crate) fn inset(area: Rect) -> Rect {
    let side = INSET.min(area.width.saturating_sub(1) / 2);
    Rect {
        x: area.x + side,
        width: area.width - 2 * side,
        ..area
    }
}

/// `list` inside the inset of `at`, and the cursor's band (`band`, a
/// ground) across the whole of `at`, on the row the list drew it.
fn banded(f: &mut ratatui::Frame, list: List, at: Rect, state: &mut ListState, band: Style) {
    f.render_stateful_widget(list.highlight_style(band), inset(at), state);
    let Some(row) = state
        .selected()
        .and_then(|i| i.checked_sub(state.offset()))
        .and_then(|shown| u16::try_from(shown).ok())
        .filter(|shown| *shown < at.height)
    else {
        return;
    };
    f.buffer_mut().set_style(
        Rect {
            y: at.y + row,
            height: 1,
            ..at
        },
        band,
    );
}

/// Puts the terminal back however this ends.
///
/// It is the same guard the installer has, and for the reason written there: a
/// `panic!` in the middle of drawing must not leave a terminal in raw mode. Here
/// it matters more — the terminal it would ruin may be the only one the person
/// still has. And on the console, its own palette, and its level
/// ([`ronoctua_vt::Quiet`], given back when it drops).
struct Guard(
    Terminal<CrosstermBackend<Stdout>>,
    Option<Console>,
    #[expect(
        dead_code,
        reason = "held for its Drop, which gives the console its level back"
    )]
    Option<ronoctua_vt::Quiet>,
);

impl Guard {
    /// The console's slots in the palette's tones — at the start, and again
    /// when the screen comes back from what a row handed it to.
    fn take_the_palette(&mut self) {
        if let Some(c) = &self.1 {
            let _ = write!(self.0.backend_mut(), "{}", c.palette());
            let _ = self.0.backend_mut().flush();
        }
    }

    /// The console's own palette back — on the way out, and before a row
    /// hands the screen to a shell, a pager or the process monitor, which
    /// paint in the console's colours and not in this console's slots.
    fn give_the_palette_back(&mut self) {
        if self.1.is_some() {
            let _ = write!(self.0.backend_mut(), "{}", Console::RESET);
            let _ = self.0.backend_mut().flush();
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(
            self.0.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        self.give_the_palette_back();
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
    // AND THE KERNEL QUIET ON IT: a message written over the screen stays
    // where the TUI does not repaint (`ronoctua_vt::Quiet`).
    let mut guard = Guard(
        Terminal::new(CrosstermBackend::new(output)).context("terminal")?,
        Console::of_term(SLOTS),
        ronoctua_vt::Quiet::of_term(),
    );
    guard.take_the_palette();

    let mut view = View::fresh();
    let places = ronoctua_memory::Places::default();
    view.panes = panes_of();
    let mut state_lines = state_of(&places, &texts);

    loop {
        guard
            .0
            .draw(|f| {
                draw(f, &view, &texts, &state_lines);
                if let Some(c) = &guard.1 {
                    c.paint(f.buffer_mut());
                }
            })
            .context("draw")?;

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
    Ok(match event::read().context("read an event")? {
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
    let open = Section::all_of().iter().position(|s| *s == view.section);
    let rows: Vec<ListItem> = Section::all_of()
        .iter()
        .map(|s| ListItem::new(Line::from(texts.t(s.label()))))
        .collect();
    let rows = lit(rows, open, view.on_sidebar);
    let mut state = ListState::default();
    state.select(open);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(at);
    f.render_widget(Paragraph::new(title), inset(chunks[0]));
    banded(
        f,
        List::new(rows),
        chunks[1],
        &mut state,
        highlight(view.on_sidebar),
    );
}

/// The band of what is under the cursor — never an outline (invariant 15).
/// A GROUND ONLY: the row's text says it is the cursor's ([`lit`]), and a
/// mark beside it (`needs a password`) keeps its own tone.
fn highlight(sharp: bool) -> Style {
    let bg = if sharp {
        color::SURFACE_SELECTION
    } else {
        color::SURFACE_CHROME
    };
    Style::default().bg(colour(bg))
}

/// The rows, with the cursor's (`at`) in the accent while the keyboard is on
/// their side (`sharp`) — the cursor is a colour, as on every screen of the
/// shell (`text.here.acts`), and not only the band behind it.
fn lit(rows: Vec<ListItem<'static>>, at: Option<usize>, sharp: bool) -> Vec<ListItem<'static>> {
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            if sharp && Some(i) == at {
                row.style(Style::default().fg(colour(color::TEXT_HERE_ACTS)))
            } else {
                row
            }
        })
        .collect()
}

/// Whatever the open section has to show.
fn content(f: &mut ratatui::Frame, at: Rect, view: &View, texts: &Texts, state_lines: &[String]) {
    let mut state = ListState::default();
    state.select(Some(view.row));
    let rows: Vec<ListItem> = match view.section {
        Section::State => {
            let lines: Vec<Line> = state_lines.iter().map(|l| Line::from(l.clone())).collect();
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inset(at));
            return;
        }
        Section::Windows if view.panes.is_empty() => {
            f.render_widget(
                Paragraph::new(texts.t(key::RESCUE_NO_WINDOWS)).wrap(Wrap { trim: false }),
                inset(at),
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
    banded(
        f,
        List::new(lit(rows, Some(view.row), !view.on_sidebar)),
        at,
        &mut state,
        highlight(!view.on_sidebar),
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
    f.render_widget(Paragraph::new(lines), inset(at));
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

/// Opens a connection, or says there is nobody to talk to.
///
/// # THE ONE PLACE IN THIS CRATE THAT NAMES AN ADAPTER
///
/// Everything below holds a `dyn Compositor` and asks it things, which is what
/// makes "this crate does not know which compositor it is" a fact about the
/// code rather than a sentence in a document. There were THREE of these, and
/// more than one is how a crate drifts back into naming a product: the day a
/// second adapter exists, two of the three get updated.
///
/// `only-the-composition-root-names-the-compositor` counts them.
fn open_the_compositor() -> anyhow::Result<Box<dyn ronoctua_compositor::Compositor>> {
    ronoctua_scroll::Scroll::open()
}

/// Asks the compositor to close what is focused.
fn close_focused() -> action::Outcome {
    match open_the_compositor().and_then(|mut c| c.close_window()) {
        Ok(()) => action::Outcome::Ran,
        Err(e) => action::Outcome::Failed(e.to_string()),
    }
}

/// SIGTERM to the process behind the chosen window.
///
/// It goes through `ronoctua_memory::scan::stop`, and that is the point: the
/// function refuses anything outside `app.slice`, so clicking a row can never
/// reach the compositor or the session's own units. Killing the compositor from the
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
    let Ok(mut wm) = open_the_compositor() else {
        return Vec::new();
    };
    let Ok(windows) = wm.windows() else {
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

/// Whether the compositor is not merely listening but answering.
fn compositor_answers() -> bool {
    let Ok(mut wm) = open_the_compositor() else {
        return false;
    };
    wm.version().is_ok()
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
        "compositor: {}",
        // IT IS ASKED SOMETHING, not merely opened. A hung compositor still
        // accepts a connection on its socket — and a hung compositor is the
        // state this whole application exists for, so a probe that only
        // connects would answer "running" in exactly the case that matters.
        if compositor_answers() {
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
    guard.give_the_palette_back();

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
    guard.take_the_palette();
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
        // The footer and the compositor are USER units; the machine's journal has
        // no unit at all.
        if unit.starts_with("ronoctua-") || unit.starts_with("scroll") {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn texts() -> Texts {
        Texts::fresh("pt-BR").expect("pt-BR has to load")
    }

    /// The screen drawn into a buffer, as the console draws it.
    fn drawn(view: &View, texts: &Texts) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let state = vec!["ronoctua-footer: active".to_owned()];
        t.draw(|f| draw(f, view, texts, &state)).unwrap();
        t.backend().buffer().clone()
    }

    /// Every view: each section, the cursor on either side, a row armed and
    /// a row that said something.
    fn every_view() -> Vec<View> {
        let mut all = Vec::new();
        for section in Section::all_of() {
            for on_sidebar in [true, false] {
                for (pending, said) in [(None, None), (Some("reboot"), Some("ran".to_owned()))] {
                    let mut v = View::fresh();
                    v.section = section;
                    v.on_sidebar = on_sidebar;
                    v.pending = pending;
                    v.said = said;
                    v.panes = vec![Pane {
                        name: "a window".into(),
                        id: 1,
                        pid: Some(1),
                    }];
                    all.push(v);
                }
            }
        }
        all
    }

    /// FAILS IF: a screen of the console — or of its editor — paints a
    /// colour VT 8 has no slot for (it would come out in another token's),
    /// or a ground outside the eight slots a background can be.
    #[test]
    fn every_colour_has_a_slot() {
        let t = texts();
        let console = Console::resolved(SLOTS);
        let views = every_view();
        assert!(views.len() > 20, "{} views", views.len());
        for v in &views {
            let off = console.unslotted(&drawn(v, &t));
            assert!(
                off.is_empty(),
                "{:?} (sidebar: {}): {off:#?}",
                v.section,
                v.on_sidebar
            );
        }
        let off = console.unslotted(&crate::edit::drawn_for_tests(&t));
        assert!(off.is_empty(), "the editor: {off:#?}");
    }

    /// FAILS IF: a screen of the console — or of its editor — draws a letter
    /// in the inset of either column, on the screen's edges or on the
    /// sidebar's against the content, or the cursor's band stops at the
    /// inset instead of running across its column.
    #[test]
    fn the_text_keeps_off_the_edges_and_the_band_does_not() {
        let t = texts();
        let blank = |buf: &Buffer, xs: &[u16], what: &str| {
            let area = buf.area;
            for y in 0..area.height {
                for &x in xs {
                    let symbol = buf.cell((x, y)).unwrap().symbol();
                    assert_eq!(symbol, " ", "{what}: `{symbol}` at ({x}, {y})");
                }
            }
        };
        for v in every_view() {
            let buf = drawn(&v, &t);
            let w = buf.area.width;
            let edges: Vec<u16> = (0..INSET)
                .chain(SIDEBAR - INSET..SIDEBAR + INSET)
                .chain(w - INSET..w)
                .collect();
            blank(
                &buf,
                &edges,
                &format!("{:?} (sidebar: {})", v.section, v.on_sidebar),
            );
            // The cursor's band, across its column: the sidebar's on the
            // open section (one below the title), the content's on its first row.
            let (y, from, to) = if v.on_sidebar {
                let open = Section::all_of()
                    .iter()
                    .position(|s| *s == v.section)
                    .unwrap();
                (1 + u16::try_from(open).unwrap(), 0, SIDEBAR - 1)
            } else {
                (0, SIDEBAR, w - 1)
            };
            if v.on_sidebar || !matches!(v.section, Section::State) {
                for x in [from, to] {
                    assert_eq!(
                        buf.cell((x, y)).unwrap().bg,
                        highlight(true).bg.unwrap(),
                        "{:?} (sidebar: {}): the band stops before ({x}, {y})",
                        v.section,
                        v.on_sidebar
                    );
                }
            }
        }
        let editor = crate::edit::drawn_for_tests(&t);
        let w = editor.area.width;
        let edges: Vec<u16> = (0..INSET).chain(w - INSET..w).collect();
        blank(&editor, &edges, "the editor");
    }

    /// FAILS IF: the cursor's row has the ground of the rows around it on
    /// the kernel's console — the band the console drew in the ground's own
    /// black — or its text is the others' tone while the keyboard is on its
    /// side, or a mark beside a row loses its own tone under the cursor.
    #[test]
    fn the_cursor_is_a_band_and_a_colour_on_the_console() {
        let t = texts();
        let console = Console::resolved(SLOTS);
        for on_sidebar in [true, false] {
            let mut v = View::fresh();
            v.section = Section::all_of()[1];
            v.on_sidebar = on_sidebar;
            let mut buf = drawn(&v, &t);
            console.paint(&mut buf);
            // The sidebar's rows start one below its title; the content's at
            // the top, right of the sidebar.
            let (x, here, other) = if on_sidebar {
                (0, 2, 3)
            } else {
                (SIDEBAR, 0, 1)
            };
            // The band on the column's edge, the tone on its text.
            let at = |x: u16, y: u16| buf.cell((x, y)).unwrap().clone();
            assert_ne!(
                at(x, here).bg,
                at(x, other).bg,
                "sidebar {on_sidebar}: no band"
            );
            let x = x + INSET;
            assert_ne!(
                at(x, here).fg,
                at(x, other).fg,
                "sidebar {on_sidebar}: no colour"
            );
        }
    }
}
