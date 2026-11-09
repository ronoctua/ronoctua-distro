//! The terminals no window shows, in a window of their own (`RET b w`) — the
//! second door onto [`crate::terminals`].
//!
//! A window and nothing else: which rows exist and what a key does to them is
//! `terminals.rs`, and both doors ask the same view the same questions and
//! carry out what it asks through one handler
//! ([`crate::terminals_region::carry_out`]). The plumbing every window
//! shares is `crate::door`.
//!
//! # Its own census
//!
//! A window lends nothing of the footer's, so it follows the registry
//! itself, as the footer does: the folder watched (inotify, `Notice`), and a
//! thread that reads every holder's state file on each stir — asking each
//! socket once, at the start, so a holder killed before the window opened
//! is swept and not listed. Nothing is read on the loop.

use std::path::PathBuf;
use std::sync::mpsc;

use crate::door::{Content, Door, Ui};
use crate::terminals::{Press, Reading, Target, View};
use crate::terminals_region::{carry_out, home, now_s};
use anyhow::Result;
use ronoctua_terminals::registry::Registry;
use ronoctua_terminals::state::TerminalState;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`. It tiles: a list, as every view's window is.
pub const APP_ID: &str = "br.ronoctua.terminals";

/// Opens the background terminals' view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        let (sender, readings) = channel::channel::<Vec<TerminalState>>();
        w.handle
            .insert_source(readings, |event, (), door: &mut Door<TerminalsWindow>| {
                if let channel::Event::Msg(states) = event {
                    door.content.heard(&mut door.ui, states);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the census channel: {e}"))?;
        // NOTHING READ YET: the thread's first reading comes in a moment,
        // and the cursor settles on it then (`TerminalsWindow::heard`).
        Ok(TerminalsWindow {
            view: View::default(),
            census: Vec::new(),
            home: home(),
            planted_now: None,
            _watch: follow(sender),
        })
    })
}

/// The registry followed: its folder watched, and a thread reading the state
/// files — every socket asked at the first reading, the files alone at each
/// stir after, the stirs that came meanwhile taken together. It ends when the
/// window is gone (its channel closed).
fn follow(sender: channel::Sender<Vec<TerminalState>>) -> Option<ronoctua_files::notice::Notice> {
    let Some(registry) = Registry::of_session() else {
        tracing::warn!("no runtime directory; no held terminal to list");
        return None;
    };
    // MADE BEFORE IT IS WATCHED: a watch on a folder that is not there is no
    // watch, and the first terminal may be born after the window.
    if let Err(e) = registry.ensure() {
        tracing::warn!(error = %e, "the terminals' registry could not be made");
        return None;
    }
    let (stir, stirred) = mpsc::channel::<()>();
    let notice = ronoctua_files::notice::Notice::on_dirs(
        &[registry.dir().to_path_buf()],
        &[],
        // COALESCED: a holder writes its state whole on every change, and a
        // burst of them is one reading of the folder.
        ronoctua_files::notice::Pace::Coalesced,
        move || stir.send(()).is_ok(),
    )
    .map_err(|e| tracing::warn!(error = %e, "the terminals' registry is not watched"))
    .ok();
    let started = std::thread::Builder::new()
        .name("ronoctua-terminals-window".to_owned())
        .spawn(move || {
            let mut states = registry.census().terminals;
            loop {
                if sender.send(states).is_err() || stirred.recv().is_err() {
                    return;
                }
                while stirred.try_recv().is_ok() {}
                states = registry.states();
            }
        });
    if let Err(e) = started {
        tracing::warn!(error = %e, "the terminals' census thread did not start");
        return None;
    }
    notice
}

/// The window: the view, and the census it reads.
struct TerminalsWindow {
    view: View,
    /// The census, as this window's thread read it last.
    census: Vec<TerminalState>,
    home: Option<PathBuf>,
    /// The moment the rows are drawn at, when a camera planted one — `None`
    /// is the clock's.
    planted_now: Option<i64>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl TerminalsWindow {
    /// Now, as the rows count "how long": the clock, or the camera's moment.
    fn now(&self) -> i64 {
        self.planted_now.unwrap_or_else(now_s)
    }

    fn reading(&self) -> Reading<'_> {
        Reading {
            census: &self.census,
            now_s: self.now(),
            home: self.home.as_deref(),
        }
    }

    /// A reading of the census: the cursor kept on its terminal by identity.
    fn heard(&mut self, ui: &mut Ui, states: Vec<TerminalState>) {
        let on: Option<Target> = self.view.on(&self.reading(), ui.columns, &ui.texts);
        self.census = states;
        let r = Reading {
            census: &self.census,
            now_s: self.now(),
            home: self.home.as_deref(),
        };
        self.view.follow(on.as_ref(), &r, ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let r = Reading {
            census: &self.census,
            now_s: self.now(),
            home: self.home.as_deref(),
        };
        let errand = self.view.key(press, &r, ui.columns, &ui.texts);
        carry_out(errand, ui);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::terminals::Row> {
        self.view.rows(&self.reading(), ui.columns, &ui.texts)
    }
}

impl Content for TerminalsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::TERMINALS_TITLE))
    }

    /// Its name at the right end of the first line, with no header
    /// (`door::corner`).
    fn corner(&self, ui: &Ui) -> Option<crate::door::corner::Corner> {
        Some(crate::door::corner::Corner::named(
            &ui.texts,
            crate::key::TERMINALS_TITLE,
        ))
    }

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`terminals::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::terminals::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&self.reading(), ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a terminal or an answer, and only that row lights under
    /// the pointer — the view's one answer ([`crate::terminals::Row::takes`]),
    /// which the footer's region gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::terminals::Row::takes)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        }
    }
}

