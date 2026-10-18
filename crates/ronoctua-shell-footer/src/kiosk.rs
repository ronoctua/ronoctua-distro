//! The Chrome kiosk, the footer's half — `LEADER m b`.
//!
//! # What a kiosk is
//!
//! An ordinary window of the person's own Chrome — the last profile they
//! used, their logins — whose tab strip and toolbar the COMPOSITOR hides by
//! telling the window it is fullscreen while its tile keeps its size
//! (`fullscreen enable application`). The footer stays; the modeline becomes
//! the browser's toolbar ([`Adapter`]), and the rest of what Chrome's bars
//! did is the kiosk's view (`kiosk_view`) and the `,` group.
//!
//! # A pair is made from two halves, never from timing
//!
//! `kiosk-open` starts Chrome on our extension's `kiosk.html` with a random
//! nonce in the address (`ronoctua_browser::launch`). The page puts the nonce
//! in its TITLE, which the compositor reports ([`Kiosks::compositor`]), and
//! tells the extension, whose state reaches the footer through the bridge
//! ([`Kiosks::heard`]). The two can arrive in either order; when both have,
//! the window is a kiosk: the compositor hides its bars, the page is told to
//! go to its address, and the pair is kept in `kiosk.json` so a footer that
//! restarts keeps it. A launch nothing answers is let go after [`PATIENCE`]
//! and becomes an ordinary Chrome window.
//!
//! # A copy the compositor cannot give
//!
//! Whether a window's bars are hidden is said by no reply and no event of
//! the compositor's, so each pair keeps it ([`Ui`]) and the footer says ON or
//! OFF, never the toggle. Two things move it besides the footer's own
//! commands: a real fullscreen that ends (a video) takes the bars' state
//! with it, and the footer puts it back; and the person pressing F11 inside
//! Chrome, which the footer FOLLOWS rather than fighting.
//!
//! # Pure
//!
//! Nothing here opens a socket, a file or the compositor: every method
//! answers with [`Errand`]s and the footer carries them out
//! (`Footer::kiosk_errands`). That keeps this testable without a Chrome and
//! keeps the footer's own methods from asking each other in a ring
//! (`the-footer-does-not-ask-itself`).

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::key;
use ronoctua_browser::serve::{Conn, Heard};
use ronoctua_browser::url::{self, Typed};
use ronoctua_browser::wire::{self, Envelope, Order, Said};
use ronoctua_compositor::{Event, Window};
use ronoctua_i18n::{Arg, Texts};
use ronoctua_render::layout::{self, Segment, Shrink, Side};
use ronoctua_tokens as tokens;
use serde::{Deserialize, Serialize};

use crate::adapt::{Act, Button, ModelineAdapter, Strip};

/// The `app_id` Chrome gives its own browser windows — a kiosk is one of
/// them. A web app's window is `chrome-…` (`daemon::CHROME_APP_PREFIX`) and
/// is never a kiosk.
pub const APP_ID: &str = "google-chrome";

/// The adapter's id: its switch in `modeline.json`.
pub const ADAPTER: &str = "kiosk";

/// How long a launch waits for its two halves before it is let go.
///
/// Long enough for a cold Chrome on a slow disk to start, load the profile
/// and run the extension's worker; short enough that a person who clicked
/// and saw an ordinary window is told why while they are still looking.
pub const PATIENCE: Duration = Duration::from_secs(20);

/// The tabs' pool — `Segment::id` is `&'static str`, so the modeline's tabs
/// are named by their place, like the workspace numbers (`workspaces::POOL`).
/// Sixteen around the tab in front; the rest are counted.
pub const TABS: [&str; 16] = [
    "kiosk-tab-1",
    "kiosk-tab-2",
    "kiosk-tab-3",
    "kiosk-tab-4",
    "kiosk-tab-5",
    "kiosk-tab-6",
    "kiosk-tab-7",
    "kiosk-tab-8",
    "kiosk-tab-9",
    "kiosk-tab-10",
    "kiosk-tab-11",
    "kiosk-tab-12",
    "kiosk-tab-13",
    "kiosk-tab-14",
    "kiosk-tab-15",
    "kiosk-tab-16",
];

/// The pinned extensions' pool, eight at most.
pub const EXTENSIONS: [&str; 8] = [
    "kiosk-ext-1",
    "kiosk-ext-2",
    "kiosk-ext-3",
    "kiosk-ext-4",
    "kiosk-ext-5",
    "kiosk-ext-6",
    "kiosk-ext-7",
    "kiosk-ext-8",
];

/// The counter before the tabs that are drawn.
pub const BEFORE: &str = "kiosk-tabs-before";
/// The counter after them.
pub const AFTER: &str = "kiosk-tabs-after";

/// Chrome's own zoom levels, in percent — the steps its `Ctrl +` and `Ctrl -`
/// walk, so the kiosk's steps land where Chrome's would.
pub const ZOOMS: [u32; 17] = [
    25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400, 500,
];

/// The tab in front never elides below this many cells: fewer is not a word.
const ACTIVE_FLOOR: u16 = 6;

/// Whether Chrome's own bars are hidden on a kiosk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ui {
    /// Hidden by the compositor — the kiosk.
    Hidden,
    /// Showing — Chrome as it always is ("normal mode").
    Shown,
}

/// One kiosk: its window as the compositor knows it and as Chrome does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The nonce it was born with — what finds it again in Chrome's state.
    pub nonce: String,
    /// The compositor's window id.
    pub compositor: u64,
    /// Chrome's `windowId`.
    pub chrome: i64,
    /// Whether its bars are hidden — the footer's copy.
    pub ui: Ui,
    /// Whether the compositor last said the window covers its screen for
    /// real — what a video's fullscreen looks like from outside.
    full: bool,
    /// The bars were asked hidden and Chrome has not said it is fullscreen
    /// since: its reports in between are the old picture, not an F11.
    awaiting_full: bool,
    /// Words `kiosk-open` was given, searched for once the kiosk has a tab
    /// that is not the page it was born on — the extension closes that one
    /// when it goes, so an order naming it would find nothing.
    search: Option<String>,
}

/// What `kiosk.json` keeps of a pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// The nonce.
    pub nonce: String,
    /// The compositor's window id when it was written.
    pub compositor: u64,
    /// Chrome's window id.
    pub chrome: i64,
    /// The bars.
    pub ui: Ui,
}

/// A launch waiting for its two halves.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    nonce: String,
    since: Instant,
    /// Words to search for once it is paired — `kiosk-open two words`.
    words: Option<String>,
    /// The compositor's half: the window whose title carried the nonce.
    window: Option<u64>,
    /// The extension's half: the connection and Chrome's window.
    chrome: Option<(Conn, i64)>,
}

/// Why a kiosk draws its offline mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offline {
    /// The bridge's manifest is not on this machine.
    Bridge,
    /// Chrome has not connected: its extension may be off.
    Chrome,
    /// The extension speaks another version of the wire.
    Version,
}

/// Opening a new kiosk needs these, which only the footer can read.
#[derive(Debug, Clone, Copy)]
pub struct Opening<'a> {
    /// Chrome's program, when this machine has one.
    pub chrome: Option<&'a std::path::Path>,
    /// Our extension's id, read from the bridge's manifest; `None` when the
    /// bridge is not installed.
    pub extension: Option<&'a str>,
    /// A fresh nonce; `None` when none could be read.
    pub nonce: Option<&'a str>,
}

/// What starts a kiosk window, handed to the footer's launching thread —
/// which reads the profile (Chrome's `Local State`, a file) off the loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launching {
    /// Chrome's program.
    pub chrome: std::path::PathBuf,
    /// Our extension's id.
    pub extension: String,
    /// The nonce.
    pub nonce: String,
    /// Where it goes once paired.
    pub url: Option<String>,
}

/// What the footer does for the kiosks.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Send one order to one host.
    Send(Conn, Envelope),
    /// Show or hide a window's bars — the compositor's `fullscreen
    /// application`, said ON or OFF. The ONE place the mechanism is named.
    Bars {
        /// The compositor's window.
        window: u64,
        /// Show them (normal mode), or hide them (the kiosk).
        shown: bool,
    },
    /// Give this window the focus.
    Focus(u64),
    /// Ask on the minibuffer's line, the answer coming back under `id`.
    Ask {
        /// The command that asks and answers.
        id: &'static str,
        /// What the line starts with.
        initial: Option<String>,
    },
    /// Say this on the message line.
    Echo(&'static str),
    /// Say that Chrome refused, in Chrome's words.
    Refused(String),
    /// Start a kiosk window.
    Launch(Launching),
    /// Write `kiosk.json` again.
    Persist,
    /// Be woken after this long, to let go of a launch nothing answered.
    ArmTimer(Duration),
}

/// Every kiosk, every launch under way, and what each Chrome said last.
#[derive(Debug, Clone, Default)]
pub struct Kiosks {
    pairs: Vec<Pair>,
    pending: Vec<Pending>,
    /// Each live connection's last whole state.
    conns: BTreeMap<Conn, wire::State>,
    /// The next order's serial.
    serial: u64,
    /// Our extension's id, when the bridge is installed — the lists leave
    /// it out: it is the bridge, not something to open.
    ours: Option<String>,
    /// Whether the bridge's manifest was found.
    bridge: bool,
    /// What an open question is about: the window and the tab it was asked
    /// over — the answer lands there even if the focus moved meanwhile.
    asking: Option<(u64, Option<i64>)>,
    /// The kiosk that had the focus last.
    last_focused: Option<u64>,
    /// Kiosk pages nobody here waits for, already let go — by connection
    /// and nonce, while that connection's state still lists them: a state
    /// sent before the extension took the order in is not a second page.
    let_go_of: BTreeSet<(Conn, String)>,
}

impl Kiosks {
    /// The kiosks a footer starts with: what `kiosk.json` kept (unconfirmed
    /// until the compositor's first list), and whether the bridge is there.
    #[must_use]
    pub fn fresh(kept: &[u8], ours: Option<String>) -> Self {
        let kept: Vec<Kept> = serde_json::from_slice(kept).unwrap_or_default();
        Self {
            pairs: kept
                .into_iter()
                .map(|k| Pair {
                    nonce: k.nonce,
                    compositor: k.compositor,
                    chrome: k.chrome,
                    ui: k.ui,
                    full: false,
                    awaiting_full: false,
                    search: None,
                })
                .collect(),
            bridge: ours.is_some(),
            ours,
            ..Self::default()
        }
    }

    /// What `kiosk.json` holds now.
    #[must_use]
    pub fn kept(&self) -> Vec<u8> {
        let kept: Vec<Kept> = self
            .pairs
            .iter()
            .map(|p| Kept {
                nonce: p.nonce.clone(),
                compositor: p.compositor,
                chrome: p.chrome,
                ui: p.ui,
            })
            .collect();
        serde_json::to_vec_pretty(&kept).unwrap_or_default()
    }

