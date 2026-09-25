//! The shell's history, in a window of its own — the second door onto
//! [`crate::history`].
//!
//! It DECIDES nothing: which rows exist and what a key does to them is
//! `history.rs`, and both doors ask the same view the same questions.
//!
//! It is `audio_window.rs` with the view swapped, deliberately: eleven doors
//! that drifted apart would be eleven answers to how a wheel scrolls.
//!
//! # Why THIS view gets a window and the clipboard does not
//!
//! Because `atuin` is a BINARY and `config.toml` is a FILE, and a second
//! process can read both. The clipboard's history is not: it is a `VecDeque`
//! inside the footer, filled by that process's own `zwlr_data_control`
//! watcher. A window would have to open a second watcher and would then hold a
//! second history — which is the exact defect `ronoctua-clipcascade` exists to
//! avoid, one process over. See `docs/PENDING.md`.
//!
//! # The one thing this door CANNOT do
//!
//! **Type.** The only place anything is typed in this system is the footer's
//! minibuffer, and this window has none — so a row that needs a value says so
//! and points at `LEADER s h h`.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Keysym, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.history";

/// Opens the shell history's view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `atuin.json`, watched like the theme: a setting changed in the
        // footer moves this window without it being reopened. Both doors write
        // this file, which is why it is read back rather than assumed.
        let (wish_sender, wish_changed) = channel::channel::<()>();
        let watch = ronoctua_atuin::prefs::store()
            .path()
            .and_then(|p| p.parent().map(ToOwned::to_owned))
            .and_then(|dir| {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    tracing::warn!(error = %e, "no configuration directory to watch atuin.json in");
                    return None;
                }
                ronoctua_files::notice::Notice::on_file(
                    &dir,
                    ronoctua_atuin::prefs::FILE,
                    ronoctua_files::notice::Pace::Coalesced,
                    move || wish_sender.send(()).is_ok(),
                )
                .ok()
            });
        w.handle
            .insert_source(wish_changed, |event, (), door: &mut Door<HistoryWindow>| {
                if let channel::Event::Msg(()) = event {
                    // THE FILE IS THE BROADCAST, and this door is one of its
                    // two readers. The ring is drawn HERE, so a turn made in
                    // the footer has to reach this window without it being
                    // reopened. Only the MARK is taken: the rest of the
                    // snapshot is the thread's, and a door that rebuilt it
                    // from a file would be a second reading.
                    let fresh = ronoctua_atuin::prefs::read().mark;
                    if fresh != door.content.snapshot.mark {
                        door.content.snapshot.mark = fresh;
                        door.ui.ask_paint();
                    }
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the atuin.json watch: {e}"))?;

        // ATUIN, on a thread of this process. `atuin status` is a PROCESS, so
        // what arrives is a whole `Snapshot` and never a delta — the shape the
        // Syncthing and the audio boundaries already have.
        let (atuin_sender, atuin_events) = channel::channel::<ronoctua_atuin::Event>();
        let atuin = match ronoctua_atuin::watch(move |e| {
            let _ = atuin_sender.send(e);
        }) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "atuin is not on this machine; the window opens saying so");
                None
            }
        };
        w.handle
            .insert_source(atuin_events, |event, (), door: &mut Door<HistoryWindow>| {
                if let channel::Event::Msg(e) = event {
                    door.content.atuin_event(&mut door.ui, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the atuin channel: {e}"))?;

        Ok(HistoryWindow {
            view: crate::history::View::fresh(),
            // WHAT THE FILES SAY, before the thread has said anything.
            // `default()` has `present: false`, so the window opened on "atuin
            // is not installed" and replaced it a moment later — a sentence
            // that was never true, shown to whoever was looking.
            snapshot: ronoctua_atuin::at_rest(),
            atuin,
            _watch: watch,
        })
    })
}

/// The window.
struct HistoryWindow {
    view: crate::history::View,
    /// The last reading, from this process's own threads.
    snapshot: ronoctua_atuin::Snapshot,
    /// The orders thread, when `atuin` is on this machine.
    atuin: Option<ronoctua_atuin::Hand>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl HistoryWindow {
    fn atuin_event(&mut self, ui: &mut Ui, e: ronoctua_atuin::Event) {
        match e {
            ronoctua_atuin::Event::Snapshot(s) => {
                self.snapshot = *s;
                ui.ask_paint();
            }
            // THE KEY, WHICH WAS ASKED FOR AND IS SHOWN ONCE. It is the phrase
            // that decrypts the history, and the view holds it only until
            // `Esc` — see `history.rs` for why it is shown at all.
            ronoctua_atuin::Event::Key(key) => {
                self.view.took_the_key(key);
                ui.ask_paint();
            }
            ronoctua_atuin::Event::Done { refused, .. } => {
                if let Some(reason) = refused {
                    ui.message = Some(ui.texts.t_with(
                        ronoctua_i18n::key::ECHO_ATUIN_REFUSED,
                        &[("reason", &reason)],
                    ));
                    ui.ask_paint();
                }
            }
        }
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::history::Errand) {
        use crate::history::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Order(order) => match &self.atuin {
                Some(h) => h.send(order),
                None => {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::ATUIN_ABSENT));
                }
            },
            // THE KEY IS FETCHED HERE, because `atuin key` is a process and
            // this window's loop is one thread — but it is a single short
            // call, not a wait on a socket.
            Errand::ShowKey => match ronoctua_atuin::cli::key() {
                Ok(key) => self.view.took_the_key(key),
                Err(e) => {
                    ui.message = Some(ui.texts.t_with(
                        ronoctua_i18n::key::ECHO_ATUIN_REFUSED,
                        &[("reason", &e.to_string())],
                    ));
                }
            },
            // THE ONE THING THIS DOOR CANNOT DO IS TYPE. The minibuffer is the
            // footer's, and this window has none — so it says where to go,
            // which is what every other window of this system does for a
            // password, a path and a zone.
            Errand::AskServer | Errand::Edit(_) => {
                ui.message = Some(ui.texts.t(ronoctua_i18n::key::ATUIN_TYPED_IN_THE_FOOTER));
            }
        }
        ui.ask_paint();
    }

    /// What one press means — from the keyboard, or `RET` from a click. The
    /// key clears the last order's message; the click never did.
    fn pressed(&mut self, ui: &mut Ui, press: crate::history::Press) {
        let facts = crate::history::Facts {
            shot: &self.snapshot,
        };
        let errand = self.view.key(press, &facts, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }
}

