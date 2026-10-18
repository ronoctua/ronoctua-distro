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
//! # And what is typed, it types here
//!
//! A password, a tunnel's path and a Tor value are asked on this window's own
//! header (`door::Ui::ask`) — the password hidden — and the answer goes to the
//! same `network::View::answered` the footer's minibuffer reaches, pasted
//! into or typed. The password never leaves this process except to
//! NetworkManager: not to the footer, not to an `argv`, not to a log. The
//! question afterwards, whether to keep it, is the view's own screen, and the
//! password waits for its answer here ([`Network::remember`]).

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Ui};
use anyhow::{Context as _, Result};
use smithay_client_toolkit::{
    reexports::calloop::{self, channel},
    seat::{
        keyboard::{KeyEvent, Modifiers},
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
                    door.content.ask_names();
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

        // THE LOCAL NETWORK'S SLOW WORK — the sweep, the names, the
        // gateway's round trip — a thread per piece, and its answer here
        // (`network::spawn`, the footer's own path).
        let (lan_sender, lan_answers) = channel::channel::<crate::network::Heard>();
        w.handle
            .insert_source(lan_answers, |event, (), door: &mut Door<Network>| {
                if let channel::Event::Msg(heard) = event {
                    door.content.watch.heard(heard);
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

        // THE FILE PICKER'S FOLDER, watched through the door's wake.
        let mut view = crate::network::View::fresh();
        view.watch_with(w.wake());
        // A HOTSPOT ALREADY UP shows its password from the first frame,
        // whichever door started it.
        let mut hotspot_serving = false;
        view.show_sharing(crate::link_words::sharing::follow(
            &mut hotspot_serving,
            &net,
        ));
        Ok(Network {
            view,
            wire,
            net,
            watch,
            lan_sender: Some(lan_sender),
            tunnels: ronoctua_tunnel::Scene::default(),
            hand,
            remember: None,
            hotspot_serving,
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
    /// Where the view's threads report — a sweep, the names, a measurement.
    lan_sender: Option<channel::Sender<crate::network::Heard>>,
    /// What the two tunnels say, from this window's own thread.
    tunnels: ronoctua_tunnel::Scene,
    /// The thread that reads them and carries out their orders.
    hand: Option<ronoctua_tunnel::Hand>,
    /// The network just joined with a typed password, and the password —
    /// held for the span of one question ("keep it?") and TAKEN on either
    /// answer: a password left behind would be a password in memory with no
    /// owner and no deadline.
    remember: Option<(String, crate::network::Password)>,
    /// Was a hotspot up at the last reading? Its END takes the password's
    /// file away (`link_words::sharing::follow`); the view only reads it.
    hotspot_serving: bool,
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
        // THE HOTSPOT'S END takes the password's file away; the view only
        // reads it (`network::View::show_sharing`).
        self.view.show_sharing(crate::link_words::sharing::follow(
            &mut self.hotspot_serving,
            &self.net,
        ));
        self.settle(ui);
        ui.ask_paint();
    }

    /// Asks resolved for the names of the addresses on screen that have
    /// none yet — a batch on a thread, never on this loop.
    fn ask_names(&mut self) {
        if let (Some(list), Some(back)) = (self.watch.names_wanted(), self.lan_sender.clone()) {
            crate::network::spawn(crate::network::Lan::Name(list), move |h| {
                let _ = back.send(h);
            });
        }
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
                if let Some(r) = refused {
                    ui.say(crate::link_words::tunnel_refused(&r, &ui.texts));
                    ui.ask_paint();
                }
            }
            // THE LOGIN'S PAGE, opened with whatever the person's desktop
            // opens links with — this window has no browser of its own — in a
            // scope, off the loop, as the footer opens it; a start that fails
            // replaces this sentence with the one that says so.
            ronoctua_tunnel::Event::Login(url) => {
                ui.launch(crate::link_words::OPEN, std::slice::from_ref(&url), None);
                ui.say_for(
                    ui.texts
                        .t_with(crate::key::ECHO_NETWORK_LOGIN, &[("url", &url)]),
                    crate::echo::Echo::Long,
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

    /// Carries out what the view asked for — through the one handler the
    /// footer's region carries it out with too (`network_region::carry_out`),
    /// with this window's own telephone to the daemon, its own threads and its
    /// own line.
    fn apply(&mut self, ui: &mut Ui, errand: crate::network::Errand) {
        let lan = self.lan_sender.clone();
        let back = move || -> crate::network_region::Back {
            let lan = lan.clone();
            Box::new(move |h| {
                if let Some(s) = lan {
                    let _ = s.send(h);
                }
            })
        };
        let lines = crate::network_region::Lines {
            lan: self
                .lan_sender
                .is_some()
                .then_some(&back as &dyn Fn() -> crate::network_region::Back),
            tunnels: self.hand.as_ref(),
            // THIS WINDOW FOLLOWS THE SCANS FOR ITS WHOLE LIFE already (its
            // own watcher, raised with it).
            scans: None,
        };
        let mut held = crate::network_region::Held {
            view: &mut self.view,
            wire: &mut self.wire,
            net: &mut self.net,
            watch: &mut self.watch,
            tunnels: &self.tunnels,
            remember: &mut self.remember,
        };
        crate::network_region::carry_out(errand, &mut held, &lines, ui);
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::network::Press) {
        ui.message = None;
        // A PRESS THAT REACHES THE VIEW WHILE THE LINE ASKS came from a click
        // — every key is the line's — and the view lets go of what it asked
        // on any press it hears. So the line goes with it: left up, it would
        // take a password for a question nobody is asking any more.
        if ui
            .asking
            .as_ref()
            .is_some_and(|l| crate::network::Question::is_the_views(l.id))
        {
            ui.asking = None;
        }
        let errand = {
            let mut view = std::mem::take(&mut self.view);
            view.page_is(ui.room_for_rows());
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
        let (left, room) = crate::chrome::extent(ui.columns);
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

/// The window over a PLANTED machine, for the photograph
/// (its camera, `camera`): no daemon, no thread, no kernel read — the rows the
/// scene asks for, and the question its line opens, if any.
///
/// - `lan`: a cable and a radio up, the neighbours of both families with the
///   names resolved gave, and the router's round trip measured;
/// - `hotspot`: the radio serving, with the password both doors read;
/// - `vpn`: an imported `.ovpn` that asks for a user — the line open on it.
///
/// # Errors
///
/// A scene this door does not know.
pub fn photographed(scene: &str) -> Result<(impl Content, Option<crate::line::Line>)> {
    use ronoctua_network::{Band, Link, Medium, Snapshot, State, Way};
    let link = |medium: Medium, interface: &str, serving: bool| Link {
        medium,
        interface: interface.to_owned(),
        network: (medium == Medium::Radio).then(|| "noctua-5g".to_owned()),
        state: State::Connected,
        rate_kbps: if medium == Medium::Wired {
            1_000_000
        } else {
            866_000
        },
        signal: 78,
        band: Band::Ghz5,
        address: Some(
            if medium == Medium::Wired {
                "192.168.1.20/24"
            } else {
                "192.168.1.21/24"
            }
            .to_owned(),
        ),
        gateway: Some("192.168.1.1".to_owned()),
        dns: vec!["192.168.1.1".to_owned()],
        serving,
    };
    let net = Snapshot {
        daemon: true,
        networking: true,
        radio: true,
        way: Way::Full,
        links: if scene == "phone" {
            // A PHONE LENDING ITS CONNECTION over Bluetooth: the link is its
            // IP side, `bnep0`, the name the Bluetooth view gives it too.
            vec![Link {
                medium: Medium::Phone,
                interface: "bnep0".to_owned(),
                network: Some("Pixel 8".to_owned()),
                rate_kbps: 0,
                address: Some("192.168.44.17/24".to_owned()),
                gateway: Some("192.168.44.1".to_owned()),
                dns: vec!["192.168.44.1".to_owned()],
                ..link(Medium::Phone, "bnep0", false)
            }]
        } else {
            vec![
                link(Medium::Wired, "enp1s0", false),
                link(Medium::Radio, "wlan0", scene == "hotspot"),
            ]
        },
        active: Vec::new(),
        saved: Vec::new(),
    };
    let mut watch = crate::network::Watch::default();
    let mut view = crate::network::View::fresh();
    let mut asking = None;
    match scene {
        "lan" => {
            let v4 = |last: u8, hardware: &str| ronoctua_memory::net::Neighbour {
                address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, last)),
                hardware: hardware.to_owned(),
                interface: "wlan0".to_owned(),
                fresh: true,
            };
            let phone: std::net::Ipv6Addr = "fe80::1c3a:8bff:fe20:41d7".parse()?;
            let router6: std::net::Ipv6Addr = "fe80::de08:daff:fe00:1".parse()?;
            let six = |address, hardware: &str| {
                (
                    ronoctua_lan::neigh::Neighbour6 {
                        address,
                        hardware: hardware.to_owned(),
                        interface: 3,
                        fresh: true,
                    },
                    "wlan0".to_owned(),
                )
            };
            watch.planted(
                vec![
                    v4(1, "02:00:00:00:00:01"),
                    v4(34, "02:00:00:00:00:22"),
                    v4(52, "02:00:00:00:00:34"),
                ],
                vec![
                    six(router6, "02:00:00:00:00:01"),
                    six(phone, "02:00:00:00:00:22"),
                ],
                vec![
                    (
                        std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1)),
                        crate::network::Named::Known("router.lan".to_owned()),
                    ),
                    (
                        std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 34)),
                        crate::network::Named::Known("printer.lan".to_owned()),
                    ),
                    (
                        std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 52)),
                        crate::network::Named::Nameless,
                    ),
                    (
                        std::net::IpAddr::V6(phone),
                        crate::network::Named::Known("phone.local".to_owned()),
                    ),
                ],
                Some((
                    std::net::Ipv4Addr::new(192, 168, 1, 1),
                    crate::network::Latency::Measured(ronoctua_lan::Measured {
                        fastest: std::time::Duration::from_micros(1_400),
                        slowest: std::time::Duration::from_micros(3_900),
                        answered: 3,
                    }),
                )),
            );
        }
        "hotspot" => view.sharing(Some("ab12cd34".to_owned())),
        "phone" => {}
        "vpn" => {
            let text =
                "client\nremote vpn.example.org 1194 udp\nauth-user-pass\nredirect-gateway def1\n";
            let preview = ronoctua_network::vpn::preview("/home/me/Downloads/work.ovpn", text)
                .context("the fixture is an ovpn")?;
            view.looked_at("/home/me/Downloads/work.ovpn".to_owned(), preview);
            asking = Some(crate::line::Line::new(
                crate::network::Question::VPN_USER.id,
                crate::network::Question::VPN_USER.secret,
                Some("ana"),
            ));
        }
        other => anyhow::bail!(
            "`{other}` is not a scene of the network's window; the scenes are lan, hotspot, vpn and phone"
        ),
    }
    Ok((
        Network {
            view,
            wire: crate::network::Wire::planted(net.clone(), Vec::new()),
            net,
            watch,
            lan_sender: None,
            tunnels: ronoctua_tunnel::Scene::default(),
            hand: None,
            remember: None,
            hotspot_serving: scene == "hotspot",
        },
        asking,
    ))
}

/// `ronoctua-shell photograph --door network --scene lan|hotspot|vpn|phone`: this window
/// over a planted machine, walked and drawn (`crate::photo::planted`) — the
/// camera of the network, beside the door it photographs.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(rest, "network", "lan|hotspot|vpn|phone", |asked| {
        let (content, line) = photographed(&asked.scene)?;
        crate::photo::planted::shoot(asked, content, line)
    })
}

