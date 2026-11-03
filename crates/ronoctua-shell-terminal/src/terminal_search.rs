//! FINDING TEXT IN A TERMINAL (`C-S-f`): the window's own line over the last
//! row (`crate::terminal_ask`) becomes what is being looked for, the matches
//! on screen are drawn in the selection's colours — the one walked to in the
//! cursor's, so the line's "k of n" has a place on screen — `RET` and
//! `S-RET` walk them toward older and newer output, and `Esc` ends it.
//!
//! # The search runs on a thread of its own
//!
//! A terminal's history is thousands of pages, and the engine's search reads
//! all of it ([`ronoctua_term::ghostty::Search`]). Each key typed is an order
//! to a WORKER that owns the search ([`Finding::start`]): it catches up with
//! the terminal, counts the matches, takes the ones on screen and tells the
//! window ([`Found`]), and the window draws them on its next frame. Orders
//! that came while it worked are taken together — what was typed meanwhile
//! is one search, not one a letter. The worker shares the mirror's own gate
//! with the reader that feeds it, so it never reads a terminal mid-write.
//!
//! # Not a question of the door's
//!
//! `Ui::ask` would take the line 0 of a window that has none: a terminal's
//! rows are the program's, and the window speaks over its last one. So the
//! search is laid where the window's questions are, and it keeps every key
//! while it is up — a letter meant for the search never reaches the program.

use std::sync::mpsc;

use anyhow::{Context as _, Result};
use ronoctua_render::cells::{Rgba, Sheet};
use ronoctua_shell_kit::chrome::{Piece, Register};
use ronoctua_term::ghostty::{Highlight, Mirror, Search};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

use crate::terminal_ask::{Click, Run};

/// What the worker found, as of the needle it was given last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// The needle these are the matches of.
    pub needle: String,
    /// Every match, in the whole history.
    pub total: usize,
    /// The one walked to, from 0 the newest — `None` before any walk.
    pub selected: Option<usize>,
    /// The matches on screen, in viewport cells.
    pub on_screen: Vec<Highlight>,
    /// Among them, the one walked to — `None` before any walk, and while it
    /// is off screen.
    pub current: Option<Highlight>,
}

/// An order to the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Look for this.
    Needle(String),
    /// The next match toward older output.
    Older,
    /// The next match toward newer output.
    Newer,
    /// The screen moved: the matches on it again.
    Look,
}

/// What a key did to the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The needle changed: look again.
    Typed,
    /// Walk toward older output.
    Older,
    /// Walk toward newer output.
    Newer,
    /// The search ends.
    Closed,
    /// Nothing the search does — and still taken: a key meant for the
    /// search never reaches the program under it.
    Kept,
}

/// THE SEARCH ON THE LINE: the needle being typed, what was found, and the
/// worker's queue.
pub struct Finding {
    /// What is being looked for.
    pub needle: String,
    /// Which attachment of the window it searches (`Link::generation`): a
    /// search outlives no change of the terminal in front.
    pub generation: u64,
    /// What the worker said last.
    pub found: Found,
    /// The worker's queue; `None` for a search with no worker (a test's).
    orders: Option<mpsc::Sender<Order>>,
}

impl std::fmt::Debug for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Finding")
            .field("needle", &self.needle)
            .field("generation", &self.generation)
            .field("found", &self.found)
            .finish_non_exhaustive()
    }
}

impl Finding {
    /// A search over `mirror` — the attachment `generation`'s — its worker
    /// started, every [`Found`] told through `tell` (off the loop: the window
    /// is woken by it).
    ///
    /// # Errors
    ///
    /// The engine made no search, or no thread could be started.
    pub fn start(
        mirror: &Mirror,
        generation: u64,
        tell: impl Fn(Found) + Send + 'static,
    ) -> Result<Self> {
        let search = Search::new(mirror).context("a search over the terminal")?;
        let (orders, queue) = mpsc::channel();
        std::thread::Builder::new()
            .name("term-search".to_owned())
            .spawn(move || work(search, &queue, &tell))
            .context("the search's thread")?;
        Ok(Self {
            needle: String::new(),
            generation,
            found: Found::default(),
            orders: Some(orders),
        })
    }

