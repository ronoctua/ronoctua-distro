//! What the machine did, said on the minibuffer's line.
//!
//! # Why the minibuffer, and not the band
//!
//! The owner's choice (2026-09-28): a camera or a microphone opening and
//! closing, the camera's own recording and stream, a Bluetooth device, a
//! network, a tunnel or the aeroplane coming and going, the charger, and a
//! program installed are EVENTS — facts about the machine that are worth one
//! line and nothing more. The band is where something arrives that may want an
//! answer; a fact that goes by is what the minibuffer's line already does for
//! the shell's own answers (the echo), so an event is said there, in its
//! catalogue entry's summary, for as long as any echo lasts.
//!
//! And it is said whether the modeline is showing or not: with the footer
//! hidden, an event is what brings the minibuffer up for a moment.
//!
//! # Busy
//!
//! The line is somebody's while a view is open (the leader included), a
//! question is being typed, the line's own drawer is open, or the screen is
//! under a fullscreen window and the footer is down to its edge. An event
//! that arrives then does NOT wait for the line: a camera that opened while
//! somebody was typing in the launcher is news now, not in two minutes. It
//! goes to the band instead, through the same door every notification takes,
//! under its own tag — and its body OPENS with the sentence that says why it
//! is there and not where it usually is (`notif-minibuffer-was-busy`), the
//! one line of body the band draws.
//!
//! An event already ON the line when it becomes busy was said: it goes
//! nowhere, unless it was up too briefly to be read ([`READ_IN`]).
//!
//! The Bluetooth devices keep the tags they always had, so the two filters
//! that quiet a device coming and going still quiet the fallback.
//!
//! # The queue
//!
//! Events come in bursts — a resume brings the network, the tunnel and the
//! headset back in one second — so the line keeps a FIFO of them, each shown
//! for the echo's time. A person's own echo (the shell answering something
//! that was just asked for) takes the line AT ONCE, and the event it
//! interrupted goes back to the FRONT of the queue. Past [`EVENT_ECHOES`]
//! waiting, the next one goes to the band: a queue that grows without end is
//! a line that says, a minute late, what is no longer true — and for the same
//! reason an event that says the opposite of one on the line or waiting
//! REPLACES it ([`contradicts`]), and a pointer resting on the line holds an
//! event for [`HELD_AT_MOST`] echoes at the most.
//!
//! # The first reading is not news
//!
//! Every source is compared with its own last reading, and the FIRST reading
//! is only that — the baseline. The footer starting while a headset is
//! connected is not the headset connecting; without this, every start of the
//! footer said "connected" once per device, which is what the radio used to
//! do.
//!
//! # Why this module has no Wayland in it
//!
//! Everything here is readings in and events out, and a queue with a clock
//! handed to it. The footer (`surface.rs`) reads, asks, and paints.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use ronoctua_i18n::key;

/// How many events may WAIT for the line, beside the one on it.
///
/// Four: a resume's burst — the network, a tunnel, a headset, the charger —
/// fits, and the last of it is said twelve seconds late at the worst. The
/// fifth goes to the band.
pub const EVENT_ECHOES: usize = 4;

/// A camera was opened by a program that is not ours.
pub const CAMERA_IN_USE: &str = "camera-in-use";
/// The last program holding a camera let it go.
pub const CAMERA_FREE: &str = "camera-free";
/// A program that is not ours started listening.
pub const MICROPHONE_IN_USE: &str = "microphone-in-use";
/// Nothing that is not ours listens any more.
pub const MICROPHONE_FREE: &str = "microphone-free";
/// The camera started recording.
pub const RECORDING_STARTED: &str = "studio-recording-started";
/// The camera stopped recording.
pub const RECORDING_STOPPED: &str = "studio-recording-stopped";
/// The camera started a stream.
pub const STREAMING_STARTED: &str = "studio-streaming-started";
/// The camera's stream ended.
pub const STREAMING_STOPPED: &str = "studio-streaming-stopped";
/// A cable or a radio came up on a network.
pub const NETWORK_CONNECTED: &str = "network-connected";
/// A cable or a radio lost its network.
pub const NETWORK_LOST: &str = "network-lost";
/// Networking was switched off.
pub const AIRPLANE_ON: &str = "airplane-on";
/// Networking was switched back on.
pub const AIRPLANE_OFF: &str = "airplane-off";
/// A tunnel NetworkManager holds came up.
pub const VPN_UP: &str = "vpn-up";
/// A tunnel NetworkManager holds went down.
pub const VPN_DOWN: &str = "vpn-down";
/// This machine joined its tailnet.
pub const TAILNET_UP: &str = "tailnet-up";
/// This machine left its tailnet.
pub const TAILNET_DOWN: &str = "tailnet-down";
/// There is a circuit through Tor.
pub const TOR_UP: &str = "tor-up";
/// There is no circuit through Tor any more.
pub const TOR_DOWN: &str = "tor-down";
/// The charger started delivering.
pub const CHARGER_PLUGGED: &str = "charger-plugged";
/// The charger stopped delivering.
pub const CHARGER_UNPLUGGED: &str = "charger-unplugged";

/// The programs of OURS that hold a camera, by the name `/proc` gives them.
///
/// The studio is the only one — every camera of this system is opened by it
/// (`the-camera-has-one-door`) — and what it does with one is either a
/// preview a person asked for or a recording, which is an event of its own.
/// Announcing it would be the line saying back what the hand just did.
///
/// A HOLDER IS OURS BY LINEAGE, not by its own name: the process holding
/// the device during a preview or a recording is the studio's `ffmpeg`
/// (`ronoctua_camera::Capture`), and a stranger's `ffmpeg` has the same
/// name. So what is matched against this list is the holder AND the
/// processes above it (`ronoctua_memory::camera::Holder::under`).
pub const OUR_CAMERA_HOLDERS: &[&str] = &["ronoctua-studio"];

/// Who holds a camera, as the holders' thread sends it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CameraHolder {
    /// The name `/proc` gives it.
    pub name: String,
    /// Whether it is, or runs under, one of [`OUR_CAMERA_HOLDERS`].
    pub ours: bool,
}

/// One thing the machine did, worth a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The catalogue's tag — also the id of its entry, and the stem of its
    /// two sentences (`notif-<tag>-summary`, `notif-<tag>-body`).
    pub tag: &'static str,
    /// The text key of who says it, for the band and the manager.
    pub app: &'static str,
    /// The arguments its two sentences take, by name.
    pub args: Vec<(&'static str, String)>,
    /// What its action opens, when it opens something — the desktop id of
    /// the program an install put in the launcher ([`launch_of`]).
    pub opens: Option<String>,
}

impl Event {
    /// An event with nothing to fill in.
    #[must_use]
    pub const fn of(tag: &'static str, app: &'static str) -> Self {
        Self {
            tag,
            app,
            args: Vec::new(),
            opens: None,
        }
    }

