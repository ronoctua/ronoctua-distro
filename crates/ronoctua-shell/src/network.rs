//! The network view — one implementation, two doors.
//!
//! # What it holds, and what it does not
//!
//! A cursor. It holds no reading: the [`ronoctua_network::Snapshot`] lives in
//! the footer (or the window), which listens to NetworkManager, and the list
//! of access points lives in a [`Wire`] beside it. Both are handed in when
//! rows are asked for — the shape `power::View` and `bluetooth::View` already
//! have, and what lets the two doors draw the same thing without a second
//! copy going stale.
//!
//! # One list, headings as rows
//!
//! Nine sections — what the machine believes about the way out, the links it
//! has, the Wi-Fi it can see, the tunnels it knows, the tailnet, Tor, the
//! ports it answers on, who else is on this network and what is going in and
//! out — one under the other,
//! each heading a row like the power view's, so the region draws one list and
//! pages it the way it pages everything else.
//!
//! # Why the read is synchronous, and why that does not repeat the launcher's
//! mistake
//!
//! The launcher scans the XDG on a thread because reading hundreds of
//! `.desktop` files costs tens of milliseconds. Here there is no disk:
//! NetworkManager keeps the access points it has already seen in memory, and
//! returning them is one trip to the bus.
//!
//! What DOES take time is the radio's scan itself, and that is **not** waited
//! for: `RequestScan` returns at once and the result appears in the daemon's
//! list a few seconds later. `s` asks for the scan and re-reads what is
//! already there; what the radio finds afterwards arrives through
//! `ronoctua_network::watch_scan`. Blocking until the radio answers would
//! freeze the whole footer for seconds — the `systemd-run --scope` trap
//! CLAUDE.md records.
//!
//! # The typing is the footer's
//!
//! A network that is secured and not saved needs a password, and a password
//! is typed in the minibuffer. The window has none, so there it says so —
//! the arrangement the Bluetooth view's pairing questions already have, and
//! for the same reason: a door that cannot ask must not pretend to.

use ronoctua_memory::net::{Counters, Socket};
use ronoctua_network::{Link, Manager, Medium, Network, Saved, Snapshot, State};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit as FilesHit};

/// The `systemctl`, by PATH and never by name: a user unit's `PATH` is
/// minimal, and a bare name fails at EXEC — far from its cause. It is the
/// same constant `brain.rs` and `keyboard.rs` carry, for the same reason.
pub const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};

/// `xdg-open`, baked at build time — what opens a tailnet's login page.
///
/// A LOGIN IS A PAGE, and this system has no browser of its own: the address
/// the daemon hands back goes to whatever the person's own desktop opens
/// links with, which is what `xdg-open` answers.
pub const OPEN: &str = match option_env!("RONOCTUA_XDG_OPEN") {
    Some(p) => p,
    None => "xdg-open",
};

/// The keys this view answers to, and the hint line is generated from it —
/// see `chrome::hint_of`. Bare letters are free here (nothing is typed), and
/// the order is the order they are taught in.
/// The width `key()` builds rows at, and why a constant is honest here.
///
/// A key acts on the row under the cursor, so it has to know the rows — and it
/// does not know how wide the screen is: the two doors measure differently and
/// the footer's region is not the window's. It builds them at a fixed width
/// instead, which is only correct while the row COUNT and the row TARGETS do
/// not depend on the width. They do not: `rows()` reaches the width in
/// `elide_end` and in a field's `at`, both of which are about drawing.
///
/// That is an invariant, not an observation, so it is checked rather than
/// trusted: `the_rows_are_the_same_things_at_every_width` walks several
/// widths and compares the targets. Without it, the day one section drops a
/// row on a narrow screen, the key acts on a different row from the one the
/// footer drew — and nothing would say so.
const DECIDING_WIDTH: u16 = 80;

pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "network-down"),
    ("<up>", "network-up"),
    ("RET", "network-connect"),
    ("s", "network-scan"),
    ("r", "network-wifi-toggle"),
    ("d", "network-disconnect"),
    ("f", "network-forget"),
    ("a", "network-airplane"),
    ("i", "network-link"),
    ("l", "network-lan"),
    ("v", "network-import"),
];

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, skipping headings.
    Up,
    /// Down one row, skipping headings.
    Down,
    /// Act on the row.
    Enter,
    /// One level back.
    Escape,
    /// Ask the radio to look again.
    Scan,
    /// Turn the radio on or off.
    Radio,
    /// Drop the link that is carrying the traffic.
    Disconnect,
    /// Forget what is under the cursor — a saved network, a stored tunnel.
    Forget,
    /// Enter or leave aeroplane mode.
    Airplane,
    /// Say the link's facts on the message line.
    Facts,
    /// Look for the other machines on this network.
    Discover,
    /// Bring a tunnel in from a file.
    Import,
}

/// What `RET` on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a heading, a fact, a note.
    Nothing,
    /// Enter aeroplane mode, or leave it — the bool is what `RET` would set.
    Airplane(bool),
    /// Turn the radio on or off.
    Radio(bool),
    /// Join this network.
    Wireless(Box<Network>),
    /// Drop the link of this medium.
    Link(Medium),
    /// Take this connection down; the string is its object path.
    Tunnel(String),
    /// Bring this stored profile up; the string is its object path.
    Saved(String),
    /// A switch of one of the two tunnels — the bool is what `RET` sets.
    Tunnels(Switch, bool),
    /// Send everything through this node, or through none when it is the one
    /// already in use.
    ExitNode(String),
    /// Bring in the file that has been looked at.
    Bring,
    /// Put the looked-at file down without importing it.
    Drop,
    /// Become an access point, or stop being one.
    Hotspot(bool),
}

/// One of the switches the two tunnel sections carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    /// Tailscale up or down.
    Tailnet,
    /// Log in, or out.
    Login,
    /// Accept the tailnet's routes.
    Routes,
    /// Refuse incoming connections.
    Shields,
    /// Tor's unit, started or stopped.
    Tor,
    /// Fresh circuits.
    Identity,
}

/// One row the view draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// Is the cursor on it?
    pub focused: bool,
    /// A heading names the section and is not in it: the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub target: Target,
    /// Drawn dim and REFUSING — a network the radio cannot reach, a tunnel
    /// with no daemon to bring it up. Reachable, so the row can say why.
    pub disabled: bool,
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Join this network. Whether it needs a password typed is the DOOR's
    /// question: only one of the two can ask.
    Connect(Box<Network>),
    /// Forget this network's stored profile.
    Forget(String),
    /// Turn the radio on or off.
    Radio(bool),
    /// Enter aeroplane mode, or leave it.
    Airplane(bool),
    /// Drop the link of this medium.
    Disconnect(Medium),
    /// Ask the radio to look again.
    Scan,
    /// Say the link's facts on the message line.
    Facts,
    /// Look for the other machines on this network.
    Discover,
    /// Take this connection down.
    Drop(String),
    /// Bring this stored profile up.
    Raise(String),
    /// Delete this stored profile.
    Delete(String),
    /// Send this to the tunnel thread.
    Tunnel(ronoctua_tunnel::Order),
    /// Ask for the path of a file to bring in.
    Ask,
    /// Bring this file in — it has been read and looked at.
    Bring(String, ronoctua_network::vpn::Kind),
    /// Become an access point, or stop being one.
    Hotspot(bool),
    /// Close the view.
    Close,
}

/// Everything the rows are drawn from, in one bundle.
///
/// ONE PARAMETER AND NOT FOUR: this view reads three sources that grow — the
/// daemon's snapshot, the access points of the last scan, and `/proc` — and a
/// signature that grew with them would have to be edited in both doors, the
/// click, and every test, each time a section is added.
pub struct Facts<'a> {
    /// What NetworkManager says.
    pub net: &'a Snapshot,
    /// The access points of the last read.
    pub seen: &'a [Network],
    /// What `/proc` says: the sockets and the counters.
    pub watch: &'a Watch,
    /// What the two tunnels say.
    pub tunnels: &'a ronoctua_tunnel::Scene,
}

/// What `/proc` says, sampled by whichever door has a clock.
///
/// It holds the PREVIOUS counters as well as the current ones, because a rate
/// is a difference over a time: one reading is a total since boot, which is a
/// different fact and is drawn beside it.
#[derive(Debug, Default)]
pub struct Watch {
    places: ronoctua_memory::Places,
    sockets: Vec<Socket>,
    counters: Vec<Counters>,
    /// Bytes per second, in and out, by interface — from the last two
    /// samples. Empty until there have been two.
    rates: std::collections::HashMap<String, (u64, u64)>,
    last: Option<(std::time::Instant, Vec<Counters>)>,
    /// The machines this one has spoken to, out of the kernel's own table.
    neighbours: Vec<ronoctua_memory::net::Neighbour>,
    /// Who answered the last sweep, and how many were asked.
    found: Option<ronoctua_lan::Found>,
    /// Is a sweep running? It takes a second on a thread, and a row that did
    /// not say so would look like a key that did nothing.
    sweeping: bool,
    /// The maker behind a hardware address — read once, when the first sweep
    /// asks for it. MEASURED at 6-9 ms for nmap's 52 000 lines, which is half
    /// a frame and happens once a session: a thread for it would be a thread
    /// to synchronise for one read.
    vendors: std::sync::Arc<ronoctua_lan::oui::Vendors>,
    /// Which sample this is, so the dear half runs on some of them.
    beat: u8,
}

/// One reading of the sockets every this many of the counters — see
/// [`Watch::sample`] for the measurement that chose the number.
const SOCKETS_EVERY: u8 = 5;

impl Watch {
    /// Reads the sockets and the counters once.
    ///
    /// TWO HALVES AT TWO RATES, and the numbers are measured: the counters
    /// are four pseudo-files (about 60 µs) and ride every second; the sockets
    /// are a walk of `/proc/<pid>/fd` (14 ms on the owner's machine) and ride
    /// one second in five. Nothing here touches a disk, and nothing here
    /// blocks — which is what lets it run inside the door's own loop without
    /// the trap `systemd-run --scope` set.
    pub fn sample(&mut self) {
        // THE VENDOR TABLE IS WANTED BY THE NEIGHBOUR ROWS, not only by a
        // sweep: the kernel's own table is drawn from the first frame, and
        // its maker column was blank until somebody pressed `l`. Reading it
        // is 6-9 ms once a session — this is the `is_empty` guard's whole
        // job.
        self.read_vendors();
        // THE SOCKETS ARE NOT CHEAP, and the rates are — so they do not ride
        // the same second.
        //
        // MEASURED on the owner's machine: one reading of the sockets is
        // 14 ms, almost all of it walking `/proc/<pid>/fd` to find which
        // process holds which inode. Every second that is 1,5% of a core for
        // as long as the view is open, and what it buys is a list of ports
        // that changes a few times an hour. The counters underneath ARE worth
        // a second, because a rate is a difference over a time.
        if self.beat.is_multiple_of(SOCKETS_EVERY) {
            self.sockets = ronoctua_memory::net::sockets(&self.places);
            self.neighbours = ronoctua_memory::net::neighbours(&self.places);
        }
        self.beat = self.beat.wrapping_add(1);
        let fresh = ronoctua_memory::net::counters(&self.places);
        let now = std::time::Instant::now();
        if let Some((then, before)) = self.last.take() {
            let seconds = now.duration_since(then).as_secs_f64();
            if seconds >= 0.5 {
                self.rates.clear();
                for c in &fresh {
                    let Some(was) = before.iter().find(|b| b.interface == c.interface) else {
                        continue;
                    };
                    // SATURATING: an interface that goes away and comes back
                    // starts its counters again, and a wrap would draw a rate
                    // of eighteen exabytes a second.
                    #[allow(
                        clippy::cast_precision_loss,
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss
                    )]
                    let per_second = |now: u64, was: u64| {
                        (now.saturating_sub(was) as f64 / seconds).round() as u64
                    };
                    self.rates.insert(
                        c.interface.clone(),
                        (
                            per_second(c.received, was.received),
                            per_second(c.sent, was.sent),
                        ),
                    );
                }
            } else {
                // TOO SOON TO DIVIDE: keep the older sample, so the next tick
                // measures over a whole second instead of over a keypress.
                self.last = Some((then, before));
                self.counters = fresh;
                return;
            }
        }
        self.last = Some((now, fresh.clone()));
        self.counters = fresh;
    }

    /// The sockets of the last sample.
    #[must_use]
    pub fn sockets(&self) -> &[Socket] {
        &self.sockets
    }

    /// The counters of the last sample.
    #[must_use]
    pub fn counters(&self) -> &[Counters] {
        &self.counters
    }

    /// Bytes in and out per second, once there have been two samples.
    #[must_use]
    pub fn rate(&self, interface: &str) -> Option<(u64, u64)> {
        self.rates.get(interface).copied()
    }

    /// Forgets the last sample, so the next one is a beginning.
    ///
    /// A RATE IS A DIFFERENCE OVER A TIME, and this is a field of the door
    /// that outlives the view: opening the network an hour after closing it
    /// would otherwise divide an hour of bytes by an hour and draw that as
    /// what is happening now.
    pub fn restart(&mut self) {
        self.last = None;
        self.rates.clear();
        self.beat = 0;
    }

    /// The kernel's neighbour table, as of the last sample.
    #[must_use]
    pub fn neighbours(&self) -> &[ronoctua_memory::net::Neighbour] {
        &self.neighbours
    }

    /// A sweep is running.
    pub const fn sweeping(&mut self, yes: bool) {
        self.sweeping = yes;
    }

    /// What a finished sweep found.
    pub fn found(&mut self, found: ronoctua_lan::Found) {
        self.found = Some(found);
        self.sweeping = false;
    }

    /// The vendor table, loaded once — cheap to clone, so a thread can keep
    /// it while the view redraws.
    #[must_use]
    pub fn vendors(&self) -> std::sync::Arc<ronoctua_lan::oui::Vendors> {
        std::sync::Arc::clone(&self.vendors)
    }

    /// Reads the vendor table, once. On a machine without `nmap` the table
    /// is empty and the column stays blank, which is what it looks like for
    /// an unregistered prefix anyway.
    pub fn read_vendors(&mut self) {
        if !self.vendors.is_empty() {
            return;
        }
        if let Some(table) = ronoctua_lan::oui::table() {
            self.vendors = std::sync::Arc::new(ronoctua_lan::oui::Vendors::read(&table));
        }
    }
}