    /// The minibuffer's question was let go: an answer typed later under the
    /// same command is not taken for it.
    pub fn let_go(&mut self) {
        self.asking = None;
    }

    /// The bridge was looked for again: its manifest names our extension, or
    /// there is none.
    pub fn bridge_found(&mut self, ours: Option<String>) {
        self.bridge = ours.is_some();
        self.ours = ours;
    }

    /// Our extension's id.
    #[must_use]
    pub fn ours(&self) -> Option<&str> {
        self.ours.as_deref()
    }

    /// The pair whose window this is.
    #[must_use]
    pub fn pair_of(&self, window: u64) -> Option<&Pair> {
        self.pairs.iter().find(|p| p.compositor == window)
    }

    /// The pair's Chrome, when it is connected — in whatever version of the
    /// wire: its connection, that connection's state, and the kiosk in it.
    fn connected(&self, pair: &Pair) -> Option<(Conn, &wire::State, &wire::Kiosk)> {
        self.conns.iter().find_map(|(c, s)| {
            s.kiosks
                .iter()
                .find(|k| k.nonce == pair.nonce)
                .map(|k| (*c, s, k))
        })
    }

    /// The pair's Chrome, when it can be DRIVEN: connected, and speaking
    /// this footer's wire. The one test every extension-dependent path asks
    /// — the same one the modeline draws by ([`Kiosks::offline`]), so a
    /// kiosk drawn offline is never sent an order behind the mark's back.
    fn live(&self, pair: &Pair) -> Option<(Conn, &wire::State, &wire::Kiosk)> {
        if !self.bridge {
            return None;
        }
        self.connected(pair)
            .filter(|(_, s, _)| s.v == wire::VERSION)
    }

    /// Why a kiosk cannot be driven, or `None` when it can.
    #[must_use]
    pub fn offline(&self, window: u64) -> Option<Offline> {
        let pair = self.pair_of(window)?;
        if !self.bridge {
            return Some(Offline::Bridge);
        }
        match self.connected(pair) {
            None => Some(Offline::Chrome),
            Some((_, s, _)) if s.v != wire::VERSION => Some(Offline::Version),
            Some(_) => None,
        }
    }

    /// The kiosk's tabs and its profile's extensions, when it is connected.
    #[must_use]
    pub fn state_of(&self, window: u64) -> Option<(&wire::Kiosk, &[wire::Extension])> {
        let pair = self.pair_of(window)?;
        let (_, s, k) = self.live(pair)?;
        Some((k, s.extensions.as_slice()))
    }