impl Content for HistoryWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::ATUIN_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        if let Some(press) = press_of(event) {
            ui.message = None;
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        let facts = crate::history::Facts {
            shot: &self.snapshot,
        };
        self.view
            .rows(&facts, ui.columns, &ui.texts)
            .into_iter()
            .map(|r| r.fields)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE MOUSE AIMS, THE KEYS ACT — by the same `Enter` as the keyboard,
    /// which is what the footer does with `Click::History`: the click puts
    /// the cursor on the row, and `RET` is what applies, arms and confirms.
    /// Nothing here may be reachable only by the mouse, nor behave
    /// differently depending on how one got to it.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        self.view.clicked(row);
        self.pressed(ui, crate::history::Press::Enter);
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
///
/// BOTH DOORS GO THROUGH `history::OF_THE_VIEW`. This one turns SCTK's keysym
/// into the table's notation and asks the view; the footer turns its own
/// `Key` into the same notation and asks the same question. A key written in
/// only one of the two would be a key one door answers and the other does
/// not, which is the defect `bluetooth-keys-reach-both-doors` was written for.
fn press_of(event: &KeyEvent) -> Option<crate::history::Press> {
    let notation = match event.keysym {
        // ESCAPE IS NOT IN THE TABLE, and deliberately: every view of this
        // shape leaves one level on it, and the hint line does not teach a
        // key that is the same everywhere.
        Keysym::Escape => return Some(crate::history::Press::Escape),
        Keysym::Return | Keysym::KP_Enter => "RET",
        Keysym::Up => "<up>",
        Keysym::Down => "<down>",
        Keysym::Left => "<left>",
        Keysym::Right => "<right>",
        // AND THE BARE LETTERS, by the character SCTK hands this door. The
        // footer matches `Key::simple(sym).notation()`, which is what ITS
        // toolkit hands it; the two expressions agree on exactly the
        // unmodified lowercase letters the table holds, which is why the
        // table is the only place a letter is written.
        _ => event.utf8.as_deref()?,
    };
    crate::history::OF_THE_VIEW
        .iter()
        .find(|(t, _)| *t == notation)
        .and_then(|(_, id)| crate::history::press_of(id))
}