    /// The same, with one argument more.
    #[must_use]
    pub fn about(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    /// A program the software centre put in the launcher — `tag` is
    /// `software-installed` or `software-web-created`, `app` its name, and
    /// `opens` the desktop id its action launches.
    #[must_use]
    pub fn installed(tag: &'static str, app: &str, opens: Option<&str>) -> Self {
        let mut e = Self::of(tag, key::NOTIF_SOFTWARE_APP).about("app", app);
        e.opens = opens.map(ToOwned::to_owned);
        e
    }

    /// The arguments, as the localiser takes them.
    fn pairs(&self) -> Vec<(&str, &str)> {
        self.args.iter().map(|(n, v)| (*n, v.as_str())).collect()
    }

    /// What the line says: the entry's summary.
    #[must_use]
    pub fn summary(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t_with(&format!("notif-{}-summary", self.tag), &self.pairs())
    }

    /// The entry's body — read only when the event goes to the band.
    #[must_use]
    pub fn body(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t_with(&format!("notif-{}-body", self.tag), &self.pairs())
    }
}

/// The event as a notification the catalogue recognises by its tag — for
/// when the line was somebody else's.
///
/// THE BODY SAYS WHY IT IS HERE, and FIRST: the same fact arrives in two
/// places depending on what the person was doing, and a notification that
/// did not say so would read as the line having broken. First, because the
/// band draws ONE line of a body, cut to the room (`band::lines`), and after
/// the event's own body the reason was the part a line of ordinary width
/// cut; its entries are `full`, so that line is drawn at all.
#[must_use]
pub fn notification(e: &Event, texts: &ronoctua_i18n::Texts) -> ronoctua_notify::Notification {
    let body = format!(
        "{}\n{}",
        texts.t(key::NOTIF_MINIBUFFER_WAS_BUSY),
        e.body(texts)
    );
    let mut n = ronoctua_notify::Notification::fresh_one(
        0,
        texts.t(e.app),
        None,
        e.summary(texts),
        body,
        ronoctua_notify::Urgency::Normal,
    );
    n.tags = vec![e.tag.to_owned()];
    n
}

/// The actions an event's catalogue entry offers that a click on the line can
/// run: those that name a command of the registry — label and command.
///
/// The band can also start a program with a file or copy a path, and those
/// need the notification's file; an event has none, and its entry offers
/// none of them. A command the registry does not know is left out rather than
/// drawn as a button that does nothing.
#[must_use]
pub fn actions_of(
    e: &Event,
    book: &ronoctua_notify::Book,
    registry: &ronoctua_cmd::Registry,
    texts: &ronoctua_i18n::Texts,
) -> Vec<(String, &'static str)> {
    let Some(entry) = book.entry(e.tag) else {
        return Vec::new();
    };
    entry
        .actions
        .iter()
        .filter_map(|a| {
            let ronoctua_notify::Doing::Command(command) = &a.doing else {
                return None;
            };
            let id = registry.search(command)?.id;
            let label = match &a.label {
                ronoctua_notify::Label::Key(k) => texts.t(k),
                ronoctua_notify::Label::Text(t) => t.clone(),
            };
            Some((label, id))
        })
        .collect()
}

/// The command an install's action runs — the software centre's "open what
/// was installed".
pub const LAUNCH_LATEST: &str = "software-launch-latest";

/// The desktop id a click on `command`, among `e`'s actions, launches.
///
/// THE ONE THE LINE NAMES, and not the latest: `software-launch-latest` with
/// no argument opens whatever finished LAST, and the queue and the pointer's
/// hold keep an older install's line up while newer ones finish — "GIMP is
/// installed · Open it" opened Inkscape. `None` for any other command, or an
/// event that carries no id; the band's button keeps the latest, as it
/// always did.
#[must_use]
pub fn launch_of<'a>(e: &'a Event, command: &str) -> Option<&'a str> {
    if command == LAUNCH_LATEST {
        e.opens.as_deref()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Where an event goes
// ---------------------------------------------------------------------------

/// What holds the minibuffer's line right now, as the footer sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Busy {
    /// A view is open — the leader included (`Footer::grabbing`).
    pub view: bool,
    /// A question is being typed on the line.
    pub typing: bool,
    /// The line's drawer is open (`crate::fold`).
    pub drawer: bool,
    /// The screen is under a fullscreen window and the footer is down to its
    /// edge: nothing on the line would be seen.
    pub covered: bool,
}

impl Busy {
    /// Is the line somebody's?
    #[must_use]
    pub const fn any(self) -> bool {
        self.view || self.typing || self.drawer || self.covered
    }

    /// Is the line OUT OF SIGHT, or covered by what the person is doing?
    ///
    /// The drawer is not: it is the line grown, and what it shows IS the
    /// event on the line — which is how its actions are read in full.
    const fn hides_the_line(self) -> bool {
        self.view || self.typing || self.covered
    }
}

/// Where an arriving event goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The minibuffer's line.
    Echo,
    /// The band, as a notification that says why.
    Notification,
}

/// Where an event goes, given what holds the line.
#[must_use]
pub const fn route(busy: Busy) -> Route {
    if busy.any() {
        Route::Notification
    } else {
        Route::Echo
    }
}

// ---------------------------------------------------------------------------
// The line's queue
// ---------------------------------------------------------------------------

/// How long an event stays on the line — the echo's time, both being one
/// thing to read.
fn on_screen() -> Duration {
    Duration::from_secs(crate::surface::ECHO_SECONDS)
}

/// How long an event has to have been on the line to count as SAID: under
/// it, the line was taken before anybody could read it, and it goes to the
/// band like the ones still waiting ([`Line::yielded`]).
pub const READ_IN: Duration = Duration::from_secs(1);

/// The most a pointer resting on the line holds an event, in echoes, counted
/// from when it went up ([`Line::tick`]).
///
/// The hold is for a hand reaching for an action or reading a long line; the
/// minibuffer is the bottom row of the screen, where a pointer is often
/// PARKED, and with the bars hidden that row is the edge that reveals them. A
/// hold with no end kept "a camera is open by …" up long after it closed,
/// with the queue — the camera closing — stuck behind it.
pub const HELD_AT_MOST: u32 = 3;

/// The event on the line: what, since when, and until when.
#[derive(Debug)]
struct Up {
    event: Event,
    since: Instant,
    until: Instant,
}

impl Up {
    fn at(event: Event, now: Instant) -> Self {
        Self {
            event,
            since: now,
            until: now + on_screen(),
        }
    }
}

/// The two events of each source that say opposite things, and the argument
/// that says which one of the source's things it is about — a network, a
/// tunnel, a device — when there are several.
const OPPOSITES: &[(&str, &str, Option<&str>)] = &[
    (CAMERA_IN_USE, CAMERA_FREE, None),
    (MICROPHONE_IN_USE, MICROPHONE_FREE, None),
    (RECORDING_STARTED, RECORDING_STOPPED, None),
    (STREAMING_STARTED, STREAMING_STOPPED, None),
    (AIRPLANE_ON, AIRPLANE_OFF, None),
    (CHARGER_PLUGGED, CHARGER_UNPLUGGED, None),
    (TAILNET_UP, TAILNET_DOWN, None),
    (TOR_UP, TOR_DOWN, None),
    (NETWORK_CONNECTED, NETWORK_LOST, Some("network")),
    (VPN_UP, VPN_DOWN, Some("vpn")),
    (
        crate::surface::BLUETOOTH_CONNECTED,
        crate::surface::BLUETOOTH_DISCONNECTED,
        Some("device"),
    ),
];

/// Whether `new` says the opposite of `old`: the other half of its pair,
/// about the same network, tunnel or device.
#[must_use]
pub fn contradicts(new: &Event, old: &Event) -> bool {
    OPPOSITES.iter().any(|(on, off, about)| {
        let paired = (new.tag == *on && old.tag == *off) || (new.tag == *off && old.tag == *on);
        paired && subject(new, *about) == subject(old, *about)
    })
}

/// The argument `about` names — which network, tunnel or device an event is
/// about — or nothing, for a source that has one thing.
fn subject<'a>(e: &'a Event, about: Option<&str>) -> Option<&'a str> {
    let name = about?;
    e.args
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}

