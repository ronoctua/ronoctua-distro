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
use ronoctua_belongings::status::Offer;

use crate::belongings::Press;
pub use crate::belongings_region::Reach;
use crate::belongings_region::{Open, carry_out};
use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.belongings";

/// Opens the belongings in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        // THE PLACES WATCHED AND THE LISTING COUNTED ON A THREAD, both
        // handed back through the door's wake (`Content::woken`).
        let mut window = BelongingsWindow::of(Open::new(
            Places::from_env(),
            crate::belongings::now_s,
            zone,
            Reach::Queue,
            Some(w.wake()),
            OPENING_COLUMNS,
            Some(&*w.texts),
        ));
        window.loop_handle = Some(w.handle.clone());
        window.arm_the_clock();
        Ok(window)
    })
}

/// The width the cursor is first settled at, before the door has one.
const OPENING_COLUMNS: u16 = 80;

/// The window.
pub struct BelongingsWindow {
    /// The view, what it reads, and what keeps that current — the footer's
    /// region holds the same (`belongings_region::Open`).
    open: Open,
    /// The window's loop, for its clock; `None`: a photograph's.
    loop_handle: Option<calloop::LoopHandle<'static, Door<Self>>>,
    /// The timer at the moment the session forgets the passphrase, and that
    /// moment.
    clock: Option<(i64, calloop::RegistrationToken)>,
}

impl BelongingsWindow {
    /// The window over the files under `places`, its listing counted in
    /// place — a photograph's, a test's: nothing is watched.
    #[must_use]
    pub fn over(
        places: Option<Places>,
        now: fn() -> i64,
        zone: ronoctua_files::when::Zone,
        reach: Reach,
    ) -> Self {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT);
        Self::of(Open::new(
            places,
            now,
            zone,
            reach,
            None,
            OPENING_COLUMNS,
            texts.as_ref(),
        ))
    }

    /// The window over `open`.
    const fn of(open: Open) -> Self {
        Self {
            open,
            loop_handle: None,
            clock: None,
        }
    }

    /// Arms the window's one clock at the moment the session forgets the
    /// passphrase it remembers — moved when that moment moves, taken down
    /// when nothing is remembered; when it rings the window reads again. A
    /// window has no second: the footer's region reads at its own
    /// (`BelongingsRegion::tick`).
    fn arm_the_clock(&mut self) {
        let Some(handle) = self.loop_handle.clone() else {
            return;
        };
        let wanted = self.open.kept_until();
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
                door.content.open.heard();
                door.content.settled(&mut door.ui);
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(token) => self.clock = Some((at, token)),
            Err(e) => tracing::warn!(error = %e, "the belongings' unlock clock could not be hung"),
        }
    }

    /// The cursor back on its row, the clock where the readings say, and a
    /// frame.
    fn settled(&mut self, ui: &mut Ui) {
        self.open.settle(ui.columns, &ui.texts);
        self.arm_the_clock();
        ui.ask_paint();
    }

    /// Carries out what the view asked for — the footer's region's handler.
    fn apply(&mut self, ui: &mut Ui, errand: crate::belongings::Errand) {
        carry_out(errand, &mut self.open, ui);
        self.arm_the_clock();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let errand = self.open.press(press, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::belongings::Row> {
        self.open.rows(ui.columns, &ui.texts)
    }

    /// What would have been sent — a photograph's.
    #[must_use]
    pub fn sent(&self) -> &[Op] {
        self.open.sent()
    }

    /// The offers a photograph said yes to.
    #[must_use]
    pub fn accepted(&self) -> &[Offer] {
        self.open.accepted()
    }

    /// The keys a photograph asked to pin.
    #[must_use]
    pub fn pinned(&self) -> &[String] {
        self.open.pinned()
    }
}

impl Content for BelongingsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::BELONGINGS_TITLE))
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
        let errand = self.open.answered(id, text, &ui.texts);
        self.apply(ui, errand);
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.open.view.let_go();
        ui.ask_paint();
    }

    /// A watch rang or a count came back from its thread.
    fn woken(&mut self, ui: &mut Ui) {
        if self.open.woken() {
            self.settled(ui);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.open.hint(ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.open.view.cursor())
    }

    /// A new width is new rows — a label or a note wraps differently — so
    /// the cursor is found again by what it is on.
    fn resized(&mut self, ui: &mut Ui) {
        self.open.settle(ui.columns, &ui.texts);
    }

    /// And so is another language.
    fn language_changed(&mut self, ui: &mut Ui) {
        self.open.settle(ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        if self.open.clicked(row, ui.columns, &ui.texts) {
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
        crate::keep::store_of(&places)
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
        if w.open.view.cursor() == at {
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
        w.clicked(&mut ui, w.open.view.cursor());
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
        w.open.reread();
        w.settled(&mut ui);
        let row = Target::Let("example.org".to_owned());
        stand_on(&mut w, &mut ui, &row);
        w.clicked(&mut ui, w.open.view.cursor());
        assert!(ronoctua_consent::allows_under(
            &places.config,
            "example.org"
        ));
        w.open.reread();
        w.settled(&mut ui);
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
        w.clicked(&mut ui, w.open.view.cursor());
        let line = ui.asking.take().expect("the address is asked here");
        assert!(!line.secret);
        assert_eq!(
            line.text, "git@example.org:someone/b.git",
            "a changed address starts from what it is"
        );
        w.unanswered(&mut ui, line.id);
        assert!(w.open.view.asking().is_none());
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }
}
