//! The editor — small on purpose, and it does not install what it wrote.
//!
//! # Why an editor of our own
//!
//! Because the `minimal` profile leaves the editor out (`nix/modules/profile.nix`),
//! and a rescue console whose "edit the configuration" row depends on a package
//! the machine may not have is a row that fails on the installation that needs
//! it most. Fifty lines here cost less than that.
//!
//! It is deliberately poor: arrows, typing, `Backspace`, `Enter`. No search, no
//! undo, no syntax. What one does in a rescue console is fix a line and get
//! out.
//!
//! # And it never writes to `/etc` itself
//!
//! The file belongs to root and this application runs as the person. Saving
//! writes the new text to [`STAGED`] — a directory systemd creates for this
//! user — and asks a oneshot to install it. The unit VALIDATES before
//! installing: a `host.nix` that does not parse is a machine that will not
//! rebuild, and finding that out at the next boot is finding it out too late.

use std::io::Write as _;
use std::path::Path;

use crate::key;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ronoctua_i18n::Texts;
use ronoctua_tokens::color;
use ronoctua_vt::Console;

/// Where the edited text waits for the unit that has the privilege.
///
/// The same string lives in `nix/modules/rescue.nix`, and
/// `rescue-staging-agrees` is what stops the two from drifting — a saved file
/// nobody reads is an edit that silently did nothing.
pub const STAGED: &str = "/run/ronoctua-rescue/staged";

/// The unit that validates and installs it.
pub const INSTALLER_UNIT: &str = "ronoctua-rescue-host.service";

/// A file open for editing.
struct Sheet {
    lines: Vec<String>,
    line: usize,
    column: usize,
    said: String,
}

impl Sheet {
    fn of(text: &str) -> Self {
        let mut lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            line: 0,
            column: 0,
            said: String::new(),
        }
    }

    fn text(&self) -> String {
        let mut out = self.lines.join("\n");
        out.push('\n');
        out
    }

    /// Applies one key. `false` means the person asked to leave.
    fn key(&mut self, code: KeyCode, control: bool) -> bool {
        if control && matches!(code, KeyCode::Char('s')) {
            self.said = match save_it(&self.text()) {
                Ok(()) => String::from("ok"),
                Err(e) => e,
            };
            return true;
        }
        match code {
            KeyCode::Esc => return false,
            KeyCode::Up => self.line = self.line.saturating_sub(1),
            KeyCode::Down => self.line = (self.line + 1).min(self.lines.len() - 1),
            KeyCode::Left => self.column = self.column.saturating_sub(1),
            KeyCode::Right => self.column += 1,
            KeyCode::Home => self.column = 0,
            KeyCode::End => self.column = self.lines[self.line].chars().count(),
            KeyCode::Char(c) => {
                let at = self.byte_at();
                self.lines[self.line].insert(at, c);
                self.column += 1;
            }
            KeyCode::Enter => {
                let at = self.byte_at();
                let rest = self.lines[self.line].split_off(at);
                self.lines.insert(self.line + 1, rest);
                self.line += 1;
                self.column = 0;
            }
            KeyCode::Backspace => self.backspace(),
            _ => {}
        }
        self.column = self.column.min(self.lines[self.line].chars().count());
        true
    }

    /// The cursor, in bytes — a column is in CHARACTERS, and a `host.nix` with
    /// an accent in a comment would panic on a byte index.
    fn byte_at(&self) -> usize {
        self.lines[self.line]
            .char_indices()
            .nth(self.column)
            .map_or(self.lines[self.line].len(), |(i, _)| i)
    }

    fn backspace(&mut self) {
        if self.column > 0 {
            let at = self.byte_at();
            let before = self.lines[self.line][..at]
                .chars()
                .next_back()
                .map_or(0, char::len_utf8);
            self.lines[self.line].replace_range(at - before..at, "");
            self.column -= 1;
        } else if self.line > 0 {
            let tail = self.lines.remove(self.line);
            self.line -= 1;
            self.column = self.lines[self.line].chars().count();
            self.lines[self.line].push_str(&tail);
        }
    }
}

/// Opens the editor on a file and returns when the person leaves it.
pub fn edit_it(path: &Path, texts: &Texts) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            println!("{}: {e}", texts.t(key::RESCUE_FAILED));
            return;
        }
    };
    let mut sheet = Sheet::of(&text);

    let mut output = std::io::stdout();
    if enable_raw_mode().is_err() {
        return;
    }
    let _ = crossterm::execute!(output, EnterAlternateScreen);
    // ON VT 8, THE CONSOLE'S SLOTS (`crate::tui::SLOTS`): the rescue gave the
    // console its own palette back before handing the screen over, and takes
    // it again when the screen comes back.
    let console = Console::of_term(crate::tui::SLOTS);
    if let Some(c) = &console {
        let _ = write!(output, "{}", c.palette());
        let _ = output.flush();
    }
    let Ok(mut term) = Terminal::new(CrosstermBackend::new(output)) else {
        let _ = disable_raw_mode();
        return;
    };

    loop {
        let _ = term.draw(|f| {
            draw(f, &sheet, path, texts);
            if let Some(c) = &console {
                c.paint(f.buffer_mut());
            }
        });

        let Ok(Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        })) = event::read()
        else {
            continue;
        };
        if kind != KeyEventKind::Press {
            continue;
        }
        if !sheet.key(code, modifiers.contains(KeyModifiers::CONTROL)) {
            break;
        }
    }

    let _ = disable_raw_mode();
    let _ = crossterm::execute!(term.backend_mut(), LeaveAlternateScreen);
    if console.is_some() {
        let _ = write!(term.backend_mut(), "{}", Console::RESET);
        let _ = term.backend_mut().flush();
    }
}

