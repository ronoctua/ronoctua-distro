//! Syncthing, in a window of its own — the second door onto `crate::syncthing`.
//!
//! # What is here and what is not
//!
//! The plumbing of a window is `crate::door`'s; what is here is the view, the
//! daemon's thread and the one file of its own this window follows. It
//! DECIDES nothing — which rows exist and what a key does to them is
//! `syncthing.rs`, and both doors ask the same view the same questions.
//!
//! # It types what the footer types
//!
//! A machine's 63-character id, a folder's path, where an offered folder
//! goes and a text or list setting are asked on this window's own line
//! (`Ui::ask`), where `C-v` pastes — an id is nearly always pasted — and
//! answered through the same `View::answered` the footer's minibuffer
//! reaches. The order goes to this process's own Syncthing thread.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.syncthing";

/// Opens the Syncthing view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `syncthing.json`, watched like the theme: a setting changed in the
        // footer moves this window without it being reopened. Both doors write
        // this file, which is why it is read back rather than assumed.
        let (wish_sender, wish_changed) = channel::channel::<()>();
        let watch = ronoctua_syncthing::prefs::home().and_then(|dir| {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!(error = %e, "no configuration directory to watch syncthing.json in");
                return None;
            }
            ronoctua_files::notice::Notice::on_file(
                &dir,
                ronoctua_syncthing::prefs::FILE,
                ronoctua_files::notice::Pace::Coalesced,
                move || wish_sender.send(()).is_ok(),
            )
            .ok()
        });
        w.handle
            .insert_source(
                wish_changed,
                |event, (), door: &mut Door<SyncthingWindow>| {
                    if let channel::Event::Msg(()) = event {
                        let fresh = ronoctua_syncthing::prefs::store().read();
                        if fresh != door.content.view.preferences() {
                            door.content.view.took_preferences(fresh);
                            door.ui.ask_paint();
                        }
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the syncthing.json watch: {e}"))?;

        // SYNCTHING, on threads of this process — the long poll is the clock.
        let (syncthing_sender, syncthing_events) = channel::channel::<ronoctua_syncthing::Event>();
        let syncthing = Some(ronoctua_syncthing::watch(move |e| {
            let _ = syncthing_sender.send(e);
        }));
        w.handle
            .insert_source(
                syncthing_events,
                |event, (), door: &mut Door<SyncthingWindow>| {
                    if let channel::Event::Msg(e) = event {
                        door.content.syncthing_event(&mut door.ui, e);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the syncthing channel: {e}"))?;

        // THE FILE PICKER'S FOLDER, watched through the door's wake.
        let mut view = crate::syncthing::View::fresh(ronoctua_syncthing::prefs::store().read());
        view.watch_with(w.wake());
        Ok(SyncthingWindow {
            view,
            snapshot: None,
            syncthing,
            _watch: watch,
        })
    })
}

/// The window.
struct SyncthingWindow {
    view: crate::syncthing::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_syncthing::Snapshot>,
    /// The orders thread.
    syncthing: Option<ronoctua_syncthing::Handle>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl SyncthingWindow {
    fn syncthing_event(&mut self, ui: &mut Ui, e: ronoctua_syncthing::Event) {
        match e {
            ronoctua_syncthing::Event::Snapshot(s) => {
                self.snapshot = Some(*s);
                self.view
                    .settle_in(self.snapshot.as_ref(), ui.columns, &ui.texts);
                ui.ask_paint();
            }
            ronoctua_syncthing::Event::Done { refused, .. } => {
                let Some(why) = refused else { return };
                ui.say(ui.texts.t(crate::syncthing_words::refusal_key(why)));
                ui.ask_paint();
            }
        }
    }

    /// Carries out what the view asked for — the one handler the footer's
    /// region carries out too (`syncthing_region::carry_out`), with this
    /// window's own thread. The file it writes the footer watches.
    fn apply(&mut self, ui: &mut Ui, errand: crate::syncthing::Errand) {
        crate::syncthing_region::carry_out(
            errand,
            &mut self.view,
            self.syncthing.as_ref(),
            None,
            ui,
        );
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::syncthing::Press) {
        ui.message = None;
        self.view.page_is(ui.room_for_rows());
        let errand = self
            .view
            .key(press, self.snapshot.as_ref(), ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::syncthing::Row> {
        self.view
            .rows(self.snapshot.as_ref(), ui.columns, &ui.texts)
    }
}

impl Content for SyncthingWindow {
    /// A thread of the window's woke the loop — the file picker's folder
    /// moved, or its devices were read: its rows again, when they changed.
    fn woken(&mut self, ui: &mut Ui) {
        if self.view.look() {
            ui.ask_paint();
        }
    }
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::SYNCTHING_TITLE))
    }

    /// The key read whole — the door's one reader, then the view's
    /// (`syncthing::press_of_key`), which the footer calls too.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        // THE FILE PICKER, while it is up over the view, takes what it reads
        // — every letter, and its own keys.
        if self.view.picker_takes(key) {
            ui.message = None;
            self.view.page_is(ui.room_for_rows());
            let errand = self
                .view
                .pick_key(key, self.snapshot.as_ref(), ui.columns, &ui.texts);
            self.apply(ui, errand);
            return;
        }
        if let Some(press) = crate::syncthing::press_of_key(key) {
            self.pressed(ui, press);
        }
    }

    /// The clipboard's text, read off the loop for the file picker's `C-v`.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.view.picker_pasted(text);
        self.view
            .settle_in(self.snapshot.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// What was typed on this window's line: the same `View::answered` the
    /// footer's minibuffer reaches, and the order it gives goes to this
    /// window's own thread.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        ui.message = None;
        if let Some(order) = self.view.answered(id, text, self.snapshot.as_ref()) {
            self.apply(ui, crate::syncthing::Errand::Order(Box::new(order)));
        }
        ui.ask_paint();
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.view.let_go();
        ui.ask_paint();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(self.snapshot.as_ref(), ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row the cursor stands on, and only that row lights
    /// under the pointer — the view's one answer
    /// ([`crate::syncthing::Row::takes`]), which the footer's region gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::syncthing::Row::takes)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click
    /// puts the cursor on the row, and a click on the row it is already on
    /// is that row's `RET`, through the same `pressed` a key goes through.
    /// The door follows the cursor afterwards against the list as it is
    /// THEN, which matters here: clicking a folder or a drawer changes which
    /// level is showing.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        let again = self
            .view
            .clicked(row, &rows, self.snapshot.as_ref(), ui.columns, &ui.texts);
        if again {
            self.pressed(ui, crate::syncthing::Press::Enter);
        }
    }
}

/// A service that is up, with one folder, one other machine and an
/// offered folder waiting — for both doors' tests (the region's too).
#[cfg(test)]
pub(crate) fn offered() -> ronoctua_syncthing::Snapshot {
    let folder = ronoctua_syncthing::Folder {
        id: "papers".to_owned(),
        kind: "sendreceive".to_owned(),
        completion: 100,
        raw: serde_json::json!({
            "id": "papers",
            "label": "",
            "path": "/somewhere",
            "type": "sendreceive",
            "devices": [],
        }),
        ..ronoctua_syncthing::Folder::default()
    };
    let device = |id: &str, me: bool| ronoctua_syncthing::Device {
        id: id.to_owned(),
        name: id.to_lowercase(),
        me,
        raw: serde_json::json!({ "deviceID": id, "name": id }),
        ..ronoctua_syncthing::Device::default()
    };
    ronoctua_syncthing::Snapshot {
        service: ronoctua_syncthing::Service::Up(Box::new(ronoctua_syncthing::Running {
            id: "ME".to_owned(),
            address: "127.0.0.1:8384".to_owned(),
            ..ronoctua_syncthing::Running::default()
        })),
        folders: vec![folder],
        devices: vec![device("ME", true), device("THEM", false)],
        invitations: vec![ronoctua_syncthing::Invitation::Folder {
            folder: "shared".to_owned(),
            label: "the shared one".to_owned(),
            from: "THEM".to_owned(),
            who: String::new(),
        }],
        unit: ronoctua_syncthing::unit::Standing::Running,
        ..ronoctua_syncthing::Snapshot::default()
    }
}

/// [`offered`], with the service complaining twice — the second complaint
/// long enough to wrap: a complaint is prose whose first line carries a spot,
/// and no line of it is a row the cursor stands on.
#[cfg(test)]
pub(crate) fn complaining() -> ronoctua_syncthing::Snapshot {
    let mut s = offered();
    if let ronoctua_syncthing::Service::Up(running) = &mut s.service {
        running.errors = vec![
            "folder papers: the folder path is missing".to_owned(),
            "listening on tcp://0.0.0.0:22000 failed: the address is already in use by \
             another program, and the relay pool could not be reached either"
                .to_owned(),
        ];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncthing::{Ask, Press, Spot};
    use ronoctua_syncthing::Snapshot;

    /// A window with no Syncthing thread behind it: every order it gives is
    /// answered with "Syncthing is not running" on its own line — which is
    /// what a test of the wiring can see — and nothing reaches the footer.
    fn a_window(snap: Snapshot) -> (SyncthingWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = SyncthingWindow {
            view: crate::syncthing::View::fresh(ronoctua_syncthing::prefs::Preferences::default()),
            snapshot: Some(snap),
            syncthing: None,
            _watch: None,
        };
        (me, Ui::for_tests(texts))
    }

    /// Puts the cursor on the row that is `spot`, by a click.
    fn stand_on(w: &mut SyncthingWindow, ui: &mut Ui, spot: &Spot) {
        let rows = w.view_rows(ui);
        let at = rows
            .iter()
            .position(|r| r.spot == *spot)
            .unwrap_or_else(|| panic!("no row is {spot:?}"));
        w.clicked(ui, at);
        assert!(ui.asking.is_none(), "the first click acted");
    }

    /// The sentence an order gets here with no Syncthing thread: the order
    /// left for this window's own thread.
    fn ordered(ui: &Ui) -> bool {
        ui.message.as_deref() == Some(ui.texts.t(crate::key::ECHO_SYNCTHING_NOT_RUNNING).as_str())
    }

    /// Answers the line that is open, as the door does: the line leaves
    /// `ui.asking` first, and the window hears the text under its id.
    fn answer(w: &mut SyncthingWindow, ui: &mut Ui, text: &str) {
        let line = ui.asking.take().expect("no line is open");
        w.answered(ui, line.id, text);
    }

    /// EVERY QUESTION OF THE VIEW IS ASKED ON THIS WINDOW'S OWN LINE — a
    /// machine's id, a folder's path, where an offered folder goes, a text
    /// setting — and none is sent to the footer or answered with a sentence.
    #[test]
    fn every_question_is_asked_on_the_windows_own_line() {
        let cases: [(Spot, Press, Ask); 1] =
            [(Spot::Device("THEM".to_owned()), Press::Add, Ask::NewDevice)];
        for (spot, press, ask) in cases {
            let (mut w, mut ui) = a_window(offered());
            stand_on(&mut w, &mut ui, &spot);
            w.pressed(&mut ui, press);
            let line = ui.asking.as_ref().expect("the question is not asked here");
            assert_eq!(line.id, ask.command(), "{spot:?}");
            assert!(!line.secret);
            assert_eq!(w.view.asking(), Some(&ask));
            assert!(ui.message.is_none(), "{:?}", ui.message);
            assert!(ui.orders().is_empty(), "the question went to the footer");
        }

        // A TEXT SETTING, on a folder's sheet: `RET` and `S-<right>` both.
        for press in [Press::Enter, Press::More] {
            let (mut w, mut ui) = a_window(offered());
            stand_on(&mut w, &mut ui, &Spot::Folder("papers".to_owned()));
            w.pressed(&mut ui, Press::Enter);
            stand_on(&mut w, &mut ui, &Spot::Setting("folder-path"));
            w.pressed(&mut ui, press);
            let line = ui.asking.as_ref().expect("the setting is not asked here");
            assert_eq!(line.id, "syncthing-enter");
            assert_eq!(
                w.view.asking(),
                Some(&Ask::Setting("folder-path", Some("papers".to_owned())))
            );
            assert!(ui.orders().is_empty());
        }
    }

    /// A FOLDER ON THIS DISK IS CHOSEN IN THE PICKER, here — a new one to
    /// share, and where an offered one goes — never typed on a line, and the
    /// order goes from here.
    #[test]
    fn a_folder_is_chosen_in_the_picker_here() {
        use crate::syncthing::Target;
        for (spot, press) in [
            (Spot::Folder("papers".to_owned()), Press::Add),
            (Spot::Invitation(0), Press::Enter),
        ] {
            let (mut w, mut ui) = a_window(offered());
            stand_on(&mut w, &mut ui, &spot);
            w.pressed(&mut ui, press);
            assert!(ui.asking.is_none(), "{spot:?}: a line was opened");
            let rows = w.view_rows(&ui);
            let here = rows
                .iter()
                .position(|r| r.target == Target::Pick(crate::file_picker::Target::Here))
                .unwrap_or_else(|| panic!("{spot:?}: no picker"));
            w.clicked(&mut ui, here);
            w.clicked(&mut ui, here);
            assert!(ordered(&ui), "{spot:?}: {:?}", ui.message);
            assert!(ui.orders().is_empty(), "the order went to the footer");
        }
    }

    /// WHAT IS TYPED HERE IS ORDERED FROM HERE — a pasted machine id
    /// included, which is how an id arrives — and nothing typed is never
    /// mind.
    #[test]
    fn what_is_typed_in_the_window_is_ordered_from_the_window() {
        let (mut w, mut ui) = a_window(offered());
        stand_on(&mut w, &mut ui, &Spot::Device("THEM".to_owned()));
        w.pressed(&mut ui, Press::Add);
        // A COPIED ID, with its trailing line break: the line takes it as
        // one line.
        let line = ui.asking.as_mut().expect("the id is asked here");
        let _ = line.paste("ABCDEFG-HIJKLMN-OPQRSTU\n");
        assert_eq!(line.text, "ABCDEFG-HIJKLMN-OPQRSTU");
        let text = line.text.clone();
        answer(&mut w, &mut ui, &text);
        assert!(ordered(&ui), "{:?}", ui.message);
        assert!(ui.orders().is_empty(), "the order went to the footer");
        assert_eq!(w.view.asking(), None);

        // NOTHING TYPED orders nothing.
        let (mut w, mut ui) = a_window(offered());
        stand_on(&mut w, &mut ui, &Spot::Device("THEM".to_owned()));
        w.pressed(&mut ui, Press::Add);
        answer(&mut w, &mut ui, "  ");
        assert!(ui.message.is_none(), "{:?}", ui.message);
    }

    /// A QUESTION LET GO HERE is let go by the view too, as the footer's
    /// `Esc` lets it go there — `view::a_question_let_go_is_not_answered_later`
    /// holds what an answer arriving afterwards then means.
    #[test]
    fn a_question_let_go_in_the_window_is_let_go_by_the_view() {
        let (mut w, mut ui) = a_window(offered());
        stand_on(&mut w, &mut ui, &Spot::Device("THEM".to_owned()));
        w.pressed(&mut ui, Press::Add);
        let line = ui.asking.take().expect("the path is asked here");
        w.unanswered(&mut ui, line.id);
        assert_eq!(w.view.asking(), None);
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, the air between sections, any line of the
    /// service's complaint — or leaves unlit one it takes. THE LIGHT IS THE
    /// CLICK: every row clicked on a window of its own, and lit exactly where
    /// something happened. The footer's region has its own.
    ///
    /// ONE CLICK, AND NONE ON THE CURSOR'S ROW — as the door's
    /// `held_to_the_click` clicks: a second click would be
    /// the row's `RET`, and `RET` on the mark's row writes the person's own
    /// `syncthing.json` (`carry_out`'s `Keep`, through the real store) under
    /// `cargo test`. One click is enough for the light: from any other row it
    /// selects, and the cursor moving is seen; on the cursor's row it is
    /// already the `RET`, which is why that row is not clicked.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let (first, ui) = a_window(complaining());
        let lit = first.takes(&ui);
        let cursor = first.cursor();
        let held: Vec<crate::region::Held> = (0..first.rows(&ui).len())
            .map(|row| {
                let here = cursor == Some(row);
                let (mut w, mut ui) = a_window(complaining());
                let before = crate::door::seen(&w, &ui);
                if !here {
                    w.clicked(&mut ui, row);
                }
                crate::region::Held {
                    row,
                    lit: lit.get(row).copied().unwrap_or(false),
                    did: crate::door::seen(&w, &ui) != before,
                    here,
                }
            })
            .collect();
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
