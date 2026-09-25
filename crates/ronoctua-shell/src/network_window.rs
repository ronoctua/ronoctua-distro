//! The network view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14), and this view has four sections: a glance at what is
//! connected fits the footer, reading every address, every resolver and every
//! network in range does not. So this exists — and everything it knows lives
//! in [`crate::network::View`], which the footer draws from too. What this
//! module holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the arrangement `power_window.rs` and `bluetooth_window.rs`
//! share, and the same header.
//!
//! # Where its data comes from
//!
//! From NetworkManager, on threads of its own (`ronoctua_network::watch` and
//! `watch_scan`): a window has no clock, and the bus says when something
//! changed. Orders go to the daemon directly — a radio, an aeroplane, a
//! tunnel are the same call from any process, and NetworkManager keeps no
//! per-client state.
//!
//! # Except a password, which is typed in the footer
//!
//! A window has no minibuffer. A secured network with no stored profile
//! needs a password typed, so here the row says where to type it instead of
//! opening a prompt that cannot exist — the arrangement the Bluetooth
//! window's pairing questions already have.

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::{self, channel},
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.network";

/// Opens the network view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // THE DAEMON, on a thread of this process — the bus is the clock.
        let (net_sender, net_changes) = channel::channel::<ronoctua_network::Snapshot>();
        if let Err(e) = ronoctua_network::watch(move |s| {
            let _ = net_sender.send(s);
        }) {
            tracing::warn!(error = %e, "no NetworkManager; the window opens with nothing to read");
        }
        w.handle
            .insert_source(net_changes, |event, (), door: &mut Door<Network>| {
                if let channel::Event::Msg(s) = event {
                    door.content.net_arrived(&mut door.ui, s);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the network channel: {e}"))?;

        // AND THE SCAN, which is a different clock: `RequestScan` returns at
        // once and the radio answers seconds later. Without this the list
        // would sit still until somebody pressed something.
        let (scan_sender, scans) = channel::channel::<()>();
        if let Err(e) = ronoctua_network::watch_scan(move || {
            let _ = scan_sender.send(());
        }) {
            tracing::debug!(error = %e, "no scan watcher; the list only rereads on request");
        }
        w.handle
            .insert_source(scans, |event, (), door: &mut Door<Network>| {
                if let channel::Event::Msg(()) = event {
                    door.content.wire.reread();
                    door.ui.ask_paint();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the scan channel: {e}"))?;

        // A CLOCK, AND IT IS HERE because this view has a section that
        // MEASURES: a rate is a difference over a time, and the bus's signals
        // do not arrive on a schedule. One second is the same coarse beat the
        // footer uses, for the same reason — the resolution that matters is
        // "it is moving", not the exact byte.
        w.handle
            .insert_source(
                calloop::timer::Timer::from_duration(std::time::Duration::from_secs(1)),
                |_, (), door: &mut Door<Network>| {
                    door.content.watch.sample();
                    door.content.settle(&door.ui);
                    door.ui.ask_paint();
                    calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the second: {e}"))?;

        let wire = crate::network::Wire::fresh();
        let net = wire.look();
        let mut watch = crate::network::Watch::default();
        watch.sample();

        // THE LOCAL NETWORK'S SWEEP, a thread per press — see the footer's own.
        let (lan_sender, lan_answers) = channel::channel::<ronoctua_lan::Found>();
        w.handle
            .insert_source(lan_answers, |event, (), door: &mut Door<Network>| {
                if let channel::Event::Msg(found) = event {
                    door.content.watch.found(found);
                    door.content.settle(&door.ui);
                    door.ui.ask_paint();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the sweep channel: {e}"))?;

        // AND THE TWO TUNNELS, on this window's own thread — the same
        // arrangement as the footer's, and for the reason the module header
        // gives: neither daemon signals, so somebody has to ask.
        let (tunnel_sender, tunnel_events) = channel::channel::<ronoctua_tunnel::Event>();
        let hand = match ronoctua_tunnel::watch(
            // `systemctl` AND THE FRAGMENT DIRECTORY ARE IN `Places` NOW. They
            // used to be a second parameter and a constant; both are answers
            // to "where is it", which is what `Places` is, and Tor needed to
            // be able to ask systemd where the unit stands before it could
            // offer to start it.
            ronoctua_tunnel::Places::default(),
            move |e| {
                let _ = tunnel_sender.send(e);
            },
        ) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "no tunnel thread; the two sections stay empty");
                None
            }
        };
        w.handle
            .insert_source(tunnel_events, |event, (), door: &mut Door<Network>| {
                if let channel::Event::Msg(e) = event {
                    door.content.tunnel_event(&mut door.ui, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the tunnel channel: {e}"))?;

        Ok(Network {
            view: crate::network::View::fresh(),
            wire,
            net,
            watch,
            lan_sender: Some(lan_sender),
            tunnels: ronoctua_tunnel::Scene::default(),
            hand,
        })
    })
}

/// The window: the view, and what it reads.
struct Network {
    view: crate::network::View,
    /// This window's own telephone to the daemon, and the access points it
    /// has seen.
    wire: crate::network::Wire,
    /// The last reading, from this process's own thread.
    net: ronoctua_network::Snapshot,
    /// What `/proc` says: the sockets and the interface counters, sampled on
    /// this window's own second.
    watch: crate::network::Watch,
    /// Where a finished sweep of the local network reports.
    lan_sender: Option<channel::Sender<ronoctua_lan::Found>>,
    /// What the two tunnels say, from this window's own thread.
    tunnels: ronoctua_tunnel::Scene,
    /// The thread that reads them and carries out their orders.
    hand: Option<ronoctua_tunnel::Hand>,
}

/// What this window keeps for ONE pointer, past what every door keeps.
#[derive(Debug, Default)]
struct Shelving {
    /// WHICH shelf's bar this pointer is dragging, when it is one.
    ///
    /// A shelf's bar is drawn at its CONTAINER's right edge and the list's at
    /// the surface's (`chrome::shelf_box`), so a press has to reach the bar
    /// that was drawn at that HEIGHT. Without this the shelf's thumb was
    /// visible, and pressing it dragged the whole list.
    dragging_shelf: Option<crate::network::Shelf>,
}

impl Network {
    /// A fresh reading of the daemon's whole state.
    fn net_arrived(&mut self, ui: &mut Ui, s: ronoctua_network::Snapshot) {
        // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: a cable plugged
        // in brings six facts, a tunnel coming up brings a row, and a cursor
        // past the end focuses nothing.
        self.net = s;
        self.settle(ui);
        ui.ask_paint();
    }

    /// A fresh reading of the two tunnels, or the end of an order.
    fn tunnel_event(&mut self, ui: &mut Ui, e: ronoctua_tunnel::Event) {
        match e {
            ronoctua_tunnel::Event::Scene(scene) => {
                self.tunnels = *scene;
                self.settle(ui);
                ui.ask_paint();
            }
            ronoctua_tunnel::Event::Done { refused, .. } => {
                if let Some(words) = refused {
                    ui.message = Some(ui.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_REFUSED,
                        &[("error", &words)],
                    ));
                    ui.ask_paint();
                }
            }
            // THE LOGIN'S PAGE, opened with whatever the person's desktop
            // opens links with — this window has no browser of its own.
            ronoctua_tunnel::Event::Login(url) => {
                let _ = std::process::Command::new(crate::network::OPEN)
                    .arg(&url)
                    .spawn();
                ui.message = Some(
                    ui.texts
                        .t_with(ronoctua_i18n::key::ECHO_NETWORK_LOGIN, &[("url", &url)]),
                );
                ui.ask_paint();
            }
        }
    }

    /// The view settles on the rows as they are now. An ARRIVAL does not
    /// follow the cursor: only a key or a click moves the page.
    fn settle(&mut self, ui: &Ui) {
        let rows = self.view_rows(ui);
        self.view.settle(&rows);
    }

    /// Carries out what the view asked for.
    ///
    /// EVERY ORDER RE-READS: the watch thread would bring the same reading a
    /// moment later, and this is what makes the screen answer the key that
    /// was just pressed rather than a quarter of a second afterwards.
    fn apply(&mut self, ui: &mut Ui, errand: crate::network::Errand) {
        use crate::network::Errand;
        use ronoctua_i18n::key;
        let refused = |e: &anyhow::Error, texts: &ronoctua_i18n::Texts| {
            texts.t_with(key::ECHO_NETWORK_REFUSED, &[("error", &e.to_string())])
        };
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
            // A PATH IS TYPED IN THE MINIBUFFER, which this window has not
            // got — the pairing question's own arrangement, and its reason.
            Errand::Ask => {
                ui.message = Some(ui.texts.t(ronoctua_i18n::key::NETWORK_ANSWER_IN_THE_FOOTER));
            }
            Errand::Bring(path, kind) => {
                let name = ronoctua_network::vpn::name_of(&path);
                ui.message = Some(match self.wire.import(&path, kind) {
                    Ok(()) => ui
                        .texts
                        .t_with(key::ECHO_NETWORK_IMPORTED, &[("network", &name)]),
                    Err(e) => ui.texts.t_with(
                        key::ECHO_NETWORK_IMPORT_FAILED,
                        &[("file", &name), ("reason", &e.to_string())],
                    ),
                });
            }
            // BECOMING THE NETWORK, from this door too: NetworkManager keeps
            // no per-client state, so the second telephone is not a second
            // truth.
            Errand::Hotspot(true) => {
                let name = crate::network::hostname();
                ui.message = Some(
                    match ronoctua_network::hotspot_password()
                        .and_then(|word| self.wire.hotspot(&name, &word).map(|()| word))
                    {
                        Ok(word) => {
                            self.view.sharing(Some(word.clone()));
                            ui.texts.t_with(
                                key::ECHO_NETWORK_HOTSPOT,
                                &[("network", &name), ("password", &word)],
                            )
                        }
                        Err(e) => ui.texts.t_with(
                            key::ECHO_NETWORK_HOTSPOT_FAILED,
                            &[("reason", &e.to_string())],
                        ),
                    },
                );
            }
            Errand::Hotspot(false) => {
                self.view.sharing(None);
                ui.message = Some(
                    ui.texts.t(
                        if self
                            .wire
                            .disconnect(ronoctua_network::Medium::Radio)
                            .is_ok()
                        {
                            key::ECHO_NETWORK_DISCONNECTED
                        } else {
                            key::ECHO_NETWORK_UNAVAILABLE
                        },
                    ),
                );
            }
            Errand::Scan => self.wire.scan(),
            // THE SWEEP GOES ON A THREAD HERE TOO, and this window's own:
            // it takes a second, and a window that froze for it would be a
            // window whose title bar stopped answering.
            Errand::Discover => {
                self.watch.read_vendors();
                let address = crate::network::carrying(&self.net)
                    .and_then(|l| l.address.clone())
                    .or_else(|| self.net.links.iter().find_map(|l| l.address.clone()));
                match (address, self.lan_sender.clone()) {
                    (Some(address), Some(echo)) => {
                        self.watch.sweeping(true);
                        if let Err(e) = std::thread::Builder::new()
                            .name("lan-sweep".to_owned())
                            .spawn(move || {
                                let found = ronoctua_lan::sweep(&address).unwrap_or_default();
                                let _ = echo.send(found);
                            })
                        {
                            tracing::warn!(error = %e, "no thread for the sweep");
                            self.watch.sweeping(false);
                        }
                    }
                    _ => ui.message = Some(ui.texts.t(key::ECHO_NETWORK_NO_LINK)),
                }
            }
            Errand::Radio(on) => {
                ui.message = Some(ui.texts.t(match self.wire.radio(on) {
                    Ok(()) if on => key::ECHO_NETWORK_RADIO_ON,
                    Ok(()) => key::ECHO_NETWORK_RADIO_OFF,
                    Err(_) => key::ECHO_NETWORK_UNAVAILABLE,
                }));
            }
            Errand::Airplane(on) => {
                if let Err(e) = self.wire.airplane(on) {
                    ui.message = Some(refused(&e, &ui.texts));
                }
            }
            Errand::Disconnect(medium) => {
                ui.message = Some(ui.texts.t(if self.wire.disconnect(medium).is_ok() {
                    key::ECHO_NETWORK_DISCONNECTED
                } else {
                    key::ECHO_NETWORK_UNAVAILABLE
                }));
            }
            Errand::Facts => {
                ui.message = Some(match crate::network::carrying(&self.net) {
                    Some(l) if l.medium == ronoctua_network::Medium::Wired => ui.texts.t_with(
                        key::ECHO_NETWORK_LINK_WIRED,
                        &[
                            ("network", &l.interface),
                            ("rate", &crate::network::rate_of(l, &ui.texts)),
                        ],
                    ),
                    Some(l) => ui.texts.t_with(
                        key::ECHO_NETWORK_LINK,
                        &[
                            ("network", l.network.as_deref().unwrap_or(&l.interface)),
                            ("band", &crate::network::band_text(l.band, &ui.texts)),
                            ("rate", &crate::network::rate_of(l, &ui.texts)),
                            ("signal", &format!("{}%", l.signal)),
                        ],
                    ),
                    None => ui.texts.t(key::ECHO_NETWORK_NO_LINK),
                });
            }
            Errand::Forget(ssid) => {
                ui.message = Some(if self.wire.forget(&ssid).is_ok() {
                    ui.texts
                        .t_with(key::ECHO_NETWORK_FORGOTTEN, &[("network", &ssid)])
                } else {
                    ui.texts.t(key::ECHO_NETWORK_UNAVAILABLE)
                });
            }
            // A PASSWORD IS TYPED IN THE FOOTER, which this window has not
            // got. A network that is saved or open joins from here; the one
            // that needs typing says where, rather than looking broken.
            Errand::Connect(target) => {
                if target.secured && !target.saved {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::NETWORK_ANSWER_IN_THE_FOOTER));
                } else {
                    ui.message = Some(match self.wire.connect(&target.ssid, None, false) {
                        Ok(()) => ui
                            .texts
                            .t_with(key::ECHO_NETWORK_CONNECTED, &[("network", &target.ssid)]),
                        Err(e) => ui.texts.t_with(
                            key::ECHO_NETWORK_FAILED,
                            &[("network", &target.ssid), ("reason", &e.to_string())],
                        ),
                    });
                }
            }
            Errand::Drop(at) => {
                let name = name_of(
                    self.net.active.iter().find(|a| a.at == at).map(|a| &a.id),
                    &at,
                );
                ui.message = Some(match self.wire.drop_it(&at) {
                    Ok(()) => ui
                        .texts
                        .t_with(key::ECHO_NETWORK_TUNNEL_DOWN, &[("network", &name)]),
                    Err(e) => refused(&e, &ui.texts),
                });
            }
            Errand::Raise(at) => {
                let name = name_of(
                    self.net.saved.iter().find(|s| s.at == at).map(|s| &s.id),
                    &at,
                );
                ui.message = Some(match self.wire.raise(&at) {
                    Ok(()) => ui
                        .texts
                        .t_with(key::ECHO_NETWORK_TUNNEL_UP, &[("network", &name)]),
                    Err(e) => refused(&e, &ui.texts),
                });
            }
            // THE TUNNEL ORDERS GO TO THIS WINDOW'S OWN THREAD: neither
            // daemon keeps per-client state, so a second telephone to them
            // is not a second truth.
            Errand::Tunnel(order) => match self.hand.as_ref() {
                Some(h) => h.send(order),
                None => {
                    ui.message = Some(ui.texts.t(key::ECHO_NETWORK_UNAVAILABLE));
                }
            },
            // TOR'S SETTINGS GO THROUGH THE SAME THREAD, and the sentence
            // afterwards is not the same one: a fragment written while Tor is
            // stopped is written and NOT reloaded, which is the ordinary way
            // to configure bridges before the first start. Saying "written"
            // when nothing was told about it would be the façade this project
            // undoes.
            Errand::TorConfigure(lines) => {
                let running = matches!(
                    self.tunnels.standing,
                    ronoctua_tunnel::tor::Standing::Running
                );
                match self.hand.as_ref() {
                    Some(h) => {
                        h.send(ronoctua_tunnel::Order::Configure(lines));
                        ui.message = Some(ui.texts.t(if running {
                            key::ECHO_TOR_WRITTEN
                        } else {
                            key::ECHO_TOR_NOT_RUNNING
                        }));
                    }
                    None => {
                        ui.message = Some(ui.texts.t(key::ECHO_NETWORK_UNAVAILABLE));
                    }
                }
            }
            Errand::TorAutostart(on) => match self.hand.as_ref() {
                Some(h) => h.send(ronoctua_tunnel::Order::Autostart(on)),
                None => {
                    ui.message = Some(ui.texts.t(key::ECHO_NETWORK_UNAVAILABLE));
                }
            },
            // THE ONE THING THIS DOOR CANNOT DO IS TYPE. The minibuffer is the
            // footer's, and this window has none — so it says where to go,
            // which is what it already does for a Wi-Fi password and a VPN
            // path.
            Errand::TorEdit(_) => {
                ui.message = Some(ui.texts.t(key::NETWORK_ANSWER_IN_THE_FOOTER));
            }
            Errand::Delete(at) => {
                let name = name_of(
                    self.net.saved.iter().find(|s| s.at == at).map(|s| &s.id),
                    &at,
                );
                // ONE KEY, ONE IDEA — the same rule the footer's door states:
                // `f` on a Wi-Fi row and `f` on the same network in the
                // remembered shelf are one act, and the profile's kind is what
                // picks the sentence.
                let saying = if self
                    .net
                    .saved
                    .iter()
                    .find(|s| s.at == at)
                    .is_some_and(|s| s.kind == crate::network::WIRELESS)
                {
                    key::ECHO_NETWORK_FORGOTTEN
                } else {
                    key::ECHO_NETWORK_DELETED
                };
                ui.message = Some(match self.wire.delete(&at) {
                    Ok(()) => ui.texts.t_with(saying, &[("network", &name)]),
                    Err(e) => refused(&e, &ui.texts),
                });
            }
        }
        self.net = self.wire.look();
        let rows = self.view_rows(ui);
        self.view.settle(&rows);
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::network::Press) {
        ui.message = None;
        let errand = {
            let mut view = std::mem::take(&mut self.view);
            let errand = view.key(press, &self.facts(), ui.columns, &ui.texts);
            self.view = view;
            errand
        };
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::network::Row> {
        self.view.rows(&self.facts(), ui.columns, &ui.texts)
    }

    /// Everything the rows are drawn from — see `network::Facts`.
    fn facts(&self) -> crate::network::Facts<'_> {
        crate::network::Facts {
            net: &self.net,
            seen: self.wire.seen(),
            watch: &self.watch,
            tunnels: &self.tunnels,
        }
    }

    /// The box every container of this view is drawn in.
    ///
    /// ONE BOX FOR ALL OF THEM, and it comes from the same `extent` the rows
    /// were built with — a ground measured from a different width than the
    /// text inside it is a ground that cuts its own rows.
    fn shelf_box(ui: &Ui) -> crate::chrome::ShelfBox {
        let (left, room) = crate::network::extent(ui.columns);
        crate::chrome::shelf_box(left, room)
    }

    /// Every shelf whose band is WHOLLY on screen and does not fit — the ones
    /// that draw a bar — and the line it starts on.
    ///
    /// The same filter the drawing uses, so what can be grabbed is exactly
    /// what was drawn — see `chrome::on_the_band` for why a band that is half
    /// out gets no bar.
    fn bands(&self, ui: &Ui, rows: &[crate::network::Row]) -> Vec<(u16, crate::network::Band)> {
        let room = ui.room_for_rows();
        self.view
            .shelves(rows)
            .into_iter()
            .filter(|b| b.total > b.lines)
            .filter_map(|b| {
                let (from, to) = (b.first, b.first + b.lines);
                if from < ui.offset || to > ui.offset + room {
                    return None;
                }
                let line = u16::try_from(from - ui.offset).ok()?;
                Some((ui.first_of_list() + line, b))
            })
            .collect()
    }

    /// The shelf whose BAR is under `(x, y)`, if any.
    fn shelf_bar_at(
        &self,
        ui: &Ui,
        rows: &[crate::network::Row],
        x: f32,
        y: f32,
    ) -> Option<crate::network::Band> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let box_of = Self::shelf_box(ui);
        self.bands(ui, rows).into_iter().find_map(|(line, b)| {
            let inner = crate::chrome::list_scroll(b.total, b.lines, b.offset);
            crate::chrome::on_the_band(inner, box_of, line, w, h, x, y).then_some(b)
        })
    }

    /// Where a drag of that shelf's bar puts it.
    fn shelf_offset_at(
        &self,
        ui: &Ui,
        rows: &[crate::network::Row],
        which: crate::network::Shelf,
        y: f32,
    ) -> Option<usize> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        self.bands(ui, rows).into_iter().find_map(|(line, b)| {
            (b.shelf == which).then(|| {
                let inner = crate::chrome::list_scroll(b.total, b.lines, b.offset);
                crate::chrome::shelf_offset_at(inner, line, w, h, y)
            })
        })
    }
}

impl Content for Network {
    type Hand = Shelving;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::NETWORK_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        if let Some(press) = press_of(event) {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click
    /// puts the cursor on the row, and a click on the row it is already on is
    /// that row's `RET`, through the same `pressed` a key goes through.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::network::Press::Enter);
        }
    }

    /// The standard list, with the SHELVES' bars taken first.
    ///
    /// A SHELF'S BAR FIRST: it is drawn at its container's right edge and the
    /// list's at the surface's, so the two are different columns — but the
    /// list's TRACK runs the whole height, so asking it first would let it
    /// answer for a press on a container's thumb and drag the wrong list.
    ///
    /// THE WHEEL IS THE PAGE'S. A shelf took it, on the manager's rule that
    /// the gesture scrolls the section under the hand — but the manager's
    /// sections are COLUMNS side by side and a shelf is a band inside one
    /// list, so with five of them the page stopped scrolling almost
    /// everywhere. A shelf has its own bar, its own column and the arrows at
    /// its edge — so the wheel is left to the standard list.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Shelving>, events: &[PointerEvent]) {
        // THE ROWS ARE ONLY BUILT WHEN A SHELF'S BAR CAN BE IN PLAY: a
        // `Motion` arrives on every pixel of movement. AND A SHELF'S DRAG
        // COUNTS AS ONE — without it every Motion frame of the drag found no
        // rows and no band, and the shelf froze under a held thumb.
        let in_play = hand.extra.dragging_shelf.is_some()
            || events.iter().any(|e| {
                matches!(e.kind, PointerEventKind::Press { button, .. } if button == LEFT_BUTTON)
            });
        let rows = if in_play {
            self.view_rows(ui)
        } else {
            Vec::new()
        };
        let mut shelved: Option<(crate::network::Shelf, usize)> = None;
        let mut rest: Vec<PointerEvent> = Vec::with_capacity(events.len());
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if let Some(which) = hand.extra.dragging_shelf {
                        shelved = self
                            .shelf_offset_at(ui, &rows, which, py)
                            .map(|at| (which, at));
                        continue;
                    }
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if let Some(band) = self.shelf_bar_at(ui, &rows, px, py) {
                        hand.extra.dragging_shelf = Some(band.shelf);
                        shelved = self
                            .shelf_offset_at(ui, &rows, band.shelf, py)
                            .map(|at| (band.shelf, at));
                        continue;
                    }
                }
                // AND THE DRAG ENDS WITH THE BUTTON, or with the pointer: a
                // grab broken by an unmap drops the release.
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    hand.extra.dragging_shelf = None;
                }
                PointerEventKind::Leave { .. } => hand.extra.dragging_shelf = None,
                _ => {}
            }
            rest.push(e.clone());
        }
        if let Some((which, at)) = shelved {
            self.view.put_shelf(which, at);
            ui.ask_paint();
        }
        if !rest.is_empty() {
            crate::door::list::pointer(self, ui, hand, &rest);
        }
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        crate::door::list::header(self, ui, canvas);
        let rows = self.view_rows(ui);
        let room = ui.room_for_rows();
        // A VIEW MAY ASK FOR A LINE AT THE TOP — see
        // `network::View::take_anchor`. `TAB` lands on a section and has to
        // SHOW it, and keeping the cursor visible is satisfied by the
        // section's title being off the screen above it. Honoured ONCE.
        if let Some(top) = self.view.take_anchor() {
            ui.offset = top.min(rows.len().saturating_sub(room));
        }
        ui.settle_offset(rows.len());
        let first = ui.first_of_list();
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let cell = ui.frame(0).cell();
        let box_of = Self::shelf_box(ui);
        let shelves = self.view.shelves(&rows);
        // EVERY CONTAINER ON SCREEN GETS ITS GROUND, and it goes in FIRST —
        // before the page's bar and before every other rectangle. Quads draw
        // in order and there is no depth test, so a ground pushed after the
        // page's bar paints the page's THUMB out of existence wherever a
        // container is on screen, which is what the owner read off the
        // screen: *"ela acaba desaparecendo e mudando de tamanho"*. The box
        // (`chrome::shelf_box`) now makes that impossible by geometry as well;
        // the order is the second lock, and the footer's door has both.
        for band in &shelves {
            let (from, to) = (band.first, band.first + band.lines);
            let seen_from = from.max(ui.offset);
            let seen_to = to.min(ui.offset + room);
            if seen_to > seen_from
                && let Ok(line) = u16::try_from(seen_from - ui.offset)
            {
                canvas.under.push(crate::chrome::shelf_ground(
                    box_of,
                    first + line,
                    seen_to - seen_from,
                    w,
                    h,
                ));
            }
        }
        // THE LIST, WITHOUT ITS HOVER — drawn below, cut to the shelves.
        let fields: Vec<Vec<crate::files::Field>> = rows.iter().map(|r| r.fields.clone()).collect();
        let lit = canvas.lit;
        canvas.lit = &[];
        crate::door::list::body(ui, canvas, &fields);
        canvas.lit = lit;
        // EVERY CURSOR'S HOVER, in its own colour — see `crate::seats`. A row
        // on the page stops at the PAGE's bar (`Room::band_width`); a row
        // inside a shelf is CUT to the shelf's box — the band says which line
        // of THAT list is under the pointer, and a band past the box would
        // paint over the page — and stops at the SHELF's bar while one is
        // drawn: a scrollbar's column is its own while it is drawn.
        let barred = self.bands(ui, &rows);
        for &(color, row) in lit {
            let Some(row) = row else {
                continue;
            };
            let shelf = shelves
                .iter()
                .find(|s| (s.first..s.first + s.lines).contains(&row));
            for mut band in crate::chrome::hover_bands(
                std::iter::once((color, Some(row))),
                ui.offset,
                room,
                rows.len(),
                first,
                ui.room.band_width(),
            ) {
                if let Some(s) = shelf {
                    let own_bar = barred.iter().any(|(_, b)| b.shelf == s.shelf);
                    let cells = box_of.cells.saturating_sub(u16::from(own_bar));
                    band.x = f32::from(box_of.from) * cell;
                    band.width = f32::from(cells) * cell;
                }
                canvas.under.push(band);
            }
        }
        // AND A CONTAINER'S OWN BAR, when the whole band is on screen — for
        // the reason the footer's door writes down: a track shorter than its
        // band would move the list by a different amount than the drag.
        for (line, band) in barred {
            let inner = crate::chrome::list_scroll(band.total, band.lines, band.offset);
            if let Some(q) = crate::chrome::shelf_bar(inner, box_of, line, w, h) {
                // THE WHOLE TRACK is the bar's column, not only the thumb:
                // from the box's last cell, over every line of the shelf.
                let f = ui.frame(line);
                let track = crate::bars::Track::of((
                    q.x,
                    f.y_of(line),
                    q.width,
                    f.line() * f32::from(u16::try_from(band.lines).unwrap_or(u16::MAX)),
                ));
                canvas.bar(Some(track));
                canvas.under.push(q);
            }
        }
    }
}

/// The name of a thing, or its path when the daemon has forgotten it between
/// the keypress and the sentence.
fn name_of(found: Option<&String>, at: &str) -> String {
    found.cloned().unwrap_or_else(|| at.to_owned())
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent) -> Option<crate::network::Press> {
    use crate::network::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        // `ISO_Left_Tab` IS `S-TAB`: xkb has already applied the shift, and a
        // door that tested for a modifier would never see one.
        Keysym::Tab => Press::Section,
        Keysym::ISO_Left_Tab => Press::SectionBack,
        // AND THE BARE LETTERS, out of the view's own table — the footer
        // reads it the same way, so the two doors cannot teach different
        // keys. A letter with no press there is a letter that does nothing,
        // and the test beside `network_words` is what says so.
        _ => {
            let typed = event.utf8.as_deref()?;
            let id = crate::network::OF_THE_VIEW
                .iter()
                .find(|(notation, _, _)| *notation == typed)
                .map(|(_, id, _)| *id)?;
            crate::network_words::press_of(id)?
        }
    })
}
