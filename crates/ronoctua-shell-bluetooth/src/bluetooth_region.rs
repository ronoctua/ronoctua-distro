//! THE RADIO'S VIEW IN THE FOOTER'S REGION (`LEADER s b b`) — and the one
//! errand handler both its doors carry out ([`carry_out`]).
//!
//! # Why the errands are here, and handled once
//!
//! What the view asks — an order to the radio, a profile, a volume, a name to
//! type, an answer to the pairing question — was written twice: the footer's
//! `bluetooth_errand` and the window's `apply`, each over its own door. It is
//! written once now, over the door's [`Doorway`].
//!
//! # What stays the footer's: the radio, not the view
//!
//! The pairing AGENT is the footer's (bluetoothd asks the default agent, and
//! only the process that outlives every window may register it), and so are
//! the radio's alarms, the question's card, the modeline's marks and the
//! orders that act on the radio with no view open (`LEADER s b s`, reconnect,
//! drop every link). This region is the VIEW: it is handed the footer's
//! reading, its line to the orders thread and the question standing, and it
//! answers the question the way the window does — through the registry's
//! command (`bluetooth-accept`, `bluetooth-refuse`), naming the question it
//! was given, which the footer holds the answer to.

use crate::bluetooth::{Errand, Press, View};
use crate::bluez_words::{ACCEPT, Answer, REFUSE, RENAME};
use crate::echo::Echo;
use crate::line::Line;
use crate::region::{Closed, Doorway, Entry, Seen, Taken};
use ronoctua_bluetooth::agent::Question;

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "bluetooth";

/// The view as the binary registers it with the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What the radio's view reads and acts on, in either door: the reading, the
/// line to the orders thread, and the question the agent holds.
pub struct Radio<'a> {
    /// The last reading.
    pub scene: &'a ronoctua_bluetooth::Scene,
    /// The orders thread, when bluetoothd was there.
    pub hand: Option<&'a ronoctua_bluetooth::Hand>,
    /// The pairing question standing, as this door was told it.
    pub standing: Option<&'a Question>,
}

/// Carries out what the view asked for, in whichever door it is drawn — the
/// footer's region or the window ([`crate::bluetooth_window`]).
pub fn carry_out(errand: Errand, view: &mut View, radio: &Radio<'_>, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        // AN INFORMATIVE ROW COPIES ITSELF — see `chrome::information_of`,
        // and `chrome::copied` for the one place the six doors share.
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, Echo::Brief);
        }
        // A NAME NOBODY FINISHED TYPING is not a name waiting for the next
        // device: the view that remembered it goes.
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Order(ask) => match radio.hand {
            Some(h) => h.send(ask),
            None => {
                let said = way.texts().t(crate::key::BLUETOOTH_NO_DAEMON);
                way.say(said, Echo::Normal);
            }
        },
        // THE PROFILE IS PIPEWIRE'S, so it does not go through the orders
        // thread — that one holds a BlueZ connection. AND THE CARD IS READ
        // AGAIN, because what is true is what PipeWire says and not what was
        // asked.
        Errand::Audio(card, profile) => {
            if let Err(e) = ronoctua_bluetooth::audio::choose(card, profile) {
                tracing::warn!(error = %e, "the audio profile did not change");
            }
            view.look_at_the_device(radio.scene);
        }
        // A COMMAND OF THE REGISTRY, run where commands are run — the
        // footer's, which a window reaches down the control socket.
        Errand::Run(id, about) => way.run(id, (!about.is_empty()).then_some(about)),
        // THE VOLUME AND THE MUTE DO NOT RE-READ THE GRAPH: changing a
        // profile recreates the node, changing a volume does not, and
        // `pw-dump` under a held arrow is a fork per frame.
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
        // THE ANSWER GOES TO THE AGENT, which is the footer's — through the
        // registry's command, NAMING THE QUESTION this door showed, so one
        // replaced on the way is not answered in its place (`Answer::fits`).
        Errand::Answer(yes) => {
            let id = if yes { ACCEPT } else { REFUSE };
            match radio.standing {
                Some(q) => way.run(id, Some(Answer::named(q, None))),
                None => tracing::info!("an answer with no question standing; not sent"),
            }
        }
        // A PHONE'S INTERNET, through NetworkManager — one sentence for both
        // doors (`bluetooth::tethered`).
        Errand::Tether { address, name, on } => {
            let said = crate::bluetooth::tethered(&address, &name, on, way.texts());
            way.say(said, Echo::Normal);
        }
        // THE DIGITS ARE TYPED ON THE DOOR'S LINE, MASKED, and come back as
        // the argument of `bluetooth-accept`.
        Errand::Type => way.ask(Line::new(ACCEPT, true, None)),
        // A NAME IS TYPED ON THE DOOR'S LINE, starting from the one it has,
        // and comes back as the argument of `bluetooth-rename` — which hands
        // it to the view (`View::renamed`).
        Errand::Rename(_) => {
            let now = view.name_being_changed(radio.scene);
            way.ask(Line::new(RENAME, false, now.as_deref()));
        }
    }
    way.repaint();
}