/// The event on the minibuffer's line, and the ones waiting for it.
#[derive(Debug, Default)]
pub struct Line {
    /// On the line.
    on: Option<Up>,
    /// Waiting, first in first out.
    waiting: VecDeque<Event>,
}

impl Line {
    /// An event for the line. `person` says a person's echo holds it now.
    ///
    /// Gives back what does not fit — the event past [`EVENT_ECHOES`] waiting
    /// — for the band.
    ///
    /// AN EVENT THAT SAYS THE OPPOSITE of one on the line or waiting
    /// ([`contradicts`]) REPLACES it — on the line, with the whole time again;
    /// waiting, in its place. Behind it, the line would say, seconds late, a
    /// fact that had already flipped: "a camera is open" after it closed.
    pub fn arrive(&mut self, e: Event, person: bool, now: Instant) -> Option<Event> {
        if let Some(up) = self.on.as_mut()
            && contradicts(&e, &up.event)
        {
            *up = Up::at(e, now);
            return None;
        }
        if let Some(w) = self.waiting.iter_mut().find(|w| contradicts(&e, w)) {
            *w = e;
            return None;
        }
        if self.on.is_none() && !person && self.waiting.is_empty() {
            self.on = Some(Up::at(e, now));
            return None;
        }
        if self.waiting.len() >= EVENT_ECHOES {
            return Some(e);
        }
        self.waiting.push_back(e);
        None
    }

    /// A person's echo took the line: the event on it goes back to the FRONT
    /// of the queue, to be said whole once the line is free. Says whether one
    /// was on it.
    pub fn interrupted(&mut self) -> bool {
        match self.on.take() {
            Some(up) => {
                self.waiting.push_front(up.event);
                true
            }
            None => false,
        }
    }

    /// The footer's second: the event on the line goes when its time is up —
    /// not while a pointer rests on the line (`held`), and for a whole
    /// echo's time after the hand leaves, but never past [`HELD_AT_MOST`]
    /// echoes from when it went up — and the next one takes the line when
    /// nothing else holds it. Says whether the line changed.
    pub fn tick(&mut self, now: Instant, person: bool, held: bool) -> bool {
        let mut changed = false;
        if let Some(up) = self.on.as_mut() {
            if held {
                let most = up.since + on_screen() * HELD_AT_MOST;
                up.until = up.until.max(now + on_screen()).min(most);
            }
            if now >= up.until {
                self.on = None;
                changed = true;
            }
        }
        if self.on.is_none()
            && !person
            && let Some(e) = self.waiting.pop_front()
        {
            self.on = Some(Up::at(e, now));
            changed = true;
        }
        changed
    }

    /// The line became somebody's: what is still to be said goes to the band,
    /// in the order it would have been said. With only the drawer open the
    /// event ON the line stays, because the drawer is showing it.
    ///
    /// # The event on the line, when the line goes out of sight
    ///
    /// Already said, when it has been up [`READ_IN`] or more: it leaves the
    /// line and goes nowhere. Handed to the band it was the same fact twice —
    /// the person presses the leader two seconds after reading "charger
    /// plugged in", and the band says it again, with a sentence claiming the
    /// line was busy. Up for less, nobody could have read it, and it goes to
    /// the band with the ones still waiting.
    pub fn yielded(&mut self, busy: Busy, now: Instant) -> Vec<Event> {
        if !busy.any() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if busy.hides_the_line()
            && let Some(up) = self.on.take()
            && now.saturating_duration_since(up.since) < READ_IN
        {
            out.push(up.event);
        }
        out.extend(self.waiting.drain(..));
        out
    }

    /// The event on the line was acted on — one of its actions clicked — and
    /// leaves it: given back, so the click knows WHICH event it was about
    /// ([`launch_of`]).
    pub fn done(&mut self) -> Option<Event> {
        self.on.take().map(|up| up.event)
    }

    /// The event on the line, when there is one.
    #[must_use]
    pub fn on_the_line(&self) -> Option<&Event> {
        self.on.as_ref().map(|up| &up.event)
    }

    /// Nothing on the line and nothing waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.on.is_none() && self.waiting.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The sources
// ---------------------------------------------------------------------------

/// A reading remembered — and NOTHING remembered before the first.
#[derive(Debug, Clone)]
pub struct Edge<T> {
    last: Option<T>,
}

impl<T> Default for Edge<T> {
    fn default() -> Self {
        Self { last: None }
    }
}

impl<T: Clone + PartialEq> Edge<T> {
    /// A new reading. The one BEFORE it when it changed; `None` when it did
    /// not, and on the first reading, which is only the baseline.
    pub fn saw(&mut self, now: T) -> Option<T> {
        match self.last.replace(now.clone()) {
            Some(was) if was != now => Some(was),
            _ => None,
        }
    }
}

/// Every source, read reading by reading.
#[derive(Debug, Default)]
pub struct Watch {
    camera: Edge<bool>,
    microphone: Edge<bool>,
    recording: Edge<bool>,
    streaming: Edge<bool>,
    /// The devices connected: object path → the name drawn for it.
    radio: Edge<BTreeMap<String, String>>,
    /// The links up: interface → the network it is on.
    ways: Edge<BTreeMap<String, String>>,
    airplane: Edge<bool>,
    /// The tunnels NetworkManager holds up, by the name given to them.
    vpns: Edge<BTreeSet<String>>,
    tailnet: Edge<bool>,
    tor: Edge<bool>,
    charger: Edge<Option<bool>>,
}

/// Whether this machine is on its tailnet, when tailscaled ANSWERED.
fn tailnet_read(scene: &ronoctua_tunnel::Scene) -> Option<bool> {
    scene
        .tailscale
        .as_ref()
        .map(ronoctua_tunnel::tailscale::Scene::marked)
}

/// Whether there is a circuit through Tor, when that is KNOWN: the control
/// socket answered, or systemd says the unit is not running — or not here —
/// so there is nothing to answer. A silent socket under a unit that runs, is
/// moving, or cannot be asked about, is no reading.
fn tor_read(scene: &ronoctua_tunnel::Scene) -> Option<bool> {
    use ronoctua_tunnel::tor::Standing;
    match (&scene.tor, scene.standing) {
        (Some(tor), _) => Some(tor.marked()),
        (None, Standing::Stopped | Standing::Absent) => Some(false),
        (None, Standing::Running | Standing::Moving | Standing::Unknown) => None,
    }
}

/// One event for a flag that turned, or none.
fn turned(was: Option<bool>, now: bool, on: Event, off: Event) -> Vec<Event> {
    match was {
        Some(_) if now => vec![on],
        Some(_) => vec![off],
        None => Vec::new(),
    }
}

impl Watch {
    /// Who has a camera open — as the holders' thread sends them. Ours are
    /// left out: a holder that is, or runs under, one of
    /// [`OUR_CAMERA_HOLDERS`].
    pub fn camera(&mut self, holders: &[CameraHolder]) -> Vec<Event> {
        let theirs: Vec<&str> = holders
            .iter()
            .filter(|h| !h.ours)
            .map(|h| h.name.as_str())
            .collect();
        let open = !theirs.is_empty();
        turned(
            self.camera.saw(open),
            open,
            Event::of(CAMERA_IN_USE, key::NOTIF_PRIVACY_APP).about("programs", theirs.join(", ")),
            Event::of(CAMERA_FREE, key::NOTIF_PRIVACY_APP),
        )
    }

    /// Whether a stranger listens — see `Snapshot::someone_else_listens`.
    pub fn microphone(&mut self, s: &ronoctua_audio::Snapshot) -> Vec<Event> {
        let open = s.someone_else_listens();
        turned(
            self.microphone.saw(open),
            open,
            Event::of(MICROPHONE_IN_USE, key::NOTIF_PRIVACY_APP),
            Event::of(MICROPHONE_FREE, key::NOTIF_PRIVACY_APP),
        )
    }

