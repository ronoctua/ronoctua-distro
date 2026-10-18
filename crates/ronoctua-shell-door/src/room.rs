//! THE SCREEN RESIZER'S STATE (`LEADER / r`): the room it leaves, as a file
//! every door reads, the keeper process that holds it, and the arithmetic of
//! a size in whole cells and lines.
//!
//! # Why a module of its own
//!
//! The curtains are layer surfaces of their own (`ronoctua_shell_overlays::resizer`). What the
//! resizer DECIDED — that it is on, and how big the room is — is read by the
//! footer, by every column that fits itself into the room, and by the
//! capture: none of which should name the curtains to know a size.

use ronoctua_tokens as tokens;
use std::path::{Path, PathBuf};

/// The runtime file that holds the cut, and exists exactly while it is on.
pub const FILE: &str = "resizer.json";

/// The runtime file that holds the keeper's pid.
pub const KEEPER_FILE: &str = "resizer.pid";

/// The word after the program's name that starts the keeper.
pub const KEEPER_WORD: &str = "resizer";

/// The cut, as the file holds it — every size in logical pixels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct State {
    /// The connector of the screen that is cut.
    pub output: String,
    /// That screen's whole width.
    pub screen_width_px: u32,
    /// That screen's whole height.
    pub screen_height_px: u32,
    /// The width of the room the curtains leave.
    pub width_px: u32,
    /// The height of the room the curtains leave.
    pub height_px: u32,
}

/// What a cell and a line measure now, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Units {
    /// One cell's width.
    pub cell: f32,
    /// One line's height.
    pub line: f32,
}

impl Units {
    /// At the size of the text the person chose.
    #[must_use]
    pub fn now() -> Self {
        let size = tokens::text::size();
        Self {
            cell: tokens::metrics::cell_width(size),
            line: tokens::metrics::line_height(size),
        }
    }
}

/// How far the cut may go, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The narrowest room: `space.resizer.minColumns` cells.
    pub min_width_px: u32,
    /// The shortest room: `space.resizer.minLines` lines.
    pub min_height_px: u32,
    /// The thinnest right curtain: `space.resizer.edgeCells` cells, so it
    /// can still be grabbed.
    pub right_min_px: u32,
    /// The thinnest bottom curtain: one line, the one that says the size.
    pub bottom_min_px: u32,
}

/// How the keeper ended, as the footer reads it off its output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// It could not cut the screen, and why.
    Failed(String),
    /// It ended — a curtain was closed, or the footer ended it.
    Ended,
}

/// A rectangle in a curtain's own logical pixels: `(x, y, width, height)`.
pub type Box4 = (f32, f32, f32, f32);

/// How many whole `unit`s fit in `px`.
#[allow(clippy::cast_possible_truncation)]
pub fn count(px: u32, unit: f32) -> i64 {
    if unit <= 0.0 {
        return 0;
    }
    (f64::from(px) / f64::from(unit)).floor() as i64
}

/// `px` moved by `by` whole units: the SMALLEST size that holds exactly the
/// units it had plus `by` — so one step changes what fits by exactly one, at
/// any size of cell, and a size that was not on the grid lands on it.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn stepped(px: u32, unit: f32, by: i64) -> u32 {
    if unit <= 0.0 {
        return px;
    }
    let wanted = count(px, unit).saturating_add(by).max(0);
    let mut out = (wanted as f64 * f64::from(unit)).ceil() as u32;
    // THE FLOATING POINT IS ASKED, not trusted: the product can land a hair
    // either side of the boundary, and the answer is whatever the same
    // division the grid uses says.
    while count(out, unit) < wanted && out < u32::MAX {
        out += 1;
    }
    while out > 0 && count(out - 1, unit) >= wanted {
        out -= 1;
    }
    out
}

/// The limits at these units.
#[must_use]
pub fn limits(u: Units) -> Limits {
    Limits {
        min_width_px: stepped(0, u.cell, i64::from(tokens::space::RESIZER_MINCOLUMNS)),
        min_height_px: stepped(0, u.line, i64::from(tokens::space::RESIZER_MINLINES)),
        right_min_px: stepped(0, u.cell, i64::from(tokens::space::RESIZER_EDGECELLS)),
        bottom_min_px: stepped(0, u.line, 1),
    }
}

/// The room on a screen, in that screen's logical pixels from its top-left
/// corner: `(x, y, width, height)`. The whole screen where the resizer does
/// not cut it — what the capture takes as "the whole screen": the curtains
/// are the tool, not the screen being tested.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn room_box(output: &str, size: (u32, u32), cut: Option<&State>) -> Box4 {
    (
        0.0,
        0.0,
        room_width(size.0, Some(output), cut) as f32,
        room_height(size.1, Some(output), cut) as f32,
    )
}

/// How many columns and lines the room holds.
#[must_use]
pub fn grid_of(s: &State, u: Units) -> (i64, i64) {
    (count(s.width_px, u.cell), count(s.height_px, u.line))
}

/// How the keeper ended, from what it printed: `failed <why>` or nothing.
#[must_use]
pub fn last_word(said: &str) -> Heard {
    said.lines()
        .rev()
        .find_map(|l| l.strip_prefix("failed "))
        .map_or(Heard::Ended, |why| Heard::Failed(why.to_owned()))
}

