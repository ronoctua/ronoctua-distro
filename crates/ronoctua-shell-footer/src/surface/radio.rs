//! THE RADIO, AS THE FOOTER KEEPS IT FOR THE SESSION — not a view.
//!
//! The pairing AGENT is the footer's: bluetoothd asks the default agent, and
//! only one process may register it — the one that outlives every window. So
//! the questions arrive here, and so do the radio's alarms, the question's
//! card on the band, the modeline's marks and the orders that act on the
//! whole radio with no view open (`LEADER s b s`, reconnect, drop every
//! link). The radio's VIEW is a region of its own crate
//! (`ronoctua-shell-bluetooth`), told every reading, the
//! line to the orders thread and the question as the footer hears them; its
//! answers come back through the registry's two commands, which this file
//! answers (`radio_commands`).

use super::*;

/// The radio's view's id, as the footer ranks it (`PRECEDENCE`) — the
/// region the binary registers under it, hosted by its id alone.
const RADIO: &str = "bluetooth";

impl Footer {
    /// The radio's three alarms, raised on a CROSSING and never on a state.
    ///
    /// The machine's own battery does this in `ronoctua_power::alarms`, which
    /// is a pure function over two readings; this one compares two SCENES the
    /// same way. A device that sits at 12% for an afternoon is one line and
    /// not a hundred, and a keyboard that drops says so once.
    ///
    /// # Why the low one remembers, and the other two do not
    ///
    /// Connecting and disconnecting are edges by construction: the device is
    /// in one list and then the other. A battery is a NUMBER that wanders —
    /// 16, 14, 15, 14 — so "it crossed" has to be remembered, and what
    /// remembers is `bluetooth_alarmed`. A device leaving takes its memory
    /// with it, so plugging the same mouse back in at 12% says so again.
    ///
    /// # Why coming and going are not raised here any more
    ///
    /// They are EVENTS now, said on the minibuffer's line (`crate::events`),
    /// and this gives them back for the caller to say once the view has
    /// settled. They keep their tags, so on the band — when the line was busy
    /// — the two filters that quiet a device coming and going still quiet
    /// them. And the first scene is only the baseline: this used to raise
    /// "connected" once per device at every start of the footer.
    pub(super) fn radio_alarms(&mut self) -> Vec<crate::events::Event> {
        use std::collections::BTreeSet;
        let now = std::time::Instant::now();
        let named = |d: &ronoctua_bluetooth::Device| d.label.clone();
        let said = self.events.radio(&self.bluetooth_scene);
        // THE LIST IS DECIDED FIRST, and only then is anything raised.
        // `arrived` takes `&mut self`, and the scene lives in `self` — so
        // walking the scene while raising borrows it twice. This is the same
        // shape `power_arrived` has for the same reason.
        let there: BTreeSet<String> = self
            .bluetooth_scene
            .connected()
            .iter()
            .map(|d| d.at.clone())
            .collect();
        // THE ONES THAT ARE RUNNING OUT, on the crossing of the same number
        // the modeline turns red at.
        let mut running_out: Vec<(String, Option<u8>)> = Vec::new();
        let mut told: BTreeSet<String> = std::mem::take(&mut self.bluetooth_alarmed);
        for d in self.bluetooth_scene.connected() {
            let Some(pct) = d.battery else { continue };
            let low = pct <= ronoctua_bluetooth::DANGER_FROM;
            if low && !told.contains(&d.at) {
                told.insert(d.at.clone());
                running_out.push((named(d), Some(pct)));
            } else if !low {
                // IT CHARGED. Forgetting is what lets it say so again.
                told.remove(&d.at);
            }
        }
        // A DEVICE THAT LEFT TAKES ITS MEMORY WITH IT.
        told.retain(|at| there.contains(at));
        self.bluetooth_alarmed = told;

        let mut raised = false;
        for (name, pct) in running_out {
            let n = self.radio_notification(BLUETOOTH_BATTERY_LOW, &name, pct);
            self.arrived(n, now);
            raised = true;
        }
        if raised {
            self.after_the_queue_moved();
        }
        said
    }