/// The digits typed for the pairing question, or a name: what the door's
/// line was asked for, handed back the same way in both doors. An empty
/// answer to the digits is asked again — a yes with no digits is an empty PIN
/// for the other side — and digits for a question that is over are sent to
/// nobody.
pub fn answered(id: &str, text: &str, view: &mut View, radio: &Radio<'_>, way: &mut dyn Doorway) {
    match id {
        ACCEPT => {
            if text.trim().is_empty() {
                way.ask(Line::new(ACCEPT, true, None));
                return;
            }
            // THE DIGITS GO TO THE AGENT, and only there — with the question
            // they were typed for, which the footer and the agent both hold
            // them to. No argv, no log, no copy kept here.
            let Some(q) = radio.standing else {
                tracing::info!("digits for a question that is over; not sent");
                return;
            };
            let named = Answer::named(q, Some(text));
            view.answered();
            way.run(ACCEPT, Some(named));
            way.repaint();
        }
        RENAME => {
            let errand = view.renamed(text);
            carry_out(errand, view, radio, way);
        }
        _ => {}
    }
}

/// What a command of the view does while it is CLOSED: the view's own
/// gestures do nothing with no view to act on — exactly as the key does —
/// and are TAKEN; the window opens; `bluetooth-open`, and `bluetooth-device`
/// with a device's path (a click on its modeline mark), open the region. What
/// acts on the RADIO with no view open — the agent's two answers, the three
/// bare letters of the adapter, reconnect, drop every link — is the footer's
/// radio, and not this view's.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "bluetooth-open" => Closed::Opens,
        "bluetooth-device" if argument.is_some() => Closed::Opens,
        "bluetooth-window" => {
            way.open_window(vec!["bluetooth".to_owned()]);
            Closed::Taken
        }
        "bluetooth-scan"
        | "bluetooth-power"
        | "bluetooth-discoverable"
        | ACCEPT
        | REFUSE
        | "bluetooth-reconnect"
        | "bluetooth-disconnect-all" => Closed::NotMine,
        _ if crate::bluetooth_words::press_of(command).is_some() => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view — the footer asked whether the region is free first — on
/// the list, or on one device: a click on a device's modeline mark carries
/// its path (`bluetooth-device <path>`), and opens the same page `RET` opens.
fn open(
    command: &str,
    argument: Option<&str>,
    _: &mut dyn Doorway,
) -> Box<dyn crate::region::Region> {
    let view = match (command, argument) {
        ("bluetooth-device", Some(at)) => View::fresh_at(at),
        _ => View::fresh(),
    };
    Box::new(BluetoothRegion {
        view,
        scene: ronoctua_bluetooth::Scene::default(),
        hand: None,
        standing: None,
        opened: false,
    })
}

/// The radio's view, as a region of the footer.
pub struct BluetoothRegion {
    view: View,
    /// The footer's last reading.
    scene: ronoctua_bluetooth::Scene,
    /// The footer's line to the orders thread.
    hand: Option<ronoctua_bluetooth::Hand>,
    /// The question the footer's agent holds, as the footer told it.
    standing: Option<Question>,
    /// Whether the command that opened it has run.
    opened: bool,
}

