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
//! stands (`bluez_words::standing`), watched by inotify. It is drawn and
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

/// The window's `app_id` — the radio's words hold it
/// (`bluez_words::APP_ID`), where the footer asks whether the person is at
/// this window when a pairing question arrives.
pub const APP_ID: &str = crate::bluez_words::APP_ID;

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
        // `bluez_words::standing`.
        let (asked_sender, asked_changed) = channel::channel::<()>();
        let question_watch =
            crate::bluez_words::standing::watch(move || asked_sender.send(()).is_ok());
        w.handle
            .insert_source(
                asked_changed,
                |event, (), door: &mut Door<BluetoothWindow>| {
                    if let channel::Event::Msg(()) = event {
                        let now = crate::bluez_words::standing::read();
                        door.content.stand(&mut door.ui, now);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the question's channel: {e}"))?;

        let mut window = BluetoothWindow::of(radio);
        window._question_watch = question_watch;
        // AND ONE ALREADY STANDING as the window opens is asked at once.
        if let Some(q) = crate::bluez_words::standing::read() {
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

/// The window over a PLANTED radio, for the photograph
/// (its camera, `camera`): `phone` is a paired phone's page whose internet is
/// in use, through `bnep0` — the switch the network's view agrees with.
///
/// # Errors
///
/// A scene this door does not know.
pub fn photographed(scene: &str) -> anyhow::Result<(impl Content, Option<Line>)> {
    use ronoctua_bluetooth::{Adapter, Device, Kind, Pan, Scene};
    anyhow::ensure!(
        scene == "phone",
        "`{scene}` is not a scene of the Bluetooth window; the scene is phone"
    );
    let mut me = BluetoothWindow::of(None);
    me.scene = Scene {
        adapters: vec![Adapter {
            at: "/org/bluez/hci0".to_owned(),
            alias: "noctua".to_owned(),
            powered: true,
            power_state: "on".to_owned(),
            pairable: true,
            ..Adapter::default()
        }],
        devices: vec![Device {
            at: "/org/bluez/hci0/dev_02_00_00_00_00_09".to_owned(),
            address: "02:00:00:00:00:09".to_owned(),
            address_type: "public".to_owned(),
            label: "Ana's phone".to_owned(),
            name: "Ana's phone".to_owned(),
            kind: Kind::Phone,
            connected: true,
            paired: true,
            bonded: true,
            trusted: true,
            services_resolved: true,
            uuids: vec![ronoctua_bluetooth::NAP.to_owned()],
            pan: Some(Pan {
                connected: true,
                interface: "bnep0".to_owned(),
            }),
            ..Device::default()
        }],
        daemon: true,
    };
    me.view = crate::bluetooth::View::fresh_at("/org/bluez/hci0/dev_02_00_00_00_00_09");
    Ok((me, None))
}

/// `ronoctua-shell photograph --door bluetooth --scene phone`: this window
/// over a planted machine, walked and drawn (`crate::photo::planted`) — the
/// camera of Bluetooth, beside the door it photographs.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(rest, "bluetooth", "phone", |asked| {
        let (content, line) = photographed(&asked.scene)?;
        crate::photo::planted::shoot(asked, content, line)
    })
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
                    ui.say(crate::bluez_words::refused(&ask, why, &words, &ui.texts));
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
            .is_some_and(|l| l.id == crate::bluez_words::ACCEPT)
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

    /// What this window reads and acts on, as the shared handler takes it.
    fn radio(&self) -> crate::bluetooth_region::Radio<'_> {
        crate::bluetooth_region::Radio {
            scene: &self.scene,
            hand: self.radio.as_ref(),
            standing: self.standing.as_ref(),
        }
    }

    /// Carries out what the view asked for — through the one handler the
    /// footer's region carries it out with too (`bluetooth_region::carry_out`).
    /// THE ANSWER GOES TO THE AGENT, which is the footer's: down the control
    /// socket, naming the question this window showed.
    fn apply(&mut self, ui: &mut Ui, errand: crate::bluetooth::Errand) {
        let mut view = std::mem::take(&mut self.view);
        crate::bluetooth_region::carry_out(errand, &mut view, &self.radio(), ui);
        self.view = view;
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

    /// Every pointer's hover band — each row, cursor's colour and `x` across
    /// the surface — laid ONLY where a click there takes something
    /// (`View::takes_at`, the click's own answer): over a device's page side
    /// by side, the half under the pointer and never both; elsewhere the
    /// whole row. Its own function, so a test holds it: a frame's canvas
    /// needs a GPU.
    fn hover(&self, ui: &Ui, pointers: &[Pointing]) -> Vec<ronoctua_render::quad::Quad> {
        let rows = self.view_rows(ui).len();
        let cell_px = ui.frame(0).cell();
        let mut out = Vec::new();
        for &((colour, row), x) in pointers {
            let (Some(row), Some(x)) = (row, x) else {
                continue;
            };
            let cell = cell_at(ui, f64::from(x));
            if !self
                .view
                .takes_at(row, cell, &self.scene, ui.columns, &ui.texts)
            {
                continue;
            }
            let all = [true];
            for mut band in crate::door::list::hover_bands(
                &[(colour, Some(row))],
                &all.repeat(rows),
                None,
                ui.offset,
                ui.room_for_rows(),
                rows,
                ui.first_of_list(),
                ui.room.band_width(),
            ) {
                if let Some((from, to)) = self.view.half_at(cell, ui.columns) {
                    let left = f32::from(ui.margin + from) * cell_px;
                    let right = (f32::from(ui.margin + to) * cell_px).min(band.width);
                    band.x = if from == 0 { 0.0 } else { left };
                    band.width = (right - band.x).max(0.0);
                }
                out.push(band);
            }
        }
        out
    }

    /// What the row under the pointer has to add: its whole sentence, which
    /// the footer puts on its own message line from the same field.
    fn about_the_pointer(&self, ui: &Ui) -> Option<String> {
        let row = self.pointed?;
        self.view_rows(ui).into_iter().nth(row)?.description
    }
}

/// One pointer as a frame paints it: its cursor's colour and the row it is
/// on, and where across the surface it is (`Canvas::lit`, `Canvas::lit_x`).
type Pointing = ((tokens::Color, Option<usize>), Option<f32>);

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
        format!("*{}*", texts.t(crate::key::BLUETOOTH_TITLE))
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
    /// name — the same answers the footer's minibuffer hands its region,
    /// answered alike (`bluetooth_region::answered`).
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let mut view = std::mem::take(&mut self.view);
        crate::bluetooth_region::answered(id, text, &mut view, &self.radio(), ui);
        self.view = view;
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

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row where one of its halves does — the view's one
    /// answer (`View::takes`), which the footer's region gives too; the
    /// half the band lights is [`BluetoothWindow::hover`]'s.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view.takes(&self.scene, ui.columns, &ui.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// The standard window, with one thing more on line 0: what the row
    /// under the pointer has to add, as the footer says it on its own line.
    ///
    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    ///
    /// THE POINTER LIGHTS WHAT THE CLICK TAKES: the list is drawn without
    /// its hover, and the hover is [`BluetoothWindow::hover`]'s — the half
    /// of a device's page the pointer is over, as the footer lights it.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let about = self.about_the_pointer(ui);
        crate::door::list::header_about(ui, canvas, None, about, |ui| self.view.hint(&ui.texts));
        let rows = self.rows(ui);
        crate::door::list::body(ui, canvas, &rows, &[], None);
        let pointers: Vec<_> = canvas
            .lit
            .iter()
            .copied()
            .zip(canvas.lit_x.iter().copied().chain(std::iter::repeat(None)))
            .collect();
        canvas.under.extend(self.hover(ui, &pointers));
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
        // A CLICK WHERE IT TAKES NOTHING DOES NOTHING — a heading, the blank
        // beside the other half's row: `RET` was pressed on the cursor's row
        // (`View::takes_at`, the answer the light is drawn by).
        if !self
            .view
            .takes_at(row, cell, &self.scene, ui.columns, &ui.texts)
        {
            return;
        }
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
    use crate::bluez_words::{ACCEPT, REFUSE, RENAME};
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
            // NAMING THE QUESTION it was given for (`bluez_words::Answer`).
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
        use crate::bluez_words::Answer;
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
        press(&mut w, &mut ui, "e");
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
            Some(ui.texts.t(crate::key::BLUETOOTH_NO_DAEMON).as_str())
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
            Some(ui.texts.t(crate::key::BLUETOOTH_NO_DAEMON).as_str()),
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

    /// FAILS IF: the camera's planted phone stops building.
    #[test]
    fn the_planted_phone_builds() {
        assert!(photographed("phone").is_ok());
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a heading, a sentence — or leaves unlit one it takes, on
    /// the list. THE LIGHT IS THE CLICK: every row clicked once, on a
    /// window of its own, and lit exactly where something happened. The
    /// footer's region has its own.
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

    /// FAILS IF: on a device's page side by side the window's pointer
    /// lights a half a click there does not take, or leaves unlit one it
    /// takes, or lights across both halves — the footer's region holds the
    /// same (`a_devices_page_lights_the_half_a_click_takes`).
    #[test]
    fn the_windows_device_page_lights_the_half_a_click_takes() {
        let page = || {
            let (mut w, mut ui) = a_window();
            if let Some(d) = w.scene.devices.first_mut() {
                d.wake = Some(false);
            }
            w.view = crate::bluetooth::View::fresh_at("/d/1");
            w.view.look_at_the_device(&w.scene);
            ui.columns = 120;
            // THE ROOM A FRAME LEAVES THE LIST — what a band is cut to.
            #[allow(clippy::cast_precision_loss)]
            let (width, height) = (ui.width as f32, ui.height as f32);
            ui.room = crate::chrome::Room::of(crate::chrome::list_scroll(0, 0, 0), width, height);
            (w, ui)
        };
        let (first, ui) = page();
        let rows = first.rows(&ui).len();
        assert!(rows >= 3, "{rows} rows");
        let split = crate::bluetooth::split_at(ui.columns);
        let cell_px = ui.frame(0).cell();
        let (mut left, mut right, mut dark) = (false, false, false);
        for n in 0..rows {
            for cell in [2, split + 2] {
                #[allow(clippy::cast_precision_loss)]
                let x = f32::from(ui.margin + cell) * cell_px + 1.0;
                let bands = first.hover(&ui, &[((tokens::color::SURFACE_HOVER, Some(n)), Some(x))]);
                for b in &bands {
                    assert!(
                        b.x <= x && x < b.x + b.width,
                        "row {n}: the band is not under the pointer"
                    );
                    let split_px = f32::from(ui.margin + split) * cell_px;
                    assert!(
                        b.x + b.width <= split_px + 0.5 || b.x >= split_px - 0.5,
                        "row {n}: the band crosses the halves"
                    );
                }
                let lit = !bands.is_empty();
                let (mut w, mut ui) = page();
                let before = crate::door::seen(&w, &ui);
                w.clicked_cell = Some(cell);
                w.clicked(&mut ui, n);
                let did = crate::door::seen(&w, &ui) != before;
                // THE ROW THE CURSOR STANDS ON, in the half the keyboard is
                // in, is the click's whatever its `RET` does
                // (`region::Held::here`).
                let here = n == first.view.cursor() && cell < split;
                assert!(
                    lit == did || (lit && here),
                    "row {n} at cell {cell} is lit {lit} and a click did {did}"
                );
                left |= lit && cell < split;
                right |= lit && cell >= split;
                dark |= !lit;
            }
        }
        assert!(left && right && dark, "the halves were not both reached");
    }
}