    /// One of the radio's three, in the shape the catalogue recognises.
    pub(super) fn radio_notification(
        &self,
        tag: &str,
        device: &str,
        percent: Option<u8>,
    ) -> ronoctua_notify::Notification {
        let body = self.texts.t_args(
            &format!("notif-{tag}-body"),
            &[
                ("device", ronoctua_i18n::Arg::Text(device)),
                (
                    "percent",
                    ronoctua_i18n::Arg::Number(i64::from(percent.unwrap_or_default())),
                ),
            ],
        );
        let summary = self.texts.t_args(
            &format!("notif-{tag}-summary"),
            &[
                ("device", ronoctua_i18n::Arg::Text(device)),
                (
                    "percent",
                    ronoctua_i18n::Arg::Number(i64::from(percent.unwrap_or_default())),
                ),
            ],
        );
        let mut n = ronoctua_notify::Notification::fresh_one(
            0,
            self.texts.t(crate::key::NOTIF_BLUETOOTH_APP),
            None,
            summary,
            body,
            ronoctua_notify::Urgency::Normal,
        );
        n.tags = vec![tag.to_owned()];
        n
    }

    /// What the radio's threads sent.
    pub(super) fn bluetooth_event(&mut self, e: ronoctua_bluetooth::Event) {
        use ronoctua_bluetooth::Event;
        match e {
            Event::Scene(scene) => self.bluetooth_arrived(*scene),
            Event::Done { ask, refused } => {
                if let Some((why, words)) = refused {
                    let said = crate::bluez_words::refused(&ask, why, &words, &self.texts);
                    self.echo_message(said);
                    self.draw_and_paint();
                }
            }
            // A QUESTION OPENS THE VIEW, because nothing pairs until it is
            // answered and a question nobody sees is a pairing that fails
            // sixty seconds later with no explanation.
            //
            // AND WHEN THE VIEW CANNOT OPEN — the file manager is in the
            // footer, holding a directory somebody navigated to — the
            // question goes to the NOTIFICATION instead, with its two
            // actions. It is the one path that neither steals the screen
            // nor loses the question.
            //
            // THE WINDOW HEARS IT TOO, from the file this writes
            // (`bluetooth::standing`), and answers it as this door does. When
            // the person is AT the window — `p` pressed there — the question
            // is the window's to take: this door neither opens its region nor
            // takes the keyboard, which is what would send them here to type
            // a PIN the window can take.
            Event::Asked(question) => {
                // A QUESTION THAT REPLACES ANOTHER takes the other's digits
                // with it, as the window's line goes: a PIN typed for the
                // one that ended is a PIN for nobody — and its `RET` would
                // have answered this one.
                if self.bluetooth_asked.as_ref() != Some(&question) {
                    self.bluetooth_let_go_of_the_digits();
                }
                self.bluetooth_asked = Some(question.clone());
                crate::bluez_words::standing::publish(Some(&question));
                let at_the_window = crate::bluez_words::has_the_person(
                    self.windows.focused().and_then(|w| w.app_id.as_deref()),
                );
                let id = RADIO;
                if !self.regions.is_open(id) && !at_the_window {
                    self.open_the_radio(None);
                }
                if self.regions.is_open(id) {
                    self.hosted_heard::<ronoctua_bluetooth::agent::Question>(&question);
                } else if !at_the_window {
                    let now = std::time::Instant::now();
                    let n = self.bluetooth_notification(&question);
                    self.arrived(n, now);
                    self.after_the_queue_moved();
                }
                self.draw_and_paint();
            }
            Event::Unasked => {
                self.bluetooth_unasked();
                self.draw_and_paint();
            }
        }
    }