/// THE BACKGROUND TERMINALS' CAMERA: `photograph --door terminals
/// --terminals <file.json>` — the window over a planted census (a JSON list
/// of the holders' states, as each writes its file), drawn as of `--now`;
/// nothing of the machine is read, and no holder is told anything.
pub mod photograph {
    use anyhow::{Context as _, Result};
    use ronoctua_shell_door::photo::scenes::{self, Asked, Scene};
    use ronoctua_terminals::state::TerminalState;

    use super::TerminalsWindow;
    use crate::terminals::{Reading, View};

    /// The camera's own words.
    #[derive(Debug, Default)]
    pub struct Planted {
        file: Option<std::path::PathBuf>,
        now_s: Option<i64>,
        home: Option<std::path::PathBuf>,
    }

    impl Scene for Planted {
        const DOOR: &'static str = "terminals";
        const IS: &'static str = "the terminals with no window, over a planted census";
        const WORDS: &'static str = "\
  --terminals <file>    the held terminals' states, a JSON list as each holder
                        writes its file (ronoctua_terminals::state)  (required)
  --now <seconds>       the moment the rows are drawn at, since the epoch
                        (default: the newest state's, plus a minute)
  --home <folder>       the home a folder is shown under   (default: none)";

        fn word(&mut self, flag: &str, value: &mut dyn FnMut() -> Result<String>) -> Result<bool> {
            match flag {
                "--terminals" => self.file = Some(value()?.into()),
                "--now" => {
                    let raw = value()?;
                    self.now_s = Some(
                        raw.parse()
                            .with_context(|| format!("`--now {raw}` is not seconds"))?,
                    );
                }
                "--home" => self.home = Some(value()?.into()),
                _ => return Ok(false),
            }
            Ok(true)
        }

        fn shoot(self, ui: crate::door::Ui, asked: &Asked) -> Result<()> {
            let file = self
                .file
                .context("which terminals? `--terminals <file.json>`")?;
            let raw = std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
            let census: Vec<TerminalState> = serde_json::from_slice(&raw)
                .with_context(|| format!("{} is no list of states", file.display()))?;
            let now_s = self.now_s.unwrap_or_else(|| {
                census
                    .iter()
                    .filter_map(|t| t.alone_since_s)
                    .max()
                    .unwrap_or(0)
                    + 60
            });
            let view = View::fresh(
                &Reading {
                    census: &census,
                    now_s,
                    home: self.home.as_deref(),
                },
                ui.columns,
                &ui.texts,
            );
            let window = TerminalsWindow {
                view,
                census,
                home: self.home,
                planted_now: Some(now_s),
                _watch: None,
            };
            scenes::walked(crate::door::Detached::new(ui, window), asked)
        }
    }

