//! The belongings, in a window of their own — the second door onto
//! `crate::belongings` (`LEADER s v w`).
//!
//! # What is here and what is not
//!
//! The plumbing of a window is `crate::door`'s; what is here is the view,
//! the files it reads and the three watches that keep them current. It
//! DECIDES nothing — which rows exist, what a key does and which file a
//! gesture writes is `belongings.rs`, and both doors ask the same view the
//! same questions.
//!
//! # It types what the footer types
//!
//! The passphrase, the new one twice, the token, the address, a path to keep:
//! asked on this window's own line (`Ui::ask`), hidden where they are
//! secrets, answered through the same `View::answered` the footer's
//! minibuffer reaches. The order goes to the queue from here, and what the
//! queue said — or that it is not there — is said on this window's line.
//!
//! # What a photograph reaches
//!
//! Nothing past the door: a photograph that pressed `RET` on "Sync now"
//! would put this machine's belongings on its queue. Everything past the
//! door goes through one field ([`Reach`]), and a photograph's writes down
//! what would have been sent instead (`crate::belongings_photograph`).

use ronoctua_belongings::order::Op;
use ronoctua_belongings::places::Places;

use crate::belongings::{Errand, Press, Readings, View};
use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.belongings";

/// Where the orders of this window go.
#[derive(Debug)]
pub enum Reach {
    /// The transfer queue.
    Queue,
    /// Nowhere: what would have been sent is written down — a photograph's.
    Nowhere(Vec<Op>),
}