/// The sheet on the screen, and the two lines under it.
fn draw(f: &mut ratatui::Frame, sheet: &Sheet, path: &Path, texts: &Texts) {
    let base = Style::default()
        .fg(colour(color::TEXT_PRIMARY))
        .bg(colour(color::SURFACE_BASE));
    f.render_widget(Block::default().style(base), f.area());
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(f.area());

    // The window follows the cursor: a file longer than the screen
    // would otherwise be edited blind from the first page down.
    let height = usize::from(split[0].height).max(1);
    let top = sheet.line.saturating_sub(height / 2);
    let shown: Vec<Line> = sheet
        .lines
        .iter()
        .skip(top)
        .take(height)
        .map(|l| Line::from(l.clone()))
        .collect();
    f.render_widget(Paragraph::new(shown), split[0]);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                sheet.said.clone(),
                Style::default().fg(colour(color::STATE_WARNING)),
            )),
            Line::from(Span::styled(
                format!("{}  {}", path.display(), texts.t(key::RESCUE_KEYS)),
                Style::default().fg(colour(color::TEXT_DIM)),
            )),
        ]),
        split[1],
    );
    #[expect(clippy::cast_possible_truncation, reason = "it fits on one screen")]
    f.set_cursor_position((
        split[0].x + sheet.column as u16,
        split[0].y + (sheet.line - top) as u16,
    ));
}

/// Stages the text and asks the unit to install it.
fn save_it(text: &str) -> Result<(), String> {
    let staged = Path::new(STAGED);
    if let Some(parent) = staged.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(staged, text).map_err(|e| e.to_string())?;
    let Some(systemctl) = option_env!("RONOCTUA_SYSTEMCTL") else {
        return Err(String::from("RONOCTUA_SYSTEMCTL"));
    };
    let out = std::process::Command::new(systemctl)
        .args(["start", INSTALLER_UNIT])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// A token, as this terminal wants it — sRGB, never linear.
fn colour(c: ronoctua_tokens::Color) -> ratatui::style::Color {
    let [r, g, b, _] = c.to_rgba8();
    ratatui::style::Color::Rgb(r, g, b)
}

/// The editor's screen, drawn into a buffer — what the console's tests
/// look at (`tui::tests::every_colour_has_a_slot`).
#[cfg(test)]
pub(crate) fn drawn_for_tests(texts: &Texts) -> ratatui::buffer::Buffer {
    let mut sheet = Sheet::of("{ }\n");
    sheet.said = "saved".into();
    let mut t = Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    t.draw(|f| draw(f, &sheet, Path::new("host.nix"), texts))
        .unwrap();
    t.backend().buffer().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_erasing_survive_an_accent() {
        // THE DEFECT THIS CATCHES: a column is in CHARACTERS and a `String` is
        // indexed in BYTES. A `host.nix` with an accented comment — which is
        // most of them in this repository — would panic mid-edit, on the screen
        // someone opened to fix the machine.
        let mut s = Sheet::of("# configuração\n");
        s.line = 0;
        s.column = s.lines[0].chars().count();
        assert!(s.key(KeyCode::Char('!'), false));
        // pt-BR: a multibyte character is the subject — the cut has to land
        // between characters and not between bytes.
        assert_eq!(s.lines[0], "# configuração!");
        assert!(s.key(KeyCode::Backspace, false));
        assert!(s.key(KeyCode::Backspace, false));
        assert_eq!(s.lines[0], "# configuraçã");
    }

    #[test]
    fn enter_splits_and_backspace_joins() {
        let mut s = Sheet::of("ab\n");
        s.column = 1;
        assert!(s.key(KeyCode::Enter, false));
        assert_eq!(s.lines, vec!["a".to_owned(), "b".to_owned()]);
        assert!(s.key(KeyCode::Backspace, false));
        assert_eq!(s.lines, vec!["ab".to_owned()]);
        assert_eq!(s.column, 1);
    }

    #[test]
    fn what_it_writes_ends_in_a_newline() {
        // A `host.nix` whose last line has no newline is a file the module's
        // writer refuses (`nix/modules/memory.nix` checks the last line is `}`),
        // and the refusal would show up two features away.
        let s = Sheet::of("{ }\n");
        assert!(s.text().ends_with('\n'));
    }

    #[test]
    fn escape_leaves_and_nothing_else_does() {
        let mut s = Sheet::of("x\n");
        assert!(s.key(KeyCode::Char('y'), false));
        assert!(s.key(KeyCode::Down, false));
        assert!(!s.key(KeyCode::Esc, false));
    }
}