    /// The pairing question, as a notification the catalogue recognises by
    /// its tag — the `power_notification` shape, and the plan's own answer to
    /// "a question that arrives while one is somewhere else".
    pub(super) fn bluetooth_notification(
        &self,
        question: &ronoctua_bluetooth::agent::Question,
    ) -> ronoctua_notify::Notification {
        use ronoctua_bluetooth::agent::Asked;
        let name = self
            .bluetooth_scene
            .device(&question.device)
            .map_or_else(|| question.device.clone(), |d| d.label.clone());
        let body = match &question.asked {
            Asked::Confirm(code) => self.texts.t_with(
                crate::key::BLUETOOTH_PAIRING_CONFIRM,
                &[("name", &name), ("code", code)],
            ),
            Asked::Passkey(code) => self.texts.t_with(
                crate::key::BLUETOOTH_PAIRING_PASSKEY,
                &[("name", &name), ("code", code)],
            ),
            Asked::Pin => self
                .texts
                .t_with(crate::key::BLUETOOTH_PAIRING_PIN, &[("name", &name)]),
            Asked::Typed => self.texts.t_with(
                crate::key::BLUETOOTH_PAIRING_PASSKEY_HERE,
                &[("name", &name)],
            ),
            Asked::Authorize => self
                .texts
                .t_with(crate::key::BLUETOOTH_PAIRING_AUTHORIZE, &[("name", &name)]),
            Asked::Service(uuid) => {
                let service = ronoctua_bluetooth::profile_key(uuid)
                    .map_or_else(|| uuid.clone(), |k| self.texts.t(k));
                self.texts.t_with(
                    crate::key::BLUETOOTH_PAIRING_SERVICE,
                    &[("name", &name), ("service", &service)],
                )
            }
        };
        // AND THE TWO KEYS, on a line of their own.
        //
        // The card lives twenty seconds (`long`, and there is no longer one)
        // while the agent holds the question for fifty-five — so for
        // thirty-five seconds the two BUTTONS are gone and the question is
        // not. The keys work for the whole time, and this is where somebody
        // reads them. They are arguments and never text in the `.ftl`, which
        // `keys-are-never-text` holds to.
        //
        // AND THEY ARE THE KEYS FROM WHERE A SEQUENCE STARTS: `s b` is the
        // radio itself in the leader's default set, so the answer is a tap of
        // `Ctrl` away — `taught_keys` says so, and not the full tree's
        // constant, which typed as it is would open the radio and stop.
        let leader = crate::bindings::label();
        let keys = |command: &str| {
            crate::bindings::taught_keys(self.tree, command, None)
                .map_or_else(String::new, |k| format!("{leader} {k}"))
        };
        let body = format!(
            "{body}\n{}",
            self.texts.t_with(
                crate::key::NOTIF_BLUETOOTH_PAIRING_KEYS,
                &[
                    ("yes", &keys("bluetooth-accept")),
                    ("no", &keys("bluetooth-refuse")),
                ],
            )
        );
        let mut n = ronoctua_notify::Notification::fresh_one(
            0,
            self.texts.t(crate::key::BLUETOOTH_TITLE),
            None,
            self.texts.t(crate::key::NOTIF_BLUETOOTH_PAIRING_SUMMARY),
            body,
            ronoctua_notify::Urgency::Critical,
        );
        n.tags = vec![BLUETOOTH_PAIRING.to_owned()];
        n
    }