/// The view's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    cursor: usize,
    /// A file that has been read and not yet brought in.
    ///
    /// IT LIVES IN THE VIEW, and the FILE stays on disk: what is held here
    /// is what the tunnel would DO, never its key — see
    /// `ronoctua_network::vpn`, which is where the reading refuses to carry
    /// a secret out.
    bringing: Option<(String, ronoctua_network::vpn::Preview)>,
    /// The password of the hotspot this machine is serving, while it is.
    ///
    /// ON SCREEN ON PURPOSE: it is typed on somebody else's phone from
    /// across a room. It is not stored anywhere by the view — the profile
    /// NetworkManager wrote is where it lives.
    sharing: Option<String>,
}

/// Where a row's cells begin and how wide they are, in the ROW's cells —
/// both doors add `chrome::FIELDS_LEAD`; see `notifications::extent_of`.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
    let end = columns.saturating_sub(crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT);
    (left, end.saturating_sub(left))
}

/// A link's speed, in the person's own language.
///
/// THE DECIMAL SEPARATOR BELONGS TO THE LANGUAGE, which is why the crate
/// hands back a number and this hands back a sentence: `1,9 Gb/s` on a
/// Portuguese screen and `1.9 Gb/s` on an English one, out of ONE reading.
/// The unit is not translated — `Gb/s` is the same everywhere, and the six
/// `.ftl` keys carry only the shape.
#[must_use]
pub fn rate_text(rate: ronoctua_network::Rate, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::{Arg, key};
    match rate {
        ronoctua_network::Rate::Kilobits(n) => texts.t_args(
            key::NETWORK_RATE_KILOBITS,
            &[("rate", Arg::Number(i64::from(n)))],
        ),
        ronoctua_network::Rate::Megabits(n) => texts.t_args(
            key::NETWORK_RATE_MEGABITS,
            &[("rate", Arg::Number(i64::from(n)))],
        ),
        ronoctua_network::Rate::Gigabits(n) => {
            texts.t_args(key::NETWORK_RATE_GIGABITS, &[("rate", Arg::Fraction(n))])
        }
    }
}

/// The same for a link with no speed to report.
#[must_use]
pub fn rate_of(link: &Link, texts: &ronoctua_i18n::Texts) -> String {
    link.rate().map_or_else(
        || texts.t(ronoctua_i18n::key::NETWORK_NO_RATE),
        |r| rate_text(r, texts),
    )
}

/// The band, in the person's own language — `2,4 GHz` or `2.4 GHz`.
#[must_use]
pub fn band_text(band: ronoctua_network::Band, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::{Arg, key};
    band.gigahertz().map_or_else(
        || texts.t(key::NETWORK_BAND_UNKNOWN),
        |ghz| texts.t_args(key::NETWORK_BAND, &[("ghz", Arg::Fraction(ghz))]),
    )
}

/// What to call the hotspot: this machine's own name.
///
/// FROM THE KERNEL AT RUNTIME, never a literal: the machine's name is written
/// in one place (`nix/modules/base.nix`, invariant 16) and read here from
/// `/proc/sys/kernel/hostname` — which is what every other process reads too.
/// A machine with no name gets the fallback, and the row shows what it will
/// be called before anybody presses anything.
#[must_use]
pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|n| n.trim().to_owned())
        .ok()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "hotspot".to_owned())
}

/// The signal's glyph, in four steps.
///
/// Turning a number into a step is [`bars`]; here only the token is chosen.
/// Two places deciding the same range would give a bar that does not match
/// the other part of the screen.
#[must_use]
pub const fn bars_glyph(signal: u8) -> char {
    match bars(signal) {
        0 => tokens::icon::WIFI_NONE,
        1 => tokens::icon::WIFI_WEAK,
        2 => tokens::icon::WIFI_FAIR,
        3 => tokens::icon::WIFI_GOOD,
        _ => tokens::icon::WIFI_STRONG,
    }
}

/// The link the traffic is going over, when there is one.
///
/// The cable beats the radio when both are connected — it is where the
/// traffic goes, and it is its rate that answers "why is this slow?".
#[must_use]
pub fn carrying(net: &Snapshot) -> Option<&Link> {
    net.links
        .iter()
        .filter(|e| e.state == State::Connected)
        .min_by_key(|e| u8::from(e.medium == Medium::Radio))
}

/// Does this machine have a radio at all?
///
/// Telling "there is no radio" from "the radio is off" matters: the first
/// cannot be fixed by a key. The Wi-Fi section draws neither the switch nor
/// the hotspot without one — and `r` still exists, because a key that is
/// taught by the hint line has to answer everywhere the line is drawn; with
/// no radio the daemon simply refuses and the message line says so.
#[must_use]
pub fn has_radio(net: &Snapshot) -> bool {
    net.links.iter().any(|e| e.medium == Medium::Radio)
}

impl View {
    /// A fresh view, on the first row that is not a heading.
    #[must_use]
    pub fn fresh() -> Self {
        // ROW 1, NOT 0: row 0 is the first heading, and the section always
        // has a row under it — the way out, or the sentence saying there is
        // no daemon. On a heading nothing is focused and the first `RET`
        // does nothing, which the power view's first screen proofs showed.
        Self {
            cursor: 1,
            bringing: None,
            sharing: None,
        }
    }

    /// A fresh view with the cursor on the first row of a kind — how a click
    /// on the aeroplane mark lands on the aeroplane row.
    #[must_use]
    pub fn fresh_at(facts: &Facts<'_>, want: &Target, texts: &ronoctua_i18n::Texts) -> Self {
        let mut v = Self::fresh();
        let rows = v.rows(facts, 80, texts);
        if let Some(i) = rows.iter().position(|r| &r.target == want) {
            v.cursor = i;
        }
        v
    }

    /// The row the cursor is on — what the region scrolls to.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The network under the cursor, when the row is one.
    ///
    /// Asked by the door that has to type a password: the answer comes back
    /// out of the minibuffer as a command argument, and by then the only
    /// thing that says which network it was for is the cursor — which has not
    /// moved, because the minibuffer had the keyboard.
    #[must_use]
    pub fn focused_network(
        &self,
        facts: &Facts<'_>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Network> {
        match self.rows(facts, DECIDING_WIDTH, texts).get(self.cursor)? {
            Row {
                target: Target::Wireless(n),
                ..
            } => Some((**n).clone()),
            _ => None,
        }
    }

