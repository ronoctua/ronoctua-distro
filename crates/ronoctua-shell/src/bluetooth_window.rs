//! The bluetooth view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14), and one device carries thirty facts and a row per service:
//! a glance at what is connected fits the footer, reading a device whole does
//! not. So this exists — and everything it knows lives in
//! [`crate::bluetooth::View`], which the footer draws from too. What this
//! module holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the arrangement `power_window.rs` and `files_window.rs`
//! share, and the same header.
//!
//! # Where its data comes from
//!
//! From BlueZ, on threads of its own (`ronoctua_bluetooth::watch`): a window
//! has no clock, and the bus says when something changed. Orders go to the
//! daemon directly — connecting, pairing, forgetting, renaming are the same
//! call from any process.
//!
//! # The pairing question is answered here too
//!
//! The AGENT is the footer's: bluetoothd asks the default agent, and only one
//! door may register it — the footer's own connection, because it is the
//! process that outlives this window. So a question arrives in the footer's
//! process, and this window reads it from the file the footer writes while it
//! stands (`bluetooth::standing`), watched by inotify. It is drawn and
//! answered here exactly as there: `y`, `n`, `Esc`, `RET` on a row, and a PIN
//! or a passkey typed on this window's own line, masked. The answer goes back
//! to the agent down the control socket (`Ui::tell_the_footer`) — never in an
//! argv — and a footer that is not there to take it is a sentence on the
//! header, never silence.

use crate::door::{Canvas, Content, Door, Hand, Ui};
use crate::line::Line;
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.bluetooth";

/// Whether the person is at THIS window, by what the compositor says has the
/// focus — asked by the footer when a pairing question arrives.
///
/// The window then answers it, and the footer neither opens its region nor
/// takes the keyboard: a question the window started with `p` is typed into
/// the window, and a footer that grabbed the keyboard for it would be the
/// one door sending the person to the other.
#[must_use]
pub fn has_the_person(focused_app_id: Option<&str>) -> bool {
    focused_app_id == Some(APP_ID)
}