    /// What the kiosk's view draws its lists from.
    #[must_use]
    pub fn reading(&self, window: u64) -> crate::kiosk_view::Reading<'_> {
        let (kiosk, extensions) = self
            .state_of(window)
            .map_or((None, &[][..]), |(k, e)| (Some(k), e));
        crate::kiosk_view::Reading {
            kiosk,
            extensions,
            ours: self.ours(),
        }
    }

    /// The address of the kiosk's tab in front.
    #[must_use]
    pub fn address_of(&self, window: u64) -> Option<&str> {
        let (k, _) = self.state_of(window)?;
        active(k).map(|t| t.url.as_str()).filter(|u| !u.is_empty())
    }

    /// Whether this kiosk's bars are hidden.
    #[must_use]
    pub fn hidden(&self, window: u64) -> bool {
        self.pair_of(window).is_some_and(|p| p.ui == Ui::Hidden)
    }

    fn envelope(&mut self, order: Order) -> Envelope {
        self.serial += 1;
        Envelope {
            serial: self.serial,
            order,
        }
    }

    fn send(&mut self, conn: Conn, order: Order) -> Errand {
        Errand::Send(conn, self.envelope(order))
    }

    /// The compositor said something. `windows` is its whole list AFTER the
    /// event was taken in.
    pub fn compositor(&mut self, e: &Event, windows: &[Window]) -> Vec<Errand> {
        let mut out = Vec::new();
        match e {
            Event::Windows(_) => {
                // THE KEPT PAIRS ARE MEASURED against every whole list — the
                // first one after a start, and the one a compositor that
                // started again sends: a pair whose window is still Chrome's
                // stays; one whose window is gone may be found again by its
                // nonce in a title (a photo scene, a pairing a restart
                // interrupted); the rest go.
                let before = self.pairs.len();
                let mut moved = false;
                for p in &mut self.pairs {
                    let same = windows.iter().any(|w| w.id == p.compositor && is_chrome(w));
                    if same {
                        continue;
                    }
                    if let Some(w) = windows
                        .iter()
                        .find(|w| is_chrome(w) && titled_with(w, &p.nonce))
                    {
                        p.compositor = w.id;
                        moved = true;
                    } else {
                        p.compositor = u64::MAX;
                    }
                }
                self.pairs.retain(|p| p.compositor != u64::MAX);
                if moved || self.pairs.len() != before {
                    out.push(Errand::Persist);
                }
                for w in windows {
                    out.extend(self.window_seen(w));
                }
            }
            Event::WindowChanged(w) => out.extend(self.window_seen(w)),
            Event::WindowClosed(id) => {
                let before = self.pairs.len();
                self.pairs.retain(|p| p.compositor != *id);
                if self.pairs.len() != before {
                    out.push(Errand::Persist);
                }
                if self.last_focused == Some(*id) {
                    self.last_focused = None;
                }
            }
            Event::FocusChanged(Some(id)) if self.pair_of(*id).is_some() => {
                self.last_focused = Some(*id);
            }
            _ => {}
        }
        out
    }

    /// One window as the compositor describes it now: a launch's title
    /// half, and a kiosk's real fullscreen.
    fn window_seen(&mut self, w: &Window) -> Vec<Errand> {
        let mut out = Vec::new();
        // THE COMPOSITOR'S HALF: Chrome's window whose title carries a
        // waiting nonce. Only Chrome's — a window of anything else that
        // happened to show the nonce is not the page this launched.
        if is_chrome(w) {
            let found: Vec<String> = self
                .pending
                .iter()
                .filter(|p| p.window.is_none() && titled_with(w, &p.nonce))
                .map(|p| p.nonce.clone())
                .collect();
            for nonce in found {
                if let Some(p) = self.pending.iter_mut().find(|p| p.nonce == nonce) {
                    p.window = Some(w.id);
                }
                out.extend(self.try_pair(&nonce));
            }
        }
        // A VIDEO'S FULLSCREEN THAT ENDS takes the bars' state with it — the
        // compositor's two fullscreens are one state per window — so the
        // footer puts the kiosk back, when it is one.
        if let Some(p) = self.pairs.iter_mut().find(|p| p.compositor == w.id) {
            let ended = p.full && !w.fullscreen;
            p.full = w.fullscreen;
            if ended && p.ui == Ui::Hidden {
                p.awaiting_full = true;
                out.push(Errand::Bars {
                    window: w.id,
                    shown: false,
                });
            }
        }
        out
    }

    /// The bridge heard something.
    pub fn heard(&mut self, h: Heard) -> Vec<Errand> {
        match h {
            // A HOST THAT CONNECTS is asked for its whole state at once:
            // what it sends by itself may already have gone by.
            Heard::Connected(conn) => {
                self.conns.insert(conn, wire::State::default());
                vec![self.send(conn, Order::Look)]
            }
            Heard::Said(conn, Said::State(state)) => self.state_arrived(conn, state),
            Heard::Said(_, Said::Done(done)) => done_said(&done).into_iter().collect(),
            // THE PAIRS STAY: the window is still the compositor's, and its
            // bars are still the footer's to show. What goes is what Chrome
            // said; the modeline draws the kiosk offline until it comes back.
            Heard::Gone(conn) => {
                self.conns.remove(&conn);
                self.let_go_of.retain(|(c, _)| *c != conn);
                Vec::new()
            }
        }
    }

    /// A connection's whole state.
    fn state_arrived(&mut self, conn: Conn, state: wire::State) -> Vec<Errand> {
        let mut out = Vec::new();
        for k in &state.kiosks {
            if let Some(p) = self.pending.iter_mut().find(|p| p.nonce == k.nonce) {
                // THE EXTENSION'S HALF of a launch.
                p.chrome = Some((conn, k.window));
                let nonce = k.nonce.clone();
                out.extend(self.try_pair(&nonce));
                continue;
            }
            if let Some(p) = self.pairs.iter_mut().find(|p| p.nonce == k.nonce) {
                p.chrome = k.window;
                let fullscreen = k.state == "fullscreen";
                if fullscreen {
                    p.awaiting_full = false;
                }
                // F11, PRESSED INSIDE CHROME: it says it is no longer
                // fullscreen while the compositor says nothing covers the
                // screen, and nothing of ours asked for that. The person
                // wanted the bars — the kiosk follows rather than fighting.
                if !fullscreen && !p.full && !p.awaiting_full && p.ui == Ui::Hidden {
                    p.ui = Ui::Shown;
                    out.push(Errand::Persist);
                }
                // THE WORDS `kiosk-open` WAS GIVEN, in the first tab that is
                // not the page the kiosk was born on: that page closes when it
                // goes, and the tab the extension opens in its place is where
                // the person is looking.
                let landed = if p.search.is_some() {
                    k.tabs.iter().find(|t| !born_on(t, &k.nonce)).map(|t| t.id)
                } else {
                    None
                };
                let words = landed.and_then(|_| p.search.take());
                if let (Some(tab), Some(text)) = (landed, words) {
                    out.push(self.send(
                        conn,
                        Order::Search {
                            tab,
                            text,
                            new_tab: false,
                        },
                    ));
                }
                continue;
            }
            // A KIOSK PAGE NOBODY HERE IS WAITING FOR — the footer restarted
            // between the launch and the pair — goes on to its address as an
            // ordinary window rather than waiting for ever; and it IS an
            // ordinary window, so the extension is told to forget it too:
            // `Go` alone keeps it a kiosk there, with the port, the host and
            // the worker alive for a window nobody here draws. Once per page:
            // a state sent before the extension took the orders in is the
            // same page, not a second one.
            if k.born && !self.let_go_of.contains(&(conn, k.nonce.clone())) {
                out.extend(self.go_unpaired(conn, k.nonce.clone(), k.window));
            }
        }
        // WHAT WAS LET GO is remembered while this connection still lists it.
        self.let_go_of.retain(|(c, nonce)| {
            *c != conn || state.kiosks.iter().any(|k| k.nonce == *nonce && k.born)
        });
        self.conns.insert(conn, state);
        out
    }

    /// A kiosk page this footer will not pair — nobody waits for it, or
    /// nothing answered in time — goes on to its address, and the extension
    /// forgets the window: it is an ordinary one from now on.
    fn go_unpaired(&mut self, conn: Conn, nonce: String, window: i64) -> [Errand; 2] {
        self.let_go_of.insert((conn, nonce.clone()));
        [
            self.send(conn, Order::Go { nonce }),
            self.send(conn, Order::Release { window }),
        ]
    }

    /// Makes the pair when both halves of `nonce` are in.
    fn try_pair(&mut self, nonce: &str) -> Vec<Errand> {
        let Some(i) = self.pending.iter().position(|p| p.nonce == nonce) else {
            return Vec::new();
        };
        let (Some(window), Some((conn, chrome))) = (self.pending[i].window, self.pending[i].chrome)
        else {
            return Vec::new();
        };
        let p = self.pending.remove(i);
        // ONE KIOSK PER WINDOW: a window paired again is the newer pair.
        self.pairs.retain(|q| q.compositor != window);
        self.pairs.push(Pair {
            nonce: p.nonce.clone(),
            compositor: window,
            chrome,
            ui: Ui::Hidden,
            full: false,
            awaiting_full: true,
            // THE WORDS WAIT ON THE PAIR. The only tab now is the page the
            // kiosk was born on, and `Go` with no address closes it for
            // Chrome's new-tab page: a search sent now would name a tab that
            // is about to go. The next state shows the tab that took its
            // place (`state_arrived`), and the search goes there.
            search: p.words,
        });
        self.last_focused = Some(window);
        // THE WORD THE KIOSK VM TEST LOOKS FOR in the footer's journal.
        tracing::info!(
            nonce = %p.nonce,
            compositor = window,
            chrome,
            "kiosk paired"
        );
        // THE BARS FIRST, then the address: the page loads at the size it
        // will be read at.
        vec![
            Errand::Bars {
                window,
                shown: false,
            },
            self.send(conn, Order::Go { nonce: p.nonce }),
            Errand::Persist,
        ]
    }

    /// The timer: launches nothing answered are let go — the ones Chrome
    /// half-answered go on to their address as ordinary windows.
    pub fn expired(&mut self, now: Instant) -> Vec<Errand> {
        let mut out = Vec::new();
        let (old, young): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| now.saturating_duration_since(p.since) >= PATIENCE);
        self.pending = young;
        for p in old {
            tracing::warn!(nonce = %p.nonce, "a kiosk launch was not answered in time");
            if let Some((conn, window)) = p.chrome {
                out.extend(self.go_unpaired(conn, p.nonce, window));
            }
            out.push(Errand::Echo(key::ECHO_KIOSK_UNMATCHED));
        }
        if let Some(next) = self
            .pending
            .iter()
            .map(|p| PATIENCE.saturating_sub(now.saturating_duration_since(p.since)))
            .min()
        {
            out.push(Errand::ArmTimer(next));
        }
        out
    }

    /// The launch of `nonce` failed before any Chrome ran — the footer has
    /// already said why, in the launch's own words. Nothing will answer it,
    /// so it stops waiting, and the clock does not say later that an
    /// ordinary window opened.
    pub fn launch_failed(&mut self, nonce: &str) {
        self.pending.retain(|p| p.nonce != nonce);
    }

    /// `kiosk-open [address or words]`.
    ///
    /// A kiosk already open is used: with nothing typed it takes the focus,
    /// and with an address it opens a tab there. Otherwise a new one is born
    /// — refused, with a sentence, when this machine cannot.
    pub fn open(&mut self, words: Option<&str>, o: Opening<'_>, now: Instant) -> Vec<Errand> {
        let typed = words.map_or(Typed::Empty, url::typed);
        if typed == Typed::Refused {
            return vec![Errand::Echo(key::ECHO_KIOSK_ADDRESS_REFUSED)];
        }
        let existing = self
            .last_focused
            .filter(|w| self.pair_of(*w).is_some())
            .or_else(|| self.pairs.last().map(|p| p.compositor));
        if let Some(window) = existing {
            if typed == Typed::Empty {
                return vec![Errand::Focus(window)];
            }
            match self.offline(window) {
                None => {
                    let mut out = self.typed_into(window, None, typed.clone(), true);
                    out.push(Errand::Focus(window));
                    return out;
                }
                // ANOTHER VERSION OF THE WIRE is the extension's, and a new
                // kiosk would be born into the same extension, as deaf as this
                // one: said, and not launched once per address.
                Some(Offline::Version) => return vec![Errand::Echo(key::ECHO_KIOSK_OFFLINE)],
                Some(Offline::Bridge | Offline::Chrome) => {}
            }
        }
        let Some(chrome) = o.chrome else {
            return vec![Errand::Echo(key::ECHO_KIOSK_NO_CHROME)];
        };
        let Some(extension) = o.extension else {
            return vec![Errand::Echo(key::ECHO_KIOSK_NO_BRIDGE)];
        };
        let Some(nonce) = o.nonce else {
            return vec![Errand::Echo(key::ECHO_KIOSK_NO_NONCE)];
        };
        let (url, words) = match typed {
            Typed::Address(a) => (Some(a), None),
            Typed::Words(w) => (None, Some(w)),
            Typed::Empty | Typed::Refused => (None, None),
        };
        self.pending.push(Pending {
            nonce: nonce.to_owned(),
            since: now,
            words,
            window: None,
            chrome: None,
        });
        vec![
            Errand::Launch(Launching {
                chrome: chrome.to_path_buf(),
                extension: extension.to_owned(),
                nonce: nonce.to_owned(),
                url,
            }),
            Errand::ArmTimer(PATIENCE),
        ]
    }

    /// A line typed for a kiosk: an address goes there, words are searched,
    /// an address this system will not open is refused.
    fn typed_into(
        &mut self,
        window: u64,
        tab: Option<i64>,
        typed: Typed,
        new_tab: bool,
    ) -> Vec<Errand> {
        let Some(pair) = self.pair_of(window).cloned() else {
            return vec![Errand::Echo(key::ECHO_KIOSK_NOT_A_KIOSK)];
        };
        let Some((conn, _, k)) = self.live(&pair) else {
            return vec![Errand::Echo(key::ECHO_KIOSK_OFFLINE)];
        };
        let tab = tab.or_else(|| active(k).map(|t| t.id));
        let order = match (typed, new_tab, tab) {
            (Typed::Refused, _, _) => return vec![Errand::Echo(key::ECHO_KIOSK_ADDRESS_REFUSED)],
            (Typed::Empty, true, _) => Order::Create {
                window: pair.chrome,
                url: None,
            },
            (Typed::Empty, false, _) => return Vec::new(),
            (Typed::Address(url), true, _) => Order::Create {
                window: pair.chrome,
                url: Some(url),
            },
            (Typed::Address(url), false, Some(tab)) => Order::Navigate { tab, url },
            (Typed::Words(text), new_tab, Some(tab)) => Order::Search { tab, text, new_tab },
            (_, _, None) => return vec![Errand::Echo(key::ECHO_KIOSK_NO_TAB)],
        };
        vec![self.send(conn, order)]
    }

    /// A command of the registry about the kiosk `on` (the view's window, or
    /// the focused one). The view's own openers are the footer's.
    pub fn command(&mut self, id: &str, arg: Option<&str>, on: Option<u64>) -> Vec<Errand> {
        // AN ANSWER LANDS WHERE ITS QUESTION WAS ASKED, whatever has the
        // focus now; typed by name with no question open, on the kiosk `on`.
        if let ("kiosk-new-tab" | "kiosk-address", Some(said)) = (id, arg) {
            let Some((at, tab)) = self.asking.take().or_else(|| on.map(|w| (w, None))) else {
                return vec![Errand::Echo(key::ECHO_KIOSK_NOT_A_KIOSK)];
            };
            return self.typed_into(at, tab, url::typed(said), id == "kiosk-new-tab");
        }
        let Some(pair) = on.and_then(|w| self.pair_of(w)).cloned() else {
            return vec![Errand::Echo(key::ECHO_KIOSK_NOT_A_KIOSK)];
        };
        let window = pair.compositor;
        // THE TWO THAT ONLY NEED THE COMPOSITOR, and so work offline.
        match id {
            "kiosk-chrome-ui" => {
                let shown = pair.ui == Ui::Hidden;
                return self.bars(window, shown);
            }
            "kiosk-release" => {
                let mut out = vec![Errand::Bars {
                    window,
                    shown: true,
                }];
                if let Some((conn, _, _)) = self.live(&pair) {
                    out.push(self.send(
                        conn,
                        Order::Release {
                            window: pair.chrome,
                        },
                    ));
                }
                self.pairs.retain(|p| p.compositor != window);
                out.push(Errand::Persist);
                return out;
            }
            _ => {}
        }
        // A QUESTION ASKS WHETHER CHROME IS THERE OR NOT — the answer is
        // what needs it, and saying so then is the honest moment.
        match (id, arg) {
            ("kiosk-new-tab", None) => {
                self.asking = Some((window, None));
                return vec![Errand::Ask {
                    id: "kiosk-new-tab",
                    initial: None,
                }];
            }
            ("kiosk-address", None) => {
                let (tab, url) = self
                    .state_of(window)
                    .and_then(|(k, _)| active(k))
                    .map_or((None, None), |t| (Some(t.id), Some(t.url.clone())));
                self.asking = Some((window, tab));
                return vec![Errand::Ask {
                    id: "kiosk-address",
                    initial: url,
                }];
            }
            _ => {}
        }
        let Some((conn, state, k)) = self.live(&pair) else {
            return vec![Errand::Echo(key::ECHO_KIOSK_OFFLINE)];
        };
        let tabs = k.tabs.clone();
        let extensions = state.extensions.clone();
        let front = active(k).cloned();
        let front_id = front.as_ref().map(|t| t.id);
        let tab_of =
            |arg: Option<&str>| arg.and_then(|a| a.trim().parse::<i64>().ok()).or(front_id);
        let order = match id {
            "kiosk-back" => front_id.map(|tab| Order::Back { tab }),
            "kiosk-forward" => front_id.map(|tab| Order::Forward { tab }),
            "kiosk-reload" => front_id.map(|tab| Order::Reload { tab }),
            "kiosk-tab" => tab_of(arg).map(|tab| Order::Activate { tab }),
            "kiosk-tab-close" => tab_of(arg).map(|tab| Order::Close { tab }),
            "kiosk-tab-next" | "kiosk-tab-previous" => {
                step_tab(&tabs, front.as_ref(), id == "kiosk-tab-next")
                    .map(|tab| Order::Activate { tab })
            }
            "kiosk-tab-left" | "kiosk-tab-right" => front.as_ref().and_then(|t| {
                let index = if id == "kiosk-tab-left" {
                    t.index.checked_sub(1)?
                } else {
                    let last = u32::try_from(tabs.len().saturating_sub(1)).unwrap_or(u32::MAX);
                    (t.index < last).then_some(t.index + 1)?
                };
                Some(Order::Move { tab: t.id, index })
            }),
            // `id index`, what a drag and the list's `K`/`J` say.
            "kiosk-tab-move" => arg.and_then(|a| {
                let (tab, index) = a.trim().split_once(' ')?;
                Some(Order::Move {
                    tab: tab.parse().ok()?,
                    index: index.trim().parse().ok()?,
                })
            }),
            "kiosk-zoom-in" | "kiosk-zoom-out" | "kiosk-zoom-reset" | "kiosk-zoom" => {
                front.as_ref().map(|t| {
                    let percent = match id {
                        "kiosk-zoom-in" => zoom_step(t.zoom_percent, true),
                        "kiosk-zoom-out" => zoom_step(t.zoom_percent, false),
                        "kiosk-zoom" => arg
                            .and_then(|a| a.trim().parse().ok())
                            .filter(|p| ZOOMS.contains(p))
                            .unwrap_or(t.zoom_percent),
                        _ => 100,
                    };
                    Order::Zoom { tab: t.id, percent }
                })
            }
            "kiosk-extension" | "kiosk-extension-options" | "kiosk-extension-details" => {
                let Some(e) = arg.and_then(|a| extensions.iter().find(|e| e.id == a.trim())) else {
                    return vec![Errand::Echo(key::ECHO_KIOSK_NO_EXTENSION)];
                };
                let details = format!("chrome://extensions/?id={}", e.id);
                let (page, said) = match id {
                    // ITS POPUP, in a tab — the page its toolbar button opens
                    // — or its details, saying there was none.
                    "kiosk-extension" => match e.popup.as_deref() {
                        Some(p) => (format!("chrome-extension://{}/{p}", e.id), None),
                        None => (details, Some(key::ECHO_KIOSK_NO_PAGE)),
                    },
                    "kiosk-extension-options" => match e.options_url.clone() {
                        Some(u) => (u, None),
                        None => (details, Some(key::ECHO_KIOSK_NO_PAGE)),
                    },
                    _ => (details, None),
                };
                // CHECKED HERE AND AGAIN BY THE EXTENSION: an options address
                // is the extension's own word, and only its own pages open.
                if !url::extension_page(&page) {
                    return vec![Errand::Echo(key::ECHO_KIOSK_NO_EXTENSION)];
                }
                let mut out = vec![self.send(
                    conn,
                    Order::OpenExtension {
                        window: pair.chrome,
                        url: page,
                    },
                )];
                out.extend(said.map(Errand::Echo));
                return out;
            }
            _ => return Vec::new(),
        };
        match order {
            Some(order) => vec![self.send(conn, order)],
            None => vec![Errand::Echo(key::ECHO_KIOSK_NO_TAB)],
        }
    }

    /// The bars of `window` shown or hidden, and the copy with them.
    fn bars(&mut self, window: u64, shown: bool) -> Vec<Errand> {
        if let Some(p) = self.pairs.iter_mut().find(|p| p.compositor == window) {
            p.ui = if shown { Ui::Shown } else { Ui::Hidden };
            p.awaiting_full = !shown;
        }
        vec![Errand::Bars { window, shown }, Errand::Persist]
    }

    /// A fullscreen deed of the compositor's ran on `window` by name
    /// (`LEADER :`, the window menu): the copy follows what it did.
    ///
    /// `Some` for the three that change the bars, and a toggle flips the
    /// copy — the deed itself already went out.
    pub fn deed_ran(&mut self, id: &str, window: u64) -> Option<Errand> {
        let p = self.pairs.iter_mut().find(|p| p.compositor == window)?;
        let shown = match id {
            "wm-full-application-on" => false,
            "wm-full-application-off" => true,
            "wm-full-application" => p.ui == Ui::Hidden,
            _ => return None,
        };
        p.ui = if shown { Ui::Shown } else { Ui::Hidden };
        p.awaiting_full = !shown;
        Some(Errand::Persist)
    }

    /// The adapter over these kiosks.
    #[must_use]
    pub const fn adapter(&self, address_line: bool) -> Adapter<'_> {
        Adapter {
            kiosks: self,
            address_line,
        }
    }
}

