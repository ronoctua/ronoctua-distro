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
//! # It types what the footer types
//!
//! The login — server, account, password, key — and a setting's value are
//! asked on this window's own line (`Ui::ask`), the password and the key
//! hidden, and answered through the same `View::answered` the footer's
//! minibuffer reaches. The order goes to this process's own Atuin thread; the
//! password goes nowhere else.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
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
            // WHAT THE FOOTER SAYS, from the same function.
            ronoctua_atuin::Event::Done { order, refused } => {
                if let Some(said) =
                    crate::history::said_when_done(&order, refused.as_deref(), &ui.texts)
                {
                    ui.message = Some(said);
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
            // ON THIS WINDOW'S OWN LINE, hidden when the view says so; the
            // answer comes back to `answered`.
            Errand::Ask(question) => {
                ui.ask(crate::line::Line::new(question.id, question.secret, None));
            }
        }
        ui.ask_paint();
    }

    /// What one press means — from the keyboard, or `RET` from a second
    /// click.
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

    /// A key, read by its whole notation through the one reader the footer
    /// calls too (`history::press_of_key`) — which is what makes `S-<right>`
    /// and `S-<left>` reach the view, from the keyboard and from the hint.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::history::press_of_key)
        {
            ui.message = None;
            self.pressed(ui, press);
        }
    }

    /// What was typed on this window's line: the same `View::answered` the
    /// footer's minibuffer reaches — the login's next question, or the order.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        ui.message = None;
        let errand = self.view.answered(id, text);
        self.apply(ui, errand);
    }

    /// The line was let go, and the view forgets the question — and a
    /// password typed for the login.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.view.let_go();
        ui.ask_paint();
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

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19, and what
    /// the footer does with `Click::History`: a click puts the cursor on the
    /// row, and a click on the row it is already on is that row's `RET`,
    /// through the same `pressed` a key goes through.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let facts = crate::history::Facts {
            shot: &self.snapshot,
        };
        if self.view.clicked(row, &facts, ui.columns, &ui.texts) {
            self.pressed(ui, crate::history::Press::Enter);
        } else {
            ui.ask_paint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Press, Question};

    /// A window with no Atuin thread behind it: every order it gives is
    /// answered with "atuin is not installed" on its own line — which is what
    /// a test of the wiring can see — and nothing reaches the footer.
    fn a_window(logged_in: bool) -> (HistoryWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = HistoryWindow {
            view: crate::history::View::fresh(),
            snapshot: ronoctua_atuin::Snapshot {
                present: true,
                logged_in,
                syncing: logged_in,
                address: "https://history.example.org".to_owned(),
                username: "someone".to_owned(),
                ..ronoctua_atuin::Snapshot::default()
            },
            atuin: None,
            _watch: None,
        };
        (me, Ui::for_tests(texts))
    }

    /// Where the row carrying `wanted` is, as this window draws it.
    fn row_carrying(w: &HistoryWindow, ui: &Ui, wanted: &str) -> usize {
        let rows = w.rows(ui);
        rows.iter()
            .position(|r| r.iter().any(|f| f.text == wanted))
            .unwrap_or_else(|| panic!("no row says `{wanted}`"))
    }

    /// The sentence an order gets here with no Atuin thread: the order left
    /// for this window's own thread.
    fn ordered(ui: &Ui) -> bool {
        ui.message.as_deref() == Some(ui.texts.t(ronoctua_i18n::key::ATUIN_ABSENT).as_str())
    }

    /// Answers the line that is open, as the door does: the line leaves
    /// `ui.asking` first, and the window hears the text under its id.
    fn answer(w: &mut HistoryWindow, ui: &mut Ui, text: &str) {
        let line = ui.asking.take().expect("no line is open");
        w.answered(ui, line.id, text);
    }

    /// THE LOGIN IS TYPED IN THE WINDOW — four questions on its own line, the
    /// password and the key hidden — and the order is this window's: nothing
    /// goes to the footer, and the password is on no line.
    #[test]
    fn the_login_is_typed_on_the_windows_own_line() {
        let (mut w, mut ui) = a_window(false);
        let log_in = row_carrying(&w, &ui, &ui.texts.t(ronoctua_i18n::key::ATUIN_LOG_IN));
        w.clicked(&mut ui, log_in);
        w.clicked(&mut ui, log_in);
        let mut asked = Vec::new();
        for said in ["https://history.example.org", "someone", "s3nh4", "one two"] {
            let line = ui
                .asking
                .as_ref()
                .expect("the next question is not asked here");
            asked.push((line.id, line.secret));
            answer(&mut w, &mut ui, said);
            assert!(ui.orders().is_empty(), "the login went to the footer");
            assert!(!ui.message.clone().unwrap_or_default().contains("s3nh4"));
        }
        assert_eq!(
            asked,
            [
                (Question::SERVER.id, false),
                (Question::ACCOUNT.id, false),
                (Question::PASSWORD.id, true),
                (Question::KEY.id, true),
            ]
        );
        assert!(ui.asking.is_none());
        assert!(ordered(&ui), "the login was not ordered: {:?}", ui.message);
        assert_eq!(w.view.question(), None);
    }

    /// A VALUE THAT IS TYPED is asked and answered here too; nothing typed
    /// orders nothing.
    #[test]
    fn a_typed_setting_is_asked_and_answered_in_the_window() {
        let (mut w, mut ui) = a_window(true);
        let label = ui.texts.t(ronoctua_i18n::key::ATUIN_OPT_HISTORY_FILTER);
        let filter = row_carrying(&w, &ui, &label);
        w.clicked(&mut ui, filter);
        assert!(ui.asking.is_none(), "the first click acted");
        w.clicked(&mut ui, filter);
        assert_eq!(
            ui.asking.as_ref().map(|l| (l.id, l.secret)),
            Some((Question::SETTING.id, false))
        );
        answer(&mut w, &mut ui, "   ");
        assert!(ui.message.is_none(), "{:?}", ui.message);
        w.pressed(&mut ui, Press::Enter);
        answer(&mut w, &mut ui, "^secret");
        assert!(ordered(&ui), "{:?}", ui.message);
        assert!(ui.orders().is_empty());
    }

    /// A QUESTION LET GO HERE is let go by the view, and the password with
    /// it: the key typed afterwards orders nothing.
    #[test]
    fn a_question_let_go_in_the_window_takes_the_password_with_it() {
        let (mut w, mut ui) = a_window(false);
        let log_in = row_carrying(&w, &ui, &ui.texts.t(ronoctua_i18n::key::ATUIN_LOG_IN));
        w.clicked(&mut ui, log_in);
        w.clicked(&mut ui, log_in);
        answer(&mut w, &mut ui, "https://history.example.org");
        answer(&mut w, &mut ui, "someone");
        answer(&mut w, &mut ui, "s3nh4");
        let line = ui.asking.take().expect("the key is asked");
        w.unanswered(&mut ui, line.id);
        assert_eq!(w.view.question(), None);
        w.answered(&mut ui, Question::KEY.id, "one two");
        assert!(ui.message.is_none(), "{:?}", ui.message);
    }

    /// `S-<right>` AND `S-<left>` TURN A VALUE in the window — the same event
    /// a click on the hint sends — and a bare arrow does not.
    #[test]
    fn shifted_arrows_turn_a_value_in_the_window() {
        let (mut w, mut ui) = a_window(true);
        let mark = row_carrying(&w, &ui, &ui.texts.t(ronoctua_i18n::key::ATUIN_MARK));
        w.clicked(&mut ui, mark);
        let press = |w: &mut HistoryWindow, ui: &mut Ui, notation: &str| {
            ui.message = None;
            let (event, mods) =
                crate::door::keys::event_of(ronoctua_keymap::Key::parse(notation).unwrap());
            w.typed(ui, &event, mods);
        };
        press(&mut w, &mut ui, "<right>");
        assert!(ui.message.is_none(), "a bare arrow turned the ring");
        press(&mut w, &mut ui, "S-<right>");
        assert!(ordered(&ui), "S-<right> did not turn the ring");
        press(&mut w, &mut ui, "S-<left>");
        assert!(ordered(&ui), "S-<left> did not turn the ring");
    }

    /// THE FIRST CLICK SELECTS, AND THE SECOND ACTS — invariant 19.
    #[test]
    fn the_first_click_selects_and_the_second_acts() {
        let (mut w, mut ui) = a_window(true);
        let sync = row_carrying(&w, &ui, &ui.texts.t(ronoctua_i18n::key::ATUIN_SYNC_NOW));
        w.clicked(&mut ui, sync);
        assert!(ui.message.is_none(), "the first click synced");
        assert_eq!(w.cursor(), Some(sync));
        w.clicked(&mut ui, sync);
        assert!(ordered(&ui), "the second click did not sync");
    }

    /// WHAT A FINISHED ORDER SAYS HERE is what the footer says: a sync, by
    /// name — it used to be nothing — and a refusal.
    #[test]
    fn a_finished_order_says_here_what_it_says_in_the_footer() {
        use ronoctua_atuin::{Event, Order};
        let (mut w, mut ui) = a_window(true);
        w.atuin_event(
            &mut ui,
            Event::Done {
                order: Order::Sync,
                refused: None,
            },
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(ronoctua_i18n::key::ECHO_ATUIN_SYNCED).as_str())
        );
        ui.message = None;
        w.atuin_event(
            &mut ui,
            Event::Done {
                order: Order::Mark(ronoctua_atuin::prefs::Mark::Never),
                refused: None,
            },
        );
        assert!(ui.message.is_none(), "{:?}", ui.message);
        w.atuin_event(
            &mut ui,
            Event::Done {
                order: Order::Sync,
                refused: Some("no route".to_owned()),
            },
        );
        assert!(ui.message.clone().unwrap_or_default().contains("no route"));
    }
}