/// Opens the bluetooth view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // THE DAEMON, on threads of this process — the bus is the clock.
        let (bt_sender, bt_events) = channel::channel::<ronoctua_bluetooth::Event>();
        // FALSE: THE FOOTER OWNS THE PAIRING AGENT. Only one door may register
        // one — `RequestDefaultAgent` gives the questions to the last caller,
        // so a window that registered would take them away from the footer
        // and hand them back when it closed. See `ronoctua_bluetooth::watch`.
        let radio = match ronoctua_bluetooth::watch(
            move |e| {
                let _ = bt_sender.send(e);
            },
            false,
        ) {
            Ok(w) => Some(w.hand),
            Err(e) => {
                tracing::warn!(error = %e, "no bluetooth; the window opens with nothing to read");
                None
            }
        };
        w.handle
            .insert_source(bt_events, |event, (), door: &mut Door<BluetoothWindow>| {
                if let channel::Event::Msg(e) = event {
                    door.content.bluetooth_event(&mut door.ui, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the bluetooth channel: {e}"))?;

        // THE QUESTION THE FOOTER'S AGENT WAS ASKED, watched — see
        // `bluetooth::standing`.
        let (asked_sender, asked_changed) = channel::channel::<()>();
        let question_watch =
            crate::bluetooth::standing::watch(move || asked_sender.send(()).is_ok());
        w.handle
            .insert_source(
                asked_changed,
                |event, (), door: &mut Door<BluetoothWindow>| {
                    if let channel::Event::Msg(()) = event {
                        let now = crate::bluetooth::standing::read();
                        door.content.stand(&mut door.ui, now);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the question's channel: {e}"))?;

        let mut window = BluetoothWindow::of(radio);
        window._question_watch = question_watch;
        // AND ONE ALREADY STANDING as the window opens is asked at once.
        if let Some(q) = crate::bluetooth::standing::read() {
            window.view.asked(q.clone());
            window.standing = Some(q);
        }
        Ok(window)
    })
}

/// The window: the view, and what it reads.
struct BluetoothWindow {
    view: crate::bluetooth::View,
    /// The last reading, from this process's own threads.
    scene: ronoctua_bluetooth::Scene,
    /// The orders thread, when bluetoothd was there.
    radio: Option<ronoctua_bluetooth::Hand>,
    /// WHERE ALONG THE LINE the last press landed, for the same reason the
    /// footer keeps it: a device's page has two halves side by side, and a
    /// click on the right one has to put the keyboard there.
    ///
    /// Written by `pointer` just before the standard list handles the frame,
    /// and read by `clicked` inside it — which is handed only the row.
    clicked_cell: Option<u16>,
    /// The pairing question the footer's agent holds, as the file last said
    /// it — kept to tell a NEW question from the same file read again, so a
    /// question already answered or dismissed here is not raised twice.
    standing: Option<ronoctua_bluetooth::agent::Question>,
    /// The row under the pointer that spoke last — whose sentence line 0
    /// says (`bluetooth::Row::description`).
    pointed: Option<usize>,
    /// The watch on the question's file, kept for as long as the window.
    _question_watch: Option<ronoctua_files::notice::Notice>,
}

impl BluetoothWindow {
    /// A window on the list, with the radio's orders thread when there is one.
    fn of(radio: Option<ronoctua_bluetooth::Hand>) -> Self {
        Self {
            view: crate::bluetooth::View::fresh(),
            scene: ronoctua_bluetooth::Scene::default(),
            radio,
            clicked_cell: None,
            standing: None,
            pointed: None,
            _question_watch: None,
        }
    }

    fn bluetooth_event(&mut self, ui: &mut Ui, e: ronoctua_bluetooth::Event) {
        use ronoctua_bluetooth::Event;
        match e {
            Event::Scene(scene) => {
                // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: a
                // device switched off takes its row away, and a scan brings
                // four in a second.
                self.scene = *scene;
                let rows = self.view_rows(ui);
                self.view.settle(&rows);
                ui.ask_paint();
            }
            Event::Done { ask, refused } => {
                if let Some((why, words)) = refused {
                    ui.message = Some(crate::bluetooth_words::refused(
                        &ask, why, &words, &ui.texts,
                    ));
                    ui.ask_paint();
                }
            }
            // THESE TWO NEVER ARRIVE HERE — the agent is not this window's
            // (`serve_agent: false`, above) — and they are answered anyway,
            // the way the file answers: a `_ => {}` would go quietly wrong the
            // day this window did register one.
            Event::Asked(question) => self.stand(ui, Some(question)),
            Event::Unasked => self.stand(ui, None),
        }
    }

    /// The question the footer's agent holds now — `None` when it is over.
    ///
    /// A NEW question takes the screen, as in the footer. One that ended
    /// leaves it, and takes with it the digits being typed for it: a PIN for
    /// a question that is over — or replaced — is a PIN for nothing.
    fn stand(&mut self, ui: &mut Ui, now: Option<ronoctua_bluetooth::agent::Question>) {
        if now == self.standing {
            return;
        }
        self.standing.clone_from(&now);
        if ui
            .asking
            .as_ref()
            .is_some_and(|l| l.id == crate::bluetooth::ACCEPT)
        {
            ui.asking = None;
        }
        match now {
            Some(q) => self.view.asked(q),
            None => self.view.answered(),
        }
        let rows = self.view_rows(ui);
        self.view.settle(&rows);
        ui.ask_paint();
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::bluetooth::Errand) {
        use crate::bluetooth::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — see
            // `chrome::information_of`, and `chrome::copied` for the
            // one place the six doors share.
            Errand::Copy(text) => {
                ui.message = Some(crate::chrome::copied(&text, &ui.texts));
            }
            Errand::Close => {
                ui.quit = true;
                return;
            }
            // THE PROFILE IS PIPEWIRE'S, and this door may do it itself:
            // `wpctl` is a process, not a connection somebody else owns, so
            // there is nothing here for the footer to hold.
            Errand::Audio(card, profile) => {
                if let Err(e) = ronoctua_bluetooth::audio::choose(card, profile) {
                    tracing::warn!(error = %e, "the audio profile did not change");
                }
                let scene = std::mem::take(&mut self.scene);
                self.view.look_at_the_device(&scene);
                self.scene = scene;
            }
            // THE VOLUME AND THE MUTE DO NOT RE-READ THE GRAPH.
            //
            // The profile above does, because changing it recreates the node
            // and the view is holding a stale id. A volume changes a number on
            // an object that stays; `pw-dump` is a process and a parse, and
            // running one per arrow press would put a fork under the key
            // somebody is holding down. The view has already moved its own
            // number, which is what the row draws.
            // A COMMAND OF THE REGISTRY is run where commands are run — the
            // footer, told off the loop (`Ui::tell_the_footer`), and a footer
            // that is not there is a sentence here.
            Errand::Run(id, about) => {
                ui.tell_the_footer(id, (!about.is_empty()).then_some(about.as_str()));
            }
            Errand::Volume(node, percent) => {
                if let Err(e) = ronoctua_bluetooth::audio::set_volume(node, percent) {
                    tracing::warn!(error = %e, "the volume did not change");
                }
            }
            Errand::Mute(node, on) => {
                if let Err(e) = ronoctua_bluetooth::audio::set_mute(node, on) {
                    tracing::warn!(error = %e, "the mute did not change");
                }
            }
            // THE ANSWER GOES TO THE AGENT, which is the footer's: bluetoothd
            // is asking the footer's connection. Down the control socket,
            // where `LEADER s b y` and the card's own buttons send it too —
            // NAMING THE QUESTION this window showed, so that one replaced on
            // the way is not answered in its place (`bluetooth::Answer`).
            Errand::Answer(yes) => {
                let id = if yes {
                    crate::bluetooth::ACCEPT
                } else {
                    crate::bluetooth::REFUSE
                };
                match &self.standing {
                    Some(q) => {
                        ui.tell_the_footer(id, Some(&crate::bluetooth::Answer::named(q, None)));
                    }
                    None => tracing::info!("an answer with no question standing; not sent"),
                }
            }
            // THE DIGITS ARE TYPED HERE, on this window's own line, masked —
            // the footer's minibuffer asks the same line (`bluetooth::ACCEPT`).
            Errand::Type => ui.ask(Line::new(crate::bluetooth::ACCEPT, true, None)),
            Errand::Order(ask) => match &self.radio {
                Some(h) => h.send(ask),
                None => {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::BLUETOOTH_NO_DAEMON));
                }
            },
            // A NAME IS TYPED HERE, starting from the one it has — the same
            // line the footer's minibuffer opens, and the same answer.
            Errand::Rename(_) => {
                let now = self.view.name_being_changed(&self.scene);
                ui.ask(Line::new(crate::bluetooth::RENAME, false, now.as_deref()));
            }
        }
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a click.
    fn pressed(&mut self, ui: &mut Ui, press: crate::bluetooth::Press) {
        let scene = std::mem::take(&mut self.scene);
        let errand = self.view.key(press, &scene, ui.columns, &ui.texts);
        self.scene = scene;
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::bluetooth::Row> {
        self.view.rows(&self.scene, ui.columns, &ui.texts)
    }

    /// What the row under the pointer has to add: its whole sentence, which
    /// the footer puts on its own message line from the same field.
    fn about_the_pointer(&self, ui: &Ui) -> Option<String> {
        let row = self.pointed?;
        self.view_rows(ui).into_iter().nth(row)?.description
    }
}

/// Which cell of the line a pointer at `x` is over.
///
/// The twin of `Ui::row_at`, in the other axis, and it answers in the SAME
/// units the view's extents are written in — the content margin is taken off
/// first, because that is what the columns are counted from.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a pointer inside a window, divided by a cell"
)]
fn cell_at(ui: &Ui, x: f64) -> Option<u16> {
    let cell = f64::from(tokens::metrics::cell_width(tokens::text::size()));
    if cell <= 0.0 {
        return None;
    }
    let inset = cell * f64::from(ui.margin);
    Some(((x - inset).max(0.0) / cell) as u16)
}

impl Content for BluetoothWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::BLUETOOTH_TITLE))
    }

    /// THE KEY IS READ WHOLE — the door's one reader (`keys::key_of`), and
    /// then the view's (`bluetooth_words::press_of_key`), which the footer
    /// calls too: the bare sideways arrows cross the columns and `S-<arrow>`
    /// is more and less, and a chord no row names does nothing in either door.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::bluetooth_words::press_of_key)
        else {
            return;
        };
        ui.message = None;
        self.pressed(ui, press);
    }

    /// What this window's line was asked for: the PIN or passkey, and a new
    /// name — the same answers the footer's minibuffer hands its commands.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        match id {
            crate::bluetooth::ACCEPT => {
                // NOTHING TYPED IS ASKED AGAIN, as the footer asks again: a
                // yes with no digits is an empty PIN for the other side.
                if text.trim().is_empty() {
                    ui.ask(Line::new(crate::bluetooth::ACCEPT, true, None));
                    return;
                }
                // THE DIGITS GO TO THE AGENT, and only there: down the
                // control socket to the process that registered it — no
                // argv, no log, no copy kept here — with the question they
                // were typed for, which the footer and the agent both hold
                // them to.
                let Some(q) = self.standing.as_ref() else {
                    tracing::info!("digits for a question that is over; not sent");
                    return;
                };
                let named = crate::bluetooth::Answer::named(q, Some(text));
                self.view.answered();
                ui.tell_the_footer(crate::bluetooth::ACCEPT, Some(&named));
                ui.ask_paint();
            }
            crate::bluetooth::RENAME => {
                let errand = self.view.renamed(text);
                self.apply(ui, errand);
            }
            _ => {}
        }
    }

    /// A QUESTION STILL UP AS THE WINDOW CLOSES goes back to the footer,
    /// which holds it: closed here, it would wait out bluetoothd's minute
    /// with nobody looking, and answering it no would be deciding for the
    /// person. The footer's view opens on it (`bluetooth-open`).
    fn closing(&mut self, ui: &mut Ui) -> bool {
        if self.view.asking() {
            ui.tell_the_footer("bluetooth-open", None);
        }
        true
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// The standard window, with one thing more on line 0: what the row
    /// under the pointer has to add, as the footer says it on its own line.
    ///
    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let about = self.about_the_pointer(ui);
        crate::door::list::header_about(ui, canvas, None, about, |ui| self.view.hint(&ui.texts));
        let rows = self.rows(ui);
        crate::door::list::body(ui, canvas, &rows);
    }

    /// The standard list, and two things more: WHERE ALONG THE LINE a press
    /// landed, which `clicked` needs and is not handed, and which row the
    /// pointer is on, whose sentence line 0 says.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        for e in events {
            if let PointerEventKind::Press { button, .. } = e.kind
                && button == crate::door::LEFT_BUTTON
            {
                self.clicked_cell = cell_at(ui, e.position.0);
            }
        }
        crate::door::list::pointer(self, ui, hand, events);
        if self.pointed != hand.row_under_pointer {
            self.pointed = hand.row_under_pointer;
            ui.ask_paint();
        }
    }

    /// A CLICK IS THE `RET` OF THE ROW IT LANDS ON — the footer's branch for
    /// this view says why, and the two doors have to agree.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let cell = self.clicked_cell;
        // THE PANE FIRST, THEN THE ROWS: they describe whichever half has
        // the keyboard, so reading them before aiming would put the cursor
        // on a row that is about to stop existing.
        self.view.aim(cell, ui.columns);
        let rows = self.view_rows(ui);
        self.view.clicked(row, cell, ui.columns, &rows);
        // EXCEPT WHILE A PAIRING QUESTION IS UP: there the pointer only
        // aims, because the question opens over whatever was on screen and
        // yes must not be one click from an unknown device.
        if !self.view.asking() {
            self.pressed(ui, crate::bluetooth::Press::Enter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bluetooth::{ACCEPT, REFUSE, RENAME};
    use ronoctua_bluetooth::agent::{Asked, Question};
    use ronoctua_bluetooth::{Adapter, Device, Kind, Scene};

    /// A window with no radio and no footer behind it — the view and the
    /// door's own line, which is all a test of the wiring needs.
    fn a_window() -> (BluetoothWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut me = BluetoothWindow::of(None);
        me.scene = Scene {
            adapters: vec![Adapter {
                at: "/org/bluez/hci0".to_owned(),
                alias: "this machine".to_owned(),
                powered: true,
                ..Adapter::default()
            }],
            devices: vec![Device {
                at: "/d/1".to_owned(),
                label: "the keyboard".to_owned(),
                name: "the keyboard".to_owned(),
                kind: Kind::Keyboard,
                paired: true,
                ..Device::default()
            }],
            daemon: true,
        };
        (me, Ui::for_tests(texts))
    }

    fn press(w: &mut BluetoothWindow, ui: &mut Ui, notation: &str) {
        let key = ronoctua_keymap::Key::parse(notation).expect(notation);
        let (event, mods) = crate::door::keys::event_of(key);
        w.typed(ui, &event, mods);
    }

    fn asked(asked: Asked) -> Question {
        Question {
            device: "/d/1".to_owned(),
            asked,
        }
    }

    /// THE QUESTION THE FOOTER'S AGENT HOLDS takes this window's screen too,
    /// and every answer the footer takes is taken here — `y`, `n`, `Esc` —
    /// and sent to the agent down the control socket.
    #[test]
    fn a_question_standing_in_the_footer_is_answered_here() {
        for (key, id) in [("y", ACCEPT), ("n", REFUSE), ("ESC", REFUSE)] {
            let (mut w, mut ui) = a_window();
            w.stand(&mut ui, Some(asked(Asked::Confirm("012345".to_owned()))));
            assert!(w.view.asking(), "the question did not reach the window");
            let said: String = w
                .view_rows(&ui)
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect();
            assert!(said.contains("012345"), "{said}");
            press(&mut w, &mut ui, key);
            // NAMING THE QUESTION it was given for (`bluetooth::Answer`).
            assert_eq!(ui.orders(), [format!("command {id} /d/1 confirm")], "{key}");
            assert!(!w.view.asking(), "{key}");
            assert!(ui.message.is_none(), "{:?}", ui.message);
        }
    }

    /// A PIN IS TYPED ON THIS WINDOW'S OWN LINE, masked, and only the agent
    /// hears it; nothing typed is asked again, as in the footer.
    #[test]
    fn a_pin_is_typed_here_masked_and_goes_only_to_the_agent() {
        for (question, word) in [(Asked::Pin, "pin"), (Asked::Typed, "typed")] {
            let (mut w, mut ui) = a_window();
            w.stand(&mut ui, Some(asked(question)));
            press(&mut w, &mut ui, "y");
            let line = ui.asking.clone().expect("the digits are asked here");
            assert_eq!(line.id, ACCEPT);
            assert!(line.secret, "the PIN is typed in the clear");
            assert!(ui.orders().is_empty(), "a bare yes went to the agent");

            ui.asking = None;
            w.answered(&mut ui, ACCEPT, "  ");
            assert!(ui.asking.is_some(), "an empty PIN was not asked again");
            assert!(ui.orders().is_empty());

            ui.asking = None;
            w.answered(&mut ui, ACCEPT, "0000");
            assert_eq!(
                ui.orders(),
                [format!("command bluetooth-accept /d/1 {word} 0000")]
            );
            assert!(!w.view.asking());
        }
    }

    /// FAILS IF: the window's answer can be taken for a question other than
    /// the one it was given for — the footer reads the order back and holds
    /// it to the question standing, and the footer's own plain answers still
    /// fit whatever it holds.
    #[test]
    fn an_answer_from_here_fits_only_its_own_question() {
        use crate::bluetooth::Answer;
        let (mut w, mut ui) = a_window();
        w.stand(&mut ui, Some(asked(Asked::Pin)));
        press(&mut w, &mut ui, "y");
        ui.asking = None;
        w.answered(&mut ui, ACCEPT, "0000");
        let order = ui.orders()[0].clone();
        let argument = order
            .strip_prefix("command bluetooth-accept ")
            .expect("an accept");
        let answer = Answer::read(Some(argument));
        assert_eq!(answer.typed(), "0000");
        assert!(answer.fits(&asked(Asked::Pin)));
        // ANOTHER DEVICE ASKING, or the same one asking something else.
        let other = Question {
            device: "/d/2".to_owned(),
            asked: Asked::Pin,
        };
        assert!(!answer.fits(&other), "a PIN reached another device");
        assert!(
            !answer.fits(&asked(Asked::Confirm("123456".to_owned()))),
            "a PIN confirmed another question"
        );
        // THE FOOTER'S OWN: its line's digits, its keys, its card.
        for plain in [None, Some("0000")] {
            let answer = Answer::read(plain);
            assert!(answer.fits(&other));
            assert_eq!(answer.typed(), plain.unwrap_or_default());
        }
        // AND NOTHING IS SENT FOR A QUESTION THIS WINDOW NO LONGER SHOWS.
        let (mut w, mut ui) = a_window();
        w.answered(&mut ui, ACCEPT, "0000");
        assert!(ui.orders().is_empty(), "{:?}", ui.orders());
    }

    /// A QUESTION THAT ENDS leaves this screen, and the digits being typed
    /// for it go with it; the same file read again raises nothing twice.
    #[test]
    fn a_question_that_ends_takes_its_digits_with_it() {
        let (mut w, mut ui) = a_window();
        w.stand(&mut ui, Some(asked(Asked::Pin)));
        press(&mut w, &mut ui, "y");
        assert!(ui.asking.is_some());
        w.stand(&mut ui, None);
        assert!(ui.asking.is_none(), "the PIN line outlived its question");
        assert!(!w.view.asking());

        // DISMISSED HERE, and the file stirred with the same question: it
        // does not come back.
        let tells = asked(Asked::Passkey("000042".to_owned()));
        w.stand(&mut ui, Some(tells.clone()));
        press(&mut w, &mut ui, "RET");
        assert!(!w.view.asking());
        w.stand(&mut ui, Some(tells));
        assert!(!w.view.asking(), "a dismissed question came back");
    }

    /// A NAME IS TYPED HERE, from the one it has, and goes to the radio from
    /// this window — the orders thread needs no footer.
    #[test]
    fn a_name_is_typed_here_from_the_one_it_has() {
        let (mut w, mut ui) = a_window();
        w.view = crate::bluetooth::View::fresh_at("/d/1");
        press(&mut w, &mut ui, "r");
        let line = ui.asking.clone().expect("the name is asked here");
        assert_eq!(line.id, RENAME);
        assert!(!line.secret);
        assert_eq!(line.text, "the keyboard");
        assert!(ui.message.is_none(), "{:?}", ui.message);
        ui.asking = None;
        w.answered(&mut ui, RENAME, "desk");
        // NO RADIO IN A TEST, so the order says so — it reached the radio's
        // arm, and nothing went to the footer.
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(ronoctua_i18n::key::BLUETOOTH_NO_DAEMON).as_str())
        );
        assert!(ui.orders().is_empty());
    }

    /// PAIRING FROM HERE says nothing about another door: the question comes
    /// to this window by the file.
    #[test]
    fn pairing_from_here_sends_nobody_elsewhere() {
        let (mut w, mut ui) = a_window();
        w.scene.devices[0].paired = false;
        w.view = crate::bluetooth::View::fresh_at("/d/1");
        press(&mut w, &mut ui, "p");
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(ronoctua_i18n::key::BLUETOOTH_NO_DAEMON).as_str()),
            "the pairing did not reach the radio's arm"
        );
        assert!(ui.orders().is_empty());
    }

    /// THE ROW UNDER THE POINTER SAYS ITS SENTENCE on line 0, below the
    /// question and the message and above the keys — the footer's order.
    #[test]
    fn the_row_under_the_pointer_says_its_sentence() {
        let (mut w, mut ui) = a_window();
        // A DEVICE'S PAGE, where every fact carries its sentence.
        w.view = crate::bluetooth::View::fresh_at("/d/1");
        let rows = w.view_rows(&ui);
        let (row, said) = rows
            .iter()
            .enumerate()
            .find_map(|(i, r)| r.description.clone().map(|d| (i, d)))
            .expect("a row with a sentence");
        assert_eq!(w.about_the_pointer(&ui), None);
        w.pointed = Some(row);
        assert_eq!(w.about_the_pointer(&ui).as_deref(), Some(said.as_str()));

        let hint = |ui: &Ui| w.view.hint(&ui.texts);
        let (pieces, hinting) = crate::door::list::line_zero(&mut ui, Some(said.clone()), hint);
        assert!(!hinting);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].text, said);

        ui.message = Some("an order said this".to_owned());
        let (pieces, _) = crate::door::list::line_zero(&mut ui, Some(said.clone()), hint);
        assert_eq!(pieces[0].text, "an order said this");

        ui.message = None;
        ui.ask(Line::new(RENAME, false, None));
        let (pieces, hinting) = crate::door::list::line_zero(&mut ui, Some(said), hint);
        assert!(hinting, "the question's ESC and RET are clicks");
        assert!(pieces.iter().any(|p| p.id == Some("line-answer")));
    }

    /// A QUESTION LEFT UNANSWERED AS THE WINDOW CLOSES goes back to the
    /// footer's view; with none up, closing says nothing.
    #[test]
    fn a_question_left_as_the_window_closes_goes_back_to_the_footer() {
        let (mut w, mut ui) = a_window();
        assert!(w.closing(&mut ui));
        assert!(ui.orders().is_empty());
        w.stand(&mut ui, Some(asked(Asked::Authorize)));
        assert!(w.closing(&mut ui));
        assert_eq!(ui.orders(), ["command bluetooth-open".to_owned()]);
    }

    /// THE FOOTER LEAVES A QUESTION TO THIS WINDOW only when the person is
    /// at it.
    #[test]
    fn the_window_answers_only_when_it_has_the_person() {
        assert!(has_the_person(Some(APP_ID)));
        assert!(!has_the_person(Some("br.ronoctua.power")));
        assert!(!has_the_person(None));
    }
}