    /// `ronoctua-shell photograph --door terminals …`, to its end.
    pub fn taken(rest: &[String]) -> ! {
        scenes::taken(rest, Planted::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminals::planted;

    /// A window over three terminals each alone — no census thread behind
    /// it and no registry watched — and never this machine's.
    fn a_window() -> (TerminalsWindow, Ui) {
        let ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let census: Vec<TerminalState> = (0..3)
            .map(|i| {
                planted(
                    "zsh",
                    &ronoctua_terminals::GroupId::fresh().unwrap(),
                    1 << 16,
                    now_s() - 60 * (i + 1),
                )
            })
            .collect();
        let view = View::fresh(
            &Reading {
                census: &census,
                now_s: now_s(),
                home: None,
            },
            ui.columns,
            &ui.texts,
        );
        let w = TerminalsWindow {
            view,
            census,
            home: None,
            planted_now: None,
            _watch: None,
        };
        (w, ui)
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a heading, the air — or leaves unlit one it takes. THE
    /// LIGHT IS THE CLICK: every row clicked once, on a window of its own,
    /// and lit exactly where something happened. The footer's region has
    /// its own.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let held = crate::door::held_to_the_click(a_window, crate::door::seen);
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }

    /// FAILS IF: a new reading of the census moves the window's cursor to
    /// another terminal (it stayed on an index while one came in above), or
    /// the second click on a terminal does not send the footer its
    /// `terminal-show` — the window's errand is not the region's.
    #[test]
    fn the_window_follows_its_census_and_shows_on_the_second_click() {
        let (mut w, mut ui) = a_window();
        Content::clicked(&mut w, &mut ui, 2);
        let on = w.view.on(&w.reading(), ui.columns, &ui.texts);
        let Some(Target::Terminal(id)) = on.clone() else {
            panic!("the click put the cursor on no terminal: {on:?}");
        };
        let mut states = w.census.clone();
        states.insert(
            0,
            planted(
                "vim",
                &ronoctua_terminals::GroupId::fresh().unwrap(),
                1,
                now_s(),
            ),
        );
        w.heard(&mut ui, states);
        assert_eq!(w.view.on(&w.reading(), ui.columns, &ui.texts), on);
        let at = w.view.cursor();
        Content::clicked(&mut w, &mut ui, at);
        assert_eq!(ui.orders(), &[format!("command terminal-show {id}")]);
    }

    /// FAILS IF: the camera stops reading its census, or takes a word that
    /// is not one of its own or of the scenes'.
    #[test]
    fn the_camera_reads_its_census_and_the_shared_words() {
        use ronoctua_shell_door::photo::scenes;
        let words: Vec<String> = ["--door", "terminals", "--terminals", "t.json", "--now", "9"]
            .iter()
            .map(|w| (*w).to_owned())
            .collect();
        let mut planted = photograph::Planted::default();
        assert!(scenes::parse(&words, &mut planted).is_ok());
        let wrong: Vec<String> = ["--door", "terminals", "--now", "soon"]
            .iter()
            .map(|w| (*w).to_owned())
            .collect();
        assert!(scenes::parse(&wrong, &mut photograph::Planted::default()).is_err());
    }
}