    /// The studio's recording and stream. A studio that is not there is one
    /// that records nothing.
    pub fn studio(&mut self, s: Option<&ronoctua_studio::wire::State>) -> Vec<Event> {
        let recording = s.is_some_and(|s| s.recording.is_some());
        let streaming = s.is_some_and(|s| s.streaming.is_some());
        let mut out = turned(
            self.recording.saw(recording),
            recording,
            Event::of(RECORDING_STARTED, key::NOTIF_STUDIO_APP),
            Event::of(RECORDING_STOPPED, key::NOTIF_STUDIO_APP),
        );
        out.extend(turned(
            self.streaming.saw(streaming),
            streaming,
            Event::of(STREAMING_STARTED, key::NOTIF_STUDIO_APP),
            Event::of(STREAMING_STOPPED, key::NOTIF_STUDIO_APP),
        ));
        out
    }

    /// The devices that came and went since the radio's last reading.
    ///
    /// WHAT WENT is named from the OLD reading, which is the only one that
    /// still has it.
    ///
    /// A READING BLUEZ DID NOT ANSWER is not a reading, as for the network:
    /// `Manager::read` gives the empty default scene on any D-Bus error, and
    /// compared, it was every device going and — the next second — coming
    /// back. The baseline stays what the last answer said.
    pub fn radio(&mut self, scene: &ronoctua_bluetooth::Scene) -> Vec<Event> {
        if !scene.daemon {
            return Vec::new();
        }
        let now: BTreeMap<String, String> = scene
            .connected()
            .iter()
            .map(|d| (d.at.clone(), d.label.clone()))
            .collect();
        let Some(was) = self.radio.saw(now.clone()) else {
            return Vec::new();
        };
        let mut out: Vec<Event> = now
            .iter()
            .filter(|(at, _)| !was.contains_key(*at))
            .map(|(_, name)| {
                Event::of(
                    crate::surface::BLUETOOTH_CONNECTED,
                    key::NOTIF_BLUETOOTH_APP,
                )
                .about("device", name.clone())
            })
            .collect();
        out.extend(
            was.iter()
                .filter(|(at, _)| !now.contains_key(*at))
                .map(|(_, name)| {
                    Event::of(
                        crate::surface::BLUETOOTH_DISCONNECTED,
                        key::NOTIF_BLUETOOTH_APP,
                    )
                    .about("device", name.clone())
                }),
        );
        out
    }

    /// The way out: the aeroplane, the cables and radios, and the tunnels
    /// NetworkManager holds.
    ///
    /// A reading with no daemon behind it is not a reading — a restarting
    /// NetworkManager answers nothing for a second, and that is not every
    /// network dropping. And what the aeroplane took down is not said once
    /// more per network: "aeroplane mode is on" already said it.
    pub fn network(&mut self, s: &ronoctua_network::Snapshot) -> Vec<Event> {
        if !s.daemon {
            return Vec::new();
        }
        let app = key::NOTIF_NETWORK_APP;
        let airplane = s.airplane();
        let mut out = turned(
            self.airplane.saw(airplane),
            airplane,
            Event::of(AIRPLANE_ON, app),
            Event::of(AIRPLANE_OFF, app),
        );
        // A LINK THAT BELIEVES IT IS CONNECTED, the same test `linked()`
        // makes — and not one that is BEING the network, a hotspot.
        let ways: BTreeMap<String, String> = s
            .links
            .iter()
            .filter(|l| l.state == ronoctua_network::State::Connected && !l.serving)
            .map(|l| {
                (
                    l.interface.clone(),
                    l.network.clone().unwrap_or_else(|| l.interface.clone()),
                )
            })
            .collect();
        if let Some(was) = self.ways.saw(ways.clone()) {
            // WHAT WENT FIRST: a radio that moved from one network to another
            // lost one and then found the other.
            if !airplane {
                for (at, name) in &was {
                    if ways.get(at) != Some(name) {
                        out.push(Event::of(NETWORK_LOST, app).about("network", name.clone()));
                    }
                }
            }
            for (at, name) in &ways {
                if was.get(at) != Some(name) {
                    out.push(Event::of(NETWORK_CONNECTED, app).about("network", name.clone()));
                }
            }
        }
        let vpns: BTreeSet<String> = s
            .active
            .iter()
            .filter(|a| a.tunnel())
            .map(|a| a.id.clone())
            .collect();
        if let Some(was) = self.vpns.saw(vpns.clone()) {
            if !airplane {
                for name in was.difference(&vpns) {
                    out.push(Event::of(VPN_DOWN, app).about("vpn", name.clone()));
                }
            }
            for name in vpns.difference(&was) {
                out.push(Event::of(VPN_UP, app).about("vpn", name.clone()));
            }
        }
        out
    }

    /// The two tunnels that are not NetworkManager's.
    ///
    /// ONLY A READING THAT ANSWERED turns them. The tailnet's local API that
    /// failed once reads `None`, and so does Tor's control socket — which is
    /// also what a Tor that is simply not running reads; `standing`, what
    /// systemd says about its unit, is what tells the two apart. A silent
    /// reading taken as "down" came back "up" ten seconds later.
    pub fn tunnels(&mut self, scene: &ronoctua_tunnel::Scene) -> Vec<Event> {
        let app = key::NOTIF_NETWORK_APP;
        let mut out = Vec::new();
        if let Some(tailnet) = tailnet_read(scene) {
            out.extend(turned(
                self.tailnet.saw(tailnet),
                tailnet,
                Event::of(TAILNET_UP, app),
                Event::of(TAILNET_DOWN, app),
            ));
        }
        if let Some(tor) = tor_read(scene) {
            out.extend(turned(
                self.tor.saw(tor),
                tor,
                Event::of(TOR_UP, app),
                Event::of(TOR_DOWN, app),
            ));
        }
        out
    }

    /// The charger. A machine with none says nothing, and one appearing or
    /// disappearing (a dock UPower has only now noticed) is not a plug.
    pub fn power(&mut self, s: &ronoctua_power::Snapshot) -> Vec<Event> {
        let online = s.adapter_online();
        match (self.charger.saw(online), online) {
            (Some(Some(false)), Some(true)) => {
                vec![Event::of(CHARGER_PLUGGED, key::NOTIF_POWER_APP)]
            }
            (Some(Some(true)), Some(false)) => {
                vec![Event::of(CHARGER_UNPLUGGED, key::NOTIF_POWER_APP)]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn tags(events: &[Event]) -> Vec<&'static str> {
        events.iter().map(|e| e.tag).collect()
    }

    fn arg<'a>(e: &'a Event, name: &str) -> Option<&'a str> {
        e.args
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }

    // --- the sources -----------------------------------------------------

    fn holder(name: &str, ours: bool) -> CameraHolder {
        CameraHolder {
            name: name.to_owned(),
            ours,
        }
    }