    /// A fresh reading of the radio.
    ///
    /// The screen repaints only when the modeline's marks changed or the view
    /// is open: a scan changes something four times a second, and a frame for
    /// a reading that draws the same pixels is a frame wasted.
    pub(super) fn bluetooth_arrived(&mut self, scene: ronoctua_bluetooth::Scene) {
        // WHAT THE FOOTER HEARD, told to every region of the slot host first:
        // the radio's view follows the rows that are left.
        self.hosted_heard::<ronoctua_bluetooth::Scene>(&scene);
        let before = crate::bluez_words::modeline(&self.bluetooth_scene);
        self.bluetooth_scene = scene;
        let after = crate::bluez_words::modeline(&self.bluetooth_scene);
        let said = self.radio_alarms();
        // A QUESTION THAT ONLY TOLD ends when the device pairs: nothing
        // calls the agent back for those, and a sentence left on the screen
        // about a pairing that finished is a screen that stopped being true.
        //
        // IT IS ASKED OF `bluetooth_asked`, which is the question the agent
        // holds — on the band too, while the file manager is open and there
        // is no view: `DisplayPasskey` registers no answer channel, so no
        // timeout and no `Cancel` ever arrive, and a card left there told the
        // owner to type a code they had already typed. (The view's question
        // is always this one: it is told it from here, and taken off with it.)
        let paired = self
            .bluetooth_asked
            .as_ref()
            .and_then(|q| self.bluetooth_scene.device(&q.device))
            .is_some_and(|d| d.paired);
        if paired {
            // THE VIEW, THE FIELD AND THE BAND, in one call — the three places
            // a standing question lives.
            self.bluetooth_unasked();
        }
        if before != after || self.regions.is_open(RADIO) {
            // AN ARRIVAL CLAMPS; IT DOES NOT FOLLOW. The radio answers on its
            // own clock and moving the page under whoever is reading it is not
            // this reading's business — the same rule the network view's
            // arrivals keep, and `settle_region` writes down why.
            self.settle_region();
            self.apply_geometry(self.interactivity());
            self.draw_and_paint();
        }
        // A DEVICE COMING OR GOING, said once the view has settled.
        self.said_now(said);
    }

    /// The radio's view opened — by the radio's own question, or by a person
    /// asking for it — on the list, or on one device. When the region is held
    /// by another view a question STANDING GOES TO THE CARD: a window that
    /// closed with one up handed it here (`bluetooth-open`), and a refused
    /// view would leave it on no screen at all, waiting out the agent's
    /// minute.
    pub(super) fn open_the_radio(&mut self, at: Option<&str>) -> bool {
        let id = RADIO;
        let was_open = self.regions.is_open(id);
        let (command, argument) = match at {
            Some(at) => ("bluetooth-device", Some(at)),
            None => ("bluetooth-open", None),
        };
        let took = self.hosted_command(command, argument);
        // REFUSED — the region held by another view, or by this one for a
        // device asked with the view already open — and the question waits
        // on the band instead.
        if !self.regions.is_open(id) || (was_open && at.is_some()) {
            self.bluetooth_card_for_the_question();
        }
        took
    }

    /// The question is over — answered, taken back, or timed out.
    ///
    /// It leaves the view, the band AND the window's file: a critical card
    /// with two buttons that answer nothing is worse than no card at all.
    pub(super) fn bluetooth_unasked(&mut self) {
        self.bluetooth_asked = None;
        self.bluetooth_let_go_of_the_digits();
        crate::bluez_words::standing::publish(None);
        // AND THE VIEW, when it is open: the question leaves its rows.
        self.hosted_heard::<ronoctua_bluetooth::Event>(&ronoctua_bluetooth::Event::Unasked);
        let standing: Vec<u32> = self
            .notifications
            .history()
            .iter()
            .filter(|r| r.notification.tags.iter().any(|t| t == BLUETOOTH_PAIRING))
            .map(|r| r.notification.id)
            .collect();
        let now = std::time::Instant::now();
        let mut moved = false;
        for id in standing {
            moved |= self.notifications.dismiss(id, now);
        }
        if moved {
            self.after_the_queue_moved();
        }
    }

    /// An answer that came as a command — the footer's own line, a key, the
    /// card, or a window down the control socket — taken only when it is
    /// for the question standing (`bluetooth::Answer::fits`).
    pub(super) fn bluetooth_ordered(&mut self, yes: bool, argument: Option<&str>) {
        let answer = crate::bluez_words::Answer::read(argument);
        let fits = self
            .bluetooth_asked
            .as_ref()
            .is_some_and(|q| answer.fits(q));
        if !fits {
            // THE QUESTION IT WAS FOR IS OVER — ended, or replaced by
            // another the person has not seen yet. Answering the one standing
            // with it is exactly the defect; the one standing is on screen,
            // in every door, and waits for its own answer.
            tracing::info!("an answer for a pairing question that is over; not given");
            return;
        }
        self.bluetooth_answer(yes, answer.typed());
    }

