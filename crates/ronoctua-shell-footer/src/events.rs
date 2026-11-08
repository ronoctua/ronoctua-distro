//! What the machine did, and every notification, said on the minibuffer's
//! line.
//!
//! # Why the minibuffer, and not the band
//!
//! The owner's choice (2026-09-28): a camera or a microphone opening and
//! closing, the camera's own recording and stream, a Bluetooth device, a
//! network, a tunnel or the aeroplane coming and going, the charger, a
//! program installed — and what a brain module tells, the disk making room by
//! itself among them — are EVENTS — facts about the machine that are worth one
//! line and nothing more. A fact that goes by is what the minibuffer's line
//! already does for the shell's own answers (the echo), so an event is said
//! there, in its catalogue entry's summary, for as long as any echo lasts.
//!
//! AND EVERY NOTIFICATION TAKES THE SAME WAY (the owner, 2026-11-01): a
//! program's `Notify`, the ones the footer raises, the brain's, the
//! out-of-memory one. The filters speak first (`Book::decide`), and one they
//! hide is shown nowhere; a shown one is said on the line when it is free —
//! what the band's single line would say, in the band's loud ink when it is
//! loud, for the time its `life` gives, its actions at the end — and recorded
//! as said there ([`deliver`]). [`route`] is the one answer to where either
//! goes.
//!
//! And it is said whether the modeline is showing or not: with the footer
//! hidden, what the line says brings the minibuffer up for a moment.
//!
//! # Busy
//!
//! The line is somebody's while a view is open (the leader included), a
//! question is being typed, the line's own drawer is open, or the screen is
//! under a fullscreen window and the footer is down to its edge. What arrives
//! then does NOT wait for the line: a camera that opened while somebody was
//! typing in the launcher is news now, not in two minutes. It goes to the
//! band instead — an event through the same door every notification takes,
//! under its own tag.
//!
//! What is already ON the line when it becomes busy was said: it goes
//! nowhere, unless it was up too briefly to be read ([`READ_IN`]) — or it is
//! a notification that offers something to do, which the band keeps for the
//! rest of its life, unanswered ([`Line::yielded`]).
//!
//! The Bluetooth devices keep the tags they always had, so the two filters
//! that quiet a device coming and going still quiet the fallback.
//!
//! # The queue
//!
//! Events come in bursts — a resume brings the network, the tunnel and the
//! headset back in one second — so the line keeps a queue, each shown for its
//! time, ordered by level as the band orders its own: a higher level goes
//! ahead, and takes the line from a lower one ([`Line::arrive`]). A person's
//! own echo (the shell answering something that was just asked for) takes
//! the line AT ONCE, and what it interrupted goes back to the FRONT of the
//! queue. Past [`EVENT_ECHOES`] waiting, the next one goes
//! to the band: a queue that grows without end is a line that says, a minute
//! late, what is no longer true — and for the same reason an event that says
//! the opposite of one on the line or waiting REPLACES it ([`contradicts`]),
//! and a pointer resting on the line holds what it says for [`HELD_AT_MOST`]
//! of its times at the most.
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
//! handed to it. The footer (`surface`) reads, asks, and paints.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use crate::key;

/// How many events and notifications may WAIT for the line, beside the one
/// on it.
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
/// A program in a terminal nobody is looking at rang its bell.
pub const TERMINAL_BELL: &str = "terminal-bell";
/// A program in a terminal nobody is looking at is blocked on the person
/// (OSC 7501's `blocked`): it cannot go on until they answer.
pub const TERMINAL_WAITING: &str = "terminal-waiting";
/// A long command ended in a terminal nobody is looking at (OSC 133 `D`).
pub const TERMINAL_FINISHED: &str = "terminal-finished";
/// A program in a terminal nobody is looking at asked for a desktop
/// notification (OSC 9/777) — a NOTIFICATION the footer raises, with the
/// program's own words, and not an event: what it says is the program's.
pub const TERMINAL_NOTICE: &str = "terminal-notice";

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

/// WHO HOLDS A CAMERA, from what the walk of `/proc` `found`: each holder
/// told ours or theirs by its lineage — and THE SHELL'S OWN READING left
/// out, for the event and the mark alike, because it is nobody using a
/// camera.
///
/// Reading the cameras opens every node for a moment: the camera's settings
/// read them while shown, in the footer's region and in their window, and a
/// control is written the same way. A walk that caught one of those moments
/// said "camera in use: ronoctua-shell" and lit the mark, two seconds later
/// "free". No process of the shell's films: what streams a camera is the
/// studio's `ffmpeg` (`the-camera-has-one-door`), which has its own name. So
/// a holder NAMED as the shell's executable (`shell`, [`process_name`]) is
/// left out — by its own name, never its lineage: what the shell starts (a
/// browser, a terminal's `ffmpeg`) runs under it with a name of its own, and
/// is still somebody. With no name for the shell, nobody is left out.
#[must_use]
pub fn holders_of(
    found: Vec<ronoctua_memory::camera::Holder>,
    shell: Option<&str>,
) -> Vec<CameraHolder> {
    let mut holders: Vec<CameraHolder> = found
        .into_iter()
        .filter(|h| shell.is_none_or(|s| h.name != s))
        .map(|h| CameraHolder {
            ours: h.under(OUR_CAMERA_HOLDERS),
            name: h.name,
        })
        .collect();
    holders.sort();
    holders.dedup();
    holders
}

/// The name `/proc` gives a process of the executable at `exe`: its file
/// name cut to the kernel's fifteen bytes (`TASK_COMM_LEN`) — a wrapped
/// program's `.ronoctua-shell-wrapped` runs as `.ronoctua-shell`. The
/// shell's doors are started from this same file (`current_exe`), so they
/// bear the footer's name.
#[must_use]
pub fn process_name(exe: &std::path::Path) -> Option<String> {
    const COMM: usize = 15;
    let name = exe.file_name()?.to_str()?;
    let mut end = name.len().min(COMM);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    Some(name[..end].to_owned()).filter(|n| !n.is_empty())
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
    /// The COUNTS among them, which cross to Fluent as numbers: a selector
    /// with a numeric branch handed the string `"1"` looks for a variant
    /// called `1`, finds none and falls to the default without a word
    /// (`numeric-selectors-get-numbers`). A brain module's event carries them.
    pub counts: Vec<(&'static str, i64)>,
    /// What its actions are about, when they are about one thing: the
    /// argument a command of its entry runs with ([`subject_of`]) — the
    /// desktop id of the program an install put in the launcher, the
    /// terminal a terminal's signal shows. It crosses to the notification
    /// the event becomes on the band ([`notification`]), whose button runs
    /// on it too.
    pub subject: Option<String>,
    /// Its level, as the filters decide it for the notification it would be
    /// on the band — what orders the line's queue as it orders the band's
    /// ([`Line::arrive`]). The footer decides it before the line hears the
    /// event (`Footer::said_now`); until then it is the lowest, and an event
    /// nobody decided goes ahead of nothing.
    pub level: u8,
}

impl Event {
    /// An event with nothing to fill in.
    #[must_use]
    pub const fn of(tag: &'static str, app: &'static str) -> Self {
        Self {
            tag,
            app,
            args: Vec::new(),
            counts: Vec::new(),
            subject: None,
            level: 0,
        }
    }

    /// The same, with one argument more.
    #[must_use]
    pub fn about(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    /// The same, with a count more — crossing to Fluent as a number.
    #[must_use]
    pub fn counting(mut self, name: &'static str, value: i64) -> Self {
        self.counts.push((name, value));
        self
    }

    /// The same, about `subject` — what its actions run on ([`Event::subject`]).
    #[must_use]
    pub fn on(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// A program the software centre put in the launcher — `tag` is
    /// `software-installed` or `software-web-created`, `app` its name, and
    /// `opens` the desktop id its action launches — the event's subject.
    #[must_use]
    pub fn installed(tag: &'static str, app: &str, opens: Option<&str>) -> Self {
        let mut e = Self::of(tag, key::NOTIF_SOFTWARE_APP).about("app", app);
        e.subject = opens.map(ToOwned::to_owned);
        e
    }

    /// The arguments, as the localiser takes them — the counts as numbers.
    fn pairs(&self) -> Vec<(&str, ronoctua_i18n::Arg<'_>)> {
        self.args
            .iter()
            .map(|(n, v)| (*n, ronoctua_i18n::Arg::Text(v.as_str())))
            .chain(
                self.counts
                    .iter()
                    .map(|(n, v)| (*n, ronoctua_i18n::Arg::Number(*v))),
            )
            .collect()
    }

    /// What the line says: the entry's summary.
    #[must_use]
    pub fn summary(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t_args(&format!("notif-{}-summary", self.tag), &self.pairs())
    }

    /// The entry's body — read only when the event goes to the band.
    #[must_use]
    pub fn body(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t_args(&format!("notif-{}-body", self.tag), &self.pairs())
    }
}

/// The event as a notification the catalogue recognises by its tag — for
/// when the line was somebody else's: its entry's summary and body, as any
/// notification of the band says them.
#[must_use]
pub fn notification(e: &Event, texts: &ronoctua_i18n::Texts) -> ronoctua_notify::Notification {
    let mut n = ronoctua_notify::Notification::fresh_one(
        0,
        texts.t(e.app),
        None,
        e.summary(texts),
        e.body(texts),
        ronoctua_notify::Urgency::Normal,
    );
    n.tags = vec![e.tag.to_owned()];
    // WHAT ITS ACTIONS ARE ABOUT goes with it: the band's button, and the
    // manager's, run the entry's command on the same thing the line's would.
    n.subject.clone_from(&e.subject);
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

/// The command a terminal's signals offer — its notice, its bell, its wait
/// and its long command — run on the terminal they are about (their
/// subject): its window in front, or a window opened for its group.
pub const TERMINAL_SHOW: &str = "terminal-show";

/// What a click on `command`, among `e`'s actions, runs on: the event's
/// subject, when `command` is one its catalogue entry offers.
///
/// THE ONE THE LINE NAMES, and not the latest: `software-launch-latest` with
/// no argument opens whatever finished LAST, and the queue and the pointer's
/// hold keep an older install's line up while newer ones finish — "GIMP is
/// installed · Open it" opened Inkscape. The same for a terminal: "show it"
/// on a bell's line shows the terminal that rang, whichever rang since.
/// `None` for a command its entry does not offer, or an event that is about
/// nothing in particular.
#[must_use]
pub fn subject_of<'a>(
    e: &'a Event,
    command: &str,
    book: &ronoctua_notify::Book,
) -> Option<&'a str> {
    let offered = book
        .entry(e.tag)?
        .actions
        .iter()
        .any(|a| matches!(&a.doing, ronoctua_notify::Doing::Command(c) if c == command));
    if offered { e.subject.as_deref() } else { None }
}

// ---------------------------------------------------------------------------
// Where an event or a notification goes
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
    /// The drawer is not: it is the line grown, and what it shows IS what
    /// the line says — which is how its actions are read in full.
    const fn hides_the_line(self) -> bool {
        self.view || self.typing || self.covered
    }
}

/// Where an arriving event or notification goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The minibuffer's line.
    Line,
    /// The band.
    Band,
}