    /// The hint line, generated from the table — the same pieces in both
    /// doors. Without it there is nothing on screen that says `f` forgets.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, texts)
    }

    /// A file has been read: show what it would do.
    ///
    /// The cursor goes to the row that brings it in, because that is the
    /// question on screen — and the row above it is the sentence that says
    /// what answering yes would mean.
    pub fn looked_at(&mut self, path: String, preview: ronoctua_network::vpn::Preview) {
        self.bringing = Some((path, preview));
    }

    /// The hotspot came up with this password: show it while it is up.
    pub fn sharing(&mut self, password: Option<String>) {
        self.sharing = password;
    }

    /// Is a file waiting to be brought in?
    ///
    /// Read by the tests, and by whoever asks the view where it is without
    /// reaching into it — the shape `bluetooth::View::asking` has.
    #[must_use]
    #[cfg_attr(not(test), expect(dead_code, reason = "the doors do not ask yet"))]
    pub const fn bringing(&self) -> bool {
        self.bringing.is_some()
    }

    /// Puts the cursor on row `n`, when it is a row one can stand on.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) {
        if rows.get(n).is_some_and(|r| !r.heading) {
            self.cursor = n;
        }
    }

    /// Keeps the cursor on a row that EXISTS.
    ///
    /// Rows come and go while the view is open — a neighbour's router goes
    /// off, a cable is plugged in, a scan finds six networks — and a cursor
    /// past the end focuses nothing: the arrows' first step is out of range
    /// too, so neither of them moves and `RET` has no target. The view
    /// swallows the keyboard, so going dead on it leaves only `Esc`.
    pub fn settle(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        if self.cursor >= rows.len() {
            self.cursor = rows.len() - 1;
        }
        while self.cursor > 0 && rows[self.cursor].heading {
            self.cursor -= 1;
        }
        if rows[self.cursor].heading
            && let Some(i) = rows.iter().position(|r| !r.heading)
        {
            self.cursor = i;
        }
    }

    /// Acts on a key.
    #[must_use]
    pub fn key(&mut self, press: Press, facts: &Facts<'_>, texts: &ronoctua_i18n::Texts) -> Errand {
        let net = facts.net;
        let rows = self.rows(facts, DECIDING_WIDTH, texts);
        self.settle(&rows);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        let disabled = rows.get(self.cursor).is_some_and(|r| r.disabled);
        match press {
            Press::Up | Press::Down => {
                let step: isize = if press == Press::Down { 1 } else { -1 };
                let mut i = self.cursor;
                loop {
                    let next = i.checked_add_signed(step);
                    match next {
                        Some(n) if n < rows.len() => i = n,
                        _ => break,
                    }
                    if !rows[i].heading {
                        self.cursor = i;
                        break;
                    }
                }
                Errand::Nothing
            }
            // ESCAPE WALKS BACK ONE LEVEL, and a file waiting to be brought
            // in is a level: closing the whole view on it would throw away
            // the reading without saying so.
            Press::Escape if self.bringing.is_some() => {
                self.bringing = None;
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            Press::Scan => Errand::Scan,
            Press::Facts => Errand::Facts,
            Press::Discover => Errand::Discover,
            // WITHOUT THE DAEMON THERE IS NOWHERE TO BRING IT IN TO, and the
            // preview rows live inside the section that reads the snapshot:
            // asking for a path would have read a file into a screen that
            // cannot draw it, and the next `Esc` would have been swallowed
            // clearing state nobody could see.
            Press::Import if !net.daemon => Errand::Nothing,
            Press::Import => Errand::Ask,
            Press::Radio => Errand::Radio(!net.radio),
            Press::Airplane => Errand::Airplane(!net.airplane()),
            Press::Disconnect => match carrying(net) {
                Some(l) => Errand::Disconnect(l.medium),
                None => Errand::Nothing,
            },
            // `f` FORGETS WHAT IS UNDER THE CURSOR, and the two things that
            // can be forgotten are a network's stored profile and a tunnel's.
            // One key, because from the person's side it is one idea.
            Press::Forget => match &target {
                Target::Wireless(n) if n.saved => Errand::Forget(n.ssid.clone()),
                Target::Saved(at) => Errand::Delete(at.clone()),
                _ => Errand::Nothing,
            },
            // A DISABLED ROW REFUSES — it is drawn dim and says why; sending
            // the order would only bring the daemon's refusal back as an echo.
            Press::Enter if disabled => Errand::Nothing,
            Press::Enter => match target {
                Target::Nothing => Errand::Nothing,
                Target::Airplane(on) => Errand::Airplane(on),
                Target::Radio(on) => Errand::Radio(on),
                Target::Wireless(n) => Errand::Connect(n),
                Target::Link(medium) => Errand::Disconnect(medium),
                Target::Tunnel(at) => Errand::Drop(at),
                Target::Saved(at) => Errand::Raise(at),
                // THE FILE THAT HAS BEEN LOOKED AT, and only now brought
                // in: the reading is what the person answered `RET` to.
                Target::Hotspot(on) => Errand::Hotspot(on),
                Target::Bring => match self.bringing.take() {
                    Some((path, preview)) => Errand::Bring(path, preview.kind),
                    None => Errand::Nothing,
                },
                Target::Drop => {
                    self.bringing = None;
                    Errand::Nothing
                }
                Target::Tunnels(switch, on) => Errand::Tunnel(match switch {
                    Switch::Tailnet => ronoctua_tunnel::Order::Tailscale(on),
                    Switch::Login if on => ronoctua_tunnel::Order::Login,
                    Switch::Login => ronoctua_tunnel::Order::Logout,
                    Switch::Routes => ronoctua_tunnel::Order::AcceptRoutes(on),
                    Switch::Shields => ronoctua_tunnel::Order::ShieldsUp(on),
                    Switch::Tor => ronoctua_tunnel::Order::Tor(on),
                    Switch::Identity => ronoctua_tunnel::Order::NewIdentity,
                }),
                // `RET` ON THE NODE IN USE TAKES IT OFF, which is the only
                // thing anybody wants a second press to do.
                Target::ExitNode(id) => {
                    let same = facts
                        .tunnels
                        .tailscale
                        .as_ref()
                        .and_then(|t| t.exit_node.clone())
                        == Some(id.clone());
                    Errand::Tunnel(ronoctua_tunnel::Order::ExitNode((!same).then_some(id)))
                }
            },
        }
    }

    /// The rows to draw, for the width given.
    #[must_use]
    pub fn rows(&self, facts: &Facts<'_>, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        use ronoctua_i18n::key;
        let (net, seen) = (facts.net, facts.seen);
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };

        // 1. WHAT THE MACHINE BELIEVES ABOUT THE WAY OUT.
        //
        // WITHOUT THE DAEMON, THE FOUR SECTIONS THAT READ IT SAY SO ONCE and
        // are not drawn — but the four that do NOT read it still are: the
        // ports, the monitor, the neighbours and the two tunnels come out of
        // `/proc` and out of their own sockets, and a machine whose
        // NetworkManager is down is exactly when somebody wants to look at
        // them. Hiding them behind its absence would be one refusal standing
        // in for four different questions.
        b.heading(key::NETWORK_HEADING_OVERVIEW);
        if !net.daemon {
            b.note(key::ECHO_NETWORK_UNAVAILABLE);
        }
        let way = texts.t(&format!("network-way-{}", net.way.id()));
        b.fact_coloured(
            key::NETWORK_WAY,
            way,
            (!net.way.out()).then_some(tokens::color::STATE_WARNING),
        );
        let flying = net.airplane();
        if net.daemon {
            b.pair(
                texts.t(key::NETWORK_AIRPLANE),
                b.word(flying),
                Target::Airplane(!flying),
                false,
            );
        }

        // 2. THE LINKS.
        if net.daemon {
            b.heading(key::NETWORK_HEADING_LINKS);
            if net.links.is_empty() {
                b.note(key::NETWORK_NO_LINK);
            }
            for link in &net.links {
                Self::link_rows(&mut b, link);
            }

            // 3. THE WI-FI.
            b.heading(key::NETWORK_HEADING_WIFI);
            if has_radio(net) {
                // THE GLYPH ONLY WHEN IT IS OFF: nothing at rest, and a
                // radio that is on is what the networks under it say.
                let label = if net.radio {
                    texts.t(key::NETWORK_RADIO)
                } else {
                    format!("{} {}", tokens::icon::WIFI_OFF, texts.t(key::NETWORK_RADIO))
                };
                b.pair(label, b.word(net.radio), Target::Radio(!net.radio), false);
            }
            // A RADIO SWITCHED OFF LISTS NO NETWORK AT ALL. Showing what the last
            // scan saw would offer a connection that cannot happen, and the key
            // would fail looking like a fault.
            let listed: &[Network] = if net.radio { seen } else { &[] };
            if listed.is_empty() {
                // THREE DIFFERENT EMPTIES, and not one generic "no networks":
                // no radio, radio off and nothing in range have different fixes —
                // the first has none, the second is one key, the third is a walk
                // to another room. One sentence for all three would send the
                // person looking in the wrong place.
                b.note(if !has_radio(net) {
                    key::NETWORK_NO_ADAPTER
                } else if net.radio {
                    key::NETWORK_EMPTY
                } else {
                    key::NETWORK_RADIO_OFF
                });
            }
            // WHAT THE RADIO IS DOING, for the row of the network it is doing it
            // with: `active_one` says "this is the one in use", which is true
            // while the link is still authenticating and asking for an address.
            let radio_state = net
                .links
                .iter()
                .find(|l| l.medium == Medium::Radio)
                .map(|l| l.state);
            for one in listed {
                Self::network_row(&mut b, one, radio_state);
            }
            // AND THE OTHER DIRECTION: this machine BEING the network.
            //
            // Only where there is a radio to do it with, and the row says what it
            // costs — a laptop with one card cannot be on a network and be one at
            // the same time, so what it shares is the cable.
            if has_radio(net) {
                let serving = net.links.iter().any(|l| l.serving);
                b.pair(
                    texts.t(if serving {
                        key::NETWORK_HOTSPOT_STOP
                    } else {
                        key::NETWORK_HOTSPOT_START
                    }),
                    String::new(),
                    Target::Hotspot(!serving),
                    // WITH THE RADIO OFF THERE IS NOTHING TO SERVE WITH, and the
                    // row says so by being dim rather than by failing.
                    !net.radio,
                );
                if serving {
                    b.note(key::NETWORK_HOTSPOT_ON);
                } else {
                    b.note(key::NETWORK_HOTSPOT_COST);
                }
                // THE PASSWORD STAYS ON SCREEN while the hotspot is up, because
                // it is typed on somebody else's phone from across a room. It is
                // shown ON PURPOSE — a hotspot whose password nobody can read is
                // a hotspot nobody can join.
                if let Some(word) = &self.sharing {
                    b.fact(key::NETWORK_HOTSPOT_PASSWORD, word.clone());
                }
            }

            // 4. THE TUNNELS — what is up, what is stored, and what is waiting
            // at the door.
            b.heading(key::NETWORK_HEADING_TUNNELS);
            let up: Vec<&ronoctua_network::Active> =
                net.active.iter().filter(|a| a.tunnel()).collect();
            let stored: Vec<&Saved> = net
                .saved
                .iter()
                .filter(|s| s.tunnel())
                .filter(|s| !up.iter().any(|a| a.id == s.id))
                .collect();
            if up.is_empty() && stored.is_empty() {
                b.note(key::NETWORK_NO_TUNNEL);
            }
            for a in up {
                b.pair(
                    format!("{} {}", tokens::icon::VPN, a.id),
                    texts.t(key::NETWORK_TUNNEL_DROP),
                    Target::Tunnel(a.at.clone()),
                    false,
                );
            }
            if let Some((path, preview)) = &self.bringing {
                Self::bringing_rows(&mut b, path, preview);
            }
            for s in stored {
                b.pair(
                    format!("{} {}", tokens::icon::VPN, s.id),
                    texts.t(key::NETWORK_TUNNEL_RAISE),
                    Target::Saved(s.at.clone()),
                    // THE AEROPLANE REFUSES IT, and the row says so by being dim
                    // rather than by sending an order NetworkManager will refuse
                    // with a sentence nobody wrote.
                    flying,
                );
            }
        }

        // 5. THE TAILNET, when there is a daemon to ask.
        //
        // A MACHINE WITH NEITHER DAEMON DRAWS NEITHER HEADING: an empty
        // section under a title is a thing to look at that does not exist,
        // and both of these are absent on an ordinary install.
        if let Some(t) = facts.tunnels.tailscale.as_ref() {
            b.heading(key::NETWORK_HEADING_TAILSCALE);
            Self::tailnet_rows(&mut b, t);
        }

        // 6. TOR, likewise.
        if let Some(t) = facts.tunnels.tor.as_ref() {
            b.heading(key::NETWORK_HEADING_TOR);
            Self::tor_rows(&mut b, t);
        }

        // 7. THE PORTS — what this machine is answering on.
        b.heading(key::NETWORK_HEADING_PORTS);
        Self::port_rows(&mut b, facts.watch);

        // 8. THE LAN — who else is on this network.
        b.heading(key::NETWORK_HEADING_LAN);
        Self::lan_rows(&mut b, net, facts.watch);

        // 9. THE MONITOR — what is going in and out, now and since boot.
        b.heading(key::NETWORK_HEADING_MONITOR);
        Self::monitor_rows(&mut b, facts.watch);
        b.out
    }

    /// A file that has been read, and what it would do once it is in.
    ///
    /// THE READING IS THE POINT. `AllowedIPs = 0.0.0.0/0` sends every packet
    /// this machine writes through somebody else's computer, and a `DNS` line
    /// moves every name it looks up. Both are one row here, in words, before
    /// anybody presses anything.
    fn bringing_rows(b: &mut Builder<'_>, path: &str, preview: &ronoctua_network::vpn::Preview) {
        use ronoctua_i18n::key;
        b.fact(key::NETWORK_IMPORT_FILE, path.to_owned());
        b.fact(
            key::NETWORK_IMPORT_KIND,
            b.texts.t(&format!("network-import-{}", preview.kind.id())),
        );
        if let Some(address) = &preview.address {
            b.fact(key::NETWORK_FACT_ADDRESS, address.clone());
        }
        for peer in &preview.peers {
            if let Some(endpoint) = &peer.endpoint {
                b.fact(key::NETWORK_IMPORT_ENDPOINT, endpoint.clone());
            }
        }
        if preview.takes_everything() {
            // THE ONE SENTENCE WORTH READING TWICE, and it is coloured like a
            // warning because it is one: everything this machine sends would
            // go through somebody else's computer.
            b.sentence_coloured(
                b.texts.t(key::NETWORK_IMPORT_EVERYTHING),
                tokens::color::STATE_WARNING,
            );
        }
        if !preview.dns.is_empty() {
            b.fact_coloured(
                key::NETWORK_IMPORT_DNS,
                preview.dns.join(&format!(" {} ", tokens::mark::SEPARATOR)),
                Some(tokens::color::STATE_WARNING),
            );
        }
        if preview.needs_secret {
            b.note(key::NETWORK_IMPORT_ASKS);
        }
        b.pair(
            b.texts.t(key::NETWORK_IMPORT_BRING),
            preview.name.clone(),
            Target::Bring,
            false,
        );
        b.pair(
            b.texts.t(key::NETWORK_IMPORT_LEAVE),
            String::new(),
            Target::Drop,
            false,
        );
    }

    /// The tailnet: what it is doing, the switches, and the other machines.
    fn tailnet_rows(b: &mut Builder<'_>, t: &ronoctua_tunnel::tailscale::Scene) {
        use ronoctua_i18n::key;
        // THE STATE IN THE DAEMON'S OWN WORD, translated where we have one
        // and shown raw where we have not: a release that invents a state is
        // a word on screen, not a blank.
        // THE DAEMON'S OWN WORD, translated where there is a translation and
        // shown RAW where there is not — a release that invents a state is a
        // word on screen rather than a blank. The five that exist today have
        // keys; `Starting`, `NeedsMachineAuth` and `InUseOtherUser` are the
        // ones a person actually meets, and they have them too.
        let state = b.texts.t(&format!(
            "network-tailscale-{}",
            t.state.to_ascii_lowercase()
        ));
        let state = if state.starts_with('[') {
            t.state.clone()
        } else {
            state
        };
        // WANTED, AND NOT RUNNING. They are different facts: a machine that
        // wants to be on the tailnet and cannot (no route, the coordination
        // server unreachable) reads `running == false` — and a switch built
        // on that offered to turn ON what was already asked for, with no way
        // left to turn it off.
        b.pair(
            b.texts.t(key::NETWORK_TAILSCALE_STATE),
            state,
            Target::Tunnels(Switch::Tailnet, !t.wanted),
            false,
        );
        if let Some(url) = &t.auth_url {
            // THE LOGIN'S ADDRESS IS A FACT AND A DOOR: `RET` opens it, and
            // the row shows it because a page that opened somewhere else is
            // a page nobody can find again.
            b.pair(
                b.texts.t(key::NETWORK_TAILSCALE_LOGIN),
                url.clone(),
                Target::Tunnels(Switch::Login, true),
                false,
            );
        } else {
            b.pair(
                b.texts.t(if t.running {
                    key::NETWORK_TAILSCALE_LOGOUT
                } else {
                    key::NETWORK_TAILSCALE_LOGIN
                }),
                String::new(),
                Target::Tunnels(Switch::Login, !t.running),
                false,
            );
        }
        if !t.name.is_empty() {
            b.fact(key::NETWORK_FACT_NAME, t.name.clone());
        }
        if !t.tailnet.is_empty() {
            b.fact(key::NETWORK_FACT_TAILNET, t.tailnet.clone());
        }
        for address in &t.addresses {
            b.fact(key::NETWORK_FACT_ADDRESS, address.clone());
        }
        b.fact(key::NETWORK_FACT_MAGIC_DNS, b.word(t.magic_dns));
        b.pair(
            b.texts.t(key::NETWORK_TAILSCALE_ROUTES),
            b.word(t.accept_routes),
            Target::Tunnels(Switch::Routes, !t.accept_routes),
            false,
        );
        b.pair(
            b.texts.t(key::NETWORK_TAILSCALE_SHIELDS),
            b.word(t.shields_up),
            Target::Tunnels(Switch::Shields, !t.shields_up),
            false,
        );
        for peer in &t.peers {
            let mut said = Vec::new();
            said.push(b.texts.t(if peer.online {
                key::NETWORK_TAILSCALE_ONLINE
            } else {
                key::NETWORK_TAILSCALE_OFFLINE
            }));
            if !peer.os.is_empty() {
                said.push(peer.os.clone());
            }
            if let Some(address) = peer.addresses.first() {
                said.push(address.clone());
            }
            // ONLY A NODE THAT OFFERS TO BE ONE can be chosen as the exit,
            // and the rest are facts: a `RET` that sent an id the daemon
            // refuses would be a key that fails for a reason nothing says.
            let target = if peer.offers_exit {
                Target::ExitNode(peer.id.clone())
            } else {
                Target::Nothing
            };
            if peer.exit_node {
                said.push(b.texts.t(key::NETWORK_TAILSCALE_EXIT));
            }
            b.pair(
                format!("{} {}", tokens::icon::TAILSCALE, peer.name),
                said.join(&format!(" {} ", tokens::mark::SEPARATOR)),
                target,
                false,
            );
        }
    }

    /// Tor: how far it got, what it has, and the two things to do to it.
    fn tor_rows(b: &mut Builder<'_>, t: &ronoctua_tunnel::tor::Scene) {
        use ronoctua_i18n::key;
        // THE SWITCH IS ABOUT THE UNIT, AND THE WORDS ARE ABOUT THE CIRCUIT.
        //
        // They are different facts, and reading the circuit as the switch was
        // a defect: this section only exists when the control socket answered
        // — that is, when the daemon is RUNNING — so `!established` sent
        // `systemctl start` to a unit that is already up, a no-op with no
        // message, and a Tor that was bootstrapping or had lost its circuits
        // could not be stopped at all.
        b.pair(
            b.texts.t(key::NETWORK_TOR_STATE),
            b.texts.t(if t.established {
                key::NETWORK_TOR_READY
            } else {
                key::NETWORK_TOR_NOT_READY
            }),
            Target::Tunnels(Switch::Tor, false),
            false,
        );
        if !t.established {
            // THE BOOTSTRAP IS THE ANSWER TO "why not yet", and it is only
            // worth a row while the answer is not "it works".
            let said = b.texts.t_args(
                key::NETWORK_TOR_BOOTSTRAP,
                &[(
                    "percent",
                    ronoctua_i18n::Arg::Number(i64::from(t.bootstrap)),
                )],
            );
            // THE DAEMON'S OWN SUMMARY, VERBATIM AND IN ENGLISH. It is the
            // only string on this screen that is not ours, and translating it
            // is not possible: Tor writes forty of them and they change with
            // its releases. A sentence that says "starting, 45%" beside it is
            // what carries the meaning; the summary is the detail, in the
            // words the person will find in Tor's own log.
            b.pair(said, t.summary.clone(), Target::Nothing, false);
        }
        if t.circuits > 0 {
            let said = b.texts.t_args(
                key::NETWORK_TOR_CIRCUITS,
                &[(
                    "count",
                    ronoctua_i18n::Arg::Number(i64::try_from(t.circuits).unwrap_or(i64::MAX)),
                )],
            );
            b.sentence(said);
        }
        if !t.version.is_empty() {
            b.fact(key::NETWORK_FACT_VERSION, t.version.clone());
        }
        b.fact(key::NETWORK_FACT_RECEIVED, readable_bytes(t.received));
        b.fact(key::NETWORK_FACT_SENT, readable_bytes(t.sent));
        b.fact(
            key::NETWORK_FACT_SOCKS,
            ronoctua_tunnel::tor::SOCKS.to_owned(),
        );
        b.pair(
            b.texts.t(key::NETWORK_TOR_IDENTITY),
            String::new(),
            Target::Tunnels(Switch::Identity, true),
            !t.established,
        );
    }

    /// How many established sockets of ours are listed before the rest
    /// become a sentence.
    ///
    /// A browser holds hundreds, and a wall of them would bury the listening
    /// rows above — which are the ones a person opens this section to read.
    const ESTABLISHED: usize = 10;

    /// What is listening, and what of ours is talking.
    fn port_rows(b: &mut Builder<'_>, watch: &Watch) {
        use ronoctua_i18n::key;
        let listening: Vec<&Socket> = watch.sockets().iter().filter(|s| s.listening).collect();
        if listening.is_empty() {
            b.note(key::NETWORK_NO_PORTS);
        }
        for s in listening {
            b.pair(
                format!("{} {}", s.protocol.label(), s.named()),
                Self::owner_of(b, s),
                Target::Nothing,
                false,
            );
        }
        let talking: Vec<&Socket> = watch
            .sockets()
            .iter()
            .filter(|s| !s.listening && s.owner.is_some())
            .collect();
        let hidden = talking.len().saturating_sub(Self::ESTABLISHED);
        for s in talking.iter().take(Self::ESTABLISHED) {
            let peer = s
                .peer
                .map_or_else(String::new, |(address, port)| match address {
                    std::net::IpAddr::V4(a) => format!("{a}:{port}"),
                    std::net::IpAddr::V6(a) => format!("[{a}]:{port}"),
                });
            b.pair(
                format!(
                    "{} {} {peer}",
                    s.protocol.label(),
                    tokens::mark::ARROW_RIGHT
                ),
                Self::owner_of(b, s),
                Target::Nothing,
                false,
            );
        }
        if hidden > 0 {
            let said = b.texts.t_args(
                key::NETWORK_PORTS_MORE,
                &[(
                    "count",
                    ronoctua_i18n::Arg::Number(i64::try_from(hidden).unwrap_or(i64::MAX)),
                )],
            );
            b.sentence(said);
        }
    }

    /// Who holds a socket — or the sentence saying the kernel would not say.
    ///
    /// A BLANK WOULD READ AS "NOTHING": `sshd` on port 22 belongs to root,
    /// `/proc/<pid>/fd` is not ours to read, and a row with an empty second
    /// column looks like a port with nobody behind it.
    fn owner_of(b: &Builder<'_>, s: &Socket) -> String {
        s.owner.as_ref().map_or_else(
            || b.texts.t(ronoctua_i18n::key::NETWORK_PORT_NOT_OURS),
            |o| o.program.clone(),
        )
    }

    /// Who else is on this network.
    ///
    /// TWO SOURCES, AND THEY ANSWER DIFFERENT QUESTIONS. The kernel's own
    /// neighbour table holds every machine this one has actually spoken to,
    /// whether or not it answers a ping — it is free and it is always there.
    /// The sweep (`l`) asks the whole network and lists what answers, which
    /// finds the machines nobody here has talked to yet. A machine that is
    /// there and drops pings is in the first list and not the second, and
    /// that is why both are drawn.
    fn lan_rows(b: &mut Builder<'_>, net: &Snapshot, watch: &Watch) {
        use ronoctua_i18n::key;
        let vendors = watch.vendors();
        let mine: Vec<String> = net
            .links
            .iter()
            .filter_map(|l| l.address.clone())
            .filter_map(|a| a.split('/').next().map(str::to_owned))
            .collect();
        let gateways: Vec<String> = net.links.iter().filter_map(|l| l.gateway.clone()).collect();

        let fresh: Vec<&ronoctua_memory::net::Neighbour> =
            watch.neighbours().iter().filter(|n| n.fresh).collect();
        if fresh.is_empty() {
            b.note(key::NETWORK_LAN_NOBODY);
        }
        for one in fresh {
            let address = one.address.to_string();
            let mut said = Vec::new();
            if gateways.contains(&address) {
                said.push(b.texts.t(key::NETWORK_LAN_GATEWAY));
            }
            if let Some(maker) = vendors.of_address(&one.hardware) {
                said.push(maker.to_owned());
            }
            said.push(one.interface.clone());
            b.pair(
                address,
                said.join(&format!(" {} ", tokens::mark::SEPARATOR)),
                Target::Nothing,
                false,
            );
        }
        // AND THIS MACHINE, which the neighbour table never holds: nothing
        // ARPs for itself, and a list of the network without the machine
        // reading it is a list with a hole in it.
        for address in &mine {
            b.pair(
                address.clone(),
                b.texts.t(key::NETWORK_LAN_THIS_MACHINE),
                Target::Nothing,
                false,
            );
        }
        if watch.sweeping {
            b.note(key::NETWORK_LAN_LOOKING);
            return;
        }
        let Some(found) = watch.found.as_ref() else {
            return;
        };
        // WHAT THE SWEEP ADDED, and only that: repeating the neighbours
        // would be the same machines twice under one heading.
        let extra: Vec<String> = found
            .replies
            .iter()
            .map(|r| r.address.to_string())
            .filter(|a| !mine.contains(a))
            .filter(|a| {
                !watch
                    .neighbours()
                    .iter()
                    .any(|n| &n.address.to_string() == a)
            })
            .collect();
        // AND IT SAYS WHICH RANGE IT ASKED. A sweep is capped at 254
        // addresses, so on anything wider than a `/24` it answers about a
        // part of the network — and a count with no range beside it reads as
        // "this is everybody".
        // A REFUSED SOCKET IS NOT AN EMPTY NETWORK, and the two must not
        // read the same: `ping_group_range` can be narrowed, and then no
        // sweep can ever answer.
        if let Some(why) = &found.refused {
            b.sentence_coloured(
                b.texts.t_with(key::NETWORK_LAN_REFUSED, &[("error", why)]),
                tokens::color::STATE_WARNING,
            );
            return;
        }
        let said = b.texts.t_args(
            key::NETWORK_LAN_SWEPT,
            &[
                (
                    "answered",
                    ronoctua_i18n::Arg::Number(
                        i64::try_from(found.replies.len()).unwrap_or(i64::MAX),
                    ),
                ),
                (
                    "asked",
                    ronoctua_i18n::Arg::Number(i64::try_from(found.asked).unwrap_or(i64::MAX)),
                ),
                ("range", ronoctua_i18n::Arg::Text(&found.range)),
            ],
        );
        b.sentence(said);
        for address in extra {
            b.pair(
                address,
                b.texts.t(key::NETWORK_LAN_ANSWERED),
                Target::Nothing,
                false,
            );
        }
    }

    /// What is going in and out of each interface.
    fn monitor_rows(b: &mut Builder<'_>, watch: &Watch) {
        use ronoctua_i18n::key;
        if watch.counters().is_empty() {
            b.note(key::NETWORK_NO_LINK);
            return;
        }
        for c in watch.counters() {
            // THE RATE ON THE INTERFACE'S OWN ROW, and the totals under it:
            // one is what is happening, the other is what has happened, and
            // reading them as one number is how a quiet link looks busy.
            let rate = watch
                .rate(&c.interface)
                .map_or_else(String::new, |(down, up)| {
                    format!(
                        "{} {} {}",
                        b.per_second(down),
                        tokens::mark::SEPARATOR,
                        b.per_second(up)
                    )
                });
            b.pair(c.interface.clone(), rate, Target::Nothing, false);
            b.fact(key::NETWORK_FACT_RECEIVED, readable_bytes(c.received));
            b.fact(key::NETWORK_FACT_SENT, readable_bytes(c.sent));
        }
    }

    /// One link, and the facts under it.
    fn link_rows(b: &mut Builder<'_>, link: &Link) {
        use ronoctua_i18n::key;
        let glyph = if link.medium == Medium::Wired {
            tokens::icon::ETHERNET
        } else {
            bars_glyph(link.signal)
        };
        let connected = link.state == State::Connected;
        let state = b.texts.t(match link.state {
            State::Connected => key::NETWORK_STATE_CONNECTED,
            State::Connecting => key::NETWORK_STATE_CONNECTING,
            State::Disconnected => key::NETWORK_STATE_DISCONNECTED,
        });
        b.pair(
            format!("{glyph} {}", link.interface),
            state,
            if connected {
                Target::Link(link.medium)
            } else {
                Target::Nothing
            },
            false,
        );
        if !connected {
            return;
        }
        if let Some(name) = &link.network {
            b.fact(key::NETWORK_FACT_NETWORK, name.clone());
        }
        if link.medium == Medium::Radio {
            b.fact(key::NETWORK_FACT_BAND, band_text(link.band, b.texts));
            b.fact(key::NETWORK_FACT_SIGNAL, format!("{}%", link.signal));
        }
        if let Some(rate) = link.rate() {
            b.fact(key::NETWORK_FACT_RATE, rate_text(rate, b.texts));
        }
        if let Some(address) = &link.address {
            b.fact(key::NETWORK_FACT_ADDRESS, address.clone());
        }
        if let Some(gateway) = &link.gateway {
            b.fact(key::NETWORK_FACT_GATEWAY, gateway.clone());
        }
        if !link.dns.is_empty() {
            let joined = link.dns.join(&format!(" {} ", tokens::mark::SEPARATOR));
            b.fact(key::NETWORK_FACT_DNS, joined);
        }
    }

    /// One network in range.
    fn network_row(b: &mut Builder<'_>, one: &Network, radio: Option<State>) {
        use ronoctua_i18n::key;
        let mut said = vec![band_text(one.band, b.texts)];
        if one.active_one {
            // THE ONE IN USE SAYS SO, and says "connecting" only while the
            // link really is on its way. It used to say "connecting…" for as
            // long as it was connected, which the owner's screen showed on a
            // network that had been up for hours.
            said.push(b.texts.t(if radio == Some(State::Connected) {
                key::NETWORK_STATE_CONNECTED
            } else {
                key::NETWORK_CONNECTING
            }));
        } else if one.saved {
            said.push(b.texts.t(key::NETWORK_SAVED));
        }
        // THE PADLOCK ONLY ON THE SECURED ONE, and never an open padlock on
        // the others: the absence is what says it, and a second glyph would
        // be a second thing to learn.
        //
        // It used to be a WORD on the open ones and nothing on the secured —
        // and the word had to give way to "saved" and to "connected", so on
        // exactly the networks one has used, the one fact worth seeing was
        // invisible. The token was declared for this and drawn by nobody.
        let lock = if one.secured {
            format!("{} ", tokens::icon::NETWORK_SECURED)
        } else {
            String::new()
        };
        b.pair(
            format!("{} {lock}{}", bars_glyph(one.signal), one.ssid),
            said.join(&format!(" {} ", tokens::mark::SEPARATOR)),
            Target::Wireless(Box::new(one.clone())),
            false,
        );
    }
}