/// Whether a window is one of Chrome's own.
fn is_chrome(w: &Window) -> bool {
    w.app_id.as_deref() == Some(APP_ID)
}

/// Whether a window's title carries this nonce.
fn titled_with(w: &Window, nonce: &str) -> bool {
    !nonce.is_empty() && w.title.as_deref().is_some_and(|t| t.contains(nonce))
}

/// Whether a tab is still the page its kiosk was born on: the nonce is in
/// that page's address (`#n=`) and is its whole title.
fn born_on(t: &wire::Tab, nonce: &str) -> bool {
    !nonce.is_empty() && (t.url.contains(nonce) || t.title == nonce)
}

/// The tab in front.
#[must_use]
pub fn active(k: &wire::Kiosk) -> Option<&wire::Tab> {
    k.tabs.iter().find(|t| t.active)
}

/// The tab beside the one in front, going round at the ends.
fn step_tab(tabs: &[wire::Tab], front: Option<&wire::Tab>, next: bool) -> Option<i64> {
    let n = tabs.len();
    let at = tabs
        .iter()
        .position(|t| Some(t.id) == front.map(|f| f.id))?;
    let to = if next { (at + 1) % n } else { (at + n - 1) % n };
    tabs.get(to).map(|t| t.id)
}

/// The next of Chrome's levels in or out from `now` — from a level Chrome
/// was set to by other means too (`Ctrl` and the wheel), which may be none
/// of the steps.
#[must_use]
pub fn zoom_step(now: u32, bigger: bool) -> u32 {
    if bigger {
        ZOOMS.iter().copied().find(|z| *z > now).unwrap_or(now)
    } else {
        ZOOMS
            .iter()
            .rev()
            .copied()
            .find(|z| *z < now)
            .unwrap_or(now)
    }
}

/// What a refusal of Chrome's says, in our words when the word is ours.
fn done_said(done: &wire::Done) -> Option<Errand> {
    if done.ok {
        return None;
    }
    Some(match done.why.as_deref() {
        Some("no-history") => Errand::Echo(key::ECHO_KIOSK_NO_HISTORY),
        Some("no-future") => Errand::Echo(key::ECHO_KIOSK_NO_FUTURE),
        Some("refused-url") => Errand::Echo(key::ECHO_KIOSK_ADDRESS_REFUSED),
        Some("no-tab") => Errand::Echo(key::ECHO_KIOSK_NO_TAB),
        Some(other) => Errand::Refused(other.to_owned()),
        None => Errand::Refused(String::new()),
    })
}

/// The address line's text, `room` cells wide: the glyph, then the scheme
/// and the host WHOLE — a deceptive subdomain must not hide behind an
/// ellipsis — and the rest elided in the middle, so the end of the path
/// stays too.
///
/// WHEN NOT EVEN THE HOST FITS (a long preview subdomain on a narrow or cut
/// screen), its START goes and its end stays: the end is the registrable
/// domain, the part that says whose the page is. Nothing runs past `room` —
/// the layout's "nothing spills off the screen".
#[must_use]
pub fn address_text(address: &str, room: u16) -> (String, String) {
    let (head, rest) = url::split_for_the_line(address);
    let glyph = format!("{} ", tokens::icon::KIOSK_ADDRESS);
    let host_room = room.saturating_sub(layout::width(&glyph));
    let lead = layout::elide_end(
        &format!("{glyph}{}", layout::elide_start(head, host_room)),
        room,
    );
    let left = room.saturating_sub(layout::width(&lead));
    (lead, layout::elide_middle(rest, left))
}

/// What a segment of the kiosk's modeline is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// One page back.
    Back,
    /// One page forward.
    Forward,
    /// The page again.
    Reload,
    /// How many tabs are not drawn before the first one that is.
    Before(usize),
    /// How many after the last.
    After(usize),
    /// A tab.
    Tab {
        /// Chrome's `tabId`.
        id: i64,
        /// Its place.
        index: u32,
        /// The tab in front.
        active: bool,
        /// Its title, for the hover's sentence.
        title: String,
    },
    /// A new tab, asking where.
    NewTab,
    /// A pinned extension.
    Extension {
        /// Its id.
        id: String,
        /// Its name.
        name: String,
    },
    /// Every extension — of THIS kiosk's profile: Chrome starts one host per
    /// profile, and the hover names what this kiosk's own list holds.
    Extensions {
        /// The compositor's window the segment was drawn for.
        window: u64,
    },
    /// The zoom of the tab in front, in percent.
    Zoom(u32),
    /// The address line's switch, and whether the line is up.
    AddressLine(bool),
    /// Chrome's bars, and whether they are hidden now.
    ChromeUi(Ui),
    /// The kiosk's view.
    More,
    /// The kiosk cannot be driven, and why.
    Offline(Offline),
}

/// The kiosk's modeline: the kiosks, and whether the address line is up.
pub struct Adapter<'a> {
    kiosks: &'a Kiosks,
    address_line: bool,
}

/// Which of a kiosk's tabs have a place in the pool: `(first, how many)`,
/// centred on the tab in front.
fn pooled(k: &wire::Kiosk) -> (usize, usize) {
    let n = k.tabs.len();
    let len = n.min(TABS.len());
    let at = k.tabs.iter().position(|t| t.active).unwrap_or(0);
    let first = at.saturating_sub(len / 2).min(n - len);
    (first, len)
}

/// A glyph-only segment.
fn mark(id: &'static str, glyph: char, priority: u8) -> Segment {
    Segment {
        id,
        text: glyph.to_string(),
        priority,
        min_cols: 1,
        side: Side::Left,
        shrink: Shrink::Cut,
        removable: true,
    }
}

/// A tab's words: the audible mark, the pinned one, and its title — cut to
/// `cells` — or its address when it has no title yet.
fn tab_text(t: &wire::Tab, cells: u16) -> String {
    let mut lead = String::new();
    if t.audible {
        lead.push(tokens::icon::KIOSK_AUDIBLE);
        lead.push(' ');
    }
    if t.pinned {
        lead.push(tokens::icon::PINNED);
        lead.push(' ');
    }
    let title = if t.title.trim().is_empty() {
        t.url.as_str()
    } else {
        t.title.as_str()
    };
    // THE MARKS COME OUT OF THE TITLE'S ROOM, so a pinned or a playing tab
    // takes the cells any other does and the line does not jump when a tab
    // starts making a sound.
    let room = cells.saturating_sub(layout::width(&lead));
    format!("{lead}{}", layout::elide_end(title, room))
}