impl Content for Network {
    /// A thread of the window's woke the loop — the file picker's folder
    /// moved, or its devices were read: its rows again, when they changed.
    fn woken(&mut self, ui: &mut Ui) {
        if self.view.look() {
            ui.ask_paint();
        }
    }
    type Hand = Shelving;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::NETWORK_TITLE))
    }

    /// A key, read against the view's table by its whole notation — the one
    /// reader the footer calls too (`network_words::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        // THE FILE PICKER, while it is up over the view, takes what it reads
        // — every letter, and its own keys.
        if self.view.picker_takes(key) {
            self.pressed(ui, crate::network::Press::Pick(key));
            return;
        }
        if let Some(press) = crate::network_words::press_of_key(key) {
            self.pressed(ui, press);
        }
    }

    /// The clipboard's text, read off the loop for the file picker's `C-v`.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.view.picker_pasted(text);
        ui.ask_paint();
    }

    /// What was typed on this window's line: the same `View::answered` the
    /// footer's minibuffer reaches, and the same errands after it.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        ui.message = None;
        let errand = {
            let mut view = std::mem::take(&mut self.view);
            let errand = view.answered(id, text, &self.facts(), ui.columns, &ui.texts);
            self.view = view;
            errand
        };
        self.apply(ui, errand);
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
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
        let fields: Vec<Vec<crate::fields::Field>> =
            rows.iter().map(|r| r.fields.clone()).collect();
        let lit = canvas.lit;
        canvas.lit = &[];
        crate::door::list::body(ui, canvas, &fields);
        canvas.lit = lit;
        // EVERY CURSOR'S HOVER, in its own colour — see `ronoctua_shell_kit::seats`. A row
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{Password, Press, Question, Target};
    use ronoctua_network::{Band, Link, Medium, Snapshot, State, Way};

    /// A machine with a radio that is on and the daemon answering.
    fn machine() -> Snapshot {
        Snapshot {
            daemon: true,
            networking: true,
            radio: true,
            way: Way::Full,
            links: vec![Link {
                medium: Medium::Radio,
                interface: "wlan0".into(),
                network: None,
                state: State::Connected,
                rate_kbps: 400_000,
                signal: 70,
                band: Band::Other,
                address: Some("10.0.0.2/24".to_owned()),
                gateway: Some("10.0.0.1".to_owned()),
                dns: Vec::new(),
                serving: false,
            }],
            active: Vec::new(),
            saved: Vec::new(),
        }
    }

    fn café() -> ronoctua_network::Network {
        ronoctua_network::Network {
            ssid: "a café".into(),
            signal: 80,
            band: Band::Ghz5,
            secured: true,
            wpa3_only: false,
            saved: false,
            active_one: false,
            max_rate_kbps: 0,
        }
    }

    /// A window with no session behind it: a wire with no daemon (every
    /// order is refused as its absence), no tunnel thread, no sweep — the
    /// view and the door's own line, which is all a test of the wiring needs.
    fn a_window() -> (Network, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = Network {
            view: crate::network::View::fresh(),
            wire: crate::network::Wire::offline(vec![café()]),
            net: machine(),
            watch: crate::network::Watch::default(),
            lan_sender: None,
            tunnels: ronoctua_tunnel::Scene::default(),
            hand: None,
            remember: None,
            hotspot_serving: false,
        };
        (me, Ui::for_tests(texts))
    }

    /// Puts the cursor on the first row whose target `wanted` accepts.
    fn stand_on(w: &mut Network, ui: &Ui, wanted: impl Fn(&Target) -> bool) {
        let rows = w.view_rows(ui);
        let at = rows
            .iter()
            .position(|r| wanted(&r.target))
            .expect("a row to stand on");
        w.view.clicked(at, &rows);
    }

    /// EVERY TYPED QUESTION IS ASKED ON THIS WINDOW'S OWN LINE — the
    /// password hidden — and none is sent to the footer.
    #[test]
    fn every_typed_question_is_asked_on_the_windows_own_line() {
        // THE PASSWORD of a secured network with no profile.
        let (mut w, mut ui) = a_window();
        stand_on(&mut w, &ui, |t| matches!(t, Target::Wireless(_)));
        w.pressed(&mut ui, Press::Enter);
        let line = ui.asking.as_ref().expect("the password is asked here");
        assert_eq!(line.id, Question::PASSWORD.id);
        assert!(line.secret, "the password is typed in the clear");
        assert!(ui.orders().is_empty(), "the password went to the footer");
        assert!(ui.message.is_none(), "{:?}", ui.message);

        // A TUNNEL'S FILE is chosen in the picker, here, and never asked
        // on a line: typing is the picker's.
        let (mut w, mut ui) = a_window();
        w.pressed(&mut ui, Press::Import);
        assert!(ui.asking.is_none(), "a line was opened for the file");
        let key = ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Char('a'));
        assert!(w.view.picker_takes(key), "a letter is not the picker's");
        assert!(ui.orders().is_empty());

        // A TOR VALUE that has no ring to turn.
        let (mut w, mut ui) = a_window();
        w.view = crate::network::View::fresh_on_the_tor_sheet();
        stand_on(&mut w, &ui, |t| *t == Target::TorSetting("exit-nodes"));
        w.pressed(&mut ui, Press::Enter);
        let line = ui.asking.as_ref().expect("the Tor value is asked here");
        assert_eq!(line.id, Question::TOR_VALUE.id);
        assert!(ui.orders().is_empty());
    }

    /// THE PASSWORD TYPED HERE JOINS the network it was asked about, from
    /// this process: the refusal names the network and never the password,
    /// and nothing is handed to the footer.
    #[test]
    fn the_password_typed_in_the_window_joins_from_the_window() {
        let (mut w, mut ui) = a_window();
        stand_on(&mut w, &ui, |t| matches!(t, Target::Wireless(_)));
        w.pressed(&mut ui, Press::Enter);
        ui.asking = None;
        w.answered(&mut ui, "network-connect", "s3nh4-ç");
        let said = ui.message.clone().expect("the join says what happened");
        assert!(said.contains("a café"), "{said}");
        assert!(!said.contains("s3nh4"), "the password reached the screen");
        assert!(ui.orders().is_empty(), "the password went to the footer");
        // THE DAEMON REFUSED, so there is nothing to keep and no question.
        assert!(w.remember.is_none());
        assert!(!w.view.remembering());
    }

    /// FAILS IF: a click that reaches the view while the password is asked
    /// leaves the line up — the view has let go of its question, and the
    /// password typed next would be for nobody, or for whatever the cursor
    /// stands on.
    #[test]
    fn a_click_on_the_view_takes_the_password_line_with_it() {
        let (mut w, mut ui) = a_window();
        stand_on(&mut w, &ui, |t| matches!(t, Target::Wireless(_)));
        w.pressed(&mut ui, Press::Enter);
        assert!(ui.asking.is_some(), "the password is asked here");
        // TWO CLICKS ON ANOTHER ROW: the first selects, the second is its
        // `RET`, through `pressed` — here the first fact of the page, which
        // asks nothing.
        let other = w
            .view_rows(&ui)
            .iter()
            .enumerate()
            .position(|(i, r)| i != w.view.cursor() && r.target == Target::Nothing && !r.heading)
            .expect("a fact to click");
        w.clicked(&mut ui, other);
        assert!(ui.asking.is_some(), "the first click only selects");
        w.clicked(&mut ui, other);
        assert!(
            ui.asking.is_none(),
            "the password line outlived the question it asked"
        );
        // AND AN ANSWER THAT COMES ANYWAY JOINS NOTHING.
        ui.asking = None;
        let mut view = std::mem::take(&mut w.view);
        let _ = view.key(Press::Up, &w.facts(), ui.columns, &ui.texts);
        let errand = view.answered(
            "network-connect",
            "s3nh4",
            &w.facts(),
            ui.columns,
            &ui.texts,
        );
        assert_eq!(errand, crate::network::Errand::Nothing);
    }

    /// A PATH TYPED HERE SHOWS THE FILE'S READING — the level with `bring it
    /// in` and `leave it` — and a file that is not a tunnel says so.
    #[test]
    fn a_path_typed_in_the_window_shows_the_files_reading() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-network-window-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("an-office.conf");
        std::fs::write(&file, "[Interface]\nAddress = 198.51.100.2/32\n").unwrap();
        let (mut w, mut ui) = a_window();
        w.pressed(&mut ui, Press::Import);
        // THE OFFLINE WIRE READS AS NO DAEMON after every order, so the
        // machine is put back where a real reading would have left it.
        w.net = machine();
        w.answered(&mut ui, "network-import", &file.to_string_lossy());
        assert!(w.view.bringing(), "the file was not looked at");
        w.net = machine();
        let rows = w.view_rows(&ui);
        assert!(rows.iter().any(|r| r.target == Target::Bring));
        assert!(rows.iter().any(|r| r.target == Target::Drop));
        // `RET` BRINGS IT IN from here too — the daemon is absent, and the
        // sentence says the import failed rather than nothing at all.
        stand_on(&mut w, &ui, |t| *t == Target::Bring);
        w.pressed(&mut ui, Press::Enter);
        assert!(
            !w.view.bringing(),
            "the file was not put down after answering"
        );
        assert!(ui.message.is_some());
        // A FILE THAT IS NOT A TUNNEL is refused by its name.
        let other = dir.join("notes.txt");
        std::fs::write(&other, "nothing\n").unwrap();
        w.answered(&mut ui, "network-import", &other.to_string_lossy());
        assert_eq!(
            ui.message.as_deref(),
            Some(
                ui.texts
                    .t_with(
                        crate::key::ECHO_NETWORK_NOT_A_TUNNEL,
                        &[(
                            "file",
                            &ronoctua_network::vpn::name_of(&other.to_string_lossy())
                        )]
                    )
                    .as_str()
            )
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A TOR VALUE TYPED HERE is the sheet's new value, as in the footer.
    #[test]
    fn a_tor_value_typed_in_the_window_is_the_sheets_value() {
        let (mut w, mut ui) = a_window();
        w.view = crate::network::View::fresh_on_the_tor_sheet();
        stand_on(&mut w, &ui, |t| *t == Target::TorSetting("exit-nodes"));
        w.pressed(&mut ui, Press::Enter);
        ui.asking = None;
        w.answered(&mut ui, "tor-setting", "{nl}");
        // NO TUNNEL THREAD in a test, and the line says so rather than
        // pretending it was written.
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::ECHO_NETWORK_UNAVAILABLE).as_str())
        );
        let rows = w.view_rows(&ui);
        assert!(
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text.contains("{nl}"))),
            "the value typed in the window is not on the sheet"
        );
    }

    /// `S-<right>` AND `S-<left>` TURN A TOR SETTING in the window, from the
    /// keyboard and from a click on the hint — the same event either way.
    #[test]
    fn shifted_arrows_turn_a_tor_setting_in_the_window() {
        let (mut w, mut ui) = a_window();
        w.view = crate::network::View::fresh_on_the_tor_sheet();
        stand_on(&mut w, &ui, |t| {
            *t == Target::TorSetting("bridge-transport")
        });
        let value = |w: &Network, ui: &Ui| {
            let rows = w.view_rows(ui);
            rows[w.view.cursor()]
                .fields
                .iter()
                .map(|f| f.text.clone())
                .collect::<Vec<_>>()
        };
        let press = |w: &mut Network, ui: &mut Ui, notation: &str| {
            let (event, mods) =
                crate::door::keys::event_of(ronoctua_keymap::Key::parse(notation).unwrap());
            w.typed(ui, &event, mods);
        };
        // FROM NOTHING WRITTEN, the first turn lands on the ring's first word
        // — `none`, which is also what nothing written reads as — and the
        // second on the next one.
        let before = value(&w, &ui);
        press(&mut w, &mut ui, "S-<right>");
        press(&mut w, &mut ui, "S-<right>");
        let turned = value(&w, &ui);
        assert_ne!(turned, before, "S-<right> did not turn the setting");
        press(&mut w, &mut ui, "S-<left>");
        assert_eq!(value(&w, &ui), before, "S-<left> did not turn it back");
    }

    /// "KEEP IT?" IS ANSWERED HERE, by key or by click, and the password
    /// leaves this window on either answer.
    #[test]
    fn keep_it_is_answered_in_the_window_and_the_password_is_let_go() {
        // `no`, with `RET` on the row the cursor is born on.
        let (mut w, mut ui) = a_window();
        w.apply(
            &mut ui,
            crate::network::Errand::Remember("a café".into(), false),
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::ECHO_NETWORK_NO_STORED).as_str())
        );
        let ask = |w: &mut Network, ui: &Ui| {
            w.remember = Some(("a café".into(), Password::typed("s3nh4")));
            let mut view = std::mem::take(&mut w.view);
            view.ask_remember("a café".into(), &w.facts(), ui.columns, &ui.texts);
            w.view = view;
        };
        ask(&mut w, &ui);
        let rows = w.view_rows(&ui);
        assert_eq!(rows[w.view.cursor()].target, Target::Remember(false));
        w.pressed(&mut ui, Press::Enter);
        assert!(w.remember.is_none(), "the password stayed after `no`");
        assert!(!w.view.remembering());
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::ECHO_NETWORK_NO_STORED).as_str())
        );
        // `yes`, BY THE POINTER: the first click selects, the second acts.
        ask(&mut w, &ui);
        let rows = w.view_rows(&ui);
        let yes = rows
            .iter()
            .position(|r| r.target == Target::Remember(true))
            .unwrap();
        w.clicked(&mut ui, yes);
        assert!(w.view.remembering(), "the first click answered");
        w.clicked(&mut ui, yes);
        assert!(!w.view.remembering(), "the second click did not answer");
        assert!(w.remember.is_none(), "the password stayed after `yes`");
        // NO DAEMON, so sealing it fails — and says why.
        let said = ui.message.clone().unwrap_or_default();
        assert!(!said.contains("s3nh4"), "{said}");
        assert!(ui.orders().is_empty());
    }

    /// FAILS IF: one of the camera's planted networks stops building — or
    /// one it does not know builds.
    #[test]
    fn every_planted_network_builds_and_an_unknown_one_is_refused() {
        for scene in ["lan", "hotspot", "vpn", "phone"] {
            assert!(photographed(scene).is_ok(), "{scene}");
        }
        assert!(photographed("nothing").is_err());
    }
}