/// The conversation with NetworkManager, and the access points it has seen.
///
/// SEPARATE FROM THE VIEW on purpose: the view is what is on screen and is
/// drawn by two doors; this is the bus, and each door has one of its own.
/// NetworkManager keeps no per-client state, so two of these are not two
/// truths — they are two telephones to the same daemon.
pub struct Wire {
    manager: Option<Manager>,
    seen: Vec<Network>,
}

impl Wire {
    /// Opens the conversation and reads what the daemon already knows.
    #[must_use]
    pub fn fresh() -> Self {
        let mut w = Self {
            manager: Manager::fresh().ok(),
            seen: Vec::new(),
        };
        w.reread();
        w
    }

    /// The access points, as of the last read.
    #[must_use]
    pub fn seen(&self) -> &[Network] {
        &self.seen
    }

    /// The whole of what the daemon knows, now.
    #[must_use]
    pub fn look(&self) -> Snapshot {
        self.manager
            .as_ref()
            .map_or_else(Snapshot::default, Manager::look)
    }

    /// Re-reads the access point list without asking for a new scan.
    pub fn reread(&mut self) {
        if let Some(g) = &self.manager {
            self.seen = g.networks().unwrap_or_default();
        }
    }

    /// Asks the radio to scan and re-reads what there ALREADY is.
    pub fn scan(&mut self) {
        if let Some(g) = &self.manager {
            let _ = g.scan();
        }
        self.reread();
    }

