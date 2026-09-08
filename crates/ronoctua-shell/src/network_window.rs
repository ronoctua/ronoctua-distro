//! The network view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
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

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::{self, EventLoop, channel},
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface as _,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.network";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list. Above it: the hint line, and nothing else —
/// the arrangement the notification window has, for the reason it gives.
const FIRST_OF_LIST: u16 = 1;

/// The margin inside the window, in cells — see `chrome::FIELDS_LEAD`.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Opens the network view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<NetworkWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    window.set_title(format!("*{}*", texts.t(ronoctua_i18n::key::NETWORK_TITLE)));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<NetworkWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window without it being reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut NetworkWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, watched for the same reason and by the same
    // means. A size chosen anywhere moves this window without it being
    // reopened — and without it this door would sit beside the footer at a
    // different size, which is the photograph that made this system have one
    // size at all.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut NetworkWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE DAEMON, on a thread of this process — the bus is the clock.
    let (net_sender, net_changes) = channel::channel::<ronoctua_network::Snapshot>();
    if let Err(e) = ronoctua_network::watch(move |s| {
        let _ = net_sender.send(s);
    }) {
        tracing::warn!(error = %e, "no NetworkManager; the window opens with nothing to read");
    }
    handle
        .insert_source(net_changes, |event, (), state: &mut NetworkWindow| {
            if let channel::Event::Msg(s) = event {
                state.net_arrived(s);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the network channel: {e}"))?;

    // AND THE SCAN, which is a different clock: `RequestScan` returns at once
    // and the radio answers seconds later. Without this the list would sit
    // still until somebody pressed something.
    let (scan_sender, scans) = channel::channel::<()>();
    if let Err(e) = ronoctua_network::watch_scan(move || {
        let _ = scan_sender.send(());
    }) {
        tracing::debug!(error = %e, "no scan watcher; the list only rereads on request");
    }
    handle
        .insert_source(scans, |event, (), state: &mut NetworkWindow| {
            if let channel::Event::Msg(()) = event {
                state.wire.reread();
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the scan channel: {e}"))?;

    // A CLOCK, AND IT IS HERE because this view has a section that MEASURES: a rate is a difference over a
    // time, and the bus's signals do not arrive on a schedule. One second is
    // the same coarse beat the footer uses, for the same reason — the
    // resolution that matters is "it is moving", not the exact byte.
    handle
        .insert_source(
            calloop::timer::Timer::from_duration(std::time::Duration::from_secs(1)),
            |_, (), state: &mut NetworkWindow| {
                state.watch.sample();
                let rows = state.rows();
                state.view.settle(&rows);
                state.ask_paint();
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
    handle
        .insert_source(lan_answers, |event, (), state: &mut NetworkWindow| {
            if let channel::Event::Msg(found) = event {
                state.watch.found(found);
                let rows = state.rows();
                state.view.settle(&rows);
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the sweep channel: {e}"))?;

    // AND THE TWO TUNNELS, on this window's own thread — the same arrangement
    // as the footer's, and for the reason the module header gives: neither
    // daemon signals, so somebody has to ask.
    let (tunnel_sender, tunnel_events) = channel::channel::<ronoctua_tunnel::Event>();
    let hand = match ronoctua_tunnel::watch(
        ronoctua_tunnel::Places::default(),
        std::path::PathBuf::from(crate::network::SYSTEMCTL),
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
    handle
        .insert_source(tunnel_events, |event, (), state: &mut NetworkWindow| {
            if let channel::Event::Msg(e) = event {
                state.tunnel_event(e);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the tunnel channel: {e}"))?;
    let mut state = NetworkWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointer: None,
        window,
        width: 0,
        height: 0,
        asked: (0, 0),
        // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE: the surface exists before
        // the compositor has said which output it is on, and
        // `scale_factor_changed` is what says so.
        scale: 1,
        scale_told: 1,
        columns: 80,
        lines: 24,
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        dragging: false,
        lines_owed: 0.0,
        dragging_shelf: None,
        view: crate::network::View::fresh(),
        wire,
        net,
        watch,
        lan_sender: Some(lan_sender),
        tunnels: ronoctua_tunnel::Scene::default(),
        hand,
        texts,
        row_under_pointer: None,
        offset: 0,
        message: None,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            e.settle();
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The window.
struct NetworkWindow {
    conn: Connection,
    /// The loop, for the ONE thing that needs it here: a keyboard that
    /// REPEATS. SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — so this is
    /// carried rather than reached for. Without it `repeat_key` below is dead
    /// code and holding an arrow moves exactly one row, which is the trap
    /// AGENTS.md records and which nine windows inherited.
    handle: calloop::LoopHandle<'static, Self>,
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    window: Window,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See the field above.
    height: u32,
    /// The logical size the COMPOSITOR configured, before any clamp.
    ///
    /// `width`/`height` are what the BUFFER can show — which is what the
    /// columns, the bars and the hit tests are laid out against; this is what
    /// the GPU is asked for. They are the same pair on every machine anybody
    /// has, and they come apart exactly at the GPU's texture ceiling. Keeping
    /// only one of them is a ratchet: a window clamped once at scale 3 would
    /// ask for the size it was cut down to and could never grow back. See
    /// [`crate::hidpi`].
    asked: (u32, u32),
    /// How many physical pixels one logical pixel is on the screen this
    /// window is on — see [`crate::hidpi`], where the rule and its
    /// measurement live.
    scale: i32,
    /// What the SURFACE has been told, which lags `scale` by one frame.
    ///
    /// `set_buffer_scale` has to travel in the commit that carries a buffer of
    /// the matching size, and the only such commit is the present inside
    /// `Gpu::frame` — see [`crate::hidpi::told_before_present`].
    scale_told: i32,
    columns: u16,
    lines: u16,
    quit: bool,
    /// Whether a frame is owed — see `notifications_window`'s field of the
    /// same name for why nothing here paints straight away.
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    /// The one rectangle this window draws: the list's scrollbar.
    ///
    /// It had none — see `chrome::list_bar` for what that cost — and the wheel
    /// was the only way down a nine-section list.
    quads: Option<ronoctua_render::quad::Quads>,
    /// Is the pointer dragging the bar?
    ///
    /// Held, because a drag is three events: the press that grabs it, every
    /// motion until the release, and the release. Without the flag the list
    /// would only move where the pointer was PRESSED.
    dragging: bool,
    /// How many LINES the wheel and the touchpad have asked for and not yet
    /// been paid.
    ///
    /// LINES, and the unit is in the name on purpose: the footer's own
    /// accumulator holds DETENTS, because one of its two modes turns a page per
    /// detent. Two fields of one crate holding two units under one name is the
    /// `ink-is-not-a-count` confusion, and it costs a factor of three.
    ///
    /// A touchpad sends fractions of a line and a mouse sends a detent as
    /// fifteen pixels, which is also a fraction. Spending only the whole part
    /// and keeping the rest is what makes a slow gesture move the list at the
    /// speed of the finger instead of standing still and then jumping — the
    /// reason is written over `surface::wheel_arrived`.
    lines_owed: f64,
    /// And WHICH shelf, when the bar grabbed was a shelf's.
    ///
    /// A shelf's bar is drawn at its CONTAINER's right edge and the list's at
    /// the surface's (`chrome::shelf_box`), so a press has to
    /// reach the bar that was drawn at that HEIGHT. Without this the shelf's
    /// thumb was visible, and pressing it dragged the whole list.
    dragging_shelf: Option<crate::network::Shelf>,
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
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    row_under_pointer: Option<usize>,
    /// The first visible row. A window has no page: it scrolls.
    offset: usize,
    message: Option<String>,
}

impl NetworkWindow {
    /// A fresh reading of the daemon's whole state.
    fn net_arrived(&mut self, s: ronoctua_network::Snapshot) {
        // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: a cable plugged
        // in brings six facts, a tunnel coming up brings a row, and a cursor
        // past the end focuses nothing.
        self.net = s;
        let rows = self.rows();
        self.view.settle(&rows);
        self.ask_paint();
    }

    /// A fresh reading of the two tunnels, or the end of an order.
    fn tunnel_event(&mut self, e: ronoctua_tunnel::Event) {
        match e {
            ronoctua_tunnel::Event::Scene(scene) => {
                self.tunnels = *scene;
                let rows = self.rows();
                self.view.settle(&rows);
                self.ask_paint();
            }
            ronoctua_tunnel::Event::Done { refused, .. } => {
                if let Some(words) = refused {
                    self.message = Some(self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_REFUSED,
                        &[("error", &words)],
                    ));
                    self.ask_paint();
                }
            }
            // THE LOGIN'S PAGE, opened with whatever the person's desktop
            // opens links with — this window has no browser of its own.
            ronoctua_tunnel::Event::Login(url) => {
                let _ = std::process::Command::new(crate::network::OPEN)
                    .arg(&url)
                    .spawn();
                self.message = Some(
                    self.texts
                        .t_with(ronoctua_i18n::key::ECHO_NETWORK_LOGIN, &[("url", &url)]),
                );
                self.ask_paint();
            }
        }
    }

    /// How many rows fit under the hint line.
    const fn room(&self) -> usize {
        self.lines.saturating_sub(FIRST_OF_LIST) as usize
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
    /// Keeps the offset inside the list, and nothing else.
    ///
    /// # Why this is not `follow`
    ///
    /// Because a paint may not move the list. `follow` slaves the offset to
    /// the CURSOR, and calling it from `paint` undid, on the same frame, every
    /// offset the pointer had just set: a drag of the scrollbar and a turn of
    /// the wheel both wrote an offset and then watched the paint put it back
    /// under the cursor — which on a freshly-opened window (cursor on row 1)
    /// means back to the top, every time. The bar looked like a bar that does
    /// not work, which is exactly the report it was built to answer.
    ///
    /// The footer never made this mistake: `Footer::follow_cursor` is called
    /// from twenty places and every one of them is an EVENT.
    ///
    /// What a paint may do is keep the offset inside a list that changed size
    /// under it — the watch thread brings a new reading while nobody is
    /// touching anything — and honour an anchor the view asked for.
    fn settle_offset(&mut self, rows: usize) {
        let room = self.room();
        if room == 0 {
            return;
        }
        // A VIEW MAY ASK FOR A LINE AT THE TOP — see
        // `network::View::take_anchor`. `TAB` lands on a section and has to
        // SHOW it, and keeping the cursor visible is satisfied by the
        // section's title being off the screen above it.
        if let Some(top) = self.view.take_anchor() {
            self.offset = top.min(rows.saturating_sub(room));
            return;
        }

        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    fn follow(&mut self, rows: usize) {
        const MARGIN: usize = 2;
        let room = self.room();
        if room == 0 {
            return;
        }
        let cursor = self.view.cursor();
        if cursor < self.offset + MARGIN {
            self.offset = cursor.saturating_sub(MARGIN);
        } else if cursor + MARGIN >= self.offset + room {
            self.offset = cursor + MARGIN + 1 - room;
        }
        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    /// Carries out what the view asked for.
    ///
    /// EVERY ORDER RE-READS: the watch thread would bring the same reading a
    /// moment later, and this is what makes the screen answer the key that
    /// was just pressed rather than a quarter of a second afterwards.
    fn apply(&mut self, errand: crate::network::Errand) {
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
                self.message = Some(crate::chrome::copied(&text, &self.texts));
            }
            Errand::Close => {
                self.quit = true;
                return;
            }
            // A PATH IS TYPED IN THE MINIBUFFER, which this window has not
            // got — the pairing question's own arrangement, and its reason.
            Errand::Ask => {
                self.message = Some(
                    self.texts
                        .t(ronoctua_i18n::key::NETWORK_ANSWER_IN_THE_FOOTER),
                );
            }
            Errand::Bring(path, kind) => {
                let name = ronoctua_network::vpn::name_of(&path);
                self.message = Some(match self.wire.import(&path, kind) {
                    Ok(()) => self
                        .texts
                        .t_with(key::ECHO_NETWORK_IMPORTED, &[("network", &name)]),
                    Err(e) => self.texts.t_with(
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
                self.message = Some(
                    match ronoctua_network::hotspot_password()
                        .and_then(|word| self.wire.hotspot(&name, &word).map(|()| word))
                    {
                        Ok(word) => {
                            self.view.sharing(Some(word.clone()));
                            self.texts.t_with(
                                key::ECHO_NETWORK_HOTSPOT,
                                &[("network", &name), ("password", &word)],
                            )
                        }
                        Err(e) => self.texts.t_with(
                            key::ECHO_NETWORK_HOTSPOT_FAILED,
                            &[("reason", &e.to_string())],
                        ),
                    },
                );
            }
            Errand::Hotspot(false) => {
                self.view.sharing(None);
                self.message = Some(
                    self.texts.t(
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
                    _ => self.message = Some(self.texts.t(key::ECHO_NETWORK_NO_LINK)),
                }
            }
            Errand::Radio(on) => {
                self.message = Some(self.texts.t(match self.wire.radio(on) {
                    Ok(()) if on => key::ECHO_NETWORK_RADIO_ON,
                    Ok(()) => key::ECHO_NETWORK_RADIO_OFF,
                    Err(_) => key::ECHO_NETWORK_UNAVAILABLE,
                }));
            }
            Errand::Airplane(on) => {
                if let Err(e) = self.wire.airplane(on) {
                    self.message = Some(refused(&e, &self.texts));
                }
            }
            Errand::Disconnect(medium) => {
                self.message = Some(self.texts.t(if self.wire.disconnect(medium).is_ok() {
                    key::ECHO_NETWORK_DISCONNECTED
                } else {
                    key::ECHO_NETWORK_UNAVAILABLE
                }));
            }
            Errand::Facts => {
                self.message = Some(match crate::network::carrying(&self.net) {
                    Some(l) if l.medium == ronoctua_network::Medium::Wired => self.texts.t_with(
                        key::ECHO_NETWORK_LINK_WIRED,
                        &[
                            ("network", &l.interface),
                            ("rate", &crate::network::rate_of(l, &self.texts)),
                        ],
                    ),
                    Some(l) => self.texts.t_with(
                        key::ECHO_NETWORK_LINK,
                        &[
                            ("network", l.network.as_deref().unwrap_or(&l.interface)),
                            ("band", &crate::network::band_text(l.band, &self.texts)),
                            ("rate", &crate::network::rate_of(l, &self.texts)),
                            ("signal", &format!("{}%", l.signal)),
                        ],
                    ),
                    None => self.texts.t(key::ECHO_NETWORK_NO_LINK),
                });
            }
            Errand::Forget(ssid) => {
                self.message = Some(if self.wire.forget(&ssid).is_ok() {
                    self.texts
                        .t_with(key::ECHO_NETWORK_FORGOTTEN, &[("network", &ssid)])
                } else {
                    self.texts.t(key::ECHO_NETWORK_UNAVAILABLE)
                });
            }
            // A PASSWORD IS TYPED IN THE FOOTER, which this window has not
            // got. A network that is saved or open joins from here; the one
            // that needs typing says where, rather than looking broken.
            Errand::Connect(target) => {
                if target.secured && !target.saved {
                    self.message = Some(
                        self.texts
                            .t(ronoctua_i18n::key::NETWORK_ANSWER_IN_THE_FOOTER),
                    );
                } else {
                    self.message = Some(match self.wire.connect(&target.ssid, None, false) {
                        Ok(()) => self
                            .texts
                            .t_with(key::ECHO_NETWORK_CONNECTED, &[("network", &target.ssid)]),
                        Err(e) => self.texts.t_with(
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
                self.message = Some(match self.wire.drop_it(&at) {
                    Ok(()) => self
                        .texts
                        .t_with(key::ECHO_NETWORK_TUNNEL_DOWN, &[("network", &name)]),
                    Err(e) => refused(&e, &self.texts),
                });
            }
            Errand::Raise(at) => {
                let name = name_of(
                    self.net.saved.iter().find(|s| s.at == at).map(|s| &s.id),
                    &at,
                );
                self.message = Some(match self.wire.raise(&at) {
                    Ok(()) => self
                        .texts
                        .t_with(key::ECHO_NETWORK_TUNNEL_UP, &[("network", &name)]),
                    Err(e) => refused(&e, &self.texts),
                });
            }
            // THE TUNNEL ORDERS GO TO THIS WINDOW'S OWN THREAD: neither
            // daemon keeps per-client state, so a second telephone to them
            // is not a second truth.
            Errand::Tunnel(order) => match self.hand.as_ref() {
                Some(h) => h.send(order),
                None => {
                    self.message = Some(self.texts.t(key::ECHO_NETWORK_UNAVAILABLE));
                }
            },
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
                self.message = Some(match self.wire.delete(&at) {
                    Ok(()) => self.texts.t_with(saying, &[("network", &name)]),
                    Err(e) => refused(&e, &self.texts),
                });
            }
        }
        self.net = self.wire.look();
        let rows = self.rows();
        self.view.settle(&rows);
        self.ask_paint();
    }

    /// The size of the text changed under this door.
    ///
    /// THE SHAPER HAS TO HEAR IT TOO. It was built with a size and keeps it;
    /// moving everything around it — the columns, the cell, every hit test —
    /// and leaving the letters gives a grid at one size with text at another,
    /// which reads as a broken font rather than as a setting that took.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(ronoctua_tokens::text::size());
        }
        self.recount();
        self.ask_paint();
    }

    /// Columns and rows from the size in pixels.
    fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        let usable = (self.width as f32) - 2.0 * cell * f32::from(CONTENT_MARGIN);
        self.columns = tokens::metrics::columns(usable.max(0.0), size);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = (self.height as f32 / line).floor().max(0.0) as u16;
        self.lines = lines;
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast()),
            std::ptr::NonNull::new(self.window.wl_surface().id().as_ptr().cast()),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what produced them.
        // ONE CALL, not the same one twice: two calls to a pure
        // function are two places the arguments can be edited apart.
        let first = crate::hidpi::physical(self.asked, self.scale);
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                first.0,
                first.1,
            )
        };
        let Ok(mut g) = gpu else {
            tracing::error!("no usable GPU; the window will not draw");
            return;
        };
        // AND THE FIRST BUFFER IS DIVISIBLE TOO. `Gpu::new` clamps at the
        // GPU's ceiling like `resize` does, and the first frame sends the
        // scale — see `crate::hidpi::resized`.
        let (shown_w, shown_h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
        self.width = shown_w;
        self.height = shown_h;
        // AND THE COLUMNS ARE COUNTED AGAIN. `configure` counts them before the
        // GPU exists, against the size the compositor gave; if the buffer came
        // back smaller than that, the count that is on file is for a picture
        // wider than the one this door draws into.
        self.recount();
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            self.scale,
            tokens::text::size(),
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "assembling the text failed"),
        }
        // AND THE ONE RECTANGLE THIS WINDOW DRAWS: the list's scrollbar.
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "assembling the quads failed"),
        }
        self.gpu = Some(g);
    }

    /// One key, from a press or from a REPEAT.
    ///
    /// Both doors of every view take this path: SCTK delivers a repeat
    /// through the callback given to `get_keyboard_with_repeat`, which has no
    /// `Connection` and no `QueueHandle` to hand on — so the work cannot live
    /// in `press_key`, and while it did, `repeat_key` was dead code and
    /// holding an arrow moved exactly one row.
    fn typed(&mut self, event: KeyEvent) {
        let Some(press) = press_of(&event) else {
            return;
        };
        self.message = None;
        let errand = {
            let mut view = std::mem::take(&mut self.view);
            let errand = view.key(press, &self.facts(), &self.texts);
            self.view = view;
            errand
        };
        // THE CURSOR MOVED, so the list follows it — here, on the EVENT, and
        // never in the paint. See `settle_offset` for what following from a
        // paint cost.
        let rows = self.rows().len();
        self.follow(rows);
        self.apply(errand);
    }

    /// Asks for a frame. It is drawn when the event queue empties.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Draws the owed frame, if one is owed.
    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
        }
    }

    fn rows(&self) -> Vec<crate::network::Row> {
        self.view.rows(&self.facts(), self.columns, &self.texts)
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

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::text::size();
        let margin = CONTENT_MARGIN;
        let rows = self.rows();
        self.settle_offset(rows.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();

        // LINE 0: what the last order said, or the keys — the footer's
        // minibuffer, through the other door. `columns` is already net of both
        // margins (`recount`), so the whole of it is the room.
        let room = self.columns;
        if let Some(text) = self.message.clone() {
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: ronoctua_render::layout::elide_end(&text, room),
                column: margin,
                line: 0,
                color: tokens::color::TEXT_SUBTLE_STRONG,
                edge: false,
                foot: false,
            });
        } else {
            spans.extend(crate::chrome::hint_spans(
                &self.view.hint(&self.texts),
                ronoctua_render::Size::Px(size),
                margin,
                0,
                room,
            ));
        }

        // THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
        // draws for these rows.
        for (i, row) in rows.iter().skip(self.offset).take(self.room()).enumerate() {
            let y = FIRST_OF_LIST + u16::try_from(i).unwrap_or(0);
            for f in &row.fields {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: f.text.clone(),
                    column: margin + f.at,
                    line: y,
                    color: f.color,
                    edge: false,
                    foot: false,
                });
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let (bar_w, bar_h) = (self.width as f32, self.height as f32);
        let mut bar: Vec<ronoctua_render::quad::Quad> = Vec::new();
        // EVERY CONTAINER ON SCREEN GETS ITS GROUND, and it goes in FIRST —
        // before the page's bar and before every other rectangle. Quads draw in
        // order and there is no depth test, so a ground pushed after the page's
        // bar paints the page's THUMB out of existence wherever a container is
        // on screen, which is what the owner read off the screen: *"ela acaba
        // desaparecendo e mudando de tamanho"*. The box (`chrome::shelf_box`)
        // now makes that impossible by geometry as well; the order is the
        // second lock, and the footer's door has both.
        let box_of = self.shelf_box();
        for band in self.view.shelves(&rows) {
            let (from, to) = (band.first, band.first + band.lines);
            let seen_from = from.max(self.offset);
            let seen_to = to.min(self.offset + self.room());
            if seen_to > seen_from
                && let Ok(line) = u16::try_from(seen_from - self.offset)
            {
                bar.push(crate::chrome::shelf_ground(
                    box_of,
                    FIRST_OF_LIST + line,
                    seen_to - seen_from,
                    bar_w,
                    bar_h,
                ));
            }
        }
        // THE PAGE'S BAR, from the same `Scroll` every other bar in this system
        // uses.
        let scroll = crate::chrome::list_scroll(rows.len(), self.room(), self.offset);
        bar.extend(crate::chrome::list_bar(scroll, FIRST_OF_LIST, bar_w, bar_h));
        // AND A CONTAINER'S OWN, when the whole band is on screen — for the
        // reason the footer's door writes down: a track shorter than its band
        // would move the list by a different amount than the drag.
        for band in self.view.shelves(&rows) {
            let (from, to) = (band.first, band.first + band.lines);
            if band.total <= band.lines {
                continue;
            }
            if from < self.offset || to > self.offset + self.room() {
                continue;
            }
            let Ok(line) = u16::try_from(from - self.offset) else {
                continue;
            };
            let inner = crate::chrome::list_scroll(band.total, band.lines, band.offset);
            if let Some(q) =
                crate::chrome::shelf_bar(inner, box_of, FIRST_OF_LIST + line, bar_w, bar_h)
            {
                bar.push(q);
            }
        }
        // THE SURFACE AND THE TWO SCALE NUMBERS, taken before the borrows:
        // `set_buffer_scale` has to travel in the commit `Gpu::frame` makes,
        // and nothing else in this function may borrow `self` by then. See
        // `crate::hidpi::told_before_present`.
        let surface = self.window.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        // THE WINDOW'S OWN PIXELS — `resized` already made them what the
        // buffer can show, so there is one number and this is it.
        let (logical_w, logical_h) = (self.width, self.height);
        if let Err(e) = text.prepare(
            &gpu.device,
            &gpu.queue,
            gpu.config.width,
            gpu.config.height,
            ronoctua_render::Atlas::Pinned,
            &spans,
        ) {
            tracing::error!(error = %e, "preparing the text failed");
            return;
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(&gpu.device, &gpu.queue, logical_w, logical_h, &bar);
        }
        // THE RECTANGLE FIRST, THEN THE TEXT. There is no depth test: whoever
        // draws later is on top, and a bar over a letter would hide it.
        let quads_ref = self.quads.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw(pass);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            },
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }

    /// The box every container of this view is drawn in.
    ///
    /// ONE BOX FOR ALL OF THEM, and it comes from the same `extent` the rows
    /// were built with — a ground measured from a different width than the
    /// text inside it is a ground that cuts its own rows.
    fn shelf_box(&self) -> crate::chrome::ShelfBox {
        let (left, room) = crate::network::extent(self.columns);
        crate::chrome::shelf_box(left, room)
    }

    /// Every shelf whose band is WHOLLY on screen, and the line it starts on.
    ///
    /// The same filter the drawing uses, so what can be grabbed is exactly
    /// what was drawn — see `chrome::on_the_band` for why a band that is half
    /// out gets no bar.
    fn bands(&self, rows: &[crate::network::Row]) -> Vec<(u16, crate::network::Band)> {
        self.view
            .shelves(rows)
            .into_iter()
            .filter(|b| b.total > b.lines)
            .filter_map(|b| {
                let (from, to) = (b.first, b.first + b.lines);
                if from < self.offset || to > self.offset + self.room() {
                    return None;
                }
                let line = u16::try_from(from - self.offset).ok()?;
                Some((FIRST_OF_LIST + line, b))
            })
            .collect()
    }

    /// The shelf whose BAR is under `(x, y)`, if any.
    fn shelf_bar_at(
        &self,
        rows: &[crate::network::Row],
        x: f32,
        y: f32,
    ) -> Option<crate::network::Band> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        self.bands(rows).into_iter().find_map(|(line, b)| {
            let inner = crate::chrome::list_scroll(b.total, b.lines, b.offset);
            crate::chrome::on_the_band(inner, self.shelf_box(), line, w, h, x, y).then_some(b)
        })
    }

    /// Where a drag of that shelf's bar puts it.
    fn shelf_offset_at(
        &self,
        rows: &[crate::network::Row],
        which: crate::network::Shelf,
        y: f32,
    ) -> Option<usize> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        self.bands(rows).into_iter().find_map(|(line, b)| {
            (b.shelf == which).then(|| {
                let inner = crate::chrome::list_scroll(b.total, b.lines, b.offset);
                crate::chrome::shelf_offset_at(inner, line, w, h, y)
            })
        })
    }

    /// Which drawn row is at height `y`.
    fn row_at(&self, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line <= 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (y / line) as usize;
        which
            .checked_sub(usize::from(FIRST_OF_LIST))
            .map(|i| i + self.offset)
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
                .find(|(notation, _)| *notation == typed)
                .map(|(_, id)| *id)?;
            crate::network_words::press_of(id)?
        }
    })
}

impl WindowHandler for NetworkWindow {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        cfg: WindowConfigure,
        _: u32,
    ) {
        // WHAT THE COMPOSITOR SAID IS KEPT SEPARATELY, because what the
        // buffer can show is what comes back below — see `asked`. It is
        // taken from the CONFIGURE and from nothing else: built out of
        // the door's own fields it would be the size `resized` last
        // answered, which can only ever shrink.
        self.asked = (
            cfg.new_size.0.map_or(900, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(600, std::num::NonZeroU32::get),
        );
        self.width = self.asked.0;
        self.height = self.asked.1;
        let asked = self.asked;
        let scale = self.scale;
        if let Some(g) = self.gpu.as_mut() {
            // AND THE WINDOW KEEPS WHAT THE BUFFER CAN SHOW. Below the GPU's
            // ceiling that is exactly what the compositor configured; above
            // it, laying out against the bigger number would put the columns
            // and the bars in a space the buffer does not have.
            let (shown_w, shown_h) = crate::hidpi::resized(g, asked, scale);
            self.width = shown_w;
            self.height = shown_h;
        }
        self.recount();
        self.ask_paint();
    }
}

impl SeatHandler for NetworkWindow {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            // A KEYBOARD THAT REPEATS — see the `handle` field. The rate and
            // the delay are the COMPOSITOR's, arriving as `RepeatInfo`;
            // nothing here invents either.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, _, event| state.typed(event)),
            ) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                Err(e) => tracing::warn!(error = %e, "no pointer; keyboard only"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if cap == Capability::Pointer
            && let Some(p) = self.pointer.take()
        {
            p.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for NetworkWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.typed(event);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, kb, serial, event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for NetworkWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = self.row_under_pointer;
        let mut clicked: Option<usize> = None;
        let mut turned = false;
        // THE BAR IS A CONTROL, and a drag is three events. `Scroll::offset_at`
        // maps any point of the TRACK, so nobody has to aim at the handle.
        // THE ROWS ARE ONLY BUILT WHEN THE BAR IS IN PLAY.
        //
        // A `Motion` arrives on every pixel of movement, and these windows
        // deliberately skip a REPAINT on motion because it "spent a GPU frame
        // per row crossed". Building the whole list to answer "is the pointer
        // on the bar?" would spend the more expensive half of that anyway.
        // AND A SHELF'S DRAG COUNTS AS ONE. It did not, and the consequence was
        // exactly the defect this bar was built to answer: the press builds the
        // rows (a Press is in that frame) and the shelf jumps once, then every
        // Motion frame carries no press, `rows` comes back empty, `bands(&[])`
        // is empty and `shelf_offset_at` answers `None` — the thumb is held and
        // the shelf is frozen until the button comes up. The footer's twin
        // never had it: `drag_shelf_to` builds its own rows.
        let bar_in_play = self.dragging
            || self.dragging_shelf.is_some()
            || events.iter().any(|e| {
                matches!(e.kind, PointerEventKind::Press { button, .. } if button == LEFT_BUTTON)
            });
        let rows = if bar_in_play { self.rows() } else { Vec::new() };
        let scroll = crate::chrome::list_scroll(rows.len(), self.room(), self.offset);
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        let mut dragged: Option<usize> = None;
        let mut shelved: Option<(crate::network::Shelf, usize)> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if let Some(which) = self.dragging_shelf {
                        shelved = self.shelf_offset_at(&rows, which, py).map(|at| (which, at));
                    } else if self.dragging {
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            FIRST_OF_LIST,
                            w,
                            h,
                            py,
                        ));
                    } else {
                        self.row_under_pointer = self.row_at(e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    // AND THE DRAG ENDS WITH THE POINTER. Wayland's implicit
                    // grab usually delivers the release, but a popup grab, an
                    // unmap or a broken grab drops it — and a `dragging` left
                    // true makes the next ENTER, with no button held, jump the
                    // list to wherever the pointer came in.
                    self.dragging = false;
                    self.dragging_shelf = None;
                    self.row_under_pointer = None;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    // A SHELF'S BAR FIRST: it is drawn at its container's
                    // right edge and the list's at the surface's, so the two
                    // are different columns — but the list's TRACK runs the
                    // whole height, so asking it first would let it answer for
                    // a press on a container's thumb and drag the wrong list.
                    if let Some(band) = self.shelf_bar_at(&rows, px, py) {
                        self.dragging_shelf = Some(band.shelf);
                        shelved = self
                            .shelf_offset_at(&rows, band.shelf, py)
                            .map(|at| (band.shelf, at));
                    } else if crate::chrome::on_the_list_bar(scroll, FIRST_OF_LIST, w, h, px, py) {
                        self.dragging = true;
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            FIRST_OF_LIST,
                            w,
                            h,
                            py,
                        ));
                    } else {
                        self.row_under_pointer = self.row_at(e.position.1);
                        clicked = self.row_under_pointer;
                    }
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.dragging = false;
                    self.dragging_shelf = None;
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    // THE ARITHMETIC IS SHARED — see `surface::wheel_arrived`.
                    // Written out here it was `(pixels / line) * 3`, which
                    // moved nothing for a slow gesture and three lines for the
                    // next one: a list that jumps.
                    crate::surface::wheel_arrived(&mut self.lines_owed, &vertical, &horizontal);
                    turned = true;
                }
                _ => {}
            }
        }
        if let Some((which, at)) = shelved {
            self.view.put_shelf(which, at);
            self.ask_paint();
            return;
        }
        if let Some(offset) = dragged {
            self.offset = offset;
            self.ask_paint();
            return;
        }
        // THE MOUSE AIMS, THE KEYS ACT — a click puts the cursor on the row,
        // and `RET` is what joins, drops and brings up.
        if let Some(row) = clicked {
            let rows = self.rows();
            self.view.clicked(row, &rows);
            self.follow(rows.len());
            self.ask_paint();
            return;
        }
        if turned {
            let by = crate::surface::wheel_lines(&mut self.lines_owed);
            if by == 0 {
                // WHAT IS LEFT OVER STAYS. A gesture too small to be a line
                // yet is not a gesture that did nothing — it is one that has
                // not finished adding up.
                return;
            }
            // THE WHEEL IS THE PAGE'S. A shelf took it, on the manager's rule
            // that the gesture scrolls the section under the hand — but the
            // manager's sections are COLUMNS side by side and a shelf is a
            // band inside one list, so with five of them the page stopped
            // scrolling almost everywhere. A shelf has its own bar, its own
            // column and the arrows at its edge.
            let rows = self.rows().len();
            let last = rows.saturating_sub(self.room());
            self.offset = usize::try_from(
                i64::try_from(self.offset)
                    .unwrap_or(0)
                    .saturating_add(by)
                    .max(0),
            )
            .unwrap_or(0)
            .min(last);
            self.ask_paint();
            return;
        }
        // NO FRAME FOR A POINTER THAT CHANGED NOTHING. Nothing on this
        // screen is drawn from `row_under_pointer` — the click reads it, and
        // the paint does not — so repainting when it moved spent a GPU frame
        // per row crossed, on an identical picture. The footer's own hover
        // band IS drawn, which is why it repaints there and not here.
        let _ = before;
    }
}

impl CompositorHandler for NetworkWindow {
    /// The window moved to a screen of another scale — or was told its first.
    ///
    /// It was EMPTY here and in eight other doors, which is what made every
    /// one of them blurry on a HiDPI screen. The rule and the measurement are
    /// in [`crate::hidpi`].
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        // THE SURFACE IS NEEDED to ask its VERSION, not to tell it anything:
        // it is told its scale inside the frame, which is the only commit that
        // carries a matching buffer — see `crate::hidpi::told_before_present`.
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if let Some((w, h)) = crate::hidpi::followed(
            &mut self.scale,
            factor,
            surface,
            self.gpu.as_mut(),
            self.text.as_mut(),
            self.asked,
        ) {
            // AND THE DOOR KEEPS WHAT THE BUFFER CAN SHOW. Below the
            // GPU's ceiling this is what the compositor configured;
            // above it, going on with the old number lays the picture
            // out in a space the buffer does not have.
            self.width = w;
            self.height = h;
            self.recount();
            self.ask_paint();
        }
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for NetworkWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for NetworkWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(NetworkWindow);
smithay_client_toolkit::delegate_dispatch2!(NetworkWindow);