    /// A search with no worker — what a test types into.
    #[must_use]
    pub fn idle(generation: u64) -> Self {
        Self {
            needle: String::new(),
            generation,
            found: Found::default(),
            orders: None,
        }
    }

    /// Sends the worker an order. A worker that is gone (the terminal's
    /// engine refused it) leaves the line as it is.
    pub fn order(&self, order: Order) {
        if let Some(orders) = &self.orders {
            let _ = orders.send(order);
        }
    }

    /// A key, while the search is up: `RET` the next match toward older
    /// output, `S-RET` toward newer, `Esc` the end, `DEL` a character off the
    /// needle, and text typed onto it.
    pub fn key(&mut self, event: &KeyEvent, mods: Modifiers) -> Step {
        match event.keysym {
            Keysym::Escape => Step::Closed,
            Keysym::Return | Keysym::KP_Enter if mods.shift => Step::Newer,
            Keysym::Return | Keysym::KP_Enter => Step::Older,
            Keysym::Up | Keysym::KP_Up => Step::Older,
            Keysym::Down | Keysym::KP_Down => Step::Newer,
            Keysym::BackSpace => {
                if self.needle.pop().is_some() {
                    Step::Typed
                } else {
                    Step::Kept
                }
            }
            _ if mods.ctrl || mods.alt || mods.logo => Step::Kept,
            _ => match event.utf8.as_deref() {
                Some(text) if !text.is_empty() && !text.chars().any(char::is_control) => {
                    self.needle.push_str(text);
                    Step::Typed
                }
                _ => Step::Kept,
            },
        }
    }

    /// Text pasted while the search is up goes onto the needle, line
    /// breaks and all control characters left out — a needle is one line.
    pub fn paste(&mut self, text: &str) -> Step {
        let before = self.needle.len();
        self.needle.extend(text.chars().filter(|c| !c.is_control()));
        if self.needle.len() == before {
            Step::Kept
        } else {
            Step::Typed
        }
    }

    /// The worker said what it found: kept when it is about the needle on
    /// the line (an answer to one typed before is already stale). `true`
    /// when it changed what is drawn.
    pub fn heard(&mut self, found: Found) -> bool {
        if found.needle != self.needle || found == self.found {
            return false;
        }
        self.found = found;
        true
    }

    /// The line's runs: what it is, the needle, how many were found, and
    /// the walk and the end as runs a click takes.
    #[must_use]
    pub fn runs(&self, texts: &ronoctua_i18n::Texts, lit: Option<Click>) -> Vec<Run> {
        use crate::key;
        let count = |n: usize| ronoctua_i18n::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX));
        let said = if self.needle.is_empty() || self.found.needle != self.needle {
            None
        } else if self.found.total == 0 {
            Some(texts.t(key::TERMINAL_SEARCH_NONE))
        } else {
            Some(texts.t_args(
                key::TERMINAL_SEARCH_COUNT,
                &[
                    ("at", count(self.found.selected.map_or(0, |s| s + 1))),
                    ("total", count(self.found.total)),
                ],
            ))
        };
        let separator = || Run::of(&Piece::separator());
        let mut runs = vec![
            Run::of(&Piece::prose(texts.t(key::TERMINAL_SEARCH))),
            Run {
                text: format!(" {}", self.needle),
                ink: Register::Said.color(),
                does: None,
            },
        ];
        if let Some(said) = said {
            runs.push(separator());
            runs.push(Run::of(&Piece::prose(said)));
        }
        runs.extend([
            separator(),
            Run::action(texts.t(key::TERMINAL_SEARCH_OLDER), Click::Older, lit),
            separator(),
            Run::action(texts.t(key::TERMINAL_SEARCH_NEWER), Click::Newer, lit),
            separator(),
            Run::action(texts.t(key::TERMINAL_SEARCH_CLOSE), Click::Close, lit),
        ]);
        runs
    }
}