    /// Connects to a network.
    ///
    /// `remember` decides the `psk-flags`: with it, the password is sealed
    /// into the TPM and the profile does not store it; without it, nothing is
    /// persisted anywhere.
    ///
    /// # Errors
    ///
    /// The network vanished, the password will not do, or the bus refused.
    pub fn connect(
        &mut self,
        ssid: &str,
        password: Option<&str>,
        remember: bool,
    ) -> anyhow::Result<()> {
        let g = self.wire()?;
        g.connect(ssid, password, remember)?;
        self.reread();
        Ok(())
    }

    /// Forgets a network's saved profile.
    ///
    /// # Errors
    ///
    /// There was no profile, or the bus refused.
    pub fn forget(&mut self, ssid: &str) -> anyhow::Result<()> {
        self.wire()?.forget(ssid)?;
        self.reread();
        Ok(())
    }

    /// Disconnects the given medium.
    ///
    /// # Errors
    ///
    /// There is no device of that medium, or the bus refused.
    pub fn disconnect(&mut self, medium: Medium) -> anyhow::Result<()> {
        self.wire()?.disconnect(medium)?;
        self.reread();
        Ok(())
    }

    /// Turns the radio on or off.
    ///
    /// # Errors
    ///
    /// The bus refused — and then the screen must NOT say it turned on.
    pub fn radio(&mut self, on: bool) -> anyhow::Result<()> {
        self.wire()?.set_radio(on)?;
        self.reread();
        Ok(())
    }

    /// Enters aeroplane mode, or leaves it.
    ///
    /// # Errors
    ///
    /// The bus refused.
    pub fn airplane(&mut self, on: bool) -> anyhow::Result<()> {
        self.wire()?.set_networking(!on)?;
        self.reread();
        Ok(())
    }

    /// Takes one connection down.
    ///
    /// # Errors
    ///
    /// The path is not one of the daemon's, or it refused.
    pub fn drop_it(&mut self, at: &str) -> anyhow::Result<()> {
        self.wire()?.deactivate(at)
    }

    /// Brings one stored profile up.
    ///
    /// # Errors
    ///
    /// The path is not one of the daemon's, or it refused.
    pub fn raise(&mut self, at: &str) -> anyhow::Result<()> {
        self.wire()?.activate(at)
    }

    /// Turns the radio into an access point.
    ///
    /// # Errors
    ///
    /// There is no radio, or NetworkManager refused.
    pub fn hotspot(&mut self, ssid: &str, password: &str) -> anyhow::Result<()> {
        self.wire()?.hotspot(ssid, password)?;
        self.reread();
        Ok(())
    }

    /// Brings a file in as a profile.
    ///
    /// # Errors
    ///
    /// There is no `nmcli`, the file is not what it says, or NetworkManager
    /// refused — its own words come back.
    pub fn import(&mut self, path: &str, kind: ronoctua_network::vpn::Kind) -> anyhow::Result<()> {
        self.wire()?.import(path, kind)?;
        self.reread();
        Ok(())
    }

    /// Deletes one stored profile.
    ///
    /// # Errors
    ///
    /// The path is not one of the daemon's, or it refused.
    pub fn delete(&mut self, at: &str) -> anyhow::Result<()> {
        self.wire()?.delete(at)
    }

    fn wire(&self) -> anyhow::Result<&Manager> {
        self.manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NetworkManager is not on the bus"))
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self::fresh()
    }
}

/// The row builder — the power view's arithmetic, one column of labels and
/// one of values.
struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
        }
    }

    /// `1,2 GB/s`, in the person's language.
    fn per_second(&self, bytes: u64) -> String {
        self.texts.t_with(
            ronoctua_i18n::key::NETWORK_PER_SECOND,
            &[("bytes", &readable_bytes(bytes))],
        )
    }

    /// `on` or `off`, in the person's language.
    fn word(&self, on: bool) -> String {
        self.texts.t(if on {
            ronoctua_i18n::key::NETWORK_ON
        } else {
            ronoctua_i18n::key::NETWORK_OFF
        })
    }

    fn heading(&mut self, key: &str) {
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                self.texts.t(key),
                tokens::color::TEXT_GROUP,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A sentence that stands for a whole section — "no networks in range".
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    /// The same, for a sentence that carries a number.
    fn sentence(&mut self, text: String) {
        let focused = self.cursor == self.out.len();
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&text, self.room),
                if focused {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_DIM
                },
            )],
            focused,
            heading: false,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A sentence in a colour of its own — a warning that is not a row.
    fn sentence_coloured(&mut self, text: String, colour: tokens::Color) {
        let focused = self.cursor == self.out.len();
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&text, self.room),
                if focused {
                    tokens::color::STATE_ACCENT
                } else {
                    colour
                },
            )],
            focused,
            heading: false,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A label and a value, with nothing to do on `RET`.
    fn fact(&mut self, key: &str, value: String) {
        self.fact_coloured(key, value, None);
    }

    fn fact_coloured(&mut self, key: &str, value: String, colour: Option<tokens::Color>) {
        let label = self.texts.t(key);
        self.coloured(label, value, Target::Nothing, false, colour);
    }

    /// A row `RET` acts on. `disabled` draws it reachable and refusing — the
    /// format buffer's XFS row, the same shape.
    fn pair(&mut self, label: String, value: String, target: Target, disabled: bool) {
        self.coloured(label, value, target, disabled, None);
    }

    /// The one that draws them all — the power view's arithmetic, and the
    /// same reason: a short value sits at the right end and the label keeps
    /// the rest of the row; a long one takes the fact's third.
    fn coloured(
        &mut self,
        label: String,
        value: String,
        target: Target,
        disabled: bool,
        colour: Option<tokens::Color>,
    ) {
        let focused = self.cursor == self.out.len();
        let (label_colour, value_colour) = match (focused, disabled) {
            (true, _) => (tokens::color::STATE_ACCENT, tokens::color::STATE_ACCENT),
            (false, true) => (tokens::color::TEXT_DIM, tokens::color::TEXT_DIM),
            (false, false) => (tokens::color::TEXT_SECONDARY, tokens::color::TEXT_PRIMARY),
        };
        let value_colour = colour.filter(|_| !focused).unwrap_or(value_colour);
        let width = layout::width(&value);
        let value_at = if value.is_empty() {
            self.left + self.room
        } else if width <= self.room / 3 {
            self.left + self.room.saturating_sub(width)
        } else {
            self.left + self.room / 3
        };
        let mut fields = vec![Self::piece(
            self.left,
            layout::elide_end(&label, value_at.saturating_sub(self.left + 1)),
            label_colour,
        )];
        if !value.is_empty() {
            fields.push(Self::piece(
                value_at,
                layout::elide_end(&value, (self.left + self.room).saturating_sub(value_at)),
                value_colour,
            ));
        }
        self.out.push(Row {
            fields,
            focused,
            heading: false,
            target,
            disabled,
        });
    }
}

/// `44220000000` -> `44,2 GB`.
///
/// One decimal place, like the link's rate: the total transferred grows all
/// the time, and the second place would be a digit flickering without
/// informing anything.
#[must_use]
pub fn readable_bytes(bytes: u64) -> String {
    const K: u64 = 1_000;
    const M: u64 = K * 1_000;
    const G: u64 = M * 1_000;
    match bytes {
        0..=999 => format!("{bytes} B"),
        1_000..=999_999 => format!("{} kB", bytes / K),
        1_000_000..=999_999_999 => format!("{} MB", bytes / M),
        _ => {
            let tenth = (bytes + G / 20) / (G / 10);
            format!("{},{} GB", tenth / 10, tenth % 10)
        }
    }
}