impl BluetoothRegion {
    /// One press — a key, a command by name, a click's `RET` — at the width
    /// the rows were drawn at: a device's page has two halves that sit side
    /// by side or stack, and `TAB` means a different move in each.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self
            .view
            .key(press, &self.scene, way.columns(), way.texts());
        self.carry(errand, way);
    }

    /// An errand of the view, carried out through the footer's door.
    fn carry(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let radio = Radio {
            scene: &self.scene,
            hand: self.hand.as_ref(),
            standing: self.standing.as_ref(),
        };
        carry_out(errand, &mut self.view, &radio, way);
    }
}

impl crate::region::Region for BluetoothRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// THE BLUETOOTH VIEW'S ROWS, as `fields`: two columns of its own. A
    /// row takes a click where one of its halves does (`View::takes`, the
    /// window's answer too), and the band then lights that half alone
    /// ([`Self::lit`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<crate::region::Row> {
        let takes = self.view.takes(&self.scene, seen.columns, seen.texts);
        self.view
            .rows(&self.scene, seen.columns, seen.texts)
            .into_iter()
            .zip(takes)
            .map(|(r, takes)| crate::region::Row {
                takes,
                fields: r.fields,
            })
            .collect()
    }

    /// ON A DEVICE'S PAGE SIDE BY SIDE, the half under the pointer — where a
    /// click there takes something (`View::takes_at`, which the click asks
    /// too), and nothing where it does not; the whole row everywhere else.
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> crate::region::Lit {
        let Some((from, to)) = self.view.half_at(cell, seen.columns) else {
            return crate::region::Lit::Row;
        };
        if self
            .view
            .takes_at(row, cell, &self.scene, seen.columns, seen.texts)
        {
            crate::region::Lit::Cells(from, to)
        } else {
            crate::region::Lit::Nothing
        }
    }

    /// WHAT THE POINTER ASKS: a label is two or three words, and some of
    /// these rows are about things nobody meets twice a year
    /// (`bluetooth::Row::description`).
    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        self.view
            .rows(&self.scene, seen.columns, seen.texts)
            .into_iter()
            .nth(row)?
            .description
    }

    /// The radio's table is bare letters: without the line there is nothing
    /// on screen that says `c` connects or `f` forgets.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// The arrows, `RET`, `Esc` and twelve bare letters — nothing is typed
    /// here, so a letter is free — read by the WHOLE notation through the
    /// reader the window calls too (`bluetooth_words::press_of_key`): `C-f`
    /// is not `f`, and does not forget the device under the cursor.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match crate::bluetooth_words::press_of_key(key) {
            Some(press) => {
                // THE SURFACE FIRST, THEN THE CURSOR: a device list of two
                // rows and a device's sheet of thirty (`Doorway::follow`).
                self.press(press, way);
                way.follow();
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        let first = !self.opened;
        self.opened = true;
        match (id, argument) {
            // ALREADY OPEN AND NO DEVICE ASKED FOR: left where it is — a
            // device's mark is one click away from the cursor somebody put on
            // a row, and rebuilding the view would take the level, the cursor
            // and any standing question with it.
            ("bluetooth-open", _) => way.follow(),
            // A PAGE OPENED STRAIGHT ONTO A DEVICE READS IT, as `RET` on its
            // row does — a volume, a profile and the page's second half; one
            // screen, one content, however one arrived. A device asked for
            // with the view already open is refused as every door refuses
            // with the region held — here, by the view itself.
            ("bluetooth-device", Some(_)) => {
                if first {
                    let scene = std::mem::take(&mut self.scene);
                    self.view.look_at_the_device(&scene);
                    self.scene = scene;
                    way.follow();
                } else {
                    tracing::info!(
                        view = id,
                        held = ID,
                        "refused: another view is open in the footer"
                    );
                    let said = way.texts().t(crate::key::ECHO_VIEW_BUSY);
                    way.say(said, Echo::Normal);
                }
            }
            ("bluetooth-window", _) => way.open_window(vec!["bluetooth".to_owned()]),
            // THE NAME TYPED on the footer's line comes back as the argument,
            // and the VIEW says what it names (`View::renamed`).
            (RENAME, Some(name)) => {
                let errand = self.view.renamed(name);
                self.carry(errand, way);
            }
            // THE AGENT'S ANSWERS AND WHAT ACTS ON THE WHOLE RADIO are the
            // footer's radio — the question and the reading it holds.
            (ACCEPT | REFUSE | "bluetooth-reconnect" | "bluetooth-disconnect-all", _) => {
                return Taken::No;
            }
            // THE VIEW'S OWN LETTERS, through `View::key` — the same door the
            // keyboard uses: a second path to "what does `c` do here" is a
            // second answer waiting to disagree. AND THE PAGE FOLLOWS, as
            // after a key.
            (other, _) => match crate::bluetooth_words::press_of(other) {
                Some(press) => {
                    self.press(press, way);
                    way.follow();
                }
                None => return Taken::No,
            },
        }
        way.repaint();
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// A CLICK IS THE `RET` OF THE ROW IT LANDS ON — the owner's rule for this
    /// view: the second column's rows are a volume, a switch and two doors,
    /// and nothing a row does is one way. WHERE ALONG THE LINE, because a
    /// device's page has two halves side by side and a click on the right
    /// one has to put the keyboard there — THE PANE FIRST, THEN THE ROWS.
    /// EXCEPT WHILE A PAIRING QUESTION IS UP, where the pointer goes on only
    /// aiming: yes must not be one movement of a hand from an unknown device.
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let columns = way.columns();
        // A CLICK WHERE IT TAKES NOTHING DOES NOTHING — a heading, a
        // sentence, the blank beside the other half's row: it used to press
        // `RET` on whatever row the cursor was on (`View::takes_at`, the
        // answer the light is drawn by).
        if !self
            .view
            .takes_at(row, cell, &self.scene, columns, way.texts())
        {
            return;
        }
        self.view.aim(cell, columns);
        let rows = self.view.rows(&self.scene, columns, way.texts());
        self.view.clicked(row, cell, columns, &rows);
        if self.view.asking() {
            way.repaint();
        } else {
            self.press(Press::Enter, way);
            way.follow();
        }
    }

    /// The digits typed for the pairing question, or a name — the window's
    /// line's answers, answered alike ([`answered`]).
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let radio = Radio {
            scene: &self.scene,
            hand: self.hand.as_ref(),
            standing: self.standing.as_ref(),
        };
        answered(id, text, &mut self.view, &radio, way);
    }

    /// What the footer heard of the radio: the reading (the view follows the
    /// new rows: a device switched off takes its row away, a scan brings four
    /// in a second), its line to the orders thread, the question its agent
    /// was asked, and the question over.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(scene) = news.downcast_ref::<ronoctua_bluetooth::Scene>() {
            self.scene = scene.clone();
            let rows = self.view.rows(&self.scene, way.columns(), way.texts());
            self.view.settle(&rows);
            way.repaint();
        } else if let Some(hand) = news.downcast_ref::<ronoctua_bluetooth::Hand>() {
            self.hand = Some(hand.clone());
        } else if let Some(question) = news.downcast_ref::<Question>() {
            self.standing = Some(question.clone());
            self.view.asked(question.clone());
            way.repaint();
        } else if let Some(ronoctua_bluetooth::Event::Unasked) =
            news.downcast_ref::<ronoctua_bluetooth::Event>()
        {
            self.standing = None;
            self.view.answered();
            way.repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder, Region};
    use ronoctua_bluetooth::agent::Asked;

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    fn region() -> BluetoothRegion {
        BluetoothRegion {
            view: View::fresh(),
            scene: ronoctua_bluetooth::Scene::default(),
            hand: None,
            standing: None,
            opened: true,
        }
    }

    fn question(asked: Asked) -> Question {
        Question {
            device: "/org/bluez/hci0/dev_02_00_00_00_00_09".to_owned(),
            asked,
        }
    }

    /// FAILS IF: the answer to the pairing question leaves the view by any
    /// road but the registry's command naming the question it was given for
    /// — the footer would answer a question that replaced it — or with no
    /// question standing an answer is sent at all.
    #[test]
    fn an_answer_names_the_question_it_was_given_for() {
        let mut w = way();
        let mut r = region();
        r.carry(Errand::Answer(true), &mut w);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Run(..))),
            "an answer with no question was sent"
        );
        let q = question(Asked::Confirm("012345".to_owned()));
        r.heard(&q, &mut w);
        assert!(r.view.asking());
        let mut w = way();
        r.carry(Errand::Answer(false), &mut w);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::Run(REFUSE, Some(Answer::named(&q, None))))
        );
        r.heard(&ronoctua_bluetooth::Event::Unasked, &mut w);
        assert!(!r.view.asking() && r.standing.is_none());
    }

    /// FAILS IF: the digits are sent with nothing typed (an empty PIN for the
    /// other side), are not asked masked, or leave without the question
    /// they were typed for.
    #[test]
    fn the_digits_are_typed_masked_and_go_named() {
        let mut w = way();
        let mut r = region();
        r.carry(Errand::Type, &mut w);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::Ask(Line::new(ACCEPT, true, None)))
        );
        let q = question(Asked::Pin);
        r.heard(&q, &mut w);
        let mut w = way();
        r.answered(ACCEPT, "  ", &mut w);
        assert_eq!(w.owed, vec![Effect::Ask(Line::new(ACCEPT, true, None))]);
        let mut w = way();
        r.answered(ACCEPT, "1234", &mut w);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::Run(ACCEPT, Some(Answer::named(&q, Some("1234")))))
        );
        assert!(!r.view.asking(), "the digits went and the question stayed");
    }

    /// FAILS IF: what acts on the radio — the agent's answers, the three bare
    /// letters of the adapter, reconnect and drop-all — is taken by the view
    /// (the footer's radio, which holds the question and the reading, would
    /// never see it), or a gesture of the view is answered closed by
    /// anything but nothing.
    #[test]
    fn the_radios_commands_are_the_footers() {
        let mut w = way();
        for id in [
            ACCEPT,
            REFUSE,
            "bluetooth-reconnect",
            "bluetooth-disconnect-all",
        ] {
            assert_eq!(region().command(id, None, &mut w), Taken::No, "{id}");
            assert_eq!(closed(id, None, &mut w), Closed::NotMine, "{id}");
        }
        for id in [
            "bluetooth-scan",
            "bluetooth-power",
            "bluetooth-discoverable",
        ] {
            assert_eq!(closed(id, None, &mut w), Closed::NotMine, "{id}");
        }
        assert_eq!(closed("bluetooth-connect", None, &mut w), Closed::Taken);
        assert_eq!(closed("bluetooth-device", None, &mut w), Closed::Taken);
        assert_eq!(
            closed("bluetooth-device", Some("/d/1"), &mut w),
            Closed::Opens
        );
        assert!(w.owed.is_empty(), "a gesture with no view did something");
    }

    /// FAILS IF: the command that opened the view says the region is held,
    /// or a device asked for with the view already open is not refused.
    #[test]
    fn the_first_open_is_the_opening_and_a_device_later_is_refused() {
        let mut w = way();
        let mut r = region();
        r.opened = false;
        assert_eq!(r.command("bluetooth-open", None, &mut w), Taken::Yes);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Say(..))),
            "the opening said something: {:?}",
            w.owed
        );
        let mut w = way();
        r.command("bluetooth-device", Some("/d/1"), &mut w);
        let busy = w.texts().t(crate::key::ECHO_VIEW_BUSY);
        assert_eq!(w.owed.first(), Some(&Effect::Say(busy, Echo::Normal)));
    }

    /// FAILS IF: an order reaches no thread without a word.
    #[test]
    fn an_order_with_no_thread_says_so() {
        let mut w = way();
        let mut r = region();
        r.carry(Errand::Order(ronoctua_bluetooth::Ask::Scan(true)), &mut w);
        let said = w.texts().t(crate::key::BLUETOOTH_NO_DAEMON);
        assert_eq!(w.owed.first(), Some(&Effect::Say(said, Echo::Normal)));
    }

    /// A radio with one paired keyboard that may wake the machine — a
    /// second half to its page, with no address: nothing asks PipeWire.
    fn a_scene() -> ronoctua_bluetooth::Scene {
        ronoctua_bluetooth::Scene {
            adapters: vec![ronoctua_bluetooth::Adapter {
                at: "/org/bluez/hci0".to_owned(),
                alias: "this machine".to_owned(),
                powered: true,
                ..ronoctua_bluetooth::Adapter::default()
            }],
            devices: vec![ronoctua_bluetooth::Device {
                at: "/d/1".to_owned(),
                label: "the keyboard".to_owned(),
                name: "the keyboard".to_owned(),
                kind: ronoctua_bluetooth::Kind::Keyboard,
                paired: true,
                wake: Some(false),
                ..ronoctua_bluetooth::Device::default()
            }],
            daemon: true,
        }
    }

    /// The region over [`a_scene`], on the list or on the keyboard's page.
    fn over_the_scene(page: bool) -> BluetoothRegion {
        let scene = a_scene();
        let mut view = if page {
            View::fresh_at("/d/1")
        } else {
            View::fresh()
        };
        view.look_at_the_device(&scene);
        BluetoothRegion {
            view,
            scene,
            hand: None,
            standing: None,
            opened: true,
        }
    }

    fn wide_way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        Recorder::new(
            std::rc::Rc::new(texts),
            WIDE,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    /// Wide enough for a device's page to be two halves side by side.
    const WIDE: u16 = 120;

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a heading, a sentence — or leaves unlit one
    /// it takes, on the list. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a view of its own, and lit exactly where something
    /// happened.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: WIDE,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let held = crate::region::held_to_the_click(
            || (over_the_scene(false), wide_way()),
            &seen,
            |r, w: &Recorder| crate::region::seen(r, &w.owed, &seen),
        );
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }

    /// FAILS IF: on a device's page side by side, the pointer over one half
    /// of a line lights what a click there does not take — the blank beside
    /// a row of the other half, a heading — or leaves unlit a half whose
    /// click takes it, or lights across both halves; or a click where
    /// nothing lights does something (it pressed `RET` on the cursor's row).
    /// The light came from the half the keyboard was in, wrong both ways.
    #[test]
    fn a_devices_page_lights_the_half_a_click_takes() {
        use crate::region::Lit;
        let t = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: WIDE,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let split = crate::bluetooth::split_at(WIDE);
        let first = over_the_scene(true);
        let rows = first.rows(&seen);
        assert!(rows.len() >= 3, "{} rows", rows.len());
        let mut lit_left = false;
        let mut lit_right = false;
        let mut dark = false;
        for (n, row) in rows.iter().enumerate() {
            for cell in [2, split + 2] {
                let lit = first.lit(n, Some(cell), &seen);
                let lights = row.takes && lit != Lit::Nothing;
                if let Lit::Cells(from, to) = lit {
                    assert!(
                        (from..to).contains(&cell) && (to <= split || from >= split),
                        "row {n} at {cell} lights {from}..{to}, across the halves"
                    );
                }
                let mut r = over_the_scene(true);
                let mut w = wide_way();
                let before = crate::region::seen(&r, &w.owed, &seen);
                r.clicked_at(n, Some(cell), &mut w);
                let did = crate::region::seen(&r, &w.owed, &seen) != before;
                // THE ROW THE CURSOR STANDS ON, in the half the keyboard is
                // in, is the click's whatever its `RET` does
                // (`region::Held::here`).
                let here = n == first.view.cursor() && cell < split;
                assert!(
                    lights == did || (lights && here),
                    "row {n} at cell {cell} is lit {lights} and a click there did {did}"
                );
                lit_left |= lights && cell < split;
                lit_right |= lights && cell >= split;
                dark |= !lights;
            }
        }
        assert!(
            lit_left && lit_right && dark,
            "the halves were not both reached"
        );
    }
}