/// Where an event or a shown notification goes, given what holds the line —
/// THE ONE ANSWER, for both: the line when it is free, the band when it is
/// somebody's. (The line's queue full sends the next one to the band too:
/// [`Line::arrive`] gives it back.)
#[must_use]
pub const fn route(busy: Busy) -> Route {
    if busy.any() { Route::Band } else { Route::Line }
}

/// A notification said on the line: what arrived and what the book decided,
/// the band's own pair, kept whole so the line says what the band would.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    /// What arrived, with our sentence and the catalogue's actions on it.
    pub notification: ronoctua_notify::Notification,
    /// What the filters decided.
    pub presentation: ronoctua_notify::Presentation,
}

/// What the minibuffer's line says, beside the shell's own echo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// What the machine did.
    Event(Event),
    /// A notification — recorded in the history as said on the line.
    Notification(Box<Told>),
}

impl From<Event> for Said {
    fn from(e: Event) -> Self {
        Self::Event(e)
    }
}

impl Said {
    /// The event, when it is one.
    #[must_use]
    pub const fn event(&self) -> Option<&Event> {
        match self {
            Self::Event(e) => Some(e),
            Self::Notification(_) => None,
        }
    }

    /// The notification, when it is one.
    #[must_use]
    pub fn told(&self) -> Option<&Told> {
        match self {
            Self::Event(_) => None,
            Self::Notification(t) => Some(t),
        }
    }

    /// How long it stays on the line: an event the echo's time, both being
    /// one thing to read; a notification the time its `life` gives it on
    /// the band.
    fn lasts(&self) -> Duration {
        match self {
            Self::Event(_) => on_screen(),
            Self::Notification(t) => t.presentation.life.how_long(),
        }
    }

    /// Whether it is the notification of record `key`.
    fn is(&self, key: ronoctua_notify::RecordKey) -> bool {
        self.told().is_some_and(|t| t.notification.key() == key)
    }

    /// Its level, which orders the line's queue as it orders the band's.
    #[must_use]
    pub fn level(&self) -> u8 {
        match self {
            Self::Event(e) => e.level,
            Self::Notification(t) => t.presentation.level,
        }
    }

    /// Whether, on a line that becomes somebody's, it goes to the band even
    /// after it was read there: a notification that offers something to do
    /// is not done with by being read, and the band keeps it for the rest of
    /// its life, unanswered.
    fn waits_for_an_answer(&self) -> bool {
        self.told()
            .is_some_and(|t| !t.notification.actions.is_empty())
    }
}

/// Where a notification the book decided went ([`deliver`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    /// A filter hid it: recorded, and shown nowhere.
    Nowhere,
    /// Said on the minibuffer's line.
    Line,
    /// On the band.
    Band,
}

impl Placed {
    /// Its name, for the log a test reads.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Nowhere => "nowhere",
            Self::Line => "line",
            Self::Band => "band",
        }
    }
}

/// A NOTIFICATION THE BOOK DECIDED, put where it goes — every notification,
/// from wherever it came, passes here and nowhere else.
///
/// Hidden by a filter, it is recorded and shown nowhere, as it always was.
/// Shown, it takes the events' way ([`route`]): said on the line when the
/// line is free, and recorded as said there — read or unread as the band
/// would leave it, the bell keeping what was not read; on the band when the
/// line is somebody's, or its queue is full.
pub fn deliver(
    line: &mut Line,
    queue: &mut ronoctua_notify::Queue,
    n: ronoctua_notify::Notification,
    p: ronoctua_notify::Presentation,
    busy: Busy,
    person: bool,
    now: Instant,
) -> Placed {
    if p.show == ronoctua_notify::Show::Hidden {
        queue.enqueue(n, p, now);
        return Placed::Nowhere;
    }
    if route(busy) == Route::Line {
        let told = Said::Notification(Box::new(Told {
            notification: n.clone(),
            presentation: p.clone(),
        }));
        if line.arrive(told, person, now).is_none() {
            queue.record_on_the_line(n, p);
            return Placed::Line;
        }
    }
    queue.enqueue(n, p, now);
    Placed::Band
}

// ---------------------------------------------------------------------------
// The line's queue
// ---------------------------------------------------------------------------

/// How long an event stays on the line — the echo's time, both being one
/// thing to read.
fn on_screen() -> Duration {
    Duration::from_secs(crate::echo::ECHO_SECONDS)
}

/// How long what the line says has to have been up to count as SAID: under
/// it, the line was taken before anybody could read it, and it goes to the
/// band like the ones still waiting ([`Line::yielded`]).
pub const READ_IN: Duration = Duration::from_secs(1);

/// The most a pointer resting on the line holds what it says, in its own
/// times, counted from when it went up ([`Line::tick`]).
///
/// The hold is for a hand reaching for an action or reading a long line, and
/// it is the band's: the clock STOPS, and the pie says so (2026-11-08). But
/// the minibuffer is the bottom row of the screen, where a pointer is often
/// PARKED, and with the bars hidden that row is the edge that reveals them. A
/// hold with no end kept "a camera is open by …" up long after it closed,
/// with the queue — the camera closing — stuck behind it. So a hand still
/// there this long lets go, and holds again only once it has left and come
/// back.
pub const HELD_AT_MOST: u32 = 3;

/// What is on the line: what, since when, until when — and which of its
/// actions the cursor is on.
#[derive(Debug)]
struct Up {
    said: Said,
    since: Instant,
    /// Its time, which a hand on the line stops ([`crate::echo::Countdown`]).
    clock: crate::echo::Countdown,
    /// A hand has held it for [`HELD_AT_MOST`] of its times: the clock runs
    /// under it until it leaves.
    parked: bool,
    /// The action a click picked, by its place among the line's actions —
    /// BORN ON THE FIRST, a question's `no`, each time something goes up:
    /// the line rises by itself, under a hand that may be clicking at
    /// something else, so the first click picks and only a click on the
    /// person's pick runs it (`crate::aim`, the band's card's rule).
    pick: crate::aim::Aim<usize>,
}

