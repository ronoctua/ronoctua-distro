//! THE TERMINAL'S CAMERA: `photograph --door terminal --corpus <file.vt>`.
//!
//! A recorded stream — the bytes a program wrote, `tests/corpus-vt/*.vt` —
//! fed to an engine in this process at the size the window is laid out at,
//! its snapshot decoded into the window's mirror by the road a holder's
//! attach takes, and the frame the window draws of it developed: the grid of
//! cells, its colours, its cursor and its scrollbar, exactly as on a screen.
//! Nothing runs: no program, no PTY, no holder.
//!
//! A GROUP is photographed as a window of several terminals
//! (`--group N`), the stream the one in front's (`--front K`), with the
//! strip as it is drawn: a line under the pointer (`--lit H` — the scenes'
//! own `--hover` is a cell of the window), lines whose
//! terminals ask for the person (`--attention A,B`). Lines count from 1, as
//! the person counts them.

use anyhow::{Context as _, Result, ensure};
use ronoctua_shell_door::photo::scenes::{self, Asked, Measure, Scene};

use crate::terminal::{Identity, TerminalWindow};
use crate::terminal_pointer;
use ronoctua_tokens as tokens;

/// The camera's own words.
#[derive(Debug, Default)]
pub struct Corpus {
    /// The recorded stream.
    file: Option<std::path::PathBuf>,
    /// How many terminals the window holds.
    group: Option<usize>,
    /// The one in front, from 1.
    front: Option<usize>,
    /// The strip's line under the pointer, from 1.
    hover: Option<usize>,
    /// The strip's lines whose terminals ask for the person, from 1.
    attention: Vec<usize>,
}

/// A line's number as the person counts it (from 1), or why not.
fn line_number(flag: &str, word: &str) -> Result<usize> {
    word.parse::<usize>()
        .ok()
        .filter(|&n| n >= 1)
        .with_context(|| format!("`{flag} {word}`: a line is counted from 1"))
}

impl Scene for Corpus {
    const DOOR: &'static str = "terminal";
    const IS: &'static str = "the terminal's window over a recorded stream";
    const WORDS: &'static str = "\
  --corpus <file.vt>    the bytes a program wrote, fed to an engine in this
                        process at the window's size (tests/corpus-vt)
  --group N             the window holds N terminals, its strip drawn
  --front K             the one in front, from 1 (the corpus is its)
  --lit H               the strip's line under the pointer, from 1
  --attention A[,B]     the lines whose terminals ask for the person
  --columns and --lines are the TERMINAL's cells, at its own size of text";