    /// FAILS IF: a camera found open when the footer starts is announced, a
    /// camera opened afterwards is not, the programs are not named, or the
    /// studio's own `ffmpeg` — ours by lineage, the process that really holds
    /// the device — counts as somebody opening one, while a stranger's
    /// `ffmpeg` of the same name does not.
    #[test]
    fn a_camera_is_news_after_the_first_reading_and_never_ours() {
        let mut w = Watch::default();
        let firefox = vec![holder("firefox", false)];
        assert!(
            w.camera(&firefox).is_empty(),
            "the first reading is the baseline"
        );
        assert!(w.camera(&firefox).is_empty(), "nothing changed");
        assert_eq!(tags(&w.camera(&[])), vec![CAMERA_FREE]);
        let opened = w.camera(&[holder("obs", false), holder("zoom", false)]);
        assert_eq!(tags(&opened), vec![CAMERA_IN_USE]);
        assert_eq!(arg(&opened[0], "programs"), Some("obs, zoom"));
        // THE STUDIO'S FFMPEG IS OURS, on its own or beside a stranger.
        let mut w = Watch::default();
        assert!(w.camera(&[]).is_empty());
        assert!(
            w.camera(&[holder("ffmpeg", true)]).is_empty(),
            "the studio's preview is not news"
        );
        let both = w.camera(&[holder("ffmpeg", true), holder("ffmpeg", false)]);
        assert_eq!(tags(&both), vec![CAMERA_IN_USE], "a stranger's ffmpeg");
        assert_eq!(arg(&both[0], "programs"), Some("ffmpeg"));
    }

    fn recording(id: u32, app: Option<&str>) -> ronoctua_audio::Node {
        ronoctua_audio::Node {
            id,
            kind: Some(ronoctua_audio::Kind::Recording),
            app: app.map(ToOwned::to_owned),
            ..ronoctua_audio::Node::default()
        }
    }

    /// FAILS IF: the sound view's meter or the studio's capture — our own
    /// streams — turn the microphone into news, or a stranger's does not.
    #[test]
    fn a_microphone_is_news_only_when_it_is_not_ours() {
        let mut w = Watch::default();
        let mut s = ronoctua_audio::Snapshot::default();
        assert!(w.microphone(&s).is_empty(), "the first reading");
        s.nodes
            .push(recording(7, Some(ronoctua_audio::pipewire::OWN_STREAM)));
        assert!(w.microphone(&s).is_empty(), "the meter is ours");
        s.nodes.push(recording(8, Some("a recorder")));
        assert_eq!(tags(&w.microphone(&s)), vec![MICROPHONE_IN_USE]);
        s.nodes.retain(|n| n.id != 8);
        assert_eq!(tags(&w.microphone(&s)), vec![MICROPHONE_FREE]);
        // A MICROPHONE ALREADY OPEN WHEN THE FOOTER STARTS is not news.
        let mut w = Watch::default();
        let mut busy = ronoctua_audio::Snapshot::default();
        busy.nodes.push(recording(9, Some("a recorder")));
        assert!(w.microphone(&busy).is_empty());
    }

    /// FAILS IF: the studio's recording or stream is not said when it starts
    /// and ends, is said for the state the footer found, or a studio that
    /// went away mid-recording is not the recording ending.
    #[test]
    fn the_studio_says_when_it_records_and_streams() {
        let rolling = || ronoctua_studio::wire::Rolling {
            since: 1,
            target: "x".to_owned(),
        };
        let mut w = Watch::default();
        let mut s = ronoctua_studio::wire::State::default();
        assert!(w.studio(Some(&s)).is_empty());
        s.recording = Some(rolling());
        assert_eq!(tags(&w.studio(Some(&s))), vec![RECORDING_STARTED]);
        s.streaming = Some(rolling());
        assert_eq!(tags(&w.studio(Some(&s))), vec![STREAMING_STARTED]);
        assert_eq!(
            tags(&w.studio(None)),
            vec![RECORDING_STOPPED, STREAMING_STOPPED]
        );
        let mut w = Watch::default();
        assert!(w.studio(Some(&s)).is_empty(), "found recording: not news");
    }

    fn device(at: &str, label: &str) -> ronoctua_bluetooth::Device {
        ronoctua_bluetooth::Device {
            at: at.to_owned(),
            label: label.to_owned(),
            connected: true,
            ..ronoctua_bluetooth::Device::default()
        }
    }

    /// FAILS IF: every device connected when the footer starts is announced
    /// as connecting — the burst the radio used to raise at every start — or
    /// a device coming or going is not, or the one that went loses its name.
    #[test]
    fn the_radio_says_what_came_and_went_and_nothing_at_the_start() {
        let mut w = Watch::default();
        let mut scene = ronoctua_bluetooth::Scene {
            daemon: true,
            ..ronoctua_bluetooth::Scene::default()
        };
        scene.devices.push(device("/a", "Keyboard"));
        scene.devices.push(device("/b", "Headset"));
        assert!(w.radio(&scene).is_empty(), "the start is no burst");
        scene.devices.retain(|d| d.at != "/b");
        scene.devices.push(device("/c", "Mouse"));
        let said = w.radio(&scene);
        assert_eq!(
            tags(&said),
            vec![
                crate::surface::BLUETOOTH_CONNECTED,
                crate::surface::BLUETOOTH_DISCONNECTED
            ]
        );
        assert_eq!(arg(&said[0], "device"), Some("Mouse"));
        assert_eq!(arg(&said[1], "device"), Some("Headset"));
    }

    /// FAILS IF: a reading BlueZ did not answer — `Manager::read` gives the
    /// empty default scene on any D-Bus error — is taken as every device
    /// going and then coming back; or it moves the baseline, so the first
    /// answered reading after a silent start announces every device.
    #[test]
    fn a_reading_bluez_did_not_answer_is_no_reading() {
        let mut w = Watch::default();
        let mut answered = ronoctua_bluetooth::Scene {
            daemon: true,
            ..ronoctua_bluetooth::Scene::default()
        };
        answered.devices.push(device("/a", "Keyboard"));
        assert!(w.radio(&answered).is_empty(), "the baseline");
        assert!(w.radio(&ronoctua_bluetooth::Scene::default()).is_empty());
        assert!(w.radio(&answered).is_empty(), "nothing came back");
        // AND A SILENT START IS NO BASELINE.
        let mut w = Watch::default();
        assert!(w.radio(&ronoctua_bluetooth::Scene::default()).is_empty());
        assert!(
            w.radio(&answered).is_empty(),
            "the first ANSWERED reading is the baseline"
        );
    }

    fn link(interface: &str, network: &str, connected: bool) -> ronoctua_network::Link {
        ronoctua_network::Link {
            medium: ronoctua_network::Medium::Radio,
            interface: interface.to_owned(),
            network: Some(network.to_owned()),
            state: if connected {
                ronoctua_network::State::Connected
            } else {
                ronoctua_network::State::Disconnected
            },
            rate_kbps: 0,
            signal: 0,
            band: ronoctua_network::Band::Other,
            address: None,
            gateway: None,
            dns: Vec::new(),
            serving: false,
        }
    }

    fn tunnel(id: &str) -> ronoctua_network::Active {
        ronoctua_network::Active {
            id: id.to_owned(),
            kind: "wireguard".to_owned(),
            activated: true,
            ..ronoctua_network::Active::default()
        }
    }