/// How many bars the signal lights, from 0 to 4.
///
/// Four bands and not a percentage: the exact number wobbles every second and
/// nobody decides anything with "67%". What one decides with signal is "can I
/// connect from here?", and four steps answer that.
#[must_use]
pub const fn bars(signal: u8) -> u8 {
    match signal {
        0..=20 => 0,
        21..=45 => 1,
        46..=65 => 2,
        66..=80 => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_network::{Active, Band, Way};

    /// A quiet `/proc`: no socket, no counter. The two sections that read it
    /// have their parsers proved in `ronoctua_memory::net`, against fixtures
    /// copied from a running machine — a `/proc` here would make every one of
    /// these tests depend on what the machine running them happens to have
    /// open.
    #[test]
    fn every_key_the_line_teaches_does_something() {
        // THE RULE, and it had no test in this view: a key the hint line
        // teaches has to be a key the view answers to. `notifications.rs` and
        // `files.rs` each hold themselves to it; the three views this work
        // added proved only that every id had a text and that the line
        // mentioned it — never that pressing it did anything.
        //
        // "Does something" is an errand, or a cursor that moved, or a level
        // that changed: a walking key answers `Nothing` and MOVES.
        let t = texts();
        // A MACHINE WITH SOMETHING ON IT. With an empty snapshot there is no
        // network to connect to, no link to look at and nothing to forget —
        // every row key would read as dead, which is a fixture saying nothing
        // rather than a view doing nothing.
        let mut snap = machine();
        // AND A SAVED PROFILE, so `f` has something to forget: without one
        // the key is honest in doing nothing, and the test would be reading
        // a silent fixture as a dead key.
        snap.saved = vec![ronoctua_network::Saved {
            at: "/s/1".to_owned(),
            id: "a tunnel".to_owned(),
            kind: "wireguard".to_owned(),
        }];
        let facts = facts(&snap, &[]);
        for (notation, id) in OF_THE_VIEW {
            let Some(press) = crate::network_words::press_of(id) else {
                panic!("`{id}` is taught and resolves to no press");
            };
            // ON EVERY ROW, and it has to work on at least ONE. A row key is
            // honest about the row it is on — `RET` on a fact does nothing,
            // and should — so the property is "somewhere on this screen", not
            // "wherever the cursor happened to land".
            let v = View::fresh();
            let rows = v.rows(&facts, 80, &t);
            let mut worked = false;
            for row in 0..rows.len() {
                let mut here = v.clone();
                here.clicked(row, &rows);
                let was = here.cursor();
                let did = here.key(press, &facts, &t);
                worked |= did != Errand::Nothing || here.cursor() != was;
            }
            assert!(
                worked,
                "`{notation}` ({id}) does nothing on any row of this view"
            );
        }
    }

    #[test]
    fn the_rows_are_the_same_things_at_every_width() {
        // `View::key` builds its rows at `DECIDING_WIDTH` because it does not
        // know how wide the screen is — and that is only honest while the row
        // COUNT and the row TARGETS do not depend on the width. The day one
        // section drops a row on a narrow screen, a key would act on a
        // different row from the one the footer drew, and nothing on screen
        // would say so.
        //
        // The FIELDS are allowed to differ — that is what eliding is.
        let t = texts();
        let snap = Snapshot::default();
        let facts = facts(&snap, &[]);
        let want = |columns: u16| -> Vec<(bool, Target)> {
            View::fresh()
                .rows(&facts, columns, &t)
                .into_iter()
                .map(|r| (r.heading, r.target))
                .collect()
        };
        let at_eighty = want(DECIDING_WIDTH);
        for columns in [40_u16, 60, 100, 160, 240] {
            assert_eq!(want(columns), at_eighty, "the rows differ at {columns}");
        }
    }

    fn facts<'a>(net: &'a Snapshot, seen: &'a [Network]) -> Facts<'a> {
        // A LEAKED DEFAULT, so the borrow lives as long as the test wants it
        // and no caller has to hold one: it is one empty struct per test
        // binary, and this is the shape a fixture takes when it has no state.
        static QUIET: std::sync::OnceLock<Watch> = std::sync::OnceLock::new();
        static NEITHER: std::sync::OnceLock<ronoctua_tunnel::Scene> = std::sync::OnceLock::new();
        Facts {
            net,
            seen,
            watch: QUIET.get_or_init(Watch::default),
            tunnels: NEITHER.get_or_init(ronoctua_tunnel::Scene::default),
        }
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn network(ssid: &str, signal: u8) -> Network {
        Network {
            ssid: ssid.into(),
            signal,
            band: Band::Ghz5,
            secured: true,
            wpa3_only: false,
            saved: false,
            active_one: false,
            max_rate_kbps: 0,
        }
    }

    fn link_of(medium: Medium, state: State, rate: u32) -> Link {
        Link {
            medium,
            interface: if medium == Medium::Wired {
                "enp1s0"
            } else {
                "wlan0"
            }
            .into(),
            network: None,
            state,
            rate_kbps: rate,
            signal: 70,
            band: Band::Other,
            address: Some("10.0.0.2/24".to_owned()),
            gateway: Some("10.0.0.1".to_owned()),
            dns: vec!["10.0.0.1".to_owned()],
            serving: false,
        }
    }

    /// A machine with a cable, a radio and the internet answering.
    fn machine() -> Snapshot {
        Snapshot {
            daemon: true,
            networking: true,
            radio: true,
            way: Way::Full,
            links: vec![
                link_of(Medium::Wired, State::Connected, 1_000_000),
                link_of(Medium::Radio, State::Connected, 400_000),
            ],
            active: Vec::new(),
            saved: Vec::new(),
        }
    }

    fn tunnel(id: &str, at: &str) -> Active {
        Active {
            at: at.to_owned(),
            id: id.to_owned(),
            kind: "wireguard".to_owned(),
            vpn: false,
            activated: true,
        }
    }

    #[test]
    fn a_heading_is_never_alone() {
        // A TITLE WITH NOTHING UNDER IT is a thing to look at that does not
        // exist — see the power view's test of the same name.
        let t = texts();
        let full = machine();
        for snap in [Snapshot::default(), full] {
            let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
            for (i, row) in rows.iter().enumerate() {
                if !row.heading {
                    continue;
                }
                assert!(
                    rows.get(i + 1).is_some_and(|r| !r.heading),
                    "the heading `{}` has nothing under it",
                    row.fields.first().map(|f| f.text.as_str()).unwrap_or("")
                );
            }
        }
    }

    #[test]
    fn nothing_leaks_at_any_width() {
        let t = texts();
        let mut snap = machine();
        snap.active = vec![tunnel("a tunnel", "/a/1")];
        snap.saved = vec![Saved {
            at: "/s/1".to_owned(),
            id: "an office".to_owned(),
            kind: "vpn".to_owned(),
        }];
        let seen = vec![network("a network with a very long name indeed", 90)];
        for columns in [40_u16, 60, 80, 120, 200] {
            for row in View::fresh().rows(&facts(&snap, &seen), columns, &t) {
                for f in &row.fields {
                    assert!(
                        f.at + layout::width(&f.text) + crate::chrome::FIELDS_LEAD <= columns,
                        "{:?} leaks at {columns}",
                        f.text
                    );
                }
            }
        }
    }

    #[test]
    fn the_rows_land_at_the_inset_and_the_hint_teaches_every_key() {
        let t = texts();
        let (at, _) = extent(80);
        assert_eq!(at + crate::chrome::FIELDS_LEAD, crate::chrome::INSET);
        let ids = OF_THE_VIEW
            .iter()
            .map(|(_, id)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(View::fresh().hint(&t).len(), 3 * ids.len() - 1);
        for (_, id) in OF_THE_VIEW {
            for key in [(*id).to_owned(), format!("cmd-{id}")] {
                // THE BRACKETS ARE THE ANSWER, not the key itself: a key
                // nobody wrote resolves to `[key]` (`Texts::t_marked`), so
                // comparing against the key could never fail.
                assert!(!t.t(&key).starts_with('['), "`{key}` has no text");
            }
        }
    }

    #[test]
    fn with_no_daemon_the_four_sections_that_read_it_say_so_once() {
        // AND THE FOUR THAT DO NOT READ IT ARE STILL THERE: the ports, the
        // monitor and the neighbours come out of `/proc`, and a machine whose
        // NetworkManager is down is exactly when somebody wants them. One
        // refusal standing in for four different questions would hide them.
        let t = texts();
        let rows = View::fresh().rows(&facts(&Snapshot::default(), &[]), 80, &t);
        let headings: Vec<String> = rows
            .iter()
            .filter(|r| r.heading)
            .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
            .collect();
        assert!(headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_OVERVIEW)));
        assert!(headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_PORTS)));
        assert!(headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_LAN)));
        assert!(headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_MONITOR)));
        // AND THE ONES THAT WOULD BE EMPTY ARE NOT DRAWN AT ALL.
        assert!(!headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_LINKS)));
        assert!(!headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_WIFI)));
        assert!(!headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_TUNNELS)));
        // AND THE SENTENCE IS SAID ONCE, under the first heading.
        let said = t.t(ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE);
        assert_eq!(
            rows.iter()
                .filter(|r| r.fields.iter().any(|f| f.text == said))
                .count(),
            1
        );
        // AND NO SWITCH IS OFFERED for a daemon that is not there.
        assert!(!rows.iter().any(|r| matches!(r.target, Target::Airplane(_))));
        assert!(!rows.iter().any(|r| matches!(r.target, Target::Radio(_))));
    }

    #[test]
    fn the_aeroplane_row_offers_the_other_state() {
        let t = texts();
        let mut snap = machine();
        let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
        assert!(
            rows.iter().any(|r| r.target == Target::Airplane(true)),
            "on the ground, the row offers to take off"
        );
        snap.networking = false;
        let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
        assert!(
            rows.iter().any(|r| r.target == Target::Airplane(false)),
            "in the air, the row offers to land"
        );
    }

    #[test]
    fn a_secured_network_wears_the_padlock_and_an_open_one_wears_nothing() {
        // IT USED TO BE A WORD on the open ones and nothing on the secured —
        // and the word gave way to "saved" and to "connected", so on exactly
        // the networks one has used, whether they ask for a key was
        // invisible. The token was declared for this and drawn by nobody.
        let t = texts();
        let snap = machine();
        let mut open_one = network("a café", 60);
        open_one.secured = false;
        open_one.saved = true;
        let seen = vec![network("a house", 90), open_one];
        let rows = View::fresh().rows(&facts(&snap, &seen), 80, &t);
        let label_of = |ssid: &str| {
            rows.iter()
                .find(|r| matches!(&r.target, Target::Wireless(n) if n.ssid == ssid))
                .and_then(|r| r.fields.first().map(|f| f.text.clone()))
                .unwrap()
        };
        assert!(
            label_of("a house").contains(tokens::icon::NETWORK_SECURED),
            "the secured one has no padlock"
        );
        assert!(
            !label_of("a café").contains(tokens::icon::NETWORK_SECURED),
            "an open network was given a padlock"
        );
    }

    #[test]
    fn the_network_in_use_says_connected_and_not_connecting() {
        // IT SAID "connecting…" FOR AS LONG AS IT WAS CONNECTED, which the
        // owner's screen showed on a network that had been up for hours:
        // `active_one` means "this is the one in use", and the word for that
        // is not "connecting".
        let t = texts();
        let mut snap = machine();
        let mut using = network("a house", 90);
        using.active_one = true;
        let seen = vec![using];
        let words = |snap: &Snapshot| {
            View::fresh()
                .rows(&facts(snap, &seen), 80, &t)
                .into_iter()
                .find(|r| matches!(&r.target, Target::Wireless(n) if n.active_one))
                .and_then(|r| r.fields.last().map(|f| f.text.clone()))
                .unwrap()
        };
        assert!(words(&snap).contains(&t.t(ronoctua_i18n::key::NETWORK_STATE_CONNECTED)));
        // AND "connecting…" WHILE IT REALLY IS ON ITS WAY.
        for l in &mut snap.links {
            l.state = State::Connecting;
        }
        assert!(words(&snap).contains(&t.t(ronoctua_i18n::key::NETWORK_CONNECTING)));
    }

    #[test]
    fn a_radio_switched_off_lists_no_network_from_the_old_scan() {
        // Showing what the last scan saw, with the radio off, would offer a
        // connection that cannot happen — the key would fail looking like a
        // radio fault.
        let t = texts();
        let mut snap = machine();
        let seen = vec![network("a house", 90)];
        let wireless = |snap: &Snapshot| {
            View::fresh()
                .rows(&facts(snap, &seen), 80, &t)
                .into_iter()
                .filter(|r| matches!(r.target, Target::Wireless(_)))
                .count()
        };
        assert_eq!(wireless(&snap), 1);
        snap.radio = false;
        assert_eq!(wireless(&snap), 0, "it listed a network with the radio off");
    }

    #[test]
    fn the_wire_beats_the_radio_on_the_link_that_carries() {
        // With both connected the traffic goes over the cable. Showing the
        // radio's rate would answer "why is this slow?" wrongly.
        let snap = machine();
        assert_eq!(carrying(&snap).map(|l| l.medium), Some(Medium::Wired));
        assert!(has_radio(&snap));
    }

    #[test]
    fn a_stored_tunnel_that_is_up_is_not_listed_twice() {
        let t = texts();
        let mut snap = machine();
        snap.active = vec![tunnel("an office", "/a/1")];
        snap.saved = vec![
            Saved {
                at: "/s/1".to_owned(),
                id: "an office".to_owned(),
                kind: "vpn".to_owned(),
            },
            Saved {
                at: "/s/2".to_owned(),
                id: "a house".to_owned(),
                kind: "wireguard".to_owned(),
            },
            // AND A NETWORK IS NOT A TUNNEL: every profile the daemon has is
            // in the snapshot, and this section is about two of the words.
            Saved {
                at: "/s/3".to_owned(),
                id: "a café".to_owned(),
                kind: "802-11-wireless".to_owned(),
            },
        ];
        let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
        let tunnels: Vec<&Target> = rows
            .iter()
            .map(|r| &r.target)
            .filter(|t| matches!(t, Target::Tunnel(_) | Target::Saved(_)))
            .collect();
        assert_eq!(
            tunnels,
            vec![
                &Target::Tunnel("/a/1".to_owned()),
                &Target::Saved("/s/2".to_owned())
            ]
        );
    }

    #[test]
    fn the_aeroplane_greys_out_what_it_refuses() {
        // A tunnel cannot come up with every device down. The row says so by
        // being dim, rather than by sending an order the daemon will refuse
        // with a sentence nobody wrote.
        let t = texts();
        let mut snap = machine();
        snap.networking = false;
        snap.saved = vec![Saved {
            at: "/s/1".to_owned(),
            id: "an office".to_owned(),
            kind: "vpn".to_owned(),
        }];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &[]), 80, &t);
        let i = rows
            .iter()
            .position(|r| r.target == Target::Saved("/s/1".to_owned()))
            .unwrap();
        assert!(rows[i].disabled);
        v.clicked(i, &rows);
        assert_eq!(v.key(Press::Enter, &facts(&snap, &[]), &t), Errand::Nothing);
    }

    #[test]
    fn forget_only_acts_on_what_can_be_forgotten() {
        let t = texts();
        let mut snap = machine();
        snap.saved = vec![Saved {
            at: "/s/1".to_owned(),
            id: "an office".to_owned(),
            kind: "vpn".to_owned(),
        }];
        let mut saved_one = network("a house", 80);
        saved_one.saved = true;
        let seen = vec![saved_one, network("a stranger", 40)];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &seen), 80, &t);

        // ON A FACT, NOTHING. `f` over the way out has nothing to forget.
        assert_eq!(
            v.key(Press::Forget, &facts(&snap, &seen), &t),
            Errand::Nothing
        );

        let saved_row = rows
            .iter()
            .position(|r| matches!(&r.target, Target::Wireless(n) if n.saved))
            .unwrap();
        v.clicked(saved_row, &rows);
        assert_eq!(
            v.key(Press::Forget, &facts(&snap, &seen), &t),
            Errand::Forget("a house".to_owned())
        );

        // AND A NETWORK NOBODY SAVED IS NOT FORGOTTEN EITHER: there is no
        // profile to delete, and the daemon's refusal would look like a fault.
        let stranger = rows
            .iter()
            .position(|r| matches!(&r.target, Target::Wireless(n) if !n.saved))
            .unwrap();
        v.clicked(stranger, &rows);
        assert_eq!(
            v.key(Press::Forget, &facts(&snap, &seen), &t),
            Errand::Nothing
        );

        // AND A STORED TUNNEL IS DELETED, which is the same idea from the
        // person's side and so is the same key.
        let stored = rows
            .iter()
            .position(|r| r.target == Target::Saved("/s/1".to_owned()))
            .unwrap();
        v.clicked(stored, &rows);
        assert_eq!(
            v.key(Press::Forget, &facts(&snap, &seen), &t),
            Errand::Delete("/s/1".to_owned())
        );
    }

    #[test]
    fn the_cursor_skips_the_headings_and_stays_in_the_list() {
        let t = texts();
        let snap = machine();
        let seen = vec![network("a house", 90)];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &seen), 80, &t);
        assert!(!rows[v.cursor()].heading, "it started on a heading");
        for _ in 0..rows.len() * 2 {
            assert_eq!(
                v.key(Press::Down, &facts(&snap, &seen), &t),
                Errand::Nothing
            );
            assert!(!rows[v.cursor()].heading, "it stopped on a heading");
        }
        assert_eq!(v.cursor(), rows.len() - 1, "it did not reach the end");
        for _ in 0..rows.len() * 2 {
            assert_eq!(v.key(Press::Up, &facts(&snap, &seen), &t), Errand::Nothing);
        }
        assert!(!rows[v.cursor()].heading);
        assert!(v.cursor() < rows.len());
    }

    #[test]
    fn the_cursor_steps_back_when_the_list_shrinks() {
        // The focused network disappears between two readings — the
        // neighbour switched the router off. Without the step back, the
        // cursor would point past the end and the next key would act on
        // nothing, with the keyboard swallowed and only `Esc` left.
        let t = texts();
        let snap = machine();
        let seen = vec![network("a", 90), network("b", 80), network("c", 70)];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &seen), 80, &t);
        v.clicked(rows.len() - 1, &rows);
        assert_eq!(v.cursor(), rows.len() - 1);
        let fewer = v.rows(&facts(&snap, &seen[..1]), 80, &t);
        v.settle(&fewer);
        assert!(v.cursor() < fewer.len(), "the cursor ended up outside");
        assert!(!fewer[v.cursor()].heading);
    }

    #[test]
    fn escape_closes_and_the_switches_offer_the_other_state() {
        let t = texts();
        let mut snap = machine();
        let mut v = View::fresh();
        assert_eq!(v.key(Press::Escape, &facts(&snap, &[]), &t), Errand::Close);
        assert_eq!(
            v.key(Press::Radio, &facts(&snap, &[]), &t),
            Errand::Radio(false)
        );
        snap.radio = false;
        assert_eq!(
            v.key(Press::Radio, &facts(&snap, &[]), &t),
            Errand::Radio(true)
        );
        assert_eq!(
            v.key(Press::Airplane, &facts(&snap, &[]), &t),
            Errand::Airplane(true)
        );
        assert_eq!(
            v.key(Press::Disconnect, &facts(&snap, &[]), &t),
            Errand::Disconnect(Medium::Wired)
        );
        assert_eq!(v.key(Press::Scan, &facts(&snap, &[]), &t), Errand::Scan);
        assert_eq!(v.key(Press::Facts, &facts(&snap, &[]), &t), Errand::Facts);
    }

    #[test]
    fn the_focused_network_is_what_the_password_comes_back_to() {
        let t = texts();
        let snap = machine();
        let seen = vec![network("a house", 90), network("a café", 60)];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &seen), 80, &t);
        let second = rows
            .iter()
            .position(|r| matches!(&r.target, Target::Wireless(n) if n.ssid == "a café"))
            .unwrap();
        v.clicked(second, &rows);
        assert_eq!(
            v.focused_network(&facts(&snap, &seen), &t).map(|n| n.ssid),
            Some("a café".to_owned())
        );
        assert_eq!(
            v.key(Press::Enter, &facts(&snap, &seen), &t),
            Errand::Connect(Box::new(seen[1].clone()))
        );
    }

    #[test]
    fn a_mark_opens_the_view_on_the_row_it_was_about() {
        let t = texts();
        let mut snap = machine();
        snap.active = vec![tunnel("an office", "/a/1")];
        let want = Target::Tunnel("/a/1".to_owned());
        let v = View::fresh_at(&facts(&snap, &[]), &want, &t);
        assert_eq!(v.rows(&facts(&snap, &[]), 80, &t)[v.cursor()].target, want);
    }

    #[test]
    fn a_port_nobody_owns_says_so_and_the_rest_are_counted() {
        // A BLANK SECOND COLUMN READS AS "NOTHING IS THERE". `sshd` on 22 is
        // root's, `/proc/<pid>/fd` is not ours to read, and the row has to
        // say which of the two it is.
        let t = texts();
        let snap = machine();
        let socket = |port: u16, listening: bool, owner: Option<&str>| Socket {
            protocol: ronoctua_memory::net::Protocol::Tcp,
            address: "0.0.0.0".parse().unwrap(),
            port,
            peer: (!listening).then(|| ("10.0.0.9".parse().unwrap(), 443)),
            listening,
            inode: u64::from(port),
            owner: owner.map(|program| ronoctua_memory::net::Owner {
                pid: 1,
                program: program.to_owned(),
            }),
        };
        let mut sockets = vec![socket(22, true, None), socket(8080, true, Some("a server"))];
        // TWELVE OF OURS TALKING, so two of them fall past the ceiling.
        sockets.extend((0..12).map(|i| socket(40000 + i, false, Some("a browser"))));
        let watch = Watch {
            sockets,
            counters: vec![Counters {
                interface: "wlan0".to_owned(),
                received: 44_220_000_000,
                sent: 1_000,
            }],
            ..Watch::default()
        };
        let facts = Facts {
            net: &snap,
            seen: &[],
            watch: &watch,
            tunnels: &ronoctua_tunnel::Scene::default(),
        };
        let rows = View::fresh().rows(&facts, 80, &t);
        let said = |needle: &str| {
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text.contains(needle)))
        };
        assert!(said("0.0.0.0:22"));
        assert!(said(&t.t(ronoctua_i18n::key::NETWORK_PORT_NOT_OURS)));
        assert!(said("a server"));
        assert!(said("a browser"));
        // THE CEILING SAYS WHAT IT HID: silence would read as "that is all".
        assert!(said("2"), "the two it did not draw are not counted");
        // AND THE MONITOR'S TOTALS, in the units the rest of the screen uses.
        assert!(said("44,2 GB"));
        assert!(said(&t.t(ronoctua_i18n::key::NETWORK_FACT_SENT)));
    }

    #[test]
    fn a_machine_with_neither_daemon_draws_neither_heading() {
        // AN EMPTY SECTION UNDER A TITLE is a thing to look at that does not
        // exist, and on an ordinary install both of these are absent.
        let t = texts();
        let snap = machine();
        let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
        let headings: Vec<String> = rows
            .iter()
            .filter(|r| r.heading)
            .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
            .collect();
        assert!(!headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_TAILSCALE)));
        assert!(!headings.contains(&t.t(ronoctua_i18n::key::NETWORK_HEADING_TOR)));
    }

    #[test]
    fn the_tunnels_draw_their_switches_and_the_exit_node() {
        let t = texts();
        let snap = machine();
        let tunnels = ronoctua_tunnel::Scene {
            tailscale: Some(ronoctua_tunnel::tailscale::Scene {
                state: "Running".to_owned(),
                running: true,
                name: "this one".to_owned(),
                tailnet: "a tailnet".to_owned(),
                addresses: vec!["100.64.0.1".to_owned()],
                magic_dns: true,
                wanted: true,
                accept_routes: false,
                shields_up: false,
                exit_node: None,
                auth_url: None,
                peers: vec![
                    ronoctua_tunnel::tailscale::Peer {
                        id: "n3".to_owned(),
                        name: "a relay".to_owned(),
                        os: "linux".to_owned(),
                        online: true,
                        offers_exit: true,
                        ..ronoctua_tunnel::tailscale::Peer::default()
                    },
                    ronoctua_tunnel::tailscale::Peer {
                        id: "n2".to_owned(),
                        name: "a phone".to_owned(),
                        ..ronoctua_tunnel::tailscale::Peer::default()
                    },
                ],
            }),
            tor: Some(ronoctua_tunnel::tor::Scene {
                established: false,
                bootstrap: 45,
                summary: "Asking for relay descriptors".to_owned(),
                ..ronoctua_tunnel::tor::Scene::default()
            }),
        };
        let facts = Facts {
            net: &snap,
            seen: &[],
            watch: &Watch::default(),
            tunnels: &tunnels,
        };
        let mut v = View::fresh();
        let rows = v.rows(&facts, 80, &t);
        let target = |want: &Target| rows.iter().position(|r| &r.target == want);

        // THE TAILNET'S SWITCH READS WANTED, NOT RUNNING: this scene wants to
        // run and does, so the switch offers to take it down.
        assert!(target(&Target::Tunnels(Switch::Tailnet, false)).is_some());
        assert!(target(&Target::Tunnels(Switch::Routes, true)).is_some());
        assert!(target(&Target::Tunnels(Switch::Shields, true)).is_some());
        // ONLY A NODE THAT OFFERS TO BE ONE can be chosen as the exit.
        assert!(target(&Target::ExitNode("n3".to_owned())).is_some());
        assert!(target(&Target::ExitNode("n2".to_owned())).is_none());
        // AND `RET` ON IT ASKS FOR IT.
        let at = target(&Target::ExitNode("n3".to_owned())).unwrap();
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts, &t),
            Errand::Tunnel(ronoctua_tunnel::Order::ExitNode(Some("n3".to_owned())))
        );

        // TOR'S ROW STOPS THE DAEMON, whatever the circuit is doing: the
        // section only exists because the control socket answered, which is
        // to say the unit is running. Reading the circuit here sent `start`
        // to a unit that was already up — a no-op with no message — and left
        // a bootstrapping Tor with no way to be stopped.
        let stop = target(&Target::Tunnels(Switch::Tor, false)).unwrap();
        v.clicked(stop, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts, &t),
            Errand::Tunnel(ronoctua_tunnel::Order::Tor(false))
        );
        // AND THE IDENTITY IS REFUSED while there is no circuit.
        let identity = target(&Target::Tunnels(Switch::Identity, true)).unwrap();
        assert!(rows[identity].disabled, "a new identity with no circuit");
        v.clicked(identity, &rows);
        assert_eq!(v.key(Press::Enter, &facts, &t), Errand::Nothing);

        // AND THE BOOTSTRAP IS ON SCREEN while it is not ready.
        assert!(
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text.contains("45"))),
            "the bootstrap's percentage is not drawn"
        );
    }

    #[test]
    fn the_exit_node_in_use_is_taken_off_by_a_second_press() {
        let t = texts();
        let snap = machine();
        let tunnels = ronoctua_tunnel::Scene {
            tailscale: Some(ronoctua_tunnel::tailscale::Scene {
                state: "Running".to_owned(),
                running: true,
                exit_node: Some("n3".to_owned()),
                peers: vec![ronoctua_tunnel::tailscale::Peer {
                    id: "n3".to_owned(),
                    name: "a relay".to_owned(),
                    online: true,
                    offers_exit: true,
                    exit_node: true,
                    ..ronoctua_tunnel::tailscale::Peer::default()
                }],
                ..ronoctua_tunnel::tailscale::Scene::default()
            }),
            tor: None,
        };
        let facts = Facts {
            net: &snap,
            seen: &[],
            watch: &Watch::default(),
            tunnels: &tunnels,
        };
        let mut v = View::fresh();
        let rows = v.rows(&facts, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::ExitNode("n3".to_owned()))
            .unwrap();
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts, &t),
            Errand::Tunnel(ronoctua_tunnel::Order::ExitNode(None)),
            "a second press on the node in use takes it off"
        );
    }

    #[test]
    fn this_network_lists_the_neighbours_the_kernel_knows_and_this_machine() {
        // NOTHING ARPS FOR ITSELF, so a list built only out of the kernel's
        // table is a list of the network with a hole where the machine
        // reading it should be.
        let t = texts();
        let snap = machine();
        let watch = Watch {
            neighbours: vec![
                ronoctua_memory::net::Neighbour {
                    address: "10.0.0.1".parse().unwrap(),
                    hardware: "aa:bb:cc:dd:ee:01".to_owned(),
                    interface: "wlan0".to_owned(),
                    fresh: true,
                },
                // AN ENTRY THE KERNEL GAVE UP ON is not a machine on the
                // network, and listing it would be a room that is not there.
                ronoctua_memory::net::Neighbour {
                    address: "10.0.0.9".parse().unwrap(),
                    hardware: "00:00:00:00:00:00".to_owned(),
                    interface: "wlan0".to_owned(),
                    fresh: false,
                },
            ],
            ..Watch::default()
        };
        let facts = Facts {
            net: &snap,
            seen: &[],
            watch: &watch,
            tunnels: &ronoctua_tunnel::Scene::default(),
        };
        let rows = View::fresh().rows(&facts, 80, &t);
        let said = |needle: &str| {
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text.contains(needle)))
        };
        assert!(said("10.0.0.1"));
        assert!(!said("10.0.0.9"), "a stale neighbour was listed");
        // THE GATEWAY IS MARKED, and so is this machine.
        assert!(said(&t.t(ronoctua_i18n::key::NETWORK_LAN_GATEWAY)));
        assert!(said(&t.t(ronoctua_i18n::key::NETWORK_LAN_THIS_MACHINE)));
        assert!(said("10.0.0.2"), "this machine's own address is missing");
    }

    #[test]
    fn a_sweep_says_it_is_running_and_then_what_it_found() {
        // A KEY THAT TAKES A SECOND has to say so, or it looks broken.
        let t = texts();
        let snap = machine();
        let mut watch = Watch::default();
        watch.sweeping(true);
        let looking = View::fresh().rows(
            &Facts {
                net: &snap,
                seen: &[],
                watch: &watch,
                tunnels: &ronoctua_tunnel::Scene::default(),
            },
            80,
            &t,
        );
        assert!(
            looking.iter().any(|r| r
                .fields
                .iter()
                .any(|f| f.text == t.t(ronoctua_i18n::key::NETWORK_LAN_LOOKING))),
            "the sweep does not say it is running"
        );
        watch.found(ronoctua_lan::Found {
            replies: vec![ronoctua_lan::icmp::Reply {
                address: "10.0.0.7".parse().unwrap(),
            }],
            asked: 254,
            range: "10.0.0.1-10.0.0.254".to_owned(),
            refused: None,
        });
        let done = View::fresh().rows(
            &Facts {
                net: &snap,
                seen: &[],
                watch: &watch,
                tunnels: &ronoctua_tunnel::Scene::default(),
            },
            80,
            &t,
        );
        let said = |needle: &str| {
            done.iter()
                .any(|r| r.fields.iter().any(|f| f.text.contains(needle)))
        };
        assert!(said("10.0.0.7"), "what answered is not listed");
        assert!(said("254"), "the sweep does not say how many it asked");
        // AND WHICH ADDRESSES: a count with no range beside it reads as
        // "this is everybody", and a sweep is capped at 254.
        assert!(
            said("10.0.0.1-10.0.0.254"),
            "the sweep does not say which range it asked"
        );

        // AND A KERNEL THAT REFUSED THE SOCKET says so, rather than reading
        // as a network with nobody on it.
        let mut refused = Watch::default();
        refused.found(ronoctua_lan::Found {
            replies: Vec::new(),
            asked: 254,
            range: "10.0.0.1-10.0.0.254".to_owned(),
            refused: Some("no unprivileged ICMP socket".to_owned()),
        });
        let rows = View::fresh().rows(
            &Facts {
                net: &snap,
                seen: &[],
                watch: &refused,
                tunnels: &ronoctua_tunnel::Scene::default(),
            },
            80,
            &t,
        );
        assert!(
            rows.iter().any(|r| r
                .fields
                .iter()
                .any(|f| f.text.contains("no unprivileged ICMP socket"))),
            "a refused sweep reads as an empty network"
        );
    }

    #[test]
    fn a_file_is_looked_at_before_it_is_brought_in() {
        // THE READING IS THE POINT: `AllowedIPs = 0.0.0.0/0` sends every
        // packet this machine writes through somebody else's computer, and
        // that has to be a sentence on screen before anybody presses `RET`.
        let t = texts();
        let snap = machine();
        let preview = ronoctua_network::vpn::preview(
            "/tmp/an-office.conf",
            "[Interface]\nAddress = 198.51.100.2/32\nDNS = 198.51.100.1\n\n[Peer]\nAllowedIPs = 0.0.0.0/0\nEndpoint = vpn.example.invalid:51820\n",
        )
        .unwrap();
        let mut v = View::fresh();
        v.looked_at("/tmp/an-office.conf".to_owned(), preview);
        assert!(v.bringing());
        let rows = v.rows(&facts(&snap, &[]), 80, &t);
        let said = |needle: &str| {
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text.contains(needle)))
        };
        assert!(said(&t.t(ronoctua_i18n::key::NETWORK_IMPORT_EVERYTHING)));
        assert!(said("198.51.100.1"), "the DNS it would move is not drawn");
        assert!(said("vpn.example.invalid:51820"));
        // AND THE KEY THAT BRINGS IT IN IS A ROW, like everything else here.
        let bring = rows.iter().position(|r| r.target == Target::Bring).unwrap();
        v.clicked(bring, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts(&snap, &[]), &t),
            Errand::Bring(
                "/tmp/an-office.conf".to_owned(),
                ronoctua_network::vpn::Kind::WireGuard
            )
        );
        assert!(!v.bringing(), "the file was not put down after answering");
    }

    #[test]
    fn escape_puts_the_file_down_before_it_closes_the_view() {
        // ONE LEVEL AT A TIME: closing the whole view on a file that was
        // being read would throw the reading away without saying so.
        let t = texts();
        let snap = machine();
        let preview = ronoctua_network::vpn::preview(
            "/tmp/a-house.conf",
            "[Interface]\nAddress = 198.51.100.2/32\n",
        )
        .unwrap();
        let mut v = View::fresh();
        v.looked_at("/tmp/a-house.conf".to_owned(), preview);
        assert_eq!(
            v.key(Press::Escape, &facts(&snap, &[]), &t),
            Errand::Nothing
        );
        assert!(!v.bringing());
        assert_eq!(v.key(Press::Escape, &facts(&snap, &[]), &t), Errand::Close);
    }

    #[test]
    fn sharing_offers_the_other_state_and_says_what_it_costs() {
        // A LAPTOP WITH ONE RADIO CANNOT BE ON A NETWORK AND BE ONE, and the
        // row says so before it is pressed rather than afterwards.
        let t = texts();
        let mut snap = machine();
        let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Hotspot(true))
            .expect("no row offers to share");
        assert!(!rows[at].disabled);
        assert!(
            rows.iter().any(|r| r
                .fields
                .iter()
                .any(|f| f.text == t.t(ronoctua_i18n::key::NETWORK_HOTSPOT_COST))),
            "the row does not say what sharing costs"
        );
        // ALREADY SERVING: the row offers to stop, and the password stays on
        // screen because it is typed on somebody else's phone.
        for l in &mut snap.links {
            l.serving = l.medium == Medium::Radio;
        }
        let mut v = View::fresh();
        v.sharing(Some("a-password".to_owned()));
        let rows = v.rows(&facts(&snap, &[]), 80, &t);
        assert!(rows.iter().any(|r| r.target == Target::Hotspot(false)));
        assert!(
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.text == "a-password")),
            "the password is not on screen while the hotspot is up"
        );
        // AND WITH THE RADIO OFF there is nothing to serve with.
        snap.radio = false;
        let rows = View::fresh().rows(&facts(&snap, &[]), 80, &t);
        let at = rows
            .iter()
            .position(|r| matches!(r.target, Target::Hotspot(_)))
            .unwrap();
        assert!(rows[at].disabled);
    }

    #[test]
    fn a_rate_needs_two_samples_and_a_second_between_them() {
        // ONE READING IS A TOTAL SINCE BOOT, not a rate. Dividing by the time
        // since the last keypress is how a quiet link draws megabytes a
        // second, so the second sample has to be a second later.
        let mut watch = Watch::default();
        assert_eq!(watch.rate("wlan0"), None);
        watch.sample();
        assert_eq!(watch.rate("wlan0"), None, "one sample can only be a total");
    }

    #[test]
    fn the_bytes_change_unit_without_losing_the_magnitude() {
        assert_eq!(readable_bytes(0), "0 B");
        assert_eq!(readable_bytes(70_123), "70 kB");
        assert_eq!(readable_bytes(44_220_000_000), "44,2 GB");
    }

    #[test]
    fn the_bars_are_four_steps_and_not_a_percentage() {
        assert_eq!(bars(0), 0);
        assert_eq!(bars(30), 1);
        assert_eq!(bars(50), 2);
        assert_eq!(bars(70), 3);
        assert_eq!(bars(95), 4);
        assert_eq!(bars_glyph(0), tokens::icon::WIFI_NONE);
        assert_eq!(bars_glyph(95), tokens::icon::WIFI_STRONG);
    }
}