    fn word(&mut self, flag: &str, value: &mut dyn FnMut() -> Result<String>) -> Result<bool> {
        match flag {
            "--corpus" => self.file = Some(value()?.into()),
            "--group" => self.group = Some(line_number(flag, &value()?)?),
            "--front" => self.front = Some(line_number(flag, &value()?)?),
            "--lit" => self.hover = Some(line_number(flag, &value()?)?),
            "--attention" => {
                let lines = value()?;
                for line in lines.split(',') {
                    self.attention.push(line_number(flag, line)?);
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn shoot(self, ui: crate::door::Ui, asked: &Asked) -> Result<()> {
        let file = self.file.context("which stream? `--corpus <file.vt>`")?;
        let corpus = std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
        let count = self.group.unwrap_or(1);
        for (what, line) in [("--front", self.front), ("--lit", self.hover)]
            .into_iter()
            .chain(self.attention.iter().map(|&a| ("--attention", Some(a))))
        {
            ensure!(
                line.is_none_or(|l| l <= count),
                "`{what}` names a line past the group's {count}"
            );
        }
        let mut window = TerminalWindow::photographed_group(
            Identity::Terminal,
            corpus,
            count,
            self.front.map_or(0, |f| f - 1),
        )?;
        window.hover_line(self.hover.map(|h| h - 1));
        for &line in &self.attention {
            window.ring(line - 1);
        }
        let mut d = crate::door::Detached::new(ui, window);
        let size = laid_out_at(&mut d.content, asked);
        d.lay_out_at(size);
        if let Some(why) = d.content.corpus_failed() {
            anyhow::bail!("the corpus could not be drawn: {why}");
        }
        ensure!(
            d.content.sheet_now().lines() > 0,
            "the window laid out no grid"
        );
        scenes::shoot(d, asked, true)
    }
}

/// The size the window is laid out at: `--columns`/`--lines` in the
/// TERMINAL's cells (a corpus is recorded at a grid, and a picture of it at
/// another would wrap it) under a group's strip, `--width`/`--height` in
/// logical pixels, and the declared grid where nothing is said — counted in the cell of the asked
/// `--scale` ([`TerminalWindow::cell_at`]), which the window measures there.
fn laid_out_at(window: &mut TerminalWindow, asked: &Asked) -> (u32, u32) {
    let scale = u32::from(asked.scale.max(1));
    let cell = window.cell_at(scale);
    let cells = |columns: u16, rows: u16| {
        terminal_pointer::size_of_cells(
            columns,
            rows,
            scale,
            terminal_pointer::inset_px(scale),
            &cell,
        )
    };
    let width = match asked.width {
        None => cells(tokens::space::TERMINAL_COLUMNS, 1).0,
        Some(Measure::Px(w)) => w,
        Some(Measure::Cells(n)) => cells(n, 1).0,
    };
    // A GROUP'S STRIP comes off the top of the grid: a grid asked in rows
    // is that many rows UNDER it.
    let strip = crate::terminal_strip::height(window.terminals());
    let height = match asked.height {
        None => cells(1, tokens::space::TERMINAL_LINES).1 + strip,
        Some(Measure::Px(h)) => h,
        Some(Measure::Cells(n)) => cells(1, n).1 + strip,
    };
    (width, height)
}

/// `ronoctua-shell photograph --door terminal …`, to its end.
pub fn taken(rest: &[String]) -> ! {
    scenes::taken(rest, Corpus::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_reads_its_corpus_and_the_shared_words() {
        // FAILS IF `--corpus` stops being this camera's word, or the camera
        // takes words that are not the scenes' own.
        let words: Vec<String> = ["--door", "terminal", "--corpus", "x.vt", "--columns", "80"]
            .iter()
            .map(|w| (*w).to_owned())
            .collect();
        let mut corpus = Corpus::default();
        let asked = scenes::parse(&words, &mut corpus).expect("the words parse");
        assert_eq!(corpus.file.as_deref(), Some(std::path::Path::new("x.vt")));
        assert_eq!(asked.width, Some(Measure::Cells(80)));
        let mut other = Corpus::default();
        assert!(
            scenes::parse(
                &[
                    "--door".to_owned(),
                    "terminal".to_owned(),
                    "--frobnicate".to_owned()
                ],
                &mut other
            )
            .is_err()
        );
    }

    #[test]
    fn the_camera_takes_a_group_counted_as_the_person_counts() {
        // FAILS IF the strip's words are not the camera's, a line is taken
        // from 0 (the photograph shows another line than the one asked), or
        // a line of nothing is accepted.
        let words: Vec<String> = [
            "--door",
            "terminal",
            "--corpus",
            "x.vt",
            "--group",
            "3",
            "--front",
            "2",
            "--lit",
            "3",
            "--attention",
            "1,3",
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
        let mut corpus = Corpus::default();
        scenes::parse(&words, &mut corpus).expect("the words parse");
        assert_eq!(
            (corpus.group, corpus.front, corpus.hover),
            (Some(3), Some(2), Some(3))
        );
        assert_eq!(corpus.attention, [1, 3]);
        let zero: Vec<String> = ["--door", "terminal", "--front", "0"]
            .iter()
            .map(|w| (*w).to_owned())
            .collect();
        assert!(scenes::parse(&zero, &mut Corpus::default()).is_err());
    }

    #[test]
    fn a_groups_grid_asked_in_rows_is_that_many_rows_under_the_strip() {
        // FAILS IF a group's picture asked `--lines 20` lays the window out
        // without the strip's pixels — a grid of 19 rows, the corpus drawn
        // shorter than it was recorded.
        let words: Vec<String> = [
            "--door",
            "terminal",
            "--columns",
            "80",
            "--lines",
            "20",
            "--scale",
            "2",
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
        let asked = scenes::parse(&words, &mut Corpus::default()).unwrap();
        let mut window =
            TerminalWindow::photographed_group(Identity::Terminal, Vec::new(), 3, 1).unwrap();
        let (w, h) = laid_out_at(&mut window, &asked);
        let cell = window.cell_at(2);
        let placed =
            terminal_pointer::Placed::under(w, h, 2, crate::terminal_strip::height(3), &cell);
        assert_eq!((placed.columns, placed.rows), (80, 20));
    }

    #[test]
    fn a_grid_asked_in_cells_is_the_terminals_own_cells_at_every_scale() {
        // FAILS IF `--columns 80 --lines 24` lays the window out in the
        // interface's cells, or in the one-pixel cell at another `--scale`
        // (each scale rounds its cell whole), and the corpus is drawn at
        // another grid than the one it was recorded at.
        for scale in ["1", "2", "3"] {
            let mut window = TerminalWindow::new(Identity::Terminal).unwrap();
            for (columns, lines) in [("80", "24"), ("90", "20"), ("132", "43")] {
                let words: Vec<String> = [
                    "--door",
                    "terminal",
                    "--columns",
                    columns,
                    "--lines",
                    lines,
                    "--scale",
                    scale,
                ]
                .iter()
                .map(|w| (*w).to_owned())
                .collect();
                let asked = scenes::parse(&words, &mut Corpus::default()).unwrap();
                let (w, h) = laid_out_at(&mut window, &asked);
                let s = u32::from(asked.scale);
                let placed = terminal_pointer::Placed::of(w, h, s, &window.cell_at(s));
                assert_eq!(
                    (placed.columns.to_string(), placed.rows.to_string()),
                    (columns.to_owned(), lines.to_owned()),
                    "at scale {scale}"
                );
            }
        }
    }
}
