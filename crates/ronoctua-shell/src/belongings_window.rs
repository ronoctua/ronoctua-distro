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

use std::sync::Arc;

use ronoctua_belongings::order::Op;
use ronoctua_belongings::places::Places;
use ronoctua_belongings::status::Offer;

use crate::belongings::{Counted, Counting, Ears, Errand, Press, Readings, View};
use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::{self, channel},
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.belongings";

/// Where the orders of this window go.
#[derive(Debug)]
pub enum Reach {
    /// The transfer queue — and a "yes" to an offer, turned on here.
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
        // THE SAME CHANNEL hears the listing's folders, raised after each
        // count (`belongings::Ears`).
        let ring = sender.clone();
        let watches = places.as_ref().map_or_else(Vec::new, |p| {
            crate::belongings::watch(p, move || sender.send(()).is_ok())
        });
        w.handle
            .insert_source(changed, |event, (), door: &mut Door<BelongingsWindow>| {
                if let channel::Event::Msg(()) = event {
                    door.content.heard(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the belongings' watch: {e}"))?;
        // THE LISTING, counted in a thread of its own and handed back here.
        let (counted, count) = channel::channel::<Arc<Counted>>();
        w.handle
            .insert_source(count, |event, (), door: &mut Door<BelongingsWindow>| {
                if let channel::Event::Msg(counted) = event {
                    door.content.counted(&mut door.ui, counted);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the belongings' count: {e}"))?;
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let mut window = BelongingsWindow::counting(
            places,
            crate::belongings::now_s,
            zone,
            Reach::Queue,
            watches,
            Counting::through(counted),
        );
        window.ring = Some(ring);
        window.loop_handle = Some(w.handle.clone());
        window.arm_the_clock();
        Ok(window)
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
    /// The count of what goes to the repository.
    counting: Counting,
    /// The offers a photograph said yes to, instead of turning them on.
    accepted: Vec<Offer>,
    /// The keys a photograph asked to pin (their fingerprints), instead of
    /// asking the sudo door.
    pinned: Vec<String>,
    _watches: Vec<ronoctua_files::notice::Notice>,
    /// What the listing's files are heard by.
    ears: Ears,
    /// The watches' channel, for the ears raised after a count; `None`: a
    /// photograph's, which hears nothing.
    ring: Option<channel::Sender<()>>,
    /// The window's loop, for its clock; `None`: a photograph's.
    loop_handle: Option<calloop::LoopHandle<'static, Door<Self>>>,
    /// The timer at the moment the session forgets the passphrase, and that
    /// moment.
    clock: Option<(i64, calloop::RegistrationToken)>,
}

impl BelongingsWindow {
    /// The window over the files under `places`, its listing counted in
    /// place (a photograph's, a test's).
    #[must_use]
    pub fn over(
        places: Option<Places>,
        now: fn() -> i64,
        zone: ronoctua_files::when::Zone,
        reach: Reach,
        watches: Vec<ronoctua_files::notice::Notice>,
    ) -> Self {
        Self::counting(places, now, zone, reach, watches, Counting::in_place())
    }

    /// The window over the files under `places`, its listing counted as
    /// `counting` says.
    #[must_use]
    pub fn counting(
        places: Option<Places>,
        now: fn() -> i64,
        zone: ronoctua_files::when::Zone,
        reach: Reach,
        watches: Vec<ronoctua_files::notice::Notice>,
        mut counting: Counting,
    ) -> Self {
        let mut readings = Readings::read(places.clone(), now());
        counting.ask(&mut readings);
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
            counting,
            accepted: Vec::new(),
            pinned: Vec::new(),
            _watches: watches,
            ears: Ears::default(),
            ring: None,
            loop_handle: None,
            clock: None,
        }
    }

    /// Hears the folders of the count that is in.
    fn listen(&mut self) {
        if let Some(ring) = self.ring.clone() {
            self.ears
                .follow(&self.readings, move || ring.send(()).is_ok());
        }
    }

    /// Arms the window's one clock at the moment the session forgets the
    /// passphrase it remembers — moved when that moment moves, taken down
    /// when nothing is remembered; when it rings the window reads again. The
    /// footer's twin (`Footer::belongings_arm_the_clock`).
    fn arm_the_clock(&mut self) {
        let Some(handle) = self.loop_handle.clone() else {
            return;
        };
        let wanted = self.readings.kept_until;
        if self.clock.as_ref().map(|(at, _)| *at) == wanted {
            return;
        }
        if let Some((_, token)) = self.clock.take() {
            handle.remove(token);
        }
        let Some(at) = wanted else {
            return;
        };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(crate::belongings::until(at)),
            |_, (), door: &mut Door<Self>| {
                door.content.clock = None;
                door.content.heard(&mut door.ui);
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(token) => self.clock = Some((at, token)),
            Err(e) => tracing::warn!(error = %e, "the belongings' unlock clock could not be hung"),
        }
    }

    /// What this window wrote: read again, and put the cursor back on its
    /// row — the listing kept until a watch rings for a count.
    fn reread(&mut self, ui: &mut Ui) {
        self.readings = Readings::read(self.places.clone(), (self.now)()).keeping(&self.readings);
        self.counting.reread(&mut self.readings);
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
        self.arm_the_clock();
        ui.ask_paint();
    }

    /// A watch rang (or the clock): read again, and count again.
    fn heard(&mut self, ui: &mut Ui) {
        self.readings = Readings::read(self.places.clone(), (self.now)()).keeping(&self.readings);
        self.counting.ask(&mut self.readings);
        self.listen();
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
        self.arm_the_clock();
        ui.ask_paint();
    }

    /// A count came back from its thread.
    fn counted(&mut self, ui: &mut Ui, counted: Arc<Counted>) {
        self.counting.came(&mut self.readings, counted);
        self.listen();
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
            Errand::Accept(at_s, offer) => match (&self.reach, self.places.clone()) {
                // THE KEYS' YES is a root gesture: asked in the footer's sudo
                // bar, as this window's.
                (Reach::Queue, Some(places)) => {
                    crate::belongings::accept_in_background(
                        places,
                        at_s,
                        offer,
                        crate::sudo_bar::Gate::window(),
                    );
                }
                _ => self.accepted.push(offer),
            },
            // THE PIN is a root gesture: asked in the footer's sudo bar, as
            // this window's.
            Errand::Pin { key, fingerprint } => match (&self.reach, self.places.clone()) {
                (Reach::Queue, Some(places)) => {
                    crate::belongings::pin_in_background(
                        places,
                        key,
                        fingerprint,
                        crate::sudo_bar::Gate::window(),
                    );
                }
                // A PHOTOGRAPH asks no door: it writes the pin down.
                _ => self.pinned.push(fingerprint),
            },
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
        self.readings = Readings::read(self.places.clone(), (self.now)()).keeping(&self.readings);
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

    /// The offers a photograph said yes to.
    #[must_use]
    pub fn accepted(&self) -> &[Offer] {
        &self.accepted
    }

    /// The keys a photograph asked to pin.
    #[must_use]
    pub fn pinned(&self) -> &[String] {
        &self.pinned
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
        self.readings = Readings::read(self.places.clone(), (self.now)()).keeping(&self.readings);
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
        // THE HOST LET, as typing the address does.
        ronoctua_consent::let_hosts_under(&places.config, &["example.org"], true).expect("consent");
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

    /// FAILS IF: the window draws no row asking a host nobody let, or a
    /// click on it does not let the host here — the two doors do the same.
    #[test]
    fn the_window_asks_a_host_nobody_let_on_its_row() {
        let (mut w, mut ui, places) = a_window("consent");
        std::fs::remove_file(places.config.join(ronoctua_consent::FILE)).expect("consent");
        w.reread(&mut ui);
        let row = Target::Let("example.org".to_owned());
        stand_on(&mut w, &mut ui, &row);
        w.clicked(&mut ui, w.view.cursor());
        assert!(ronoctua_consent::allows_under(
            &places.config,
            "example.org"
        ));
        w.reread(&mut ui);
        assert!(
            !w.view_rows(&ui)
                .iter()
                .any(|r| r.target.as_ref() == Some(&row)),
            "the row stays once the host is let"
        );
        assert!(w.sent().is_empty(), "letting a host sent an order");
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
