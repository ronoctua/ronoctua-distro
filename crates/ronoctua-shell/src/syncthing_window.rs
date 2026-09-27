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

        Ok(SyncthingWindow {
            view: crate::syncthing::View::fresh(ronoctua_syncthing::prefs::store().read()),
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
                ui.message = Some(ui.texts.t(crate::syncthing::refusal_key(why)));
                ui.ask_paint();
            }
        }
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::syncthing::Errand) {
        use crate::syncthing::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`,
            // and `chrome::copied` for the one place the doors share. It earns
            // its keep here more than anywhere: a machine id is 63 characters
            // nobody types twice.
            Errand::Copy(text) => {
                ui.message = Some(crate::chrome::copied(&text, &ui.texts));
            }
            Errand::Speak(key) => {
                ui.message = Some(ui.texts.t(key));
            }
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Order(order) => match &self.syncthing {
                Some(h) => h.send(*order),
                None => {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_NOT_RUNNING));
                }
            },
            // THE FILE IS WRITTEN HERE, and the footer watches it. Both doors
            // write it, which is why the crate's own `keep` is what decides
            // whether there is anything to overwrite — `Store::parsed`, not
            // `read`, and the reason is that a reader's default written back
            // over a file that will not parse is that file gone.
            Errand::Keep(prefs) => {
                let store = ronoctua_syncthing::prefs::store();
                if let Err(e) = ronoctua_syncthing::prefs::keep(&store, prefs) {
                    tracing::warn!(error = %e, "could not write syncthing.json");
                    ui.message = Some(
                        ui.texts
                            .t(ronoctua_i18n::key::ECHO_SYNCTHING_WILL_NOT_PARSE),
                    );
                }
            }
            Errand::Web(address) => Self::open_the_page(ui, &address),
            // ON THIS WINDOW'S OWN LINE, and the view is told WHAT it is being
            // asked, so that the answer knows what it answers.
            Errand::Ask(ask) => {
                let id = ask.command();
                self.view.asked(ask);
                ui.ask(crate::line::Line::new(id, false, None));
            }
        }
        ui.ask_paint();
    }

    /// Open Syncthing's own page in whatever opens links here.
    fn open_the_page(ui: &mut Ui, address: &str) {
        let Some(opener) = option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) else {
            tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
            ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED));
            return;
        };
        if address.is_empty() {
            ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED));
            return;
        }
        // IN A SCOPE OF ITS OWN, off the loop, by the path every door opens
        // by — the footer's `open_foreign` — and one that could not start is
        // a sentence on this window's line, as it is on the footer's.
        ui.launch(opener, &[format!("http://{address}")], None);
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::syncthing::Press) {
        ui.message = None;
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
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::SYNCTHING_TITLE))
    }

    /// The key read whole — the door's one reader, then the view's
    /// (`syncthing::press_of_key`), which the footer calls too.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::syncthing::press_of_key)
        {
            self.pressed(ui, press);
        }
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
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncthing::{Ask, Press, Spot};
    use ronoctua_syncthing::{Invitation, Snapshot};

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

    /// A service that is up, with one folder, one other machine and an
    /// offered folder waiting.
    fn offered() -> Snapshot {
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
        Snapshot {
            service: ronoctua_syncthing::Service::Up(Box::new(ronoctua_syncthing::Running {
                id: "ME".to_owned(),
                address: "127.0.0.1:8384".to_owned(),
                ..ronoctua_syncthing::Running::default()
            })),
            folders: vec![folder],
            devices: vec![device("ME", true), device("THEM", false)],
            invitations: vec![Invitation::Folder {
                folder: "shared".to_owned(),
                label: "the shared one".to_owned(),
                from: "THEM".to_owned(),
                who: String::new(),
            }],
            unit: ronoctua_syncthing::unit::Standing::Running,
            ..Snapshot::default()
        }
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
        ui.message.as_deref()
            == Some(
                ui.texts
                    .t(ronoctua_i18n::key::ECHO_SYNCTHING_NOT_RUNNING)
                    .as_str(),
            )
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
        let cases: [(Spot, Press, Ask); 3] = [
            (Spot::Device("THEM".to_owned()), Press::Add, Ask::NewDevice),
            (
                Spot::Folder("papers".to_owned()),
                Press::Add,
                Ask::NewFolder,
            ),
            (Spot::Invitation(0), Press::Enter, Ask::Invited(0)),
        ];
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

        // THE OFFERED FOLDER, taken where the person says.
        let (mut w, mut ui) = a_window(offered());
        stand_on(&mut w, &mut ui, &Spot::Invitation(0));
        w.pressed(&mut ui, Press::Enter);
        answer(&mut w, &mut ui, "/home/somebody/shared");
        assert!(ordered(&ui), "{:?}", ui.message);

        // NOTHING TYPED orders nothing.
        let (mut w, mut ui) = a_window(offered());
        stand_on(&mut w, &mut ui, &Spot::Folder("papers".to_owned()));
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
        stand_on(&mut w, &mut ui, &Spot::Invitation(0));
        w.pressed(&mut ui, Press::Enter);
        let line = ui.asking.take().expect("the path is asked here");
        w.unanswered(&mut ui, line.id);
        assert_eq!(w.view.asking(), None);
    }
}