/// THE WORKER: orders taken in batches, the search caught up after each,
/// and what it found told — until the window drops its queue.
fn work(mut search: Search, queue: &mpsc::Receiver<Order>, tell: &dyn Fn(Found)) {
    let mut needle = String::new();
    while let Ok(first) = queue.recv() {
        let batch: Vec<Order> = std::iter::once(first).chain(queue.try_iter()).collect();
        let done = (|| -> Result<(), ronoctua_term::ghostty::Refused> {
            for order in batch {
                match order {
                    Order::Needle(n) => {
                        if n != needle {
                            search.set_needle(&n)?;
                            needle = n;
                        }
                    }
                    // A WALK needs the matches counted first: the search
                    // catches up before it selects.
                    Order::Older => {
                        search.run()?;
                        search.select_older()?;
                    }
                    Order::Newer => {
                        search.run()?;
                        search.select_newer()?;
                    }
                    Order::Look => {}
                }
            }
            search.run()
        })();
        if let Err(e) = done {
            tracing::warn!(error = %e, "the terminal's search stopped");
            return;
        }
        // THE LIST AND THE ONE WALKED TO, from one feed: a frame never pairs
        // them from two moments of the terminal.
        let (on_screen, current) = if needle.is_empty() {
            (Vec::new(), None)
        } else {
            search.drawn()
        };
        tell(Found {
            needle: needle.clone(),
            total: if needle.is_empty() { 0 } else { search.total() },
            selected: search.selected(),
            on_screen,
            current,
        });
    }
}

/// The matches drawn on the sheet: every match in the page's selection
/// colours — the colours a selection already has on every theme, measured
/// against the terminal's ground — and the one walked to in the CURSOR's
/// (`terminal.cursor` under `terminal.cursor-text`), the other pair every
/// theme measures there, so the line's "k of n" is a place on screen and no
/// colour is chosen here. Only the rows whose cells change are written. What
/// a match leaves behind when it moves is the window's to write again (every
/// row, once, when the matches change).
pub fn marks(sheet: &mut Sheet, found: &Shown, palette: &crate::terminal_sheet::Palette) {
    mark(
        sheet,
        &found.matches,
        palette.selection_foreground,
        palette.selection_background,
    );
    if let Some(current) = found.current {
        mark(sheet, &[current], palette.cursor_text, palette.cursor);
    }
}

/// What a frame draws of a search: the matches on screen and the one
/// walked to — compared whole, so a walk that moved only the current one
/// writes the rows again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown {
    /// Every match on screen.
    pub matches: Vec<Highlight>,
    /// The one walked to.
    pub current: Option<Highlight>,
}