    /// The pairing question standing, on a card of its own — when there is
    /// one standing and no card for it yet.
    pub(super) fn bluetooth_card_for_the_question(&mut self) {
        let Some(question) = self.bluetooth_asked.clone() else {
            return;
        };
        // UNREAD: one taken off already was the person's to take off, and
        // the question still waits.
        let carded = self
            .notifications
            .history()
            .iter()
            .any(|r| !r.read && r.notification.tags.iter().any(|t| t == BLUETOOTH_PAIRING));
        if carded {
            return;
        }
        let now = std::time::Instant::now();
        let n = self.bluetooth_notification(&question);
        self.arrived(n, now);
        self.after_the_queue_moved();
    }

    /// The minibuffer's line lets go of the digits it was typing, when it
    /// was typing them — the pairing question it asked for is over.
    pub(super) fn bluetooth_let_go_of_the_digits(&mut self) {
        if self.mx.let_go_of(crate::bluez_words::ACCEPT) {
            // AND THE REGION THAT ASKED THEM lets them go too.
            if self
                .region_asking
                .is_some_and(|(_, q)| q == crate::bluez_words::ACCEPT)
            {
                self.region_asking = None;
            }
            self.apply_geometry(self.interactivity());
        }
    }

    /// Answers a pairing question from anywhere — `LEADER s b y`/`s b n`,
    /// the notification's two actions, the view's answer in either door.
    ///
    /// STRAIGHT TO THE AGENT, never through the orders thread: `Pair` is
    /// blocking that thread while the question stands.
    ///
    /// AND FOR THE QUESTION THIS FOOTER HOLDS, named to the agent, which
    /// refuses it when another has taken its place in the meantime.
    pub(super) fn bluetooth_answer(&mut self, yes: bool, typed: &str) {
        // A YES WITH NOTHING TYPED, to a question that wants digits, is an
        // empty PIN sent to the other side — a pairing that fails for a
        // reason nobody could see. The view is opened on it instead, which
        // is where digits are typed, and the digits are asked on this
        // footer's line, masked, as the view asks them.
        if yes
            && typed.is_empty()
            && self
                .bluetooth_asked
                .as_ref()
                .is_some_and(|q| q.asked.typed())
        {
            if !self.regions.is_open(RADIO) {
                self.open_the_radio(None);
            }
            if let Some(q) = self.bluetooth_asked.clone() {
                self.hosted_heard::<ronoctua_bluetooth::agent::Question>(&q);
            }
            self.mx.request(crate::bluez_words::ACCEPT, true);
            self.apply_geometry(KeyboardInteractivity::Exclusive);
            self.draw_and_paint();
            return;
        }
        let asked = self.bluetooth_asked.take();
        match (&self.bluetooth_questions, &asked) {
            (Some(q), Some(asked)) => {
                if !q.answer(asked, yes, typed) {
                    tracing::info!("the pairing question answered was no longer waiting");
                }
            }
            // NOTHING ON SCREEN IS BEING ANSWERED: whatever the agent may
            // hold, nobody has been shown it yet.
            (Some(_), None) => {}
            (None, _) => {
                let said = self.texts.t(crate::key::BLUETOOTH_NO_DAEMON);
                self.echo_message(said);
            }
        }
        // THE VIEW, THE FIELD AND THE BAND.
        self.bluetooth_unasked();
        self.draw_and_paint();
    }

    /// One order for the radio's thread — or the sentence that there is
    /// none.
    fn bluetooth_order(&mut self, ask: ronoctua_bluetooth::Ask) {
        match &self.bluetooth_orders {
            Some(h) => h.send(ask),
            None => {
                let said = self.texts.t(crate::key::BLUETOOTH_NO_DAEMON);
                self.echo_message(said);
            }
        }
        self.draw_and_paint();
    }