impl Up {
    fn at(said: Said, now: Instant) -> Self {
        let clock = crate::echo::Countdown::start(said.lasts(), now);
        Self {
            said,
            since: now,
            clock,
            parked: false,
            pick: crate::aim::Aim::born(0),
        }
    }
}

/// What one second of the line's clock did ([`Line::tick`]).
#[derive(Debug, Default)]
pub struct Ticked {
    /// Whether what the line shows changed.
    pub changed: bool,
    /// What left the line because its time was up — a notification's sender
    /// waits to be told.
    pub expired: Option<Said>,
}

/// What a line that became somebody's let go of ([`Line::yielded`]).
#[derive(Debug, Default)]
pub struct Yielded {
    /// What goes to the band, in the order it would have been said — each
    /// with the time it already spent on the line, which the band takes off
    /// its life (nothing, for the ones still waiting).
    pub to_the_band: Vec<(Said, Duration)>,
    /// What was on the line and was read there: it goes nowhere, and a
    /// notification's sender is told it closed.
    pub said: Option<Said>,
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
        crate::tags::BLUETOOTH_CONNECTED,
        crate::tags::BLUETOOTH_DISCONNECTED,
        Some("device"),
    ),
];

/// Whether `new` says the opposite of `old`: the other half of its pair,
/// about the same network, tunnel or device.
#[must_use]
pub fn contradicts(new: &Event, old: &Event) -> bool {
    OPPOSITES.iter().any(|(on, off, about)| {
        let paired = (new.tag == *on && old.tag == *off) || (new.tag == *off && old.tag == *on);
        paired && about_of(new, *about) == about_of(old, *about)
    })
}

/// The argument `about` names — which network, tunnel or device an event is
/// about — or nothing, for a source that has one thing.
fn about_of<'a>(e: &'a Event, about: Option<&str>) -> Option<&'a str> {
    let name = about?;
    e.args
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.as_str())
}

/// Whether `new` replaces `old` on the line: two events, one the opposite of
/// the other ([`contradicts`]) — or the same thing said again of the same
/// subject ([`repeats`]). A notification replaces nothing — two
/// notifications are two things somebody said.
fn replaces(new: &Said, old: &Said) -> bool {
    match (new, old) {
        (Said::Event(new), Said::Event(old)) => contradicts(new, old) || repeats(new, old),
        _ => false,
    }
}

/// Whether `new` says again what `old` said, of the same subject: a terminal
/// that rings its bell twice while the first line is up is one terminal
/// ringing — the line says it once, with its whole time again, and the queue
/// does not fill with the same sentence. An event about nothing in particular
/// repeats nothing: two networks lost are two things.
#[must_use]
pub fn repeats(new: &Event, old: &Event) -> bool {
    new.tag == old.tag && new.subject.is_some() && new.subject == old.subject
}

/// What the minibuffer's line says — an event or a notification — and what
/// waits for it.
#[derive(Debug, Default)]
pub struct Line {
    /// On the line.
    on: Option<Up>,
    /// Waiting, first in first out.
    waiting: VecDeque<Said>,
}

impl Line {
    /// An event or a notification for the line. `person` says a person's
    /// echo holds it now.
    ///
    /// Gives back what does not fit — the one past [`EVENT_ECHOES`] waiting
    /// — for the band.
    ///
    /// AN EVENT THAT SAYS THE OPPOSITE of one on the line or waiting
    /// ([`contradicts`]) REPLACES it — on the line, with the whole time again;
    /// waiting, in its place. Behind it, the line would say, seconds late, a
    /// fact that had already flipped: "a camera is open" after it closed.
    ///
    /// BY LEVEL, AS THE BAND ORDERS ITS OWN (`queue::unseats`,
    /// `queue::waiting_place`): one of a strictly higher level takes the line
    /// from what is on it, which goes back to the FRONT of the queue to be
    /// said whole; otherwise it waits after everything of at least its level.
    /// A critical one never waits behind ordinary ones — and with four
    /// already waiting it is the band's at once.
    pub fn arrive(&mut self, said: impl Into<Said>, person: bool, now: Instant) -> Option<Said> {
        use ronoctua_notify::queue::{unseats, waiting_place};
        let said = said.into();
        if let Some(up) = self.on.as_mut()
            && replaces(&said, &up.said)
        {
            *up = Up::at(said, now);
            return None;
        }
        if let Some(w) = self.waiting.iter_mut().find(|w| replaces(&said, w)) {
            *w = said;
            return None;
        }
        if self.on.is_none() && !person && self.waiting.is_empty() {
            self.on = Some(Up::at(said, now));
            return None;
        }
        if self.waiting.len() >= EVENT_ECHOES {
            return Some(said);
        }
        if !person
            && let Some(up) = self.on.as_mut()
            && unseats(said.level(), up.said.level())
        {
            let unseated = std::mem::replace(up, Up::at(said, now));
            self.waiting.push_front(unseated.said);
            return None;
        }
        let at = waiting_place(self.waiting.iter().map(Said::level), said.level());
        self.waiting.insert(at, said);
        None
    }

    /// A person's echo took the line: what was on it goes back to the FRONT
    /// of the queue, to be said whole once the line is free. Says whether
    /// something was on it.
    pub fn interrupted(&mut self) -> bool {
        match self.on.take() {
            Some(up) => {
                self.waiting.push_front(up.said);
                true
            }
            None => false,
        }
    }

    /// The footer's second: what is on the line goes when its time is up —
    /// its clock STOPPED while a pointer rests on the line (`held`) and
    /// running again from where it stopped once the hand leaves, the band's
    /// rule, but a hand still there [`HELD_AT_MOST`] of its times after it
    /// went up lets go — and the next one takes the line when nothing else
    /// holds it.
    pub fn tick(&mut self, now: Instant, person: bool, held: bool) -> Ticked {
        let mut ticked = Ticked::default();
        if let Some(up) = self.on.as_mut() {
            let lasts = up.said.lasts();
            if !held {
                up.clock.release(now);
                up.parked = false;
            } else if !up.parked {
                if now.saturating_duration_since(up.since) >= lasts * HELD_AT_MOST {
                    up.clock.release(now);
                    up.parked = true;
                } else {
                    up.clock.hold(now);
                }
            }
            if up.clock.spent(now) {
                ticked.expired = self.on.take().map(|up| up.said);
                ticked.changed = true;
            }
        }
        if self.on.is_none()
            && !person
            && let Some(said) = self.waiting.pop_front()
        {
            self.on = Some(Up::at(said, now));
            ticked.changed = true;
        }
        ticked
    }

    /// The line became somebody's: what is still to be said goes to the band,
    /// in the order it would have been said. With only the drawer open what
    /// is ON the line stays, because the drawer is showing it.
    ///
    /// # What is on the line, when the line goes out of sight
    ///
    /// Already said, when it has been up [`READ_IN`] or more: it leaves the
    /// line and goes nowhere. Handed to the band it was the same fact twice —
    /// the person presses the leader two seconds after reading "charger
    /// plugged in", and the band says it again. Up for less, nobody could
    /// have read it, and it goes to the band with the ones still waiting.
    ///
    /// BUT A NOTIFICATION THAT OFFERS SOMETHING TO DO goes to the band however
    /// long it was up — for the REST of its life, unanswered: the owner's
    /// rule is "the line busy, the band", and a question read and then
    /// covered by the leader is a question nobody answered.
    pub fn yielded(&mut self, busy: Busy, now: Instant) -> Yielded {
        let mut out = Yielded::default();
        if !busy.any() {
            return out;
        }
        if busy.hides_the_line()
            && let Some(up) = self.on.take()
        {
            let read = now.saturating_duration_since(up.since) >= READ_IN;
            if read && !up.said.waits_for_an_answer() {
                out.said = Some(up.said);
            } else {
                let left = up.clock.left(now);
                let spent = up.said.lasts().saturating_sub(left);
                out.to_the_band.push((up.said, spent));
            }
        }
        out.to_the_band
            .extend(self.waiting.drain(..).map(|s| (s, Duration::ZERO)));
        out
    }