impl ModelineAdapter for Adapter<'_> {
    type Target = Target;

    fn id(&self) -> &'static str {
        ADAPTER
    }

    fn claims(&self, w: &Window) -> bool {
        self.kiosks.pair_of(w.id).is_some()
    }

    fn segments(&self, w: &Window, texts: &Texts) -> Vec<(Segment, Target)> {
        let Some(pair) = self.kiosks.pair_of(w.id) else {
            return Vec::new();
        };
        let mut out: Vec<(Segment, Target)> = Vec::new();
        // RUNGS ARE REMOVAL ORDER — the highest number goes first — and they
        // interleave with the right-hand marks on purpose: the bell, the
        // faults and recording (1–5) outlive every kiosk extra; the
        // workspace numbers (17) outlive only the neighbour tabs and the
        // extensions' names. The floor is the program's name and the tab in
        // front; everything else stays reachable through `,` and the view.
        let bars = mark(
            "kiosk-chrome-ui",
            if pair.ui == Ui::Hidden {
                tokens::icon::KIOSK_SHOW_CHROME
            } else {
                tokens::icon::KIOSK_HIDE_CHROME
            },
            8,
        );
        let more = mark("kiosk-more", tokens::icon::KIOSK_MORE, 6);
        if let Some(reason) = self.kiosks.offline(w.id) {
            // OFFLINE: one mark that says why, and the two that still work —
            // the bars are the compositor's, and the view says what waits.
            out.push((
                mark("kiosk-offline", tokens::icon::KIOSK_OFFLINE, 5),
                Target::Offline(reason),
            ));
            out.push((bars, Target::ChromeUi(pair.ui)));
            out.push((more, Target::More));
            return out;
        }
        let Some((k, extensions)) = self.kiosks.state_of(w.id) else {
            return out;
        };
        out.push((
            mark("kiosk-back", tokens::icon::KIOSK_BACK, 9),
            Target::Back,
        ));
        out.push((
            mark("kiosk-forward", tokens::icon::KIOSK_FORWARD, 11),
            Target::Forward,
        ));
        out.push((
            mark("kiosk-reload", tokens::icon::KIOSK_RELOAD, 10),
            Target::Reload,
        ));
        // THE TABS, with a counter on either side. The counters are written
        // here as the widest they could be; `adapt::resolve` writes the
        // real number over the ones that survive.
        let (first, len) = pooled(k);
        let widest = counter(k.tabs.len(), texts);
        let counter_seg = |id: &'static str| Segment {
            id,
            text: widest.clone(),
            priority: 7,
            min_cols: 1,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
        };
        out.push((counter_seg(BEFORE), Target::Before(0)));
        let front = k.tabs.iter().position(|t| t.active).unwrap_or(first);
        for (slot, (i, t)) in k.tabs.iter().enumerate().skip(first).take(len).enumerate() {
            let target = Target::Tab {
                id: t.id,
                index: t.index,
                active: t.active,
                title: t.title.clone(),
            };
            let seg = if t.active {
                Segment {
                    id: TABS[slot],
                    text: tab_text(t, tokens::space::KIOSK_ACTIVECELLS),
                    priority: 5,
                    min_cols: ACTIVE_FLOOR,
                    side: Side::Left,
                    shrink: Shrink::AtTheEnd,
                    removable: false,
                }
            } else {
                // FARTHEST FIRST: the rung grows with the distance from the
                // tab in front, so what survives is always the tabs around
                // it, and the counters count what is outside them.
                let distance = u8::try_from(i.abs_diff(front)).unwrap_or(u8::MAX);
                Segment {
                    id: TABS[slot],
                    text: tab_text(t, tokens::space::KIOSK_TABCELLS),
                    priority: 20u8.saturating_add(distance),
                    min_cols: 1,
                    side: Side::Left,
                    shrink: Shrink::Cut,
                    removable: true,
                }
            };
            out.push((seg, target));
        }
        out.push((counter_seg(AFTER), Target::After(0)));
        out.push((
            mark("kiosk-new-tab", tokens::icon::KIOSK_NEW_TAB, 18),
            Target::NewTab,
        ));
        // THE PINNED EXTENSIONS, each shown, in the order Chrome pins them —
        // ours left out: it is the bridge, not something to open.
        for (slot, e) in pinned(extensions, self.kiosks.ours())
            .into_iter()
            .enumerate()
        {
            let Some(id) = EXTENSIONS.get(slot) else {
                break;
            };
            out.push((
                Segment {
                    id,
                    text: layout::elide_end(&e.name, tokens::space::KIOSK_EXTENSIONCELLS),
                    priority: 26u8.saturating_add(u8::try_from(slot + 1).unwrap_or(u8::MAX)),
                    min_cols: 1,
                    side: Side::Left,
                    shrink: Shrink::Cut,
                    removable: true,
                },
                Target::Extension {
                    id: e.id.clone(),
                    name: e.name.clone(),
                },
            ));
        }
        out.push((
            mark("kiosk-extensions", tokens::icon::KIOSK_EXTENSIONS, 19),
            Target::Extensions { window: w.id },
        ));
        // THE ZOOM, with its number only when it is not the page's own size:
        // a modeline at rest says nothing.
        let zoom = active(k).map_or(100, |t| t.zoom_percent);
        let zoom_text = if zoom == 100 {
            tokens::icon::KIOSK_ZOOM_IN.to_string()
        } else {
            format!(
                "{} {}",
                tokens::icon::KIOSK_ZOOM_IN,
                texts.t_args(
                    key::KIOSK_ZOOM_PERCENT,
                    &[("percent", Arg::Number(i64::from(zoom)))]
                )
            )
        };
        out.push((
            Segment {
                id: "kiosk-zoom",
                text: zoom_text,
                priority: 20,
                min_cols: 1,
                side: Side::Left,
                shrink: Shrink::Cut,
                removable: true,
            },
            Target::Zoom(zoom),
        ));
        out.push((
            mark(
                "kiosk-address-line",
                if self.address_line {
                    tokens::icon::KIOSK_ADDRESS
                } else {
                    tokens::icon::KIOSK_ADDRESS_OFF
                },
                12,
            ),
            Target::AddressLine(self.address_line),
        ));
        out.push((bars, Target::ChromeUi(pair.ui)));
        out.push((more, Target::More));
        out
    }

    fn strip(&self, w: &Window) -> Option<Strip> {
        let (k, _) = self.kiosks.state_of(w.id)?;
        let (first, len) = pooled(k);
        Some(Strip {
            before: BEFORE,
            after: AFTER,
            members: TABS[..len].to_vec(),
            beyond: (first, k.tabs.len() - first - len),
            more: key::KIOSK_TABS_MORE,
        })
    }

    fn colour(&self, t: &Target) -> Option<tokens::Color> {
        // NEVER THE ACCENT, which on this line says where the keyboard is.
        Some(match t {
            Target::Tab { active: true, .. } => tokens::color::TEXT_PRIMARY,
            _ => tokens::color::TEXT_SECONDARY,
        })
    }

    fn clicked(&self, t: &Target, b: Button) -> Option<Act> {
        use Button::{Left, Middle, Right};
        match (t, b) {
            (Target::Back, Left) => Some(Act::Run("kiosk-back")),
            (Target::Forward, Left) => Some(Act::Run("kiosk-forward")),
            (Target::Reload, Left) => Some(Act::Run("kiosk-reload")),
            (Target::Before(_) | Target::After(_), Left) => Some(Act::Run("kiosk-tabs")),
            // THE MIDDLE BUTTON CLOSES A TAB, as it does in every browser.
            (Target::Tab { id, .. }, Middle) => {
                Some(Act::RunWith("kiosk-tab-close", id.to_string()))
            }
            (Target::Tab { active: true, .. }, Left) => Some(Act::Run("kiosk-tabs")),
            (Target::Tab { id, .. }, Left) => Some(Act::RunWith("kiosk-tab", id.to_string())),
            (Target::Tab { id, .. }, Right) => Some(Act::RunWith("kiosk-tabs", id.to_string())),
            (Target::NewTab, Left) => Some(Act::Run("kiosk-new-tab")),
            (Target::Extension { id, .. }, Left) => {
                Some(Act::RunWith("kiosk-extension", id.clone()))
            }
            (Target::Extension { id, .. }, Right) => {
                Some(Act::RunWith("kiosk-extension-options", id.clone()))
            }
            (Target::Extensions { .. }, Left) => Some(Act::Run("kiosk-extensions")),
            (Target::Zoom(_), Left) => Some(Act::Run("kiosk-zoom")),
            (Target::Zoom(_), Right) => Some(Act::Run("kiosk-zoom-reset")),
            (Target::AddressLine(_), Left) => Some(Act::Run("kiosk-address-line")),
            (Target::ChromeUi(_), Left) => Some(Act::Run("kiosk-chrome-ui")),
            (Target::More | Target::Offline(_), Left) => Some(Act::Run("kiosk-more")),
            _ => None,
        }
    }

    fn wheeled(&self, t: &Target, lines: i64) -> Option<Act> {
        if lines == 0 {
            return None;
        }
        match t {
            // DOWN IS THE NEXT TAB — the tab strip's own convention.
            Target::Tab { .. } | Target::Before(_) | Target::After(_) => {
                Some(Act::Run(if lines > 0 {
                    "kiosk-tab-next"
                } else {
                    "kiosk-tab-previous"
                }))
            }
            // UP IS BIGGER — `Ctrl` and the wheel's own direction in Chrome.
            Target::Zoom(_) => Some(Act::Run(if lines < 0 {
                "kiosk-zoom-in"
            } else {
                "kiosk-zoom-out"
            })),
            _ => None,
        }
    }

    fn draggable(&self, t: &Target) -> bool {
        matches!(t, Target::Tab { .. })
    }

    fn dropped(&self, from: &Target, onto: &Target) -> Option<Act> {
        match (from, onto) {
            (
                Target::Tab { id, .. },
                Target::Tab {
                    id: other, index, ..
                },
            ) if id != other => Some(Act::RunWith("kiosk-tab-move", format!("{id} {index}"))),
            _ => None,
        }
    }

    fn hint(&self, t: &Target, texts: &Texts) -> Option<String> {
        let count = |n: usize| Arg::Number(i64::try_from(n).unwrap_or(i64::MAX));
        Some(match t {
            Target::Back => texts.t(key::HINT_KIOSK_BACK),
            Target::Forward => texts.t(key::HINT_KIOSK_FORWARD),
            Target::Reload => texts.t(key::HINT_KIOSK_RELOAD),
            Target::Before(n) => texts.t_args(key::HINT_KIOSK_TABS_BEFORE, &[("count", count(*n))]),
            Target::After(n) => texts.t_args(key::HINT_KIOSK_TABS_AFTER, &[("count", count(*n))]),
            Target::Tab {
                active: true,
                title,
                ..
            } => texts.t_with(key::HINT_KIOSK_TAB_CURRENT, &[("title", title)]),
            Target::Tab { title, .. } => texts.t_with(key::HINT_KIOSK_TAB, &[("title", title)]),
            Target::NewTab => texts.t(key::HINT_KIOSK_NEW_TAB),
            Target::Extension { name, .. } => texts.t_with(key::HINT_KIOSK_EXT, &[("name", name)]),
            Target::Extensions { window } => {
                let names: Vec<String> = self
                    .kiosks
                    .state_of(*window)
                    .map_or(&[][..], |(_, e)| e)
                    .iter()
                    .filter(|e| e.enabled && Some(e.id.as_str()) != self.kiosks.ours())
                    .map(|e| e.name.clone())
                    .collect();
                if names.is_empty() {
                    texts.t(key::HINT_KIOSK_EXTENSIONS_NONE)
                } else {
                    texts.t_with(key::HINT_KIOSK_EXTENSIONS, &[("names", &names.join(", "))])
                }
            }
            Target::Zoom(p) => texts.t_args(
                key::HINT_KIOSK_ZOOM,
                &[("percent", Arg::Number(i64::from(*p)))],
            ),
            Target::AddressLine(true) => texts.t(key::HINT_KIOSK_ADDRESS_LINE_ON),
            Target::AddressLine(false) => texts.t(key::HINT_KIOSK_ADDRESS_LINE_OFF),
            Target::ChromeUi(Ui::Hidden) => texts.t(key::HINT_KIOSK_CHROME_HIDDEN),
            Target::ChromeUi(Ui::Shown) => texts.t(key::HINT_KIOSK_CHROME_SHOWN),
            Target::More => texts.t(key::HINT_KIOSK_MORE),
            Target::Offline(Offline::Bridge) => texts.t(key::HINT_KIOSK_OFFLINE_BRIDGE),
            Target::Offline(Offline::Chrome) => texts.t(key::HINT_KIOSK_OFFLINE_CHROME),
            Target::Offline(Offline::Version) => texts.t(key::HINT_KIOSK_OFFLINE_VERSION),
        })
    }

    fn counted(&self, t: &Target, n: usize) -> Target {
        match t {
            Target::Before(_) => Target::Before(n),
            Target::After(_) => Target::After(n),
            other => other.clone(),
        }
    }

    fn local_bindings(&self) -> &'static [crate::window_menu::Generated] {
        crate::kiosk_view::local_bindings()
    }
}