    /// Connects everything trusted that is not connected.
    pub(super) fn bluetooth_reconnect(&mut self) {
        let ask: Vec<ronoctua_bluetooth::Ask> = self
            .bluetooth_scene
            .paired()
            .into_iter()
            .filter(|d| d.trusted && !d.connected && !d.blocked)
            .map(|d| ronoctua_bluetooth::Ask::Connect(d.at.clone()))
            .collect();
        for one in ask {
            self.bluetooth_order(one);
        }
    }

    /// Drops every link — the one gesture for "I am leaving the desk".
    pub(super) fn bluetooth_disconnect_all(&mut self) {
        let ask: Vec<ronoctua_bluetooth::Ask> = self
            .bluetooth_scene
            .connected()
            .into_iter()
            .map(|d| ronoctua_bluetooth::Ask::Disconnect(d.at.clone()))
            .collect();
        for one in ask {
            self.bluetooth_order(one);
        }
    }

    /// A key that acts on the RADIO with the view closed — `LEADER s b s`
    /// looks for devices from anywhere.
    ///
    /// ONLY THE RADIO'S THREE: scan, power and discoverable read the adapter
    /// and nothing else, so the radio's own answer (`bluez_words::
    /// adapter_order`, the one the open view's keys read too) is an order or
    /// nothing. A press that acts on the row under the cursor must NOT come
    /// through here: with the view open the press is the view's.
    fn bluetooth_errand_of(&mut self, key: crate::bluez_words::AdapterKey) {
        match crate::bluez_words::adapter_order(key, &self.bluetooth_scene) {
            Some(ask) => self.bluetooth_order(ask),
            None => {
                tracing::debug!(?key, "the radio's key asked nothing of the radio");
                self.draw_and_paint();
            }
        }
    }

    /// The radio's commands — the agent's two answers, what acts on the whole
    /// radio, and the radio's view opened with its question's card when the
    /// region is not free: `false` when `id` is none of them. Asked before the
    /// slot host (`Footer::run_with`): the view's own commands are the
    /// region's.
    pub(super) fn radio_commands(&mut self, id: &str, argument: Option<&str>) -> bool {
        let open = self.regions.is_open(RADIO);
        match id {
            "bluetooth-open" => {
                self.open_the_radio(None);
            }
            // A CLICK ON A MODELINE MARK carries the device's path, and
            // opens the view on it — the same screen `RET` opens.
            "bluetooth-device" if argument.is_some() => {
                self.open_the_radio(argument);
            }
            "bluetooth-scan" if !open => {
                self.bluetooth_errand_of(crate::bluez_words::AdapterKey::Scan);
            }
            "bluetooth-power" if !open => {
                self.bluetooth_errand_of(crate::bluez_words::AdapterKey::Power);
            }
            "bluetooth-discoverable" if !open => {
                self.bluetooth_errand_of(crate::bluez_words::AdapterKey::Discoverable);
            }
            "bluetooth-reconnect" => self.bluetooth_reconnect(),
            "bluetooth-disconnect-all" => self.bluetooth_disconnect_all(),
            // THE TWO ANSWERS. `bluetooth-accept` also carries the digits
            // when the other side asked for a PIN — this footer's minibuffer
            // sends them back as this command's argument, and the view's line
            // in either door down the registry: the agent is this process's.
            // An answer that NAMES a question is taken only while that
            // question stands (`bluetooth::Answer`).
            "bluetooth-accept" => {
                // THE DIGITS THE RADIO'S VIEW ASKED FOR come back here first
                // — the agent's answer is this file's — and the view's
                // question goes with them.
                if self
                    .region_asking
                    .is_some_and(|(_, q)| q == crate::bluez_words::ACCEPT)
                {
                    self.region_asking = None;
                }
                self.bluetooth_ordered(true, argument);
            }
            "bluetooth-refuse" => self.bluetooth_ordered(false, argument),
            _ => return false,
        }
        true
    }
}