    /// What is on the line was acted on — one of its actions clicked — and
    /// leaves it: given back, so the click knows WHICH it was about
    /// ([`subject_of`], a notification's record).
    pub fn done(&mut self) -> Option<Said> {
        self.on.take().map(|up| up.said)
    }

    /// Takes notification `id` off the line and out of its queue — closed by
    /// its sender, or its question over — and gives back the records it
    /// was, which the band's own closing marks read.
    pub fn forget(&mut self, id: u32) -> Vec<ronoctua_notify::RecordKey> {
        let of = |s: &Said| {
            s.told()
                .filter(|t| t.notification.id == id)
                .map(|t| t.notification.key())
        };
        let mut gone: Vec<ronoctua_notify::RecordKey> = Vec::new();
        if let Some(key) = self.on.as_ref().and_then(|up| of(&up.said)) {
            self.on = None;
            gone.push(key);
        }
        gone.extend(self.waiting.iter().filter_map(of));
        self.waiting.retain(|s| of(s).is_none());
        gone
    }

    /// Takes the notification of record `key` off the line and out of its
    /// queue — read by hand somewhere. Says whether it was there.
    pub fn read(&mut self, key: ronoctua_notify::RecordKey) -> bool {
        let mut found = false;
        if self.on.as_ref().is_some_and(|up| up.said.is(key)) {
            self.on = None;
            found = true;
        }
        let before = self.waiting.len();
        self.waiting.retain(|s| !s.is(key));
        found || self.waiting.len() != before
    }

    /// New words on the notification of record `key`, wherever it is on the
    /// line — `Queue::amend`'s half, for the copy the line says.
    pub fn amend(&mut self, key: ronoctua_notify::RecordKey, summary: &str, body: &str) -> bool {
        let mut found = false;
        let on = self.on.as_mut().map(|up| &mut up.said);
        for said in on.into_iter().chain(self.waiting.iter_mut()) {
            if let Said::Notification(t) = said
                && t.notification.key() == key
            {
                summary.clone_into(&mut t.notification.summary);
                body.clone_into(&mut t.notification.body);
                found = true;
            }
        }
        found
    }

    /// What is on the line, when there is something.
    #[must_use]
    pub fn on_the_line(&self) -> Option<&Said> {
        self.on.as_ref().map(|up| &up.said)
    }

    /// A click on action `n` of what the line says: `true` when it RUNS —
    /// the person's own pick, clicked again; otherwise `n` becomes the pick
    /// and nothing runs. Every action of the line, an event's and a
    /// notification's alike: one rule for one line, the band's
    /// (invariant 19).
    pub fn clicked(&mut self, n: usize) -> bool {
        self.on.as_mut().is_some_and(|up| up.pick.clicked(n))
    }

    /// Which action the cursor is on — the first, where it is born.
    #[must_use]
    pub fn picked(&self) -> usize {
        self.on
            .as_ref()
            .and_then(|up| up.pick.on().copied())
            .unwrap_or(0)
    }

    /// How many wait for the line.
    #[must_use]
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// The time of what is on the line, for its pie: the eighths it has left,
    /// and whether a hand holds it ([`band::cluster`](crate::band::cluster)).
    #[must_use]
    pub fn clock(&self, now: Instant) -> Option<(u8, bool)> {
        self.on
            .as_ref()
            .map(|up| (up.clock.eighths(now), up.clock.held()))
    }