/// The counter's text for `n` tabs.
fn counter(n: usize, texts: &Texts) -> String {
    texts.t_args(
        key::KIOSK_TABS_MORE,
        &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
    )
}

/// The pinned extensions, in Chrome's order, turned on, ours left out.
#[must_use]
pub fn pinned<'e>(
    extensions: &'e [wire::Extension],
    ours: Option<&str>,
) -> Vec<&'e wire::Extension> {
    let mut out: Vec<&wire::Extension> = extensions
        .iter()
        .filter(|e| e.enabled && e.pinned_at.is_some() && Some(e.id.as_str()) != ours)
        .collect();
    out.sort_by_key(|e| e.pinned_at);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapt::{Adapted, Line};

    const NONCE: &str = "0123456789abcdef0123456789abcdef";
    const OURS: &str = "abcdefghijklmnopabcdefghijklmnop";

    fn texts() -> Texts {
        Texts::fresh("en-GB").expect("the base language")
    }

    fn window(id: u64, app: &str, title: &str, fullscreen: bool) -> Window {
        Window {
            id,
            title: Some(title.to_owned()),
            app_id: Some(app.to_owned()),
            pid: Some(10),
            is_focused: true,
            floating: false,
            fullscreen,
            sticky: false,
            drawer: false,
            hidden: false,
            trailmark: false,
            rect: None,
        }
    }

    fn tab(id: i64, index: u32, active: bool, title: &str) -> wire::Tab {
        wire::Tab {
            id,
            index,
            active,
            pinned: false,
            title: title.to_owned(),
            url: format!("https://site{id}.example/path/{id}"),
            loading: false,
            audible: false,
            muted: false,
            zoom_percent: 100,
        }
    }

    fn state(nonce: &str, born: bool, chrome_state: &str, tabs: Vec<wire::Tab>) -> wire::State {
        wire::State {
            v: wire::VERSION,
            profile: Some("Default".into()),
            kiosks: vec![wire::Kiosk {
                window: 45,
                nonce: nonce.to_owned(),
                profile: "Default".into(),
                born,
                state: chrome_state.to_owned(),
                tabs,
            }],
            extensions: vec![
                wire::Extension {
                    id: OURS.into(),
                    name: "ronoctua".into(),
                    enabled: true,
                    pinned_at: Some(0),
                    ..wire::Extension::default()
                },
                wire::Extension {
                    id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                    name: "A password keeper".into(),
                    enabled: true,
                    pinned_at: Some(1),
                    popup: Some("popup.html".into()),
                    ..wire::Extension::default()
                },
            ],
            focused_window: Some(45),
        }
    }

    fn launched() -> Kiosks {
        let mut k = Kiosks::fresh(b"", Some(OURS.into()));
        let out = k.open(
            Some("example.com"),
            Opening {
                chrome: Some(std::path::Path::new("/chrome")),
                extension: Some(OURS),
                nonce: Some(NONCE),
            },
            Instant::now(),
        );
        assert!(matches!(out[0], Errand::Launch(_)), "{out:?}");
        k
    }

    fn paired(k: &Kiosks) -> bool {
        k.pair_of(7)
            .is_some_and(|p| p.chrome == 45 && p.nonce == NONCE)
    }

    fn a_paired_kiosk(tabs: Vec<wire::Tab>) -> Kiosks {
        let mut k = launched();
        let title = format!("ronoctua {NONCE}");
        k.compositor(
            &Event::WindowChanged(window(7, APP_ID, &title, false)),
            &[window(7, APP_ID, &title, false)],
        );
        k.heard(Heard::Connected(1));
        k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "fullscreen", tabs)),
        ));
        assert!(paired(&k));
        k
    }

    /// FAILS IF: the pair needs its halves in one order — the title can
    /// arrive before Chrome's state, or after it.
    #[test]
    fn a_pair_is_made_from_either_half_first() {
        let title = format!("New tab {NONCE}");
        // THE TITLE FIRST.
        let mut k = launched();
        let out = k.compositor(
            &Event::WindowChanged(window(7, APP_ID, &title, false)),
            &[window(7, APP_ID, &title, false)],
        );
        assert!(out.is_empty(), "one half is not a pair: {out:?}");
        k.heard(Heard::Connected(1));
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![])),
        ));
        assert!(paired(&k));
        assert_eq!(
            out[0],
            Errand::Bars {
                window: 7,
                shown: false
            },
            "the bars go first"
        );
        assert!(out.iter().any(|e| matches!(
            e,
            Errand::Send(
                1,
                Envelope {
                    order: Order::Go { .. },
                    ..
                }
            )
        )));
        assert!(out.contains(&Errand::Persist));
        // THE STATE FIRST.
        let mut k = launched();
        k.heard(Heard::Connected(1));
        k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![])),
        ));
        assert!(!paired(&k));
        let out = k.compositor(
            &Event::WindowChanged(window(7, APP_ID, &title, false)),
            &[window(7, APP_ID, &title, false)],
        );
        assert!(paired(&k));
        assert!(out.iter().any(|e| matches!(
            e,
            Errand::Bars {
                window: 7,
                shown: false
            }
        )));
    }

    /// FAILS IF: a window that is not Chrome's is paired because its title
    /// shows the nonce.
    #[test]
    fn only_chromes_window_is_paired() {
        let mut k = launched();
        let title = format!("a terminal printing {NONCE}");
        k.compositor(
            &Event::WindowChanged(window(7, "foot", &title, false)),
            &[window(7, "foot", &title, false)],
        );
        k.heard(Heard::Connected(1));
        k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![])),
        ));
        assert!(k.pair_of(7).is_none());
        // A WEB APP'S WINDOW IS CHROME'S TOO, and is still not a kiosk.
        let title = format!("app {NONCE}");
        k.compositor(
            &Event::WindowChanged(window(8, "chrome-app.example__-Default", &title, false)),
            &[],
        );
        assert!(k.pair_of(8).is_none());
    }

    /// FAILS IF: the bars stay shown after a real fullscreen (a video) ends,
    /// or are hidden again on a window that showed them on purpose.
    #[test]
    fn the_bars_come_back_hidden_after_a_video() {
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        k.compositor(
            &Event::WindowChanged(window(7, APP_ID, "A", true)),
            &[window(7, APP_ID, "A", true)],
        );
        let out = k.compositor(
            &Event::WindowChanged(window(7, APP_ID, "A", false)),
            &[window(7, APP_ID, "A", false)],
        );
        assert_eq!(
            out,
            vec![Errand::Bars {
                window: 7,
                shown: false
            }]
        );
        // SHOWN ON PURPOSE stays shown.
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        k.command("kiosk-chrome-ui", None, Some(7));
        k.compositor(
            &Event::WindowChanged(window(7, APP_ID, "A", true)),
            &[window(7, APP_ID, "A", true)],
        );
        let out = k.compositor(
            &Event::WindowChanged(window(7, APP_ID, "A", false)),
            &[window(7, APP_ID, "A", false)],
        );
        assert!(out.is_empty(), "{out:?}");
    }

    /// FAILS IF: F11 inside Chrome is fought instead of followed — or a
    /// report from before the bars were hidden is taken for one.
    #[test]
    fn f11_is_followed() {
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        // STILL "normal" RIGHT AFTER THE PAIR: that is the old picture.
        k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, false, "normal", vec![tab(1, 0, true, "A")])),
        ));
        assert!(k.hidden(7), "a report before the deed took is not an F11");
        // AND AFTER CHROME SAID FULLSCREEN, "normal" IS THE PERSON.
        k.heard(Heard::Said(
            1,
            Said::State(state(
                NONCE,
                false,
                "fullscreen",
                vec![tab(1, 0, true, "A")],
            )),
        ));
        assert!(k.hidden(7));
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, false, "normal", vec![tab(1, 0, true, "A")])),
        ));
        assert!(!k.hidden(7), "the kiosk follows F11");
        assert!(out.contains(&Errand::Persist));
        assert!(
            !out.iter().any(|e| matches!(e, Errand::Bars { .. })),
            "never fights"
        );
    }

    /// FAILS IF: a kept pair is lost across a restart, or kept for a window
    /// that is gone — or cannot be found again by the nonce in a title.
    #[test]
    fn a_kept_pair_is_measured_against_the_first_list() {
        let kept = br#"[{"nonce":"0123456789abcdef0123456789abcdef","compositor":99,"chrome":45,"ui":"hidden"},
                        {"nonce":"ffff","compositor":98,"chrome":46,"ui":"shown"}]"#;
        let mut k = Kiosks::fresh(kept, Some(OURS.into()));
        let title = format!("scene {NONCE}");
        let out = k.compositor(
            &Event::Windows(vec![]),
            &[
                window(5, APP_ID, &title, false),
                window(98, "foot", "x", false),
            ],
        );
        assert!(k.pair_of(5).is_some(), "found again by the title");
        assert!(k.pair_of(98).is_none(), "not Chrome's any more");
        assert!(out.contains(&Errand::Persist));
        let again = Kiosks::fresh(&k.kept(), None);
        assert!(again.pair_of(5).is_some());
    }

    /// FAILS IF: a launch nothing answered is kept for ever, or a page
    /// Chrome did open is left waiting.
    #[test]
    fn an_unanswered_launch_is_let_go() {
        let mut k = launched();
        k.heard(Heard::Connected(1));
        k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![])),
        ));
        let out = k.expired(Instant::now() + PATIENCE + Duration::from_secs(1));
        assert!(k.pending.is_empty());
        assert_eq!(
            orders(&out),
            [
                &Order::Go {
                    nonce: NONCE.into()
                },
                &Order::Release { window: 45 }
            ],
            "it goes, and the extension forgets it: an ordinary window"
        );
        assert!(out.contains(&Errand::Echo(key::ECHO_KIOSK_UNMATCHED)));
    }

    /// The orders among some errands, in the order they go.
    fn orders(out: &[Errand]) -> Vec<&Order> {
        out.iter()
            .filter_map(|e| match e {
                Errand::Send(_, envelope) => Some(&envelope.order),
                _ => None,
            })
            .collect()
    }

    /// FAILS IF: a kiosk page nobody here waits for is sent on as an
    /// ordinary window and the extension goes on counting it as a kiosk —
    /// its port, its host and its worker alive for a window nobody draws —
    /// or a second state of the same page lets it go twice.
    #[test]
    fn a_page_nobody_waits_for_is_forgotten_by_the_extension_too() {
        let mut k = Kiosks::fresh(b"", Some(OURS.into()));
        k.heard(Heard::Connected(1));
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![])),
        ));
        assert_eq!(
            orders(&out),
            [
                &Order::Go {
                    nonce: NONCE.into()
                },
                &Order::Release { window: 45 }
            ]
        );
        // THE SAME PAGE, said again before the extension took the orders in.
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![])),
        ));
        assert!(orders(&out).is_empty(), "let go once: {out:?}");
    }

    /// FAILS IF: the words of `kiosk-open two words` are sent to the page
    /// the kiosk was born on — which the extension closes when the kiosk
    /// goes — instead of the tab that took its place; or sent twice.
    #[test]
    fn words_are_searched_where_the_kiosk_lands() {
        let mut k = Kiosks::fresh(b"", Some(OURS.into()));
        let out = k.open(
            Some("two words"),
            Opening {
                chrome: Some(std::path::Path::new("/chrome")),
                extension: Some(OURS),
                nonce: Some(NONCE),
            },
            Instant::now(),
        );
        assert!(
            matches!(&out[0], Errand::Launch(Launching { url: None, .. })),
            "{out:?}"
        );
        k.compositor(
            &Event::WindowChanged(window(7, APP_ID, NONCE, false)),
            &[window(7, APP_ID, NONCE, false)],
        );
        k.heard(Heard::Connected(1));
        let birth = wire::Tab {
            title: NONCE.into(),
            url: format!("chrome-extension://{OURS}/kiosk.html#n={NONCE}&p=Default"),
            ..tab(3, 0, true, "")
        };
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, true, "normal", vec![birth.clone()])),
        ));
        assert!(paired(&k));
        assert_eq!(
            orders(&out),
            [&Order::Go {
                nonce: NONCE.into()
            }],
            "nothing is sent to the page that is going away"
        );
        // THE KIOSK WENT: Chrome's new-tab page in front, the birth page
        // still closing beside it.
        let landed = wire::Tab {
            title: "New Tab".into(),
            url: "chrome://newtab/".into(),
            ..tab(4, 1, true, "")
        };
        let closing = wire::Tab {
            active: false,
            ..birth
        };
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(
                NONCE,
                false,
                "fullscreen",
                vec![closing, landed.clone()],
            )),
        ));
        assert_eq!(
            orders(&out),
            [&Order::Search {
                tab: 4,
                text: "two words".into(),
                new_tab: false
            }]
        );
        let out = k.heard(Heard::Said(
            1,
            Said::State(state(NONCE, false, "fullscreen", vec![landed])),
        ));
        assert!(orders(&out).is_empty(), "searched once: {out:?}");
    }

    /// FAILS IF: a kiosk whose extension speaks another version of the wire
    /// — drawn offline, with the sentence that says so — is still sent
    /// orders by its commands, its questions or `kiosk-open`, or a second
    /// kiosk is launched beside it for every address.
    #[test]
    fn another_version_is_offline_everywhere() {
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        let mut newer = state(NONCE, false, "fullscreen", vec![tab(1, 0, true, "A")]);
        newer.v = wire::VERSION + 1;
        k.heard(Heard::Said(1, Said::State(newer)));
        assert_eq!(k.offline(7), Some(Offline::Version));
        let offline = vec![Errand::Echo(key::ECHO_KIOSK_OFFLINE)];
        assert_eq!(k.command("kiosk-back", None, Some(7)), offline);
        assert_eq!(k.command("kiosk-zoom-in", None, Some(7)), offline);
        assert_eq!(
            k.command("kiosk-address", Some("b.example"), Some(7)),
            offline
        );
        assert!(k.address_of(7).is_none(), "nothing read from it is drawn");
        let opening = Opening {
            chrome: Some(std::path::Path::new("/chrome")),
            extension: Some(OURS),
            nonce: Some("fedcba9876543210fedcba9876543210"),
        };
        assert_eq!(k.open(Some("b.example"), opening, Instant::now()), offline);
        // THE BARS ARE THE COMPOSITOR'S, and still answer.
        assert!(
            k.command("kiosk-chrome-ui", None, Some(7))
                .contains(&Errand::Bars {
                    window: 7,
                    shown: true
                })
        );
    }

    /// FAILS IF: a launch that failed before any Chrome ran is later said to
    /// have "opened as an ordinary Chrome window" — no window opened.
    #[test]
    fn a_launch_that_failed_is_not_said_to_have_opened() {
        let mut k = launched();
        k.launch_failed(NONCE);
        let out = k.expired(Instant::now() + PATIENCE + Duration::from_secs(1));
        assert!(
            !out.contains(&Errand::Echo(key::ECHO_KIOSK_UNMATCHED)),
            "{out:?}"
        );
        assert!(k.pending.is_empty());
        // ANOTHER NONCE'S FAILURE is not this launch's.
        let mut k = launched();
        k.launch_failed("ffffffffffffffffffffffffffffffff");
        let out = k.expired(Instant::now() + PATIENCE + Duration::from_secs(1));
        assert!(out.contains(&Errand::Echo(key::ECHO_KIOSK_UNMATCHED)));
    }

    /// FAILS IF: the extensions' hover on one kiosk names the extensions of
    /// another profile's Chrome — each profile has its own host.
    #[test]
    fn the_extensions_hover_names_this_kiosks_profile_only() {
        let t = texts();
        let other = "fedcba9876543210fedcba9876543210";
        let kept = format!(
            r#"[{{"nonce":"{NONCE}","compositor":7,"chrome":45,"ui":"hidden"}},
                {{"nonce":"{other}","compositor":8,"chrome":46,"ui":"hidden"}}]"#
        );
        let mut k = Kiosks::fresh(kept.as_bytes(), Some(OURS.into()));
        k.heard(Heard::Connected(1));
        k.heard(Heard::Said(
            1,
            Said::State(state(
                NONCE,
                false,
                "fullscreen",
                vec![tab(1, 0, true, "A")],
            )),
        ));
        let mut second = state(other, false, "fullscreen", vec![tab(2, 0, true, "B")]);
        second.profile = Some("Profile 1".into());
        second.kiosks[0].window = 46;
        second.extensions = vec![wire::Extension {
            id: "cccccccccccccccccccccccccccccccc".into(),
            name: "A reader of the other profile".into(),
            enabled: true,
            ..wire::Extension::default()
        }];
        k.heard(Heard::Connected(2));
        k.heard(Heard::Said(2, Said::State(second)));
        let a = k.adapter(false);
        let hover = |w: u64| {
            let segs = a.segments(&window(w, APP_ID, "t", false), &t);
            let (_, target) = segs
                .iter()
                .find(|(s, _)| s.id == "kiosk-extensions")
                .expect("the extensions' mark");
            a.hint(target, &t).expect("a sentence")
        };
        let first = hover(7);
        assert!(first.contains("A password keeper"), "{first}");
        assert!(!first.contains("other profile"), "{first}");
        let other_one = hover(8);
        assert!(other_one.contains("other profile"), "{other_one}");
        assert!(!other_one.contains("password"), "{other_one}");
    }

    /// FAILS IF: opening refuses nothing it should, or opens a second
    /// kiosk where one is there to use.
    #[test]
    fn opening_refuses_with_a_sentence() {
        let mut k = Kiosks::fresh(b"", None);
        let now = Instant::now();
        let none = Opening {
            chrome: None,
            extension: None,
            nonce: Some(NONCE),
        };
        assert_eq!(
            k.open(None, none, now),
            vec![Errand::Echo(key::ECHO_KIOSK_NO_CHROME)]
        );
        let no_bridge = Opening {
            chrome: Some(std::path::Path::new("/chrome")),
            ..none
        };
        assert_eq!(
            k.open(None, no_bridge, now),
            vec![Errand::Echo(key::ECHO_KIOSK_NO_BRIDGE)]
        );
        assert_eq!(
            k.open(Some("javascript:alert(1)"), no_bridge, now),
            vec![Errand::Echo(key::ECHO_KIOSK_ADDRESS_REFUSED)]
        );
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        assert_eq!(k.open(None, none, now), vec![Errand::Focus(7)]);
        let out = k.open(Some("b.example"), none, now);
        assert!(matches!(
            &out[0],
            Errand::Send(_, Envelope { order: Order::Create { window: 45, url: Some(u) }, .. }) if u == "https://b.example"
        ));
        assert_eq!(out.last(), Some(&Errand::Focus(7)));
    }

    fn adapter_line(k: &Kiosks, texts: &Texts) -> Line {
        let a = Adapted::Kiosk(k.adapter(false));
        let base = vec![Segment {
            id: "mode",
            text: "google-chrome".into(),
            priority: 6,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
        }];
        a.line(base, &window(7, APP_ID, "t", false), texts)
    }

    /// FAILS IF: a pinned extension is not drawn one by one on a line with
    /// room for it, ours (the bridge) is drawn, or on a narrow line a pinned
    /// name outlives the tab in front — the names are the first to go.
    #[test]
    fn the_pinned_extensions_are_shown_where_there_is_room() {
        let t = texts();
        let k = a_paired_kiosk(vec![
            tab(1, 0, false, "A"),
            tab(2, 1, true, "B"),
            tab(3, 2, false, "C"),
        ]);
        let a = Adapted::Kiosk(k.adapter(false));
        let line = adapter_line(&k, &t);
        let (wide, _) = crate::adapt::resolve(240, &line, Some(&a), &t);
        let names: Vec<&str> = wide
            .iter()
            .filter(|p| EXTENSIONS.contains(&p.id))
            .map(|p| p.text.as_str())
            .collect();
        assert_eq!(
            names.len(),
            1,
            "one pinned extension besides ours: {wide:?}"
        );
        assert!(
            "A password keeper".starts_with(names[0].trim_end_matches('…')),
            "{names:?}"
        );
        assert!(
            !wide.iter().any(|p| p.text.contains("ronoctua")),
            "our own extension is the bridge, not a thing to open: {wide:?}"
        );
        let (narrow, drawn) = crate::adapt::resolve(40, &line, Some(&a), &t);
        assert!(
            !narrow.iter().any(|p| EXTENSIONS.contains(&p.id)),
            "{narrow:?}"
        );
        assert!(
            drawn.iter().any(|(_, d)| matches!(
                d,
                crate::adapt::Target::Kiosk(Target::Tab { active: true, .. })
            )),
            "the tab in front went before a pinned name: {narrow:?}"
        );
    }

    /// FAILS IF: at any of four widths the counters and the drawn tabs do
    /// not add up to every tab, something runs past the width, or the tab in
    /// front is lost.
    #[test]
    fn the_strip_counts_what_it_does_not_draw() {
        let t = texts();
        let tabs: Vec<wire::Tab> = (0..24)
            .map(|i| {
                tab(
                    i,
                    u32::try_from(i).unwrap_or(0),
                    i == 17,
                    &format!("Tab number {i}"),
                )
            })
            .collect();
        let k = a_paired_kiosk(tabs);
        let a = Adapted::Kiosk(k.adapter(false));
        for columns in [60_u16, 80, 120, 240] {
            let line = adapter_line(&k, &t);
            let (placed, drawn) = crate::adapt::resolve(columns, &line, Some(&a), &t);
            let shown = placed.iter().filter(|p| TABS.contains(&p.id)).count();
            let counted: usize = drawn
                .iter()
                .map(|(_, d)| match d {
                    crate::adapt::Target::Kiosk(Target::Before(n) | Target::After(n)) => *n,
                    _ => 0,
                })
                .sum();
            assert_eq!(shown + counted, 24, "{columns} columns: {placed:?}");
            let edge = columns - tokens::space::MODELINE_EDGERIGHT;
            for p in &placed {
                assert!(
                    p.column + layout::width(&p.text) <= edge,
                    "{columns}: {p:?}"
                );
            }
            let front = drawn.iter().any(|(_, d)| {
                matches!(
                    d,
                    crate::adapt::Target::Kiosk(Target::Tab { active: true, .. })
                )
            });
            assert!(front, "{columns} columns lost the tab in front");
            // A COUNTER THAT COUNTS NOTHING IS NOT DRAWN.
            for (id, d) in &drawn {
                if let crate::adapt::Target::Kiosk(Target::Before(n) | Target::After(n)) = d {
                    assert!(*n > 0, "{id} drawn for zero");
                }
            }
        }
    }

    /// FAILS IF: a click does something other than the drawn segment's
    /// command, the middle button does not close, a drag does not move, or
    /// the wheel does not walk the tabs and the zoom.
    #[test]
    fn the_pointer_does_what_the_segment_says() {
        let k = a_paired_kiosk(vec![tab(1, 0, false, "A"), tab(2, 1, true, "B")]);
        let a = k.adapter(false);
        let neighbour = Target::Tab {
            id: 1,
            index: 0,
            active: false,
            title: "A".into(),
        };
        let front = Target::Tab {
            id: 2,
            index: 1,
            active: true,
            title: "B".into(),
        };
        assert_eq!(
            a.clicked(&neighbour, Button::Left),
            Some(Act::RunWith("kiosk-tab", "1".into()))
        );
        assert_eq!(
            a.clicked(&front, Button::Left),
            Some(Act::Run("kiosk-tabs"))
        );
        assert_eq!(
            a.clicked(&neighbour, Button::Middle),
            Some(Act::RunWith("kiosk-tab-close", "1".into()))
        );
        assert!(a.draggable(&front));
        assert_eq!(
            a.dropped(&front, &neighbour),
            Some(Act::RunWith("kiosk-tab-move", "2 0".into()))
        );
        assert_eq!(a.dropped(&front, &front), None, "the same tab is a click");
        assert_eq!(a.wheeled(&front, 3), Some(Act::Run("kiosk-tab-next")));
        assert_eq!(a.wheeled(&front, -1), Some(Act::Run("kiosk-tab-previous")));
        assert_eq!(
            a.wheeled(&Target::Zoom(100), -3),
            Some(Act::Run("kiosk-zoom-in"))
        );
        assert_eq!(
            a.clicked(&Target::Zoom(125), Button::Right),
            Some(Act::Run("kiosk-zoom-reset"))
        );
        assert_eq!(
            a.clicked(&Target::Back, Button::Left),
            Some(Act::Run("kiosk-back"))
        );
    }

    /// FAILS IF: a kiosk with no Chrome draws the browser's controls, or its
    /// mark cannot say why.
    #[test]
    fn offline_is_one_mark_and_a_sentence() {
        let t = texts();
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        k.heard(Heard::Gone(1));
        let w = window(7, APP_ID, "A", false);
        let segs = k.adapter(false).segments(&w, &t);
        let ids: Vec<&str> = segs.iter().map(|(s, _)| s.id).collect();
        assert_eq!(ids, ["kiosk-offline", "kiosk-chrome-ui", "kiosk-more"]);
        let said = k.adapter(false).hint(&segs[0].1, &t).expect("a sentence");
        assert_eq!(said, t.t(key::HINT_KIOSK_OFFLINE_CHROME));
        k.bridge_found(None);
        assert_eq!(k.offline(7), Some(Offline::Bridge));
        assert_eq!(
            k.command("kiosk-back", None, Some(7)),
            vec![Errand::Echo(key::ECHO_KIOSK_OFFLINE)]
        );
        // THE BARS ARE THE COMPOSITOR'S, and still answer.
        assert!(
            k.command("kiosk-chrome-ui", None, Some(7))
                .contains(&Errand::Bars {
                    window: 7,
                    shown: true
                })
        );
    }

    /// FAILS IF: the address line cuts the host, where a deceptive
    /// subdomain would hide.
    #[test]
    fn the_address_line_keeps_the_host_whole() {
        let address = "https://bank.example.evil.io/a/very/long/path/that/does/not/fit?x=1";
        let (lead, rest) = address_text(address, 40);
        assert!(lead.ends_with("https://bank.example.evil.io"), "{lead}");
        assert!(layout::width(&lead) + layout::width(&rest) <= 40);
        assert!(
            rest.ends_with("x=1"),
            "the path is elided in the middle: {rest}"
        );
        // NARROWER THAN THE HOST: nothing runs past the room, and what stays
        // is the host's END — the domain that says whose the page is.
        for room in [0_u16, 1, 2, 5, 12, 20] {
            let (lead, rest) = address_text(address, room);
            assert!(
                layout::width(&lead) + layout::width(&rest) <= room,
                "{room}: `{lead}` `{rest}`"
            );
        }
        let (lead, rest) = address_text(address, 12);
        assert!(lead.ends_with("evil.io"), "{lead}");
        assert!(rest.is_empty(), "{rest}");
    }

    /// FAILS IF: the address question is answered anywhere but where it was
    /// asked, or words are sent as an address.
    #[test]
    fn the_address_question_answers_where_it_was_asked() {
        let mut k = a_paired_kiosk(vec![tab(1, 0, true, "A")]);
        let asked = k.command("kiosk-address", None, Some(7));
        assert!(
            matches!(&asked[0], Errand::Ask { id: "kiosk-address", initial: Some(u) } if u.contains("site1"))
        );
        let out = k.command("kiosk-address", Some("two words"), None);
        assert!(matches!(
            &out[0],
            Errand::Send(
                _,
                Envelope {
                    order: Order::Search {
                        tab: 1,
                        new_tab: false,
                        ..
                    },
                    ..
                }
            )
        ));
        k.command("kiosk-new-tab", None, Some(7));
        let out = k.command("kiosk-new-tab", Some(""), Some(7));
        assert!(matches!(
            &out[0],
            Errand::Send(
                _,
                Envelope {
                    order: Order::Create { url: None, .. },
                    ..
                }
            )
        ));
    }

    /// FAILS IF: the zoom leaves Chrome's own steps.
    #[test]
    fn the_zoom_walks_chromes_steps() {
        assert_eq!(zoom_step(100, true), 110);
        assert_eq!(zoom_step(100, false), 90);
        assert_eq!(zoom_step(105, false), 100);
        assert_eq!(zoom_step(500, true), 500);
    }

    /// FAILS IF: a chord whose deed moves a copy the footer keeps — the
    /// kiosk's bars, the selection, every workspace at once — is bound
    /// straight to the compositor, which says none of them back: the copy
    /// would fall behind the screen.
    #[test]
    fn a_chord_that_moves_a_kept_copy_goes_through_us() {
        // A KIOSK THE FOOTER KEEPS, on window 7: what `deed_ran` answers for
        // is exactly what moves its copy of the bars.
        let kiosks = crate::kiosk::Kiosks::fresh(
            br#"[{"nonce":"n","compositor":7,"chrome":1,"ui":"hidden"}]"#,
            None,
        );
        let mut asked = 0;
        for a in crate::actions::chords() {
            let kept = kiosks.clone().deed_ran(a.command, 7).is_some()
                || crate::selection::Selection::follows(a.command)
                || a.command == "workspace-all";
            if !kept {
                continue;
            }
            asked += 1;
            // BOTH LOOPS: the nested key of a chord is the same command.
            for (key, said) in a.binds.iter().chain(a.nested.iter().flatten()) {
                assert!(
                    said.starts_with("exec ") && said.ends_with(&format!("command {}", a.command)),
                    "`{key}` runs `{}` straight on the compositor: `{said}`",
                    a.command
                );
            }
        }
        assert!(asked >= 8, "only {asked} chords move a kept copy");
    }
}