/// Opens the belongings in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        let places = Places::from_env();
        // THE THREE PLACES, watched: a sync that ended, a decision the footer
        // made, a setting another door changed — each moves this window
        // without it being reopened.
        let (sender, changed) = channel::channel::<()>();
        let watches = places.as_ref().map_or_else(Vec::new, |p| {
            crate::belongings::watch(p, move || sender.send(()).is_ok())
        });
        w.handle
            .insert_source(changed, |event, (), door: &mut Door<BelongingsWindow>| {
                if let channel::Event::Msg(()) = event {
                    door.content.reread(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the belongings' watch: {e}"))?;
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        Ok(BelongingsWindow::over(
            places,
            crate::belongings::now_s,
            zone,
            Reach::Queue,
            watches,
        ))
    })
}

/// The width the cursor is first settled at, before the door has one.
const OPENING_COLUMNS: u16 = 80;

/// The window.
pub struct BelongingsWindow {
    view: View,
    readings: Readings,
    places: Option<Places>,
    /// The clock the readings are taken at — a photograph's is fixed.
    now: fn() -> i64,
    zone: ronoctua_files::when::Zone,
    /// Where the orders go.
    pub reach: Reach,
    _watches: Vec<ronoctua_files::notice::Notice>,
}

impl BelongingsWindow {
    /// The window over the files under `places`.
    #[must_use]
    pub fn over(
        places: Option<Places>,
        now: fn() -> i64,
        zone: ronoctua_files::when::Zone,
        reach: Reach,
        watches: Vec<ronoctua_files::notice::Notice>,
    ) -> Self {
        let readings = Readings::read(places.clone(), now());
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT);
        let view = texts.as_ref().map_or_else(View::default, |t| {
            View::fresh(&readings, &zone, OPENING_COLUMNS, t)
        });
        Self {
            view,
            readings,
            places,
            now,
            zone,
            reach,
            _watches: watches,
        }
    }

    /// A file changed: read again, and put the cursor back on its row.
    fn reread(&mut self, ui: &mut Ui) {
        self.readings = Readings::read(self.places.clone(), (self.now)());
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Ask(line) => ui.ask(line),
            Errand::Say(said) => ui.say_for(said, crate::echo::Echo::Long),
            Errand::Copy(text) => {
                ui.say_for(
                    crate::chrome::copied(&text, &ui.texts),
                    crate::echo::Echo::Brief,
                );
            }
            Errand::Queue(op) => {
                let sent = match &mut self.reach {
                    Reach::Queue => crate::belongings::queue(op, self.places.as_ref()),
                    Reach::Nowhere(sent) => {
                        sent.push(op);
                        Ok(())
                    }
                };
                match sent {
                    Ok(()) => ui.say(crate::belongings::queued_said(op, &ui.texts)),
                    Err(e) => {
                        tracing::warn!(error = %e, "the transfer queue did not take the order");
                        ui.say_for(
                            ui.texts.t(ronoctua_i18n::key::TRANSFERS_OFFLINE),
                            crate::echo::Echo::Long,
                        );
                    }
                }
            }
        }
        // WHAT THIS WINDOW WROTE, read back at once: its own write is what the
        // watch will ring for too, and finding nothing new then is free.
        self.reread(ui);
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        // THE FILES AS THEY ARE NOW: a decision the other door made a moment
        // ago is the one a key here lays its own over.
        self.readings = Readings::read(self.places.clone(), (self.now)());
        let errand = self
            .view
            .press(press, &self.readings, &self.zone, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::belongings::Row> {
        self.view
            .rows(&self.readings, &self.zone, ui.columns, &ui.texts)
    }

    /// What would have been sent — a photograph's.
    #[must_use]
    pub fn sent(&self) -> &[Op] {
        match &self.reach {
            Reach::Queue => &[],
            Reach::Nowhere(sent) => sent,
        }
    }
}

impl Content for BelongingsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::BELONGINGS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`belongings::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::belongings::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    /// What was typed on this window's line: the same `View::answered` the
    /// footer's minibuffer reaches.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        ui.message = None;
        self.readings = Readings::read(self.places.clone(), (self.now)());
        let errand = self.view.answered(id, text, &self.readings, &ui.texts);
        self.apply(ui, errand);
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.view.let_go();
        ui.ask_paint();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(&self.readings, &self.zone, ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// A new width is new rows — a label or a note wraps differently — so
    /// the cursor is found again by what it is on.
    fn resized(&mut self, ui: &mut Ui) {
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
    }

    /// And so is another language.
    fn language_changed(&mut self, ui: &mut Ui) {
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        } else {
            ui.ask_paint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belongings::Target;

    fn fixed() -> i64 {
        1_700_000_000
    }

    fn a_window(name: &str) -> (BelongingsWindow, Ui, Places) {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-window-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::under(&root);
        std::fs::create_dir_all(&places.config).expect("config");
        crate::belongings::store_of(&places)
            .write(&ronoctua_belongings::prefs::Belongings::with_repository(
                "git@example.org:someone/b.git",
            ))
            .expect("prefs");
        std::fs::create_dir_all(places.keys()).expect("keys");
        std::fs::write(places.deploy_key(), "k").expect("key");
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let w = BelongingsWindow::over(
            Some(places.clone()),
            fixed,
            ronoctua_files::when::Zone::utc(),
            Reach::Nowhere(Vec::new()),
            Vec::new(),
        );
        (w, Ui::for_tests(texts), places)
    }

    fn stand_on(w: &mut BelongingsWindow, ui: &mut Ui, target: &Target) {
        let rows = w.view_rows(ui);
        let at = rows
            .iter()
            .position(|r| r.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}"));
        // THE CURSOR MAY BE BORN THERE: a click then would be the second.
        if w.view.cursor() == at {
            return;
        }
        w.clicked(ui, at);
        assert!(ui.asking.is_none(), "the first click acted");
    }

    /// FAILS IF: the passphrase is not asked on this window's own line, hidden,
    /// or its answer does not reach the queue from here — the owner's rule:
    /// the two doors do the same, typing included.
    #[test]
    fn the_passphrase_is_typed_in_the_window_and_the_order_leaves_from_it() {
        let (mut w, mut ui, places) = a_window("sync");
        stand_on(&mut w, &mut ui, &Target::Sync);
        w.clicked(&mut ui, w.view.cursor());
        let line = ui.asking.take().expect("the passphrase is not asked here");
        assert!(line.secret, "the passphrase is shown as it is typed");
        w.answered(&mut ui, line.id, "a passphrase");
        // A FIRST SYNC asks it again, on the same line.
        let again = ui.asking.take().expect("a first sync did not ask it again");
        assert!(again.secret);
        assert!(w.sent().is_empty(), "sent before it was typed twice");
        w.answered(&mut ui, again.id, "a passphrase");
        assert_eq!(w.sent(), [Op::Prepare]);
        assert!(places.passphrase().is_file());
        assert!(ui.orders().is_empty(), "the order went to the footer");
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a question let go here is answered later.
    #[test]
    fn a_question_let_go_is_let_go_by_the_view() {
        let (mut w, mut ui, places) = a_window("let-go");
        stand_on(&mut w, &mut ui, &Target::Repository);
        w.clicked(&mut ui, w.view.cursor());
        let line = ui.asking.take().expect("the address is asked here");
        assert!(!line.secret);
        assert_eq!(
            line.text, "git@example.org:someone/b.git",
            "a changed address starts from what it is"
        );
        w.unanswered(&mut ui, line.id);
        assert!(w.view.asking().is_none());
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }
}
