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
//! daemon directly — connecting, pairing, forgetting are the same call from
//! any process — EXCEPT a pairing QUESTION, which is answered in the footer:
//! the agent is registered by the footer's own connection, because it is the
//! process that outlives this window.

use crate::door::{Content, Door, Hand, Ui};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.bluetooth";

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

        Ok(BluetoothWindow {
            view: crate::bluetooth::View::fresh(),
            scene: ronoctua_bluetooth::Scene::default(),
            radio,
            clicked_cell: None,
        })
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
}

impl BluetoothWindow {
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
            // THESE TWO NEVER ARRIVE HERE, and the arm is not dead code: the
            // events are raised inside `agent::serve`, which this door does
            // not call (`serve_agent: false`, just above). A `_ => {}` would
            // go quietly wrong the day this window did register an agent.
            //
            // WHERE THE SENTENCE ACTUALLY COMES FROM is `apply`, below: this
            // window is the one that sent `Ask::Pair`, so it is the one that
            // knows a question is about to be asked somewhere else — and it
            // says so at the moment somebody presses the key, which is when
            // they are looking. Waiting for an event that cannot arrive left
            // `p` in this window doing nothing visible at all.
            Event::Asked(_) => {
                ui.message = Some(
                    ui.texts
                        .t(ronoctua_i18n::key::BLUETOOTH_ANSWER_IN_THE_FOOTER),
                );
                ui.ask_paint();
            }
            Event::Unasked => {
                ui.message = None;
                ui.ask_paint();
            }
        }
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
            // AND THIS DOOR SENDS IT rather than telling somebody to go to
            // the footer and do it themselves — the control socket exists for
            // exactly this, and the notification manager already forwards
            // through it.
            Errand::Run(id, about) => {
                let line = if about.is_empty() {
                    format!("command {id}")
                } else {
                    format!("command {id} {about}")
                };
                if let Err(e) = crate::leader::send_it(&line) {
                    tracing::warn!(id, error = %e, "the footer did not take the command");
                }
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
            // THE ANSWER IS THE FOOTER'S TOO, and for the agent's reason:
            // it is the footer's connection bluetoothd is asking.
            Errand::Answer(_) | Errand::Type => {
                ui.message = Some(
                    ui.texts
                        .t(ronoctua_i18n::key::BLUETOOTH_ANSWER_IN_THE_FOOTER),
                );
            }
            Errand::Order(ask) => {
                // AND PAIRING SAYS WHERE THE QUESTION WILL BE. bluetoothd
                // asks the DEFAULT agent, which is the footer's — so from
                // this window `p` starts something whose only visible half
                // happens on another surface. Saying so here is the whole
                // reason `bluetooth-answer-in-the-footer` exists; without it
                // the key looked like it had done nothing.
                if matches!(ask, ronoctua_bluetooth::Ask::Pair(_)) {
                    ui.message = Some(
                        ui.texts
                            .t(ronoctua_i18n::key::BLUETOOTH_ANSWER_IN_THE_FOOTER),
                    );
                }
                match &self.radio {
                    Some(h) => h.send(ask),
                    None => {
                        ui.message = Some(ui.texts.t(ronoctua_i18n::key::BLUETOOTH_NO_DAEMON));
                    }
                }
            }
            // A NAME IS TYPED IN THE MINIBUFFER, which this window has not
            // got: the footer's is the one place anything is typed, and
            // saying which is better than a key that looks broken.
            Errand::Rename(_) => {
                ui.message = Some(ui.texts.t(ronoctua_i18n::key::BLUETOOTH_TYPE_IN_THE_FOOTER));
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

    /// THE MODIFIERS ARE READ WHOLE: `KeyEvent` carries no modifier state,
    /// and this view needs one — the bare sideways arrows cross the columns
    /// and `S-<arrow>` is more and less, so a press that could not see shift
    /// would answer both with the same thing.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(press) = press_of(event, mods) else {
            return;
        };
        ui.message = None;
        self.pressed(ui, press);
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// The standard list, and one thing more: WHERE ALONG THE LINE a press
    /// landed, which `clicked` needs and is not handed.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        for e in events {
            if let PointerEventKind::Press { button, .. } = e.kind
                && button == crate::door::LEFT_BUTTON
            {
                self.clicked_cell = cell_at(ui, e.position.0);
            }
        }
        crate::door::list::pointer(self, ui, hand, events);
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

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent, mods: Modifiers) -> Option<crate::bluetooth::Press> {
    use crate::bluetooth::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        // THE THREE THAT ARE NOT LETTERS, and they need naming here.
        //
        // The fallback below matches `event.utf8` against the view's own
        // notation, which works for a bare letter and for nothing else: an
        // arrow and a `TAB` produce no text, so `<right>`, `<left>` and `TAB`
        // resolved to nothing and were unreachable in this door while the
        // footer answered them. The hint line taught all three, in both.
        //
        // AND SHIFT DECIDES WHICH PAIR THE ARROWS ARE. Bare, they cross the
        // columns; held, they are more and less of the row under the cursor.
        // The keysym is `Right` either way — X11 does not shift an arrow —
        // so the state this window keeps is the only thing that can tell
        // them apart.
        Keysym::Right if mods.shift => Press::More,
        Keysym::Left if mods.shift => Press::Less,
        Keysym::Right | Keysym::Left => Press::Cross,
        Keysym::Tab | Keysym::ISO_Left_Tab => Press::Cross,
        // AND THE BARE LETTERS, out of the view's own table — the footer
        // reads it the same way, so the two doors cannot teach different
        // keys. A letter with no press there is a letter that does nothing,
        // and the test beside `bluetooth_words` is what says so.
        _ => {
            let typed = event.utf8.as_deref()?;
            let id = crate::bluetooth::OF_THE_VIEW
                .iter()
                .find(|(notation, _, _)| *notation == typed)
                .map(|(_, id, _)| *id)?;
            crate::bluetooth_words::press_of(id)?
        }
    })
}