/// The height the footer has on a screen: the room, where the resizer cuts
/// that screen, and the screen's own height anywhere else.
#[must_use]
pub fn room_height(output_height: u32, output: Option<&str>, cut: Option<&State>) -> u32 {
    match cut {
        Some(s) if output == Some(s.output.as_str()) => output_height.min(s.height_px),
        _ => output_height,
    }
}

/// The height of the SHORTEST room among `screens` — each screen's height and
/// its connector, when it has one — under `cut`: what the footer's ceiling is
/// measured on, since its height is the same on every screen. A screen of no
/// height yet is not counted, and `None` is no screen at all.
#[must_use]
pub fn smallest_room(
    screens: impl IntoIterator<Item = (u32, Option<String>)>,
    cut: Option<&State>,
) -> Option<u32> {
    screens
        .into_iter()
        .map(|(height, name)| room_height(height, name.as_deref(), cut))
        .filter(|h| *h > 0)
        .min()
}

/// The width a column has on a screen: the room, where the resizer cuts that
/// screen, and the screen's own width anywhere else — the twin of
/// [`room_height`]. A column that took its share of the MONITOR under a cut
/// took most of a narrow room, and past the room's own width it hung off
/// its left edge.
#[must_use]
pub fn room_width(output_width: u32, output: Option<&str>, cut: Option<&State>) -> u32 {
    match cut {
        Some(s) if output == Some(s.output.as_str()) => output_width.min(s.width_px),
        _ => output_width,
    }
}

/// The file's path, when there is a runtime directory.
pub fn path() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file(FILE)
}

/// The cut, when the file is there and reads; `None` for no file and for one
/// that does not parse.
#[must_use]
pub fn read() -> Option<State> {
    read_at(&path()?)
}

/// See [`read`].
pub fn read_at(path: &Path) -> Option<State> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Whether a `/proc/<pid>/cmdline` is this program started as the keeper.
pub fn is_the_keeper(cmdline: &[u8]) -> bool {
    crate::right_column::started_as(cmdline, &[KEEPER_WORD])
}

/// The keeper's pid — only while that process is alive, is not this one,
/// and is this program started as the keeper. A pid is recycled, and a
/// signal sent to a number is sent to whoever holds it now.
#[must_use]
pub fn keeper() -> Option<i32> {
    let text = std::fs::read_to_string(ronoctua_xdg::runtime_file(KEEPER_FILE)?).ok()?;
    let pid: i32 = text.trim().parse().ok()?;
    if u32::try_from(pid).ok() == Some(std::process::id()) {
        return None;
    }
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    is_the_keeper(&line).then_some(pid)
}

/// The cut, while the resizer is on: the file reads AND its keeper lives.
#[must_use]
pub fn is_on() -> Option<State> {
    keeper()?;
    read()
}

/// Starts the keeper, with its output piped: the footer reads how it ended.
///
/// # Errors
///
/// When this program cannot find itself or the process cannot be started.
pub fn start() -> std::io::Result<std::process::Child> {
    std::process::Command::new(std::env::current_exe()?)
        .arg(KEEPER_WORD)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
}

/// Ends the resizer: the file first — both curtains watch it and leave — and
/// then the keeper, so nothing is left even if a curtain is stuck.
pub fn stop() {
    if let Some(p) = path() {
        let _ = std::fs::remove_file(p);
    }
    if let Some(pid) = keeper().and_then(rustix::process::Pid::from_raw) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    }
}

/// Deletes the files when no keeper answers for them — a keeper killed by a
/// signal (or with a session) leaves both behind.
pub fn sweep_stale() {
    if keeper().is_some() {
        return;
    }
    for leaf in [FILE, KEEPER_FILE] {
        if let Some(p) = ronoctua_xdg::runtime_file(leaf) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The directory and the name to watch the file by — out of the same
/// `runtime_file` every reader and writer uses, so the per-instance name is
/// written once.
#[must_use]
pub fn watch_target() -> Option<(PathBuf, String)> {
    let p = path()?;
    Some((
        p.parent()?.to_path_buf(),
        p.file_name()?.to_str()?.to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: an arrow moves the room by anything but one whole column or
    /// line — a step that lands a hair short of the grid moves the footer's
    /// count by nothing, and one a hair past it by two.
    #[test]
    fn a_step_moves_the_grid_by_exactly_one_unit() {
        for unit in [7.2_f32, 9.6, 10.8, 23.2, 26.1] {
            for px in (0..2400_u32).step_by(37).chain([144, 1896, 1080, 2160]) {
                for by in [-10_i64, -1, 1, 10] {
                    let out = stepped(px, unit, by);
                    let wanted = (count(px, unit) + by).max(0);
                    assert_eq!(
                        count(out, unit),
                        wanted,
                        "{px} px by {by} of {unit} gave {out} px"
                    );
                    // AND THE SMALLEST SUCH SIZE: the room stays on the grid.
                    if out > 0 {
                        assert!(count(out - 1, unit) < wanted, "{out} px is not the least");
                    }
                }
            }
        }
    }
}