    /// FAILS IF: the way out stops saying a network or a tunnel coming and
    /// going, says the one it found at the start, says each network again
    /// under the aeroplane, or reads a silent daemon as everything dropping.
    #[test]
    fn the_way_out_says_networks_tunnels_and_the_aeroplane() {
        let mut w = Watch::default();
        let mut s = ronoctua_network::Snapshot {
            daemon: true,
            networking: true,
            radio: true,
            ..ronoctua_network::Snapshot::default()
        };
        s.links.push(link("wlan0", "home", true));
        assert!(w.network(&s).is_empty(), "the first reading");
        // A SILENT DAEMON is not a reading.
        assert!(w.network(&ronoctua_network::Snapshot::default()).is_empty());
        s.active.push(tunnel("office"));
        let up = w.network(&s);
        assert_eq!(tags(&up), vec![VPN_UP]);
        assert_eq!(arg(&up[0], "vpn"), Some("office"));
        // MOVING NETWORKS is losing one and finding the other.
        s.links[0].network = Some("cafe".to_owned());
        let moved = w.network(&s);
        assert_eq!(tags(&moved), vec![NETWORK_LOST, NETWORK_CONNECTED]);
        assert_eq!(arg(&moved[0], "network"), Some("home"));
        assert_eq!(arg(&moved[1], "network"), Some("cafe"));
        // THE AEROPLANE says it once, for everything it took down.
        s.networking = false;
        s.links[0].state = ronoctua_network::State::Disconnected;
        s.active.clear();
        assert_eq!(tags(&w.network(&s)), vec![AIRPLANE_ON]);
        s.networking = true;
        assert_eq!(tags(&w.network(&s)), vec![AIRPLANE_OFF]);
        s.links[0].state = ronoctua_network::State::Connected;
        assert_eq!(tags(&w.network(&s)), vec![NETWORK_CONNECTED]);
        // AND A CABLE OUT WITH NO AEROPLANE is a network lost.
        s.links[0].state = ronoctua_network::State::Disconnected;
        assert_eq!(tags(&w.network(&s)), vec![NETWORK_LOST]);
    }

    fn tailnet(running: bool) -> Option<ronoctua_tunnel::tailscale::Scene> {
        Some(ronoctua_tunnel::tailscale::Scene {
            running,
            ..ronoctua_tunnel::tailscale::Scene::default()
        })
    }

    /// FAILS IF: the tailnet or Tor stop being said when they come and go,
    /// or are said for what the footer found at the start.
    #[test]
    fn the_two_other_tunnels_say_when_they_turn() {
        let mut w = Watch::default();
        let mut scene = ronoctua_tunnel::Scene {
            tailscale: tailnet(true),
            standing: ronoctua_tunnel::tor::Standing::Stopped,
            ..ronoctua_tunnel::Scene::default()
        };
        assert!(w.tunnels(&scene).is_empty(), "found on the tailnet");
        scene.tor = Some(ronoctua_tunnel::tor::Scene {
            established: true,
            ..ronoctua_tunnel::tor::Scene::default()
        });
        scene.standing = ronoctua_tunnel::tor::Standing::Running;
        assert_eq!(tags(&w.tunnels(&scene)), vec![TOR_UP]);
        // DOWN, AS THE DAEMONS SAY IT: tailscaled answering that it is not
        // running, and the Tor unit stopped.
        scene.tailscale = tailnet(false);
        scene.tor = None;
        scene.standing = ronoctua_tunnel::tor::Standing::Stopped;
        assert_eq!(tags(&w.tunnels(&scene)), vec![TAILNET_DOWN, TOR_DOWN]);
        scene.tailscale = tailnet(true);
        assert_eq!(tags(&w.tunnels(&scene)), vec![TAILNET_UP]);
    }

    /// FAILS IF: a reading that did not answer — the tailnet's local API
    /// failing once, or Tor's control socket silent while its unit RUNS — is
    /// taken as the tunnel going down, to come "up" again on the next one.
    #[test]
    fn a_tunnel_that_did_not_answer_is_no_reading() {
        let mut w = Watch::default();
        let up = ronoctua_tunnel::Scene {
            tailscale: tailnet(true),
            tor: Some(ronoctua_tunnel::tor::Scene {
                established: true,
                ..ronoctua_tunnel::tor::Scene::default()
            }),
            standing: ronoctua_tunnel::tor::Standing::Running,
            ..ronoctua_tunnel::Scene::default()
        };
        assert!(w.tunnels(&up).is_empty(), "the baseline");
        let silent = ronoctua_tunnel::Scene {
            standing: ronoctua_tunnel::tor::Standing::Running,
            ..ronoctua_tunnel::Scene::default()
        };
        assert!(w.tunnels(&silent).is_empty(), "no answer is no reading");
        assert!(w.tunnels(&up).is_empty(), "nothing came back up");
        // A UNIT THAT CANNOT BE ASKED says nothing either way.
        assert!(w.tunnels(&ronoctua_tunnel::Scene::default()).is_empty());
        assert!(w.tunnels(&up).is_empty());
    }

    fn charger(online: Option<bool>) -> ronoctua_power::Snapshot {
        ronoctua_power::Snapshot {
            adapter: online.map(|online| ronoctua_power::Adapter {
                online,
                ..ronoctua_power::Adapter::default()
            }),
            ..ronoctua_power::Snapshot::default()
        }
    }

    /// FAILS IF: the charger is not said when it is plugged or pulled, is
    /// said at the start, or a charger UPower only now noticed is a plug.
    #[test]
    fn the_charger_says_plugged_and_unplugged() {
        let mut w = Watch::default();
        assert!(
            w.power(&charger(Some(false))).is_empty(),
            "the first reading"
        );
        assert_eq!(tags(&w.power(&charger(Some(true)))), vec![CHARGER_PLUGGED]);
        assert!(w.power(&charger(Some(true))).is_empty());
        assert_eq!(
            tags(&w.power(&charger(Some(false)))),
            vec![CHARGER_UNPLUGGED]
        );
        assert!(w.power(&charger(None)).is_empty(), "a charger gone");
        assert!(w.power(&charger(Some(true))).is_empty(), "and one found");
    }

    // --- where it goes -----------------------------------------------------

    /// FAILS IF: an event goes to the line while the line is somebody's, or
    /// to the band while it is free.
    #[test]
    fn a_free_line_says_it_and_a_busy_one_hands_it_to_the_band() {
        assert_eq!(route(Busy::default()), Route::Echo);
        for busy in [
            Busy {
                view: true,
                ..Busy::default()
            },
            Busy {
                typing: true,
                ..Busy::default()
            },
            Busy {
                drawer: true,
                ..Busy::default()
            },
            Busy {
                covered: true,
                ..Busy::default()
            },
        ] {
            assert_eq!(route(busy), Route::Notification, "{busy:?}");
        }
    }

    fn e(tag: &'static str) -> Event {
        Event::of(tag, key::NOTIF_NETWORK_APP)
    }

    /// FAILS IF: the queue stops being first-in first-out, an event stops
    /// being shown for the echo's time, or a burst past the cap waits
    /// instead of going to the band. (Six events of six sources: two of one
    /// source would be the contradiction that replaces.)
    #[test]
    fn the_queue_says_them_in_order_and_hands_over_the_overflow() {
        let t0 = Instant::now();
        let mut line = Line::default();
        assert_eq!(line.arrive(e(TOR_UP), false, t0), None);
        for tag in [TAILNET_UP, AIRPLANE_ON, CHARGER_PLUGGED, MICROPHONE_IN_USE] {
            assert_eq!(line.arrive(e(tag), false, t0), None, "{tag} waits");
        }
        let over = line.arrive(e(CAMERA_IN_USE), false, t0);
        assert_eq!(
            over.map(|e| e.tag),
            Some(CAMERA_IN_USE),
            "the fifth waiting"
        );
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(TOR_UP));
        let step = on_screen();
        assert!(!line.tick(t0 + step / 2, false, false), "not yet");
        let mut said = vec![TOR_UP];
        let mut now = t0;
        for _ in 0..4 {
            now += step;
            assert!(line.tick(now, false, false));
            said.push(line.on_the_line().expect("the next one").tag);
        }
        assert_eq!(
            said,
            vec![
                TOR_UP,
                TAILNET_UP,
                AIRPLANE_ON,
                CHARGER_PLUGGED,
                MICROPHONE_IN_USE
            ]
        );
        assert!(line.tick(now + step, false, false));
        assert!(line.is_empty());
    }