    /// Where what is on the line stands among what waits for it — the band's
    /// counter, `(1, 1 + waiting)` — and nothing when nothing waits: a `1/1`
    /// spends three columns saying nothing.
    #[must_use]
    pub fn position(&self) -> Option<(usize, usize)> {
        (self.on.is_some() && !self.waiting.is_empty()).then(|| (1, 1 + self.waiting.len()))
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
    /// The last event of the brain's modules already heard, by its number —
    /// and nothing before the first mind, which is only the baseline.
    told: Edge<u64>,
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
                Event::of(crate::tags::BLUETOOTH_CONNECTED, key::NOTIF_BLUETOOTH_APP)
                    .about("device", name.clone())
            })
            .collect();
        out.extend(
            was.iter()
                .filter(|(at, _)| !now.contains_key(*at))
                .map(|(_, name)| {
                    Event::of(
                        crate::tags::BLUETOOTH_DISCONNECTED,
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

    /// What the brain's MODULES told, as the mind carries it (`Mind.events`,
    /// a ring numbered from the daemon's start): news when its number is past
    /// the last one heard.
    ///
    /// # Generic, and it names no module
    ///
    /// The tag is the event's catalogue entry — `<module>-<event>`, merged
    /// into the notifications table by the brain's generator — and its
    /// sentences are that entry's `notif-<tag>-summary` and `-body`, with the
    /// event's values formatted here (`mind_wire::arg_text`). Who says it is
    /// the module's own name. A tag the catalogue does not know is REFUSED —
    /// said nowhere, and said so in the log — rather than drawn as a sentence
    /// nobody declared.
    ///
    /// # What a module says twice is the module's to decide
    ///
    /// Said once per number, and nothing more: whether a second "still
    /// short" is news is a rule of the module that tells it, kept where it
    /// knows why. A footer that remembered one module's rule would be a
    /// footer naming that module.
    ///
    /// # The first mind is the baseline
    ///
    /// For the reason every source here gives: the footer starting is not the
    /// events of the last half hour happening again. And a brain that
    /// restarted numbers from one again, which is a new baseline, not news.
    pub fn of_brain(
        &mut self,
        mind: &ronoctua_brain::Mind,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Event> {
        let newest = mind.events.iter().map(|e| e.seq).max().unwrap_or(0);
        let Some(heard) = self.told.saw(newest) else {
            return Vec::new();
        };
        if newest < heard {
            return Vec::new();
        }
        let fresh: Vec<&ronoctua_brain::mind::Said> =
            mind.events.iter().filter(|e| e.seq > heard).collect();
        if fresh.is_empty() {
            return Vec::new();
        }
        let book = ronoctua_notify::Book::distro();
        let mut out = Vec::new();
        for said in fresh {
            if let Some(e) = brain_event(said, mind, &book, zone, texts) {
                out.push(e);
            }
        }
        out
    }
}

/// One event of a brain module, as the line says it — or none, for a tag the
/// catalogue does not know.
fn brain_event(
    said: &ronoctua_brain::mind::Said,
    mind: &ronoctua_brain::Mind,
    book: &ronoctua_notify::Book,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> Option<Event> {
    if book.entry(&said.tag).is_none() {
        tracing::warn!(tag = %said.tag, "a brain event with no catalogue entry; not said");
        return None;
    }
    let tag = crate::brainlink::interned(&said.tag)?;
    // WHO SAYS IT: the module whose id the tag begins with — the longest,
    // since one module's id can be the start of another's — by its own name.
    let app = mind
        .tasks
        .iter()
        .filter(|t| {
            said.tag
                .strip_prefix(t.id.as_str())
                .is_some_and(|rest| rest.starts_with('-'))
        })
        .max_by_key(|t| t.id.len())
        .map(|t| {
            if t.name_key.is_empty() {
                format!("brain-task-{}", t.id)
            } else {
                t.name_key.clone()
            }
        })
        .and_then(|k| crate::brainlink::interned(&k))
        .unwrap_or(key::CMD_BRAIN_OPEN);
    let mut e = Event::of(tag, app);
    for (name, arg) in &said.args {
        let name = crate::brainlink::interned(name)?;
        match arg {
            ronoctua_brain::module::Arg::Number(n) => e.counts.push((name, *n)),
            other => e
                .args
                .push((name, crate::mind_wire::arg_text(other, zone, texts))),
        }
    }
    Some(e)
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

    /// A process the walk of `/proc` found holding `/dev/video0`.
    fn found(pid: u32, name: &str, parents: &[&str]) -> ronoctua_memory::camera::Holder {
        ronoctua_memory::camera::Holder {
            pid,
            name: name.to_owned(),
            node: "/dev/video0".to_owned(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
        }
    }

    /// FAILS IF: the shell's own short reading of the cameras — the footer's
    /// region or the settings' window, a process named as the shell —
    /// counts as somebody using a camera ("camera in use: ronoctua-shell",
    /// and the mark), or leaving it out hides a real holder: what the shell
    /// started (a browser, a terminal's `ffmpeg`) runs under it and is still
    /// somebody, and the studio's `ffmpeg` is still ours and still a camera
    /// open. With no name for the shell, nobody is left out.
    #[test]
    fn the_shells_own_reading_is_nobody_using_a_camera() {
        let shell = process_name(std::path::Path::new(
            "/nix/store/x-ronoctua-shell/bin/.ronoctua-shell-wrapped",
        ));
        assert_eq!(shell.as_deref(), Some(".ronoctua-shell"));
        assert_eq!(
            process_name(std::path::Path::new("/run/bin/ronoctua-shell")).as_deref(),
            Some("ronoctua-shell")
        );
        let walk = || {
            vec![
                // THE FOOTER reading, and a settings window it started.
                found(10, ".ronoctua-shell", &["systemd"]),
                found(11, ".ronoctua-shell", &[".ronoctua-shell", "systemd"]),
                // WHAT IT STARTED, and the studio's own.
                found(12, "chromium", &[".ronoctua-shell"]),
                found(13, "ffmpeg", &["zsh", ".ronoctua-shell"]),
                found(14, "ffmpeg", &["ronoctua-studio"]),
            ]
        };
        assert_eq!(
            holders_of(walk(), shell.as_deref()),
            vec![
                holder("chromium", false),
                holder("ffmpeg", false),
                holder("ffmpeg", true),
            ]
        );
        assert!(
            holders_of(walk(), None)
                .iter()
                .any(|h| h.name == ".ronoctua-shell" && !h.ours),
            "with no name for the shell, a holder was left out"
        );
        // AND NO EVENT for the reading alone, nor a mark.
        let mut w = Watch::default();
        assert!(w.camera(&[]).is_empty());
        let reading = holders_of(vec![found(10, ".ronoctua-shell", &[])], shell.as_deref());
        assert!(reading.is_empty());
        assert!(w.camera(&reading).is_empty(), "the reading was news");
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
                crate::tags::BLUETOOTH_CONNECTED,
                crate::tags::BLUETOOTH_DISCONNECTED
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

    /// FAILS IF: what a brain module told is said before the first mind —
    /// which is only the baseline — or twice, or a tag the catalogue does not
    /// know is said at all, or a brain that restarted and numbers from one is
    /// taken for news.
    #[test]
    fn a_brain_modules_events_are_said_once_after_the_baseline() {
        use ronoctua_brain::mind::Said;
        use ronoctua_brain::module::Arg;
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        // A TAG THE CATALOGUE KNOWS, from whichever compiled module tells
        // something with a notification — the footer names none.
        let book = ronoctua_notify::Book::distro();
        let told = ronoctua_brain::catalogue::catalogue()
            .events()
            .map(|(_, e)| e.tag.as_str())
            .find(|tag| book.entry(tag).is_some())
            .expect("no brain module of the catalogue tells anything the book knows");
        let said = |seq: u64, tag: &str| Said {
            seq,
            tag: tag.to_owned().into(),
            args: vec![
                ("freed".into(), Arg::Bytes(12_340_000_000)),
                ("free".into(), Arg::PerMille(153)),
            ],
        };
        let mind = |events: Vec<Said>| ronoctua_brain::Mind {
            events,
            ..ronoctua_brain::Mind::default()
        };
        let mut w = Watch::default();
        assert!(
            w.of_brain(&mind(vec![said(3, told)]), &zone, &t).is_empty(),
            "the first mind is the baseline, and it was said"
        );
        let heard = w.of_brain(&mind(vec![said(3, told), said(4, told)]), &zone, &t);
        assert_eq!(tags(&heard), vec![told]);
        // ITS VALUES WERE FORMATTED HERE, in the language's units.
        assert_eq!(arg(&heard[0], "free"), Some("15.3"));
        assert_eq!(arg(&heard[0], "freed"), Some("12.3 GB"));
        // AND ITS SENTENCE IS THE CATALOGUE ENTRY'S, in the language.
        assert_eq!(
            heard[0].summary(&t),
            t.t_args(
                &format!("notif-{told}-summary"),
                &[
                    ("freed", ronoctua_i18n::Arg::Text("12.3 GB")),
                    ("free", ronoctua_i18n::Arg::Text("15.3")),
                ]
            )
        );
        // AND NOT TWICE.
        assert!(w.of_brain(&mind(vec![said(4, told)]), &zone, &t).is_empty());
        // A TAG NOBODY DECLARED is refused, not said.
        assert!(
            w.of_brain(&mind(vec![said(5, "fixture-nothing-declared")]), &zone, &t)
                .is_empty()
        );
        // A BRAIN THAT RESTARTED numbers from one again: a new baseline.
        assert!(w.of_brain(&mind(vec![said(1, told)]), &zone, &t).is_empty());
        assert_eq!(
            tags(&w.of_brain(&mind(vec![said(2, told)]), &zone, &t)),
            vec![told]
        );
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

    /// FAILS IF: an event or a notification goes to the line while the line
    /// is somebody's, or to the band while it is free.
    #[test]
    fn a_free_line_says_it_and_a_busy_one_hands_it_to_the_band() {
        assert_eq!(route(Busy::default()), Route::Line);
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
            assert_eq!(route(busy), Route::Band, "{busy:?}");
        }
    }

    fn e(tag: &'static str) -> Event {
        Event::of(tag, key::NOTIF_NETWORK_APP)
    }

    /// The tag of what the line says, when it is an event.
    fn tag_of(said: Option<&Said>) -> Option<&'static str> {
        said.and_then(Said::event).map(|e| e.tag)
    }

    /// The tags of what was handed to the band, events only.
    fn said_tags(said: &[(Said, Duration)]) -> Vec<&'static str> {
        said.iter()
            .filter_map(|(s, _)| s.event())
            .map(|e| e.tag)
            .collect()
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
            tag_of(over.as_ref()),
            Some(CAMERA_IN_USE),
            "the fifth waiting"
        );
        assert_eq!(tag_of(line.on_the_line()), Some(TOR_UP));
        let step = on_screen();
        assert!(!line.tick(t0 + step / 2, false, false).changed, "not yet");
        let mut said = vec![TOR_UP];
        let mut now = t0;
        for _ in 0..4 {
            now += step;
            assert!(line.tick(now, false, false).changed);
            said.push(tag_of(line.on_the_line()).expect("the next one"));
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
        assert!(line.tick(now + step, false, false).changed);
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
        assert!(!line.tick(t0, true, false).changed);
        assert_eq!(line.on_the_line(), None);
        // THE ECHO WENT: the interrupted one is first, whole.
        assert!(line.tick(t0, false, false).changed);
        assert_eq!(tag_of(line.on_the_line()), Some(TOR_UP));
        let step = on_screen();
        assert!(line.tick(t0 + step, false, false).changed);
        assert_eq!(tag_of(line.on_the_line()), Some(TAILNET_UP));
        assert!(line.tick(t0 + step * 2, false, false).changed);
        assert_eq!(tag_of(line.on_the_line()), Some(VPN_UP));
    }

    /// FAILS IF: an event under the pointer expires while somebody is reading
    /// it or reaching for its action, its pie does not say it is held, or it
    /// comes back with more or less than it had when the hand came — the
    /// band's rule, a clock that stops (2026-11-08).
    #[test]
    fn the_pointer_on_the_line_stops_its_clock() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        let step = on_screen();
        assert!(!line.tick(t0 + step / 2, false, true).changed, "held");
        assert!(
            !line.tick(t0 + step * 2, false, true).changed,
            "a hand that stays holds it past its time"
        );
        assert_eq!(line.clock(t0 + step * 2).map(|(_, held)| held), Some(true));
        // THE HAND LEAVES at two and a quarter, and the half it had when it
        // came runs from there: it is still up a quarter later…
        let gone = t0 + step * 2 + step / 4;
        assert!(!line.tick(gone, false, false).changed);
        assert_eq!(line.clock(gone).map(|(_, held)| held), Some(false));
        assert!(
            !line.tick(gone + step / 4, false, false).changed,
            "it had half its time left"
        );
        // …and goes once that half has run.
        assert!(
            line.tick(gone + step / 2 + Duration::from_millis(1), false, false)
                .changed,
            "and then it goes"
        );
        assert!(line.is_empty());
    }

    /// FAILS IF: the line's counter says `1/1` (three columns saying
    /// nothing), or does not count what waits behind what it says — the
    /// band's counter, on the minibuffer (2026-11-08).
    #[test]
    fn the_line_counts_what_waits_behind_it() {
        let t0 = Instant::now();
        let mut line = Line::default();
        assert_eq!(line.position(), None);
        line.arrive(e(CAMERA_IN_USE), false, t0);
        assert_eq!(line.position(), None, "one alone is not counted");
        line.arrive(e(TOR_UP), false, t0);
        line.arrive(e(TAILNET_UP), false, t0);
        assert_eq!(line.position(), Some((1, 3)));
        assert_eq!(
            line.clock(t0),
            Some((8, false)),
            "a whole pie when it goes up"
        );
    }

    /// FAILS IF: a pointer that never leaves the line — parked at the bottom
    /// of the screen, where the line is — holds an event past
    /// [`HELD_AT_MOST`] echoes from when it went up, and the one waiting
    /// behind it with it; or, let go of, the event loses the time it had, or
    /// the parked hand holds it again.
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
                tag_of(line.on_the_line()),
                Some(CAMERA_IN_USE),
                "held at {:?}",
                now - t0
            );
        }
        // LET GO OF at the cap, with the three quarters it had when the hand
        // came, and not held again by the hand still parked there.
        assert!(!line.tick(t0 + step * HELD_AT_MOST, false, true).changed);
        assert_eq!(
            line.clock(t0 + step * HELD_AT_MOST).map(|(_, held)| held),
            Some(false)
        );
        assert!(
            !line
                .tick(t0 + step * HELD_AT_MOST + step / 2, false, true)
                .changed
        );
        assert!(
            line.tick(t0 + step * HELD_AT_MOST + step, false, true)
                .changed,
            "it goes"
        );
        assert_eq!(tag_of(line.on_the_line()), Some(TOR_UP));
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
        assert_eq!(tag_of(line.on_the_line()), Some(CAMERA_FREE));
        assert!(
            !line.tick(t0 + step, false, false).changed,
            "a whole time from now"
        );
        assert!(line.tick(t0 + step + step / 2, false, false).changed);
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
                .and_then(Said::event)
                .map(|e| (e.tag, arg(e, "network").map(ToOwned::to_owned))),
        ];
        for n in 1..=4 {
            line.tick(t0 + step * n, false, false);
            said.push(
                line.on_the_line()
                    .and_then(Said::event)
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
            &about(crate::tags::BLUETOOTH_DISCONNECTED, "device", "Headset"),
            &about(crate::tags::BLUETOOTH_CONNECTED, "device", "Headset")
        ));
        assert!(!contradicts(
            &about(crate::tags::BLUETOOTH_DISCONNECTED, "device", "Headset"),
            &about(crate::tags::BLUETOOTH_CONNECTED, "device", "Mouse")
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
            line.yielded(Busy::default(), t0).to_the_band.is_empty(),
            "free: nothing moves"
        );
        let drawer = line.yielded(
            Busy {
                drawer: true,
                ..Busy::default()
            },
            t0,
        );
        assert_eq!(said_tags(&drawer.to_the_band), vec![TAILNET_UP]);
        assert_eq!(drawer.said, None, "the drawer shows what is on the line");
        assert_eq!(tag_of(line.on_the_line()), Some(TOR_UP));
        let view = line.yielded(
            Busy {
                view: true,
                ..Busy::default()
            },
            t0 + READ_IN / 2,
        );
        assert_eq!(
            said_tags(&view.to_the_band),
            vec![TOR_UP],
            "up too briefly to be read"
        );
        assert_eq!(view.said, None);
        assert!(line.is_empty());
    }

    /// FAILS IF: an event the person already read on the line — up for
    /// [`READ_IN`] or more — is raised AGAIN on the band when the line
    /// becomes busy (the leader, a view, a question); or is not given back
    /// as gone; or the ones waiting behind it stop going to the band.
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
        assert_eq!(
            said_tags(&handed.to_the_band),
            vec![TOR_UP],
            "only the one never shown"
        );
        assert_eq!(
            tag_of(handed.said.as_ref()),
            Some(CHARGER_PLUGGED),
            "the one read is told as gone"
        );
        assert!(line.is_empty());
    }

    /// FAILS IF: clicking an event's action leaves the event on the line, or
    /// the click cannot tell which event it acted on.
    #[test]
    fn an_event_acted_on_leaves_the_line() {
        let t0 = Instant::now();
        let mut line = Line::default();
        line.arrive(e(TOR_UP), false, t0);
        assert_eq!(tag_of(line.done().as_ref()), Some(TOR_UP));
        assert!(line.is_empty());
        assert_eq!(line.done(), None);
    }

    // --- every notification -------------------------------------------------

    /// A notification as the bus delivers one, decided as the footer decides.
    fn arrived(id: u32, life: ronoctua_notify::Life) -> Told {
        let notification = ronoctua_notify::Notification::fresh_one(
            id,
            "mail".to_owned(),
            None,
            format!("message {id}"),
            String::new(),
            ronoctua_notify::Urgency::Normal,
        );
        let mut presentation = ronoctua_notify::Presentation::for_arrival(&notification);
        presentation.life = life;
        Told {
            notification,
            presentation,
        }
    }

    fn deliver_one(
        line: &mut Line,
        queue: &mut ronoctua_notify::Queue,
        t: Told,
        busy: Busy,
        now: Instant,
    ) -> Placed {
        deliver(
            line,
            queue,
            t.notification,
            t.presentation,
            busy,
            false,
            now,
        )
    }

    /// FAILS IF: a shown notification does not take the events' way — said
    /// on a free line and recorded there, unread as the band leaves one; on
    /// the band when the line is somebody's or its queue is full — or one a
    /// filter hides reaches either, or is not recorded.
    #[test]
    fn every_notification_takes_the_events_way() {
        let t0 = Instant::now();
        let normal = ronoctua_notify::Life::Normal;
        let (mut line, mut queue) = (Line::default(), ronoctua_notify::Queue::fresh_one());
        let placed = deliver_one(
            &mut line,
            &mut queue,
            arrived(1, normal),
            Busy::default(),
            t0,
        );
        assert_eq!(placed, Placed::Line);
        assert!(
            queue.showing().is_none(),
            "the free line's one took the band"
        );
        assert_eq!(
            line.on_the_line()
                .and_then(Said::told)
                .map(|t| t.notification.id),
            Some(1)
        );
        assert_eq!(queue.history()[0].place, ronoctua_notify::Place::Line);
        assert_eq!(queue.unread(), 1);
        // BUSY: the band, at once.
        let view = Busy {
            view: true,
            ..Busy::default()
        };
        assert_eq!(
            deliver_one(&mut line, &mut queue, arrived(2, normal), view, t0),
            Placed::Band
        );
        assert_eq!(queue.showing().map(|s| s.notification.id), Some(2));
        assert_eq!(queue.history()[1].place, ronoctua_notify::Place::Band);
        // HIDDEN: nowhere, and still in the history.
        let mut hidden = arrived(3, normal);
        hidden.presentation.show = ronoctua_notify::Show::Hidden;
        assert_eq!(
            deliver_one(&mut line, &mut queue, hidden, Busy::default(), t0),
            Placed::Nowhere
        );
        assert_eq!(line.waiting(), 0, "a hidden one waits for the line");
        assert_eq!(queue.history().len(), 3);
        // FULL: four wait, and the fifth goes to the band.
        for id in 4..8 {
            assert_eq!(
                deliver_one(
                    &mut line,
                    &mut queue,
                    arrived(id, normal),
                    Busy::default(),
                    t0
                ),
                Placed::Line
            );
        }
        assert_eq!(line.waiting(), EVENT_ECHOES);
        assert_eq!(
            deliver_one(
                &mut line,
                &mut queue,
                arrived(8, normal),
                Busy::default(),
                t0
            ),
            Placed::Band
        );
        // TWO NOTIFICATIONS ARE TWO THINGS SAID: neither replaced the other.
        assert_eq!(line.waiting(), EVENT_ECHOES);
    }

    /// FAILS IF: a notification on the line stays the echo's time instead of
    /// what its `life` gives it, or leaves without being given back for its
    /// sender to be told.
    #[test]
    fn a_notification_stays_on_the_line_for_its_life() {
        let t0 = Instant::now();
        let long = ronoctua_notify::Life::Long;
        let mut line = Line::default();
        line.arrive(Said::Notification(Box::new(arrived(1, long))), false, t0);
        let ticked = line.tick(t0 + on_screen() * 2, false, false);
        assert!(!ticked.changed, "gone at the echo's time");
        let ticked = line.tick(t0 + long.how_long(), false, false);
        assert!(ticked.changed);
        assert_eq!(
            ticked
                .expired
                .as_ref()
                .and_then(Said::told)
                .map(|t| t.notification.id),
            Some(1)
        );
        assert!(line.is_empty());
    }

    /// FAILS IF: a notification still waiting for a line that became busy is
    /// lost, recorded twice on the band, or its record still says the line;
    /// or one closed by its sender, read by hand or given new words stays on
    /// the line as it was.
    #[test]
    fn the_line_hands_a_notification_on_and_lets_it_go() {
        let t0 = Instant::now();
        let normal = ronoctua_notify::Life::Normal;
        let (mut line, mut queue) = (Line::default(), ronoctua_notify::Queue::fresh_one());
        for id in 1..=3 {
            deliver_one(
                &mut line,
                &mut queue,
                arrived(id, normal),
                Busy::default(),
                t0,
            );
        }
        let key = |id: u32| {
            queue
                .history()
                .iter()
                .find(|r| r.notification.id == id)
                .map(ronoctua_notify::Record::key)
                .expect("recorded")
        };
        let (two, three) = (key(2), key(3));
        assert!(line.amend(two, "two, whole", "and this"));
        assert_eq!(line.forget(1), vec![key(1)], "closed by its sender");
        assert!(line.read(three), "read by hand");
        assert!(!line.read(three), "read twice");
        let handed = line.yielded(
            Busy {
                typing: true,
                ..Busy::default()
            },
            t0 + READ_IN * 2,
        );
        assert_eq!(handed.said, None);
        assert_eq!(handed.to_the_band.len(), 1);
        let (said, spent) = &handed.to_the_band[0];
        assert_eq!(*spent, Duration::ZERO, "it never was on the line");
        let told = said.told().expect("a notification").clone();
        assert_eq!(told.notification.summary, "two, whole");
        queue.show_recorded(told.notification, told.presentation, *spent, t0);
        assert_eq!(queue.history().len(), 3, "recorded twice");
        assert_eq!(queue.showing().map(|s| s.notification.key()), Some(two));
        let place = queue
            .history()
            .iter()
            .find(|r| r.key() == two)
            .map(|r| r.place);
        assert_eq!(place, Some(ronoctua_notify::Place::Band));
    }

    /// A notification that offers something to do — a program's button.
    fn asking(id: u32, life: ronoctua_notify::Life) -> Said {
        let mut t = arrived(id, life);
        t.notification.actions = vec![ronoctua_notify::Action {
            id: "reply".to_owned(),
            label: ronoctua_notify::Label::Text("Reply".to_owned()),
            doing: ronoctua_notify::Doing::Invoke,
        }];
        Said::Notification(Box::new(t))
    }

    /// FAILS IF: a notification that offers something to do goes nowhere when
    /// the line is taken after it was read — instead of to the band, for the
    /// REST of its life — or one that offers nothing stops going nowhere, or
    /// the drawer (the line grown) sends either away.
    #[test]
    fn a_line_taken_hands_the_band_what_waits_for_an_answer() {
        let t0 = Instant::now();
        let normal = ronoctua_notify::Life::Normal;
        let view = Busy {
            view: true,
            ..Busy::default()
        };
        let up = READ_IN * 2;
        let mut line = Line::default();
        line.arrive(asking(1, normal), false, t0);
        let drawer = Busy {
            drawer: true,
            ..Busy::default()
        };
        assert!(line.yielded(drawer, t0 + up).to_the_band.is_empty());
        let handed = line.yielded(view, t0 + up);
        assert_eq!(
            handed.said, None,
            "a question read is a question unanswered"
        );
        let [(said, spent)] = handed.to_the_band.as_slice() else {
            panic!("{:?}", handed.to_the_band);
        };
        assert_eq!(said.told().map(|t| t.notification.id), Some(1));
        assert_eq!(*spent, up, "the band would give it a second life");
        // ONE THAT OFFERS NOTHING keeps the events' rule: read is said.
        let mut line = Line::default();
        line.arrive(Said::Notification(Box::new(arrived(2, normal))), false, t0);
        let handed = line.yielded(view, t0 + up);
        assert!(handed.to_the_band.is_empty());
        assert_eq!(
            handed
                .said
                .as_ref()
                .and_then(Said::told)
                .map(|t| t.notification.id),
            Some(2)
        );
    }

    /// FAILS IF: an action of what the line says runs on the first click —
    /// the line rose by itself, under a hand that may be clicking at
    /// something else — instead of being picked; a click on the pick does not
    /// run it; the cursor is not born on the first action, which a question
    /// puts its `no` in; or what goes up next keeps the last one's pick. An
    /// event's actions and a notification's alike.
    #[test]
    fn an_action_on_the_line_is_picked_before_it_runs() {
        let t0 = Instant::now();
        let mut line = Line::default();
        assert!(!line.clicked(0), "a click on an empty line ran something");
        line.arrive(asking(1, ronoctua_notify::Life::Normal), false, t0);
        assert_eq!(line.picked(), 0, "not born on the first action");
        assert!(!line.clicked(0), "the cursor the line put there answered");
        assert!(line.clicked(0), "a click on the pick did not run it");
        assert!(!line.clicked(1), "another action ran on its first click");
        assert_eq!(line.picked(), 1);
        assert!(line.clicked(1));
        // AN EVENT'S ACTION the same way, and the pick is born again.
        let _ = line.done();
        line.arrive(
            Event::installed(crate::tags::SOFTWARE_INSTALLED, "GIMP", None),
            false,
            t0,
        );
        assert_eq!(line.picked(), 0, "the last one's pick stayed");
        assert!(!line.clicked(0), "an event's action ran on the first click");
        assert!(line.clicked(0));
        // AND A QUESTION PUTS ITS `no` FIRST, where the cursor is born.
        let book = ronoctua_notify::Book::distro();
        let pairing = book.entry("bluetooth-pairing").expect("the pairing card");
        assert_eq!(
            pairing.actions.first().map(|a| a.id.as_str()),
            Some("bluetooth-refuse")
        );
    }

    /// FAILS IF: the line's queue stops ordering by level as the band does —
    /// a critical one waiting behind ordinary ones, on the line or in its
    /// queue; an equal level taking the line; what a higher one unseated
    /// lost instead of coming back first; a person's echo overruled; or a
    /// critical one waiting behind four instead of taking the band.
    #[test]
    fn a_higher_level_goes_ahead_on_the_line_as_on_the_band() {
        let t0 = Instant::now();
        let normal = ronoctua_notify::Life::Normal;
        let at = |id: u32, level: u8| {
            let mut t = arrived(id, normal);
            t.presentation.level = level;
            Said::Notification(Box::new(t))
        };
        let id = |s: Option<&Said>| s.and_then(Said::told).map(|t| t.notification.id);
        let mut line = Line::default();
        let mut quiet = e(TOR_UP);
        quiet.level = 25;
        line.arrive(at(1, 50), false, t0);
        line.arrive(quiet, false, t0);
        assert_eq!(line.arrive(at(2, 50), false, t0), None);
        assert_eq!(
            id(line.on_the_line()),
            Some(1),
            "an equal level took the line"
        );
        // A CRITICAL ONE takes the line; the one it unseated comes back first.
        assert_eq!(line.arrive(at(3, 90), false, t0), None);
        assert_eq!(id(line.on_the_line()), Some(3));
        let step = normal.how_long();
        line.tick(t0 + step, false, false);
        assert_eq!(id(line.on_the_line()), Some(1), "the unseated one was lost");
        line.tick(t0 + step * 2, false, false);
        assert_eq!(
            id(line.on_the_line()),
            Some(2),
            "the 50 waited behind the 25"
        );
        line.tick(t0 + step * 3, false, false);
        assert_eq!(tag_of(line.on_the_line()), Some(TOR_UP));
        // A PERSON'S ECHO holds the line against any level.
        let mut line = Line::default();
        line.arrive(at(1, 50), false, t0);
        assert!(line.interrupted());
        assert_eq!(line.arrive(at(2, 90), true, t0), None);
        assert_eq!(line.on_the_line(), None);
        line.tick(t0, false, false);
        assert_eq!(id(line.on_the_line()), Some(2), "the critical one waited");
        // FOUR WAITING: the critical one is the band's at once.
        let mut line = Line::default();
        for n in 1..=5 {
            assert_eq!(line.arrive(at(n, 50), false, t0), None);
        }
        assert_eq!(id(line.arrive(at(9, 90), false, t0).as_ref()), Some(9));
    }

    // --- the words ---------------------------------------------------------

    /// FAILS IF: an event that went to the band stops being its entry —
    /// its summary, its body and nothing before it, its tag, its sender.
    #[test]
    fn an_event_on_the_band_is_its_entry() {
        let t = texts();
        let e = Event::of(NETWORK_LOST, key::NOTIF_NETWORK_APP).about("network", "home");
        let n = notification(&e, &t);
        assert_eq!(
            n.summary,
            t.t_with("notif-network-lost-summary", &[("network", "home")])
        );
        assert!(n.summary.contains("home"), "{}", n.summary);
        assert_eq!(n.body, e.body(&t));
        assert_eq!(n.tags, vec![NETWORK_LOST.to_owned()]);
        assert_eq!(n.app, t.t(key::NOTIF_NETWORK_APP));
    }

    /// FAILS IF: the BAND stops showing an event's own body when the line was
    /// busy — its entries are `full`, and the second line the band draws is
    /// that body. Read off the lines the band DRAWS, in both languages, at
    /// the width of a small laptop.
    #[test]
    fn the_band_says_an_events_body() {
        let book = ronoctua_notify::Book::distro();
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let words = crate::band::Words { texts: &t };
            for tag in ALL {
                if matches!(
                    *tag,
                    crate::tags::BLUETOOTH_CONNECTED | crate::tags::BLUETOOTH_DISCONNECTED
                ) || matches!(
                    *tag,
                    crate::tags::SOFTWARE_INSTALLED | crate::tags::SOFTWARE_WEB_CREATED
                ) {
                    // THE ROWS THEY ALWAYS HAD: the radio's are hidden by its
                    // filters, and an install's row is the one it had.
                    continue;
                }
                let n = notification(&Event::of(tag, key::NOTIF_NETWORK_APP), &t);
                // ITS FIRST WORDS, which is what survives a cut.
                let opening: String = n
                    .body
                    .split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(!opening.is_empty(), "{language} {tag}: no body");
                let presentation = book.decide(&n);
                let shown = ronoctua_notify::Shown {
                    notification: n,
                    presentation,
                    since: Instant::now(),
                };
                let drawn = crate::band::lines(Some(&shown), 100, None, None, false, false, &words);
                assert!(drawn.len() >= 2, "{language} {tag}: the band drew one line");
                let body: String = drawn[1]
                    .pieces
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    body.contains(&opening),
                    "{language} {tag}: the band's second line is {body:?}"
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
        crate::tags::BLUETOOTH_CONNECTED,
        crate::tags::BLUETOOTH_DISCONNECTED,
        crate::tags::SOFTWARE_INSTALLED,
        crate::tags::SOFTWARE_WEB_CREATED,
        TERMINAL_BELL,
        TERMINAL_WAITING,
        TERMINAL_FINISHED,
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
                crate::tags::BLUETOOTH_CONNECTED | crate::tags::BLUETOOTH_DISCONNECTED
            );
            assert_eq!(
                p.show == ronoctua_notify::Show::Hidden,
                quiet,
                "{tag}: {:?}",
                p.show
            );
        }
    }

    /// A registry that knows EVERY command the distro's book names, so
    /// `actions_of` leaves nothing of the book out here and a test sees each
    /// entry whole: an action an entry grows is drawn, whatever command it
    /// runs. That the binary's registry knows the same commands is the
    /// binary's test (`every_command_has_an_owner`).
    fn knowing_the_book(book: &ronoctua_notify::Book) -> ronoctua_cmd::Registry {
        let named: std::collections::BTreeSet<&str> = book
            .entries
            .iter()
            .flat_map(|e| &e.actions)
            .filter_map(|a| match &a.doing {
                ronoctua_notify::Doing::Command(c) => Some(c.as_str()),
                _ => None,
            })
            .collect();
        let mut registry = ronoctua_cmd::Registry::fresh();
        for id in named {
            // LEAKED, a few bytes per test: a `Command` holds `&'static str`,
            // as the generated keys are.
            let id: &'static str = Box::leak(id.to_owned().into_boxed_str());
            let label: &'static str =
                Box::leak(format!("{}{id}", ronoctua_cmd::KEY_PREFIX_OF).into_boxed_str());
            registry
                .register(ronoctua_cmd::Command { id, label })
                .expect("a command the book names registers");
        }
        registry
    }

    /// FAILS IF: the program installed stops offering to open it on the
    /// line, an event whose entry has no action grows one (whatever command
    /// it runs), or a command the registry does not know is drawn.
    #[test]
    fn the_actions_of_an_event_come_from_its_entry() {
        let book = ronoctua_notify::Book::distro();
        let registry = knowing_the_book(&book);
        let t = texts();
        let installed = Event::installed(crate::tags::SOFTWARE_INSTALLED, "GIMP", None);
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
        let web = Event::installed(crate::tags::SOFTWARE_WEB_CREATED, "Mail", None);
        assert_eq!(actions_of(&web, &book, &registry, &t).len(), 1);
        assert!(actions_of(&e(NETWORK_LOST), &book, &registry, &t).is_empty());
        // AND A COMMAND THE REGISTRY DOES NOT KNOW is left out, not drawn as
        // a button that does nothing.
        assert!(
            actions_of(&installed, &book, &ronoctua_cmd::Registry::fresh(), &t).is_empty(),
            "an action whose command nobody registers was drawn"
        );
    }

    /// FAILS IF: "Open it" on an install's line launches anything but the
    /// program THAT line names — the latest install, when a newer one
    /// finished while this line was up — or a command its entry does not
    /// offer runs on it.
    #[test]
    fn the_install_on_the_line_is_the_one_opened() {
        let book = ronoctua_notify::Book::distro();
        let gimp = Event::installed(
            crate::tags::SOFTWARE_INSTALLED,
            "GIMP",
            Some("org.gimp.GIMP.desktop"),
        );
        assert_eq!(
            subject_of(&gimp, LAUNCH_LATEST, &book),
            Some("org.gimp.GIMP.desktop")
        );
        assert_eq!(subject_of(&gimp, "files-open", &book), None);
        let unknown = Event::installed(crate::tags::SOFTWARE_INSTALLED, "GIMP", None);
        assert_eq!(subject_of(&unknown, LAUNCH_LATEST, &book), None);
    }

    /// FAILS IF: an event's line opens anything but what THAT line names —
    /// "show it" on a terminal's bell shows another terminal, or none — or
    /// the band's button, once the event goes there, runs on nothing: the
    /// subject has to cross to the notification the event becomes.
    #[test]
    fn an_event_opens_what_its_line_names() {
        let book = ronoctua_notify::Book::distro();
        let t = texts();
        for tag in [TERMINAL_BELL, TERMINAL_WAITING, TERMINAL_FINISHED] {
            let rang = Event::of(tag, key::NOTIF_TERMINAL_APP)
                .about("title", "vim notes.md")
                .about("program", "claude")
                .counting("code", 0)
                .on("0123456789abcdef");
            assert_eq!(
                subject_of(&rang, TERMINAL_SHOW, &book),
                Some("0123456789abcdef"),
                "{tag}"
            );
            let n = notification(&rang, &t);
            assert_eq!(n.subject.as_deref(), Some("0123456789abcdef"), "{tag}");
            let p = book.decide(&n);
            assert_eq!(p.catalogue.as_deref(), Some(tag));
            assert_ne!(p.show, ronoctua_notify::Show::Hidden, "{tag}");
        }
        // AN EVENT ABOUT NOTHING crosses as a notification about nothing.
        assert_eq!(notification(&e(NETWORK_LOST), &t).subject, None);
    }

    /// FAILS IF: a terminal ringing twice while its first line is up fills
    /// the queue with the same sentence — or one terminal's line takes the
    /// place of another's, or of an event about nothing in particular.
    #[test]
    fn the_same_thing_said_again_of_one_subject_replaces_it() {
        let now = Instant::now();
        let rang = |id: &str| Event::of(TERMINAL_BELL, key::NOTIF_TERMINAL_APP).on(id);
        let mut line = Line::default();
        assert!(line.arrive(rang("aaaaaaaaaaaaaaaa"), false, now).is_none());
        assert!(line.arrive(rang("aaaaaaaaaaaaaaaa"), false, now).is_none());
        assert!(
            line.waiting.is_empty(),
            "the second bell queued behind the first"
        );
        assert!(line.arrive(rang("bbbbbbbbbbbbbbbb"), false, now).is_none());
        assert_eq!(line.waiting.len(), 1, "another terminal's bell is its own");
        assert!(
            !repeats(&e(NETWORK_LOST), &e(NETWORK_LOST)),
            "about nothing"
        );
    }
}