/// The matches in `ink` over `ground`, writing only the rows whose cells
/// change.
pub fn mark(sheet: &mut Sheet, matches: &[Highlight], ink: Rgba, ground: Rgba) {
    let (columns, lines) = (sheet.columns(), sheet.lines());
    for m in matches {
        let (start, end) = (m.start, m.end);
        for line in start.1..=end.1.min(lines.saturating_sub(1)) {
            let from = if line == start.1 { start.0 } else { 0 };
            let to = if line == end.1 {
                end.0.saturating_add(1)
            } else {
                columns
            }
            .min(columns);
            if from >= to {
                continue;
            }
            let span = usize::from(from)..usize::from(to);
            if sheet.row(line)[span.clone()]
                .iter()
                .all(|c| c.fg == ink && c.bg == Some(ground))
            {
                continue;
            }
            for cell in &mut sheet.row_mut(line)[span] {
                cell.fg = ink;
                cell.bg = Some(ground);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_term::ghostty;
    use std::time::Duration;

    fn pressed(keysym: Keysym, utf8: Option<&str>) -> KeyEvent {
        KeyEvent {
            time: 0,
            raw_code: 0,
            keysym,
            utf8: utf8.map(str::to_owned),
        }
    }

    fn mirror_of(bytes: &[u8], columns: u16, rows: u16) -> Mirror {
        let mut vt = ghostty::Vt::new(
            columns,
            rows,
            &ghostty::Config::for_version(crate::terminal_sheet::VERSION),
        )
        .unwrap();
        vt.feed(bytes);
        ghostty::Decoder::decode(vt.snapshot().unwrap()).unwrap()
    }

    #[test]
    fn typing_edits_the_needle_and_the_walk_is_ret_and_shift_ret() {
        // FAILS IF a letter typed at the search reaches the program instead
        // of the needle, `DEL` does not take one off, `RET`/`S-RET` do not
        // walk the two ways, `Esc` does not end it, or a chord types its
        // letter into the needle.
        let mut f = Finding::idle(1);
        let none = Modifiers::default();
        assert_eq!(f.key(&pressed(Keysym::e, Some("e")), none), Step::Typed);
        assert_eq!(f.key(&pressed(Keysym::r, Some("r")), none), Step::Typed);
        assert_eq!(f.needle, "er");
        assert_eq!(f.key(&pressed(Keysym::BackSpace, None), none), Step::Typed);
        assert_eq!(f.needle, "e");
        assert_eq!(
            f.key(&pressed(Keysym::Return, Some("\r")), none),
            Step::Older
        );
        let shift = Modifiers {
            shift: true,
            ..none
        };
        assert_eq!(
            f.key(&pressed(Keysym::Return, Some("\r")), shift),
            Step::Newer
        );
        let ctrl = Modifiers { ctrl: true, ..none };
        assert_eq!(f.key(&pressed(Keysym::c, Some("\u{3}")), ctrl), Step::Kept);
        assert_eq!(f.needle, "e");
        assert_eq!(f.key(&pressed(Keysym::Escape, None), none), Step::Closed);
        assert_eq!(f.paste("a\nb"), Step::Typed);
        assert_eq!(f.needle, "eab", "a needle is one line");
    }

    #[test]
    fn the_worker_finds_the_matches_on_screen_and_walks_them() {
        // FAILS IF the matches on screen are not where the text is (the
        // highlight is drawn on other cells), the count misses one in the
        // history, an answer about an old needle is taken for the one on the
        // line, or the walk selects nothing.
        let mut feed = Vec::new();
        for n in 0..30 {
            feed.extend(format!("line {n} needle here\r\n").as_bytes());
        }
        feed.extend(b"last: a needle and a needle");
        let mirror = mirror_of(&feed, 40, 6);
        let (tx, rx) = mpsc::channel();
        let mut f = Finding::start(&mirror, 3, move |found| {
            let _ = tx.send(found);
        })
        .unwrap();
        f.needle = "needle".to_owned();
        f.order(Order::Needle(f.needle.clone()));
        let wait = || {
            rx.recv_timeout(Duration::from_secs(20))
                .expect("an answer in time")
        };
        let found = wait();
        assert_eq!(found.needle, "needle");
        assert_eq!(found.total, 32, "every match in the history");
        // THE LAST ROW holds two: columns 8..=13 and 21..=26.
        let last = 5u16;
        let on_last: Vec<Highlight> = found
            .on_screen
            .iter()
            .copied()
            .filter(|h| h.start.1 == last)
            .collect();
        assert!(
            on_last.contains(&Highlight {
                start: (8, last),
                end: (13, last)
            }),
            "{:?}",
            found.on_screen
        );
        assert!(on_last.contains(&Highlight {
            start: (21, last),
            end: (26, last)
        }));
        assert_eq!(found.current, None, "a match is current before any walk");
        assert!(f.heard(found.clone()));
        assert!(!f.heard(found), "the same answer twice is no change");
        f.order(Order::Older);
        let walked = wait();
        assert!(walked.selected.is_some(), "the walk selected nothing");
        // THE NEWEST MATCH is the first walk's: the last one on the last row.
        assert_eq!(
            walked.current,
            Some(Highlight {
                start: (21, last),
                end: (26, last)
            }),
            "the match walked to is not told apart"
        );
        // AN ANSWER ABOUT ANOTHER NEEDLE is not the line's.
        f.needle = "other".to_owned();
        assert!(!f.heard(walked));
    }

    #[test]
    fn the_matches_wear_the_selection_and_rows_are_written_once() {
        // FAILS IF a match is drawn on cells other than its own, a match
        // over two rows loses the second, or a frame that changed nothing
        // writes a row again (the GPU rebuilding it every frame of a search).
        let stroke = ronoctua_render::sprite::Stroke {
            top: 0,
            thickness: 1,
        };
        let geometry = ronoctua_render::cells::Geometry {
            cell_width: 10,
            cell_height: 20,
            baseline: 15,
            stroke: 1,
            underline: stroke,
            strike: stroke,
            overline: stroke,
            size: 14.0,
            scale: 1,
        };
        let plain = Rgba::opaque(200, 200, 200);
        let mut sheet = Sheet::new(
            geometry,
            10,
            3,
            ronoctua_render::cells::Defaults {
                foreground: plain,
                background: Rgba::opaque(0, 0, 0),
                faint: 128,
            },
        );
        let (ink, ground) = (Rgba::opaque(255, 255, 255), Rgba::opaque(60, 60, 120));
        let across = Highlight {
            start: (8, 0),
            end: (1, 1),
        };
        mark(&mut sheet, &[across], ink, ground);
        let marked = |line: u16, c: usize| sheet.row(line)[c].bg == Some(ground);
        assert!(marked(0, 8) && marked(0, 9) && marked(1, 0) && marked(1, 1));
        assert!(!marked(0, 7) && !marked(1, 2) && !marked(2, 0));
        let generations: Vec<u64> = (0..3).map(|l| sheet.generation_of(l)).collect();
        mark(&mut sheet, &[across], ink, ground);
        assert_eq!(
            (0..3).map(|l| sheet.generation_of(l)).collect::<Vec<_>>(),
            generations
        );
    }

    #[test]
    fn the_match_walked_to_wears_the_cursor_and_the_rest_the_selection() {
        // FAILS IF the current match is drawn like every other one (the line
        // says "3 of 12" and nothing on screen says which), or in a colour
        // that is not a role of the page — or the others lose the
        // selection's colours to it.
        let stroke = ronoctua_render::sprite::Stroke {
            top: 0,
            thickness: 1,
        };
        let geometry = ronoctua_render::cells::Geometry {
            cell_width: 10,
            cell_height: 20,
            baseline: 15,
            stroke: 1,
            underline: stroke,
            strike: stroke,
            overline: stroke,
            size: 14.0,
            scale: 1,
        };
        let palette = crate::terminal_sheet::Palette::of_the_page();
        let mut sheet = Sheet::new(
            geometry,
            10,
            2,
            ronoctua_render::cells::Defaults {
                foreground: palette.foreground,
                background: palette.background,
                faint: 128,
            },
        );
        let (one, two) = (
            Highlight {
                start: (0, 0),
                end: (2, 0),
            },
            Highlight {
                start: (0, 1),
                end: (2, 1),
            },
        );
        let shown = Shown {
            matches: vec![one, two],
            current: Some(two),
        };
        marks(&mut sheet, &shown, &palette);
        assert_eq!(sheet.row(0)[1].bg, Some(palette.selection_background));
        assert_eq!(sheet.row(0)[1].fg, palette.selection_foreground);
        assert_eq!(sheet.row(1)[1].bg, Some(palette.cursor));
        assert_eq!(sheet.row(1)[1].fg, palette.cursor_text);
        assert_ne!(
            palette.cursor, palette.selection_background,
            "the page draws the current match as every other"
        );
    }

    #[test]
    fn the_line_says_how_many_and_offers_the_walk_as_clicks() {
        // FAILS IF the count is not said with numbers (a selector fed text
        // falls back calmly on another variant), the walk and the end are
        // not runs a click takes, or the count of a needle the worker has
        // not answered yet is said.
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut f = Finding::idle(1);
        f.needle = "x".to_owned();
        let runs = f.runs(&texts, None);
        assert!(
            !runs
                .iter()
                .any(|r| r.text == texts.t(crate::key::TERMINAL_SEARCH_NONE))
        );
        for click in [Click::Older, Click::Newer, Click::Close] {
            assert!(runs.iter().any(|r| r.does == Some(click)), "{click:?}");
        }
        f.found = Found {
            needle: "x".to_owned(),
            total: 12,
            selected: Some(2),
            on_screen: Vec::new(),
            current: None,
        };
        let said: String = f
            .runs(&texts, None)
            .iter()
            .map(|r| r.text.clone())
            .collect();
        assert!(said.contains("12") && said.contains('3'), "{said}");
        f.found.total = 0;
        let none: String = f
            .runs(&texts, None)
            .iter()
            .map(|r| r.text.clone())
            .collect();
        assert!(none.contains(&texts.t(crate::key::TERMINAL_SEARCH_NONE)));
    }
}