    /// FAILS IF: a person's echo waits behind an event, or the event it
    /// interrupted is lost instead of coming back FIRST once the line is free.
    #[test]
    fn a_person_interrupts_and_the_event_comes_back_first() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        line.arrive(e(TAILNET_UP), false, t0);
        assert!(line.interrupted(), "the echo took the line");
        assert_eq!(line.on_the_line(), None);
        // WHILE THE PERSON'S ECHO IS UP nothing takes the line, and an event
        // arriving waits.
        assert_eq!(line.arrive(e(VPN_UP), true, t0), None);
        assert!(!line.tick(t0, true, false));
        assert_eq!(line.on_the_line(), None);
        // THE ECHO WENT: the interrupted one is first, whole.
        assert!(line.tick(t0, false, false));
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(TOR_UP));
        let step = on_screen();
        assert!(line.tick(t0 + step, false, false));
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(TAILNET_UP));
        assert!(line.tick(t0 + step * 2, false, false));
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(VPN_UP));
    }

    /// FAILS IF: an event under the pointer expires while somebody is reading
    /// it or reaching for its action, or stays for ever once the hand leaves.
    #[test]
    fn the_pointer_on_the_line_holds_it() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        let step = on_screen();
        assert!(!line.tick(t0 + step + step / 2, false, true), "held");
        assert!(line.on_the_line().is_some());
        assert!(
            !line.tick(t0 + step * 2, false, false),
            "a whole time more after the hand"
        );
        assert!(line.tick(t0 + step * 3, false, false), "and then it goes");
        assert!(line.is_empty());
    }

    /// FAILS IF: a pointer that never leaves the line — parked at the bottom
    /// of the screen, where the line is — holds an event past
    /// [`HELD_AT_MOST`] echoes from when it went up, and the one waiting
    /// behind it with it.
    #[test]
    fn a_parked_pointer_holds_an_event_only_so_long() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(CAMERA_IN_USE), false, t0);
        line.arrive(e(TOR_UP), false, t0);
        let step = on_screen();
        let mut now = t0;
        while now < t0 + step * HELD_AT_MOST - step / 4 {
            now += step / 4;
            line.tick(now, false, true);
            assert_eq!(
                line.on_the_line().map(|e| e.tag),
                Some(CAMERA_IN_USE),
                "held at {:?}",
                now - t0
            );
        }
        assert!(line.tick(t0 + step * HELD_AT_MOST, false, true), "it goes");
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(TOR_UP));
    }

    fn about(tag: &'static str, name: &'static str, value: &str) -> Event {
        e(tag).about(name, value)
    }

    /// FAILS IF: an event that says the opposite of one on the line or
    /// waiting — from the same source, about the same network, tunnel or
    /// device — queues behind it instead of replacing it, so the line says,
    /// seconds late, a fact that has already flipped; or one about ANOTHER
    /// network replaces it.
    #[test]
    fn a_contradiction_replaces_what_it_contradicts() {
        let t0 = Instant::now();
        let step = on_screen();
        // ON THE LINE: replaced there, with the whole time again.
        let mut line = Line::default();
        line.arrive(e(CAMERA_IN_USE), false, t0);
        assert_eq!(line.arrive(e(CAMERA_FREE), false, t0 + step / 2), None);
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(CAMERA_FREE));
        assert!(!line.tick(t0 + step, false, false), "a whole time from now");
        assert!(line.tick(t0 + step + step / 2, false, false));
        assert!(line.is_empty(), "and nothing was left behind it");
        // WAITING: replaced where it waits, its place kept.
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        line.arrive(about(NETWORK_LOST, "network", "home"), false, t0);
        line.arrive(e(CHARGER_PLUGGED), false, t0);
        line.arrive(about(NETWORK_CONNECTED, "network", "cafe"), false, t0);
        line.arrive(about(NETWORK_CONNECTED, "network", "home"), false, t0);
        line.arrive(e(CHARGER_UNPLUGGED), false, t0);
        let mut said = vec![
            line.on_the_line()
                .map(|e| (e.tag, arg(e, "network").map(ToOwned::to_owned))),
        ];
        for n in 1..=4 {
            line.tick(t0 + step * n, false, false);
            said.push(
                line.on_the_line()
                    .map(|e| (e.tag, arg(e, "network").map(ToOwned::to_owned))),
            );
        }
        assert_eq!(
            said,
            vec![
                Some((TOR_UP, None)),
                Some((NETWORK_CONNECTED, Some("home".to_owned()))),
                Some((CHARGER_UNPLUGGED, None)),
                Some((NETWORK_CONNECTED, Some("cafe".to_owned()))),
                None,
            ]
        );
        // THE PAIRS, and only the pairs.
        assert!(contradicts(&e(MICROPHONE_FREE), &e(MICROPHONE_IN_USE)));
        assert!(contradicts(&e(AIRPLANE_ON), &e(AIRPLANE_OFF)));
        assert!(contradicts(&e(TAILNET_DOWN), &e(TAILNET_UP)));
        assert!(contradicts(&e(RECORDING_STOPPED), &e(RECORDING_STARTED)));
        assert!(contradicts(&e(STREAMING_STARTED), &e(STREAMING_STOPPED)));
        assert!(contradicts(&e(TOR_DOWN), &e(TOR_UP)));
        assert!(contradicts(
            &about(VPN_DOWN, "vpn", "office"),
            &about(VPN_UP, "vpn", "office")
        ));
        assert!(!contradicts(
            &about(VPN_DOWN, "vpn", "office"),
            &about(VPN_UP, "vpn", "home")
        ));
        assert!(contradicts(
            &about(crate::surface::BLUETOOTH_DISCONNECTED, "device", "Headset"),
            &about(crate::surface::BLUETOOTH_CONNECTED, "device", "Headset")
        ));
        assert!(!contradicts(
            &about(crate::surface::BLUETOOTH_DISCONNECTED, "device", "Headset"),
            &about(crate::surface::BLUETOOTH_CONNECTED, "device", "Mouse")
        ));
        assert!(!contradicts(&e(CAMERA_IN_USE), &e(CAMERA_IN_USE)));
        assert!(!contradicts(&e(TOR_UP), &e(TAILNET_DOWN)));
    }

    /// FAILS IF: a line that became somebody's keeps what it had to say for
    /// later — a camera said two minutes late — or throws a WAITING one away
    /// instead of handing it to the band; or the drawer, which SHOWS the
    /// event on the line, takes it away; or an event that was on the line
    /// too briefly to be read is lost.
    #[test]
    fn a_line_that_becomes_busy_hands_over_what_was_not_said() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        line.arrive(e(TAILNET_UP), false, t0);
        assert!(
            line.yielded(Busy::default(), t0).is_empty(),
            "free: nothing moves"
        );
        let drawer = line.yielded(
            Busy {
                drawer: true,
                ..Busy::default()
            },
            t0,
        );
        assert_eq!(tags(&drawer), vec![TAILNET_UP]);
        assert_eq!(line.on_the_line().map(|e| e.tag), Some(TOR_UP));
        let view = line.yielded(
            Busy {
                view: true,
                ..Busy::default()
            },
            t0 + READ_IN / 2,
        );
        assert_eq!(tags(&view), vec![TOR_UP], "up too briefly to be read");
        assert!(line.is_empty());
    }

    /// FAILS IF: an event the person already read on the line — up for
    /// [`READ_IN`] or more — is raised AGAIN on the band when the line
    /// becomes busy (the leader, a view, a question), with a sentence saying
    /// the line was busy when it was not; or the ones waiting behind it stop
    /// going to the band.
    #[test]
    fn an_event_already_read_is_not_said_twice() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(CHARGER_PLUGGED), false, t0);
        line.arrive(e(TOR_UP), false, t0);
        let handed = line.yielded(
            Busy {
                view: true,
                ..Busy::default()
            },
            t0 + READ_IN * 2,
        );
        assert_eq!(tags(&handed), vec![TOR_UP], "only the one never shown");
        assert!(line.is_empty());
    }

    /// FAILS IF: clicking an event's action leaves the event on the line, or
    /// the click cannot tell which event it acted on.
    #[test]
    fn an_event_acted_on_leaves_the_line() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        assert_eq!(line.done().map(|e| e.tag), Some(TOR_UP));
        assert!(line.is_empty());
        assert_eq!(line.done(), None);
    }

    // --- the words ---------------------------------------------------------

    /// FAILS IF: an event that went to the band stops saying why it is there
    /// — FIRST in its body, where the band's one line of body shows it — or
    /// loses its own body, its tag, or its sender.
    #[test]
    fn the_busy_body_carries_the_sentence() {
        let t = texts();
        let e = Event::of(NETWORK_LOST, key::NOTIF_NETWORK_APP).about("network", "home");
        let n = notification(&e, &t);
        assert_eq!(
            n.summary,
            t.t_with("notif-network-lost-summary", &[("network", "home")])
        );
        assert!(n.summary.contains("home"), "{}", n.summary);
        assert!(
            n.body.starts_with(&t.t(key::NOTIF_MINIBUFFER_WAS_BUSY)),
            "{}",
            n.body
        );
        assert!(n.body.ends_with(&e.body(&t)), "{}", n.body);
        assert_eq!(n.tags, vec![NETWORK_LOST.to_owned()]);
        assert_eq!(n.app, t.t(key::NOTIF_NETWORK_APP));
    }

    /// FAILS IF: the BAND never shows why an event came to it — the reason
    /// only in a body the band does not draw (a `brief` entry is one line),
    /// or after the event's own body, where a line of ordinary width cuts it.
    /// Read off the lines the band DRAWS, in both languages, at the width of
    /// a small laptop.
    #[test]
    fn the_band_says_why_an_event_is_there() {
        let book = ronoctua_notify::Book::distro();
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let words = crate::band::Words { texts: &t };
            let reason = t.t(key::NOTIF_MINIBUFFER_WAS_BUSY);
            // ITS FIRST WORDS, which is what survives a cut.
            let opening: String = reason
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            for tag in ALL {
                if matches!(
                    *tag,
                    crate::surface::BLUETOOTH_CONNECTED | crate::surface::BLUETOOTH_DISCONNECTED
                ) || matches!(
                    *tag,
                    crate::surface::SOFTWARE_INSTALLED | crate::surface::SOFTWARE_WEB_CREATED
                ) {
                    // THE ROWS THEY ALWAYS HAD: the radio's are hidden by its
                    // filters, and an install's row is the one it had.
                    continue;
                }
                let n = notification(&Event::of(tag, key::NOTIF_NETWORK_APP), &t);
                let presentation = book.decide(&n);
                let shown = ronoctua_notify::Shown {
                    notification: n,
                    presentation,
                    since: Instant::now(),
                };
                let drawn = crate::band::lines(Some(&shown), 100, None, None, false, false, &words);
                let text: String = drawn
                    .iter()
                    .flat_map(|l| l.pieces.iter().map(|p| p.text.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    text.contains(&opening),
                    "{language} {tag}: the band drew {text:?}"
                );
            }
        }
    }

    /// Every event tag this module raises.
    const ALL: &[&str] = &[
        CAMERA_IN_USE,
        CAMERA_FREE,
        MICROPHONE_IN_USE,
        MICROPHONE_FREE,
        RECORDING_STARTED,
        RECORDING_STOPPED,
        STREAMING_STARTED,
        STREAMING_STOPPED,
        NETWORK_CONNECTED,
        NETWORK_LOST,
        AIRPLANE_ON,
        AIRPLANE_OFF,
        VPN_UP,
        VPN_DOWN,
        TAILNET_UP,
        TAILNET_DOWN,
        TOR_UP,
        TOR_DOWN,
        CHARGER_PLUGGED,
        CHARGER_UNPLUGGED,
        crate::surface::BLUETOOTH_CONNECTED,
        crate::surface::BLUETOOTH_DISCONNECTED,
        crate::surface::SOFTWARE_INSTALLED,
        crate::surface::SOFTWARE_WEB_CREATED,
    ];

    /// FAILS IF: an event's tag has no entry in the generated catalogue —
    /// its notification would be decided by no row — or an entry drawn on
    /// the band by default is hidden, or the two Bluetooth fallbacks stop
    /// being hidden by the filters that match their tags.
    #[test]
    fn every_event_has_its_row_and_the_radio_stays_quiet() {
        let book = ronoctua_notify::Book::distro();
        let t = texts();
        for tag in ALL {
            assert!(book.entry(tag).is_some(), "{tag} has no catalogue entry");
            let n = notification(&Event::of(tag, key::NOTIF_NETWORK_APP), &t);
            let p = book.decide(&n);
            assert_eq!(p.catalogue.as_deref(), Some(*tag), "{tag}");
            let quiet = matches!(
                *tag,
                crate::surface::BLUETOOTH_CONNECTED | crate::surface::BLUETOOTH_DISCONNECTED
            );
            assert_eq!(
                p.show == ronoctua_notify::Show::Hidden,
                quiet,
                "{tag}: {:?}",
                p.show
            );
        }
    }

    /// FAILS IF: the program installed stops offering to open it on the
    /// line, or an event whose entry has no action grows one.
    #[test]
    fn the_actions_of_an_event_come_from_its_entry() {
        let book = ronoctua_notify::Book::distro();
        let (_, registry) = crate::leader::default_of().expect("the registry");
        let t = texts();
        let installed = Event::installed(crate::surface::SOFTWARE_INSTALLED, "GIMP", None);
        let actions = actions_of(&installed, &book, &registry, &t);
        assert_eq!(
            actions,
            vec![(
                t.t("notif-action-software-launch-latest"),
                "software-launch-latest"
            )]
        );
        assert_eq!(
            installed.summary(&t),
            t.t_with("notif-software-installed-summary", &[("app", "GIMP")])
        );
        let web = Event::installed(crate::surface::SOFTWARE_WEB_CREATED, "Mail", None);
        assert_eq!(actions_of(&web, &book, &registry, &t).len(), 1);
        assert!(actions_of(&e(NETWORK_LOST), &book, &registry, &t).is_empty());
    }

    /// FAILS IF: "Open it" on an install's line launches anything but the
    /// program THAT line names — the latest install, when a newer one
    /// finished while this line was up — or another command launches it.
    #[test]
    fn the_install_on_the_line_is_the_one_opened() {
        let gimp = Event::installed(
            crate::surface::SOFTWARE_INSTALLED,
            "GIMP",
            Some("org.gimp.GIMP.desktop"),
        );
        assert_eq!(
            launch_of(&gimp, LAUNCH_LATEST),
            Some("org.gimp.GIMP.desktop")
        );
        assert_eq!(launch_of(&gimp, "files-open"), None);
        let unknown = Event::installed(crate::surface::SOFTWARE_INSTALLED, "GIMP", None);
        assert_eq!(launch_of(&unknown, LAUNCH_LATEST), None);
        let (_, registry) = crate::leader::default_of().expect("the registry");
        assert!(registry.search(LAUNCH_LATEST).is_some());
    }
}
