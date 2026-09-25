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
//! AGENTS.md records.
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

use crate::files::{Field, Hit as FilesHit, Ink};

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
/// The width the tests build rows at.
///
/// # Why production no longer has one
///
/// A key acts on the row under the cursor, so it has to know the rows — and it
/// used to build them at a fixed eighty columns, on the argument that the row
/// COUNT and the row TARGETS do not depend on the width. That argument held
/// while every note was CUT: an ellipsis changes what a row SAYS and never how
/// many there are.
///
/// A note WRAPS now (`chrome::note_lines`), so one sentence is a row on a wide
/// screen and three on a narrow one, and every row under it moves. A key
/// deciding at eighty would act on a different row from the one the door drew
/// — a control taken somewhere other than where it was drawn, arriving on the
/// keyboard's axis. So the DOOR's width travels in, as it already did for the
/// Bluetooth view, and what is left here is a width the tests agree on.
#[cfg(test)]
const DECIDING_WIDTH: u16 = 80;

/// What NetworkManager calls a Wi-Fi profile.
///
/// The same string [`ronoctua_network::Saved::tunnel`] tests the other two
/// against, and it is the daemon's word rather than ours: a profile's `kind`
/// is what came off the bus.
pub const WIRELESS: &str = "802-11-wireless";

/// THE ORDER IS THE TEACHING ORDER, and it is load-bearing: the hint line is
/// CUT at the width it is given (`chrome::hint_spans` elides, it does not
/// wrap), and this table is 174 cells in English against a footer that has
/// about 114. So whatever goes last is not taught at any width anybody has.
///
/// The four movements come first — down, up, act, and the section ring — and
/// the rest follows in the order somebody meets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Every screen: the arrows, and `RET`.
    Anywhere,
    /// Only the list of sections — the keys that act on a link or a network.
    OnTheList,
    /// Only Tor's own sheet.
    ///
    /// THE HINT TEACHES THE LEVEL, which is the lesson the owner photographed
    /// on the radio: a sheet that went on offering `s` to scan and `f` to
    /// forget would be teaching six keys that do nothing where you are.
    OnTor,
}

/// Where one id's key is taught.
///
/// Read by this view's own tests, which are what hold the rule that a key the
/// hint teaches does something on the level it is taught on.
#[cfg(test)]
pub(crate) fn where_of(id: &str) -> Option<Where> {
    OF_THE_VIEW
        .iter()
        .find(|(_, other, _)| *other == id)
        .map(|(_, _, w)| *w)
}

impl Where {
    /// Is this key worth teaching on the screen that is showing?
    #[must_use]
    pub const fn shown(self, level: Level) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnTheList => matches!(level, Level::List),
            Self::OnTor => matches!(level, Level::Tor),
        }
    }
}

/// Which screen of the view is showing.
///
/// TWO, AND THE SECOND ONE IS WHY THIS EXISTS. Tor has thirty-four settings,
/// and a list that already carries ten sections has no room for them — nor for
/// the six bare letters they would need. It is the shape the Bluetooth view
/// uses for a device: a field says where you are, a `match` draws it, and `Esc`
/// goes back one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Level {
    /// The ten sections.
    #[default]
    List,
    /// Tor, whole.
    Tor,
}

pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "network-down", Where::Anywhere),
    ("<up>", "network-up", Where::Anywhere),
    ("RET", "network-connect", Where::Anywhere),
    // THE SECTIONS, and this is the ring the owner asked for: ten sections in
    // one list is a lot of `<down>`, and `TAB` is the key this system already
    // uses to walk a ring (`files`, `notifications`).
    //
    // `S-TAB` is the keysym `ISO_Left_Tab` — xkb has already applied the shift
    // by the time a door sees it, which is why the notation carries the
    // modifier and the window does not test for one.
    ("TAB", "network-section-next", Where::OnTheList),
    ("S-TAB", "network-section-previous", Where::OnTheList),
    ("s", "network-scan", Where::OnTheList),
    ("r", "network-wifi-toggle", Where::OnTheList),
    ("d", "network-disconnect", Where::OnTheList),
    ("f", "network-forget", Where::OnTheList),
    ("a", "network-airplane", Where::OnTheList),
    ("i", "network-link", Where::OnTheList),
    ("l", "network-lan", Where::OnTheList),
    ("v", "network-import", Where::OnTheList),
    // TOR'S SHEET. `S-<arrows>` change the VALUE, which is this system's rule
    // everywhere a row carries one — the bare arrows walk the list, and a row
    // that stole them would be a row you could not get past.
    ("S-<right>", "network-tor-more", Where::OnTor),
    ("S-<left>", "network-tor-less", Where::OnTor),
    // `ESC`, and not `Esc`: `ronoctua_keymap::Sym::Esc` writes itself in
    // capitals, and a notation the parser does not accept is a key the
    // hint silently drops. The count in
    // `the_rows_land_at_the_inset_and_the_hint_teaches_every_key` is what
    // caught it: five ids taught where six were declared.
    ("ESC", "network-tor-back", Where::OnTor),
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
    /// To the first row of the next section.
    Section,
    /// To the first row of the previous section.
    SectionBack,
    /// Turn the value under the cursor forwards.
    ///
    /// `S-<right>`, and not the bare arrow: a sheet is a LIST, the bare arrows
    /// walk it, and a row that took them would be a row nobody could get past.
    /// It is the rule the Bluetooth and locale views already follow.
    More,
    /// Turn it back.
    Less,
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
    /// Open Tor's own sheet.
    TorSheet,
    /// One declared Tor setting, by its id.
    ///
    /// THE ID AND NOT THE `&'static Setting`, so a `Target` stays comparable
    /// and cloneable the way every other one is — the table is a lookup away
    /// (`options::by_id`), and a row that named a setting the table does not
    /// hold is a programming error rather than a state.
    TorSetting(&'static str),
    /// Open or close the sheet's drawer.
    TorDrawer,
    /// Start Tor when the session comes up, or stop doing that.
    TorAutostart(bool),
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

/// A list inside the list, with a ceiling and a bar of its own.
///
/// # Why the view has these at all
///
/// Because three of its lists are read CONTINUOUSLY and the answer changes
/// under the eye: Wi-Fi as routers come and go, the neighbours as the sweep
/// answers, the ports as programs open and close them. With each of them free
/// to grow, everything below moved up and down while nobody touched anything —
/// which the owner reported from the screen.
///
/// A shelf draws `space.shelf.lines` rows ALWAYS, padded with blanks when it
/// holds fewer, and scrolls inside itself. Nothing under it moves, ever. See
/// `View::shelves` for what the two doors ask in order to draw its bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shelf {
    /// Every network in range.
    Wifi,
    /// Every network this machine has a password for, in range or not.
    Remembered,
    /// What this machine is answering on.
    Ports,
    /// Who else is on this network.
    Neighbours,
    /// The other machines of the tailnet.
    Peers,
}

/// How many rows a shelf draws, whatever it holds.
pub const SHELF_LINES: usize = tokens::space::SHELF_LINES as usize;

/// One shelf, as it was DRAWN — what a door needs to put a bar beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// Which one.
    pub shelf: Shelf,
    /// Its first row's index in the drawn list.
    pub first: usize,
    /// How many rows it drew.
    pub lines: usize,
    /// How many it holds.
    pub total: usize,
    /// Which of them is first on screen.
    pub offset: usize,
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
    /// Which shelf this row is in, and HOW MANY rows that shelf really holds.
    ///
    /// THE TOTAL TRAVELS WITH THE ROW because the shelf is what clips it: the
    /// drawn list has `space.shelf.lines` rows and the shelf may hold fifty,
    /// and both the bar and the arrow at the shelf's edge need the fifty. The
    /// alternative was building the rows a second time unclipped to count
    /// them, which is a whole frame's work for a number the builder already
    /// had in its hand.
    pub shelf: Option<(Shelf, usize)>,
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Put this on the system clipboard — what `RET` does on an INFORMATIVE
    /// row. See `chrome::information_of` for what "this" is and why.
    Copy(String),
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
    /// Write Tor's fragment, and make a running Tor re-read it.
    TorConfigure(Box<ronoctua_tunnel::tor::fragment::Lines>),
    /// Ask for this Tor setting's value to be typed.
    ///
    /// The DOOR decides where: the footer opens its minibuffer, and the window
    /// says so — the rule the Wi-Fi password and the VPN path already follow,
    /// because the one place anything is typed in this system is the footer.
    TorEdit(&'static str),
    /// Remember, or stop remembering, that Tor should come up with the session.
    TorAutostart(bool),
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
    ///
    /// TOR'S AUTOSTART RIDES ON THIS and not on a field of its own: the
    /// `Scene` already carries what the daemon, systemd and the fragment say,
    /// and a fourth field here would be a field in both doors, the click and
    /// every test — which is exactly what this struct exists to avoid.
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
    /// The line the region should put at the TOP, once.
    ///
    /// # Why the cursor is not enough
    ///
    /// Because both doors scroll to keep the CURSOR visible, with a margin,
    /// and "visible" is satisfied by the cursor sitting at the bottom of the
    /// region with its section's title off the screen above it. Two things
    /// need more than that:
    ///
    ///   * `TAB`, which lands on a section and has to SHOW the section;
    ///   * a click on a mark in the modeline, which the owner asked to open
    ///     the view with that section's title at the top — clicking the tailnet's mark
    ///     opens the network with `tailnet` at the top, not with one of its
    ///     rows somewhere in the middle.
    ///
    /// It is taken ONCE and cleared: after that the cursor is in charge again,
    /// or the region would refuse to scroll for the arrows.
    anchor: Option<usize>,
    /// Where each shelf is scrolled to — see [`Shelf`].
    ///
    /// A map and not five fields: a shelf that has never been scrolled has no
    /// entry, which is the same thing as nought, and adding a sixth shelf is
    /// then a variant and nothing else.
    shelves: std::collections::BTreeMap<Shelf, usize>,
    /// Which screen is showing.
    level: Level,
    /// Is Tor's drawer of rarely-touched settings open?
    ///
    /// IN THE VIEW AND NOT ON DISK, the same answer `fold::Fold::toggle`
    /// gives: a drawer somebody opened once is not a preference, and closing
    /// the view is what forgets it.
    drawer: bool,
    /// Tor's settings as the sheet has them — the fragment, plus whatever has
    /// been turned since it was read.
    ///
    /// # Why the view holds a copy at all
    ///
    /// Because a key press has to change what is DRAWN before the thread has
    /// written the file and the next reading has come back. Without it, every
    /// arrow on the sheet would show the old value until the beat caught up,
    /// which reads as a key that does not work. The order goes out in the same
    /// breath; `Facts` replaces this whenever the file's own version differs,
    /// so the file stays the truth and this is only ever a moment ahead of it.
    tor_edits: Option<ronoctua_tunnel::tor::fragment::Lines>,
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
            anchor: None,
            level: Level::List,
            drawer: false,
            tor_edits: None,
            shelves: std::collections::BTreeMap::new(),
        }
    }

    /// A fresh view with the cursor on the first row of a kind — how a click
    /// on the aeroplane mark lands on the aeroplane row.
    #[must_use]
    pub fn fresh_at(
        facts: &Facts<'_>,
        want: &Target,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        let mut v = Self::fresh();
        let rows = v.rows(facts, columns, texts);
        if let Some(i) = rows.iter().position(|r| &r.target == want) {
            v.cursor = i;
            // AND THE SECTION'S TITLE GOES TO THE TOP. The owner asked for it
            // in those words: clicking a mark should open the view *"com o
            // título no topo"*, not with the row somewhere in the middle of a
            // section whose name is off the screen.
            v.anchor = Self::title_above(&rows, i);
        }
        v
    }

    /// The view, opened straight onto Tor's sheet.
    ///
    /// A LEVEL, AND NOT A ROW, which is why [`Self::fresh_at`] cannot reach
    /// it: that one looks for a row and puts the cursor on it, and the sheet is
    /// the second SCREEN of this view. Worse, on a machine with no Tor at all
    /// the row it would look for is not drawn — a machine says nothing about a
    /// program it does not have — so the search found nothing and the view
    /// opened at the top of the list. That is what came out of `.#shots`: a
    /// picture of "at a glance" and the Wi-Fi section under a name that
    /// promises Tor, which is the SECOND wrong photograph this door has
    /// produced. The first counted `TAB`s.
    #[must_use]
    pub fn fresh_on_the_tor_sheet() -> Self {
        let mut v = Self::fresh();
        v.level = Level::Tor;
        v
    }

    /// The title of the section row `n` belongs to.
    fn title_above(rows: &[Row], n: usize) -> Option<usize> {
        rows.iter()
            .take(n + 1)
            .rposition(|r| r.heading && !r.fields.is_empty())
    }

    /// One step INSIDE a shelf, when the cursor is at its edge and it has more.
    ///
    /// Answers whether it moved. `step` is the same `-1`/`+1` the arrows use.
    fn scrolled_the_shelf(&mut self, rows: &[Row], step: isize) -> bool {
        let Some((which, total)) = rows.get(self.cursor).and_then(|r| r.shelf) else {
            return false;
        };
        if total <= SHELF_LINES {
            return false;
        }
        // THE EDGE IS THE LAST ROW WITH SOMETHING ON IT: the padding under a
        // short shelf belongs to it too, and stepping into a blank row is not
        // stepping out of the shelf.
        let filled: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.shelf.map(|(s, _)| s) == Some(which) && !r.heading)
            .map(|(i, _)| i)
            .collect();
        let (Some(first), Some(last)) = (filled.first(), filled.last()) else {
            return false;
        };
        // CLAMPED HERE TOO. `Builder::shelf` clamps when it DRAWS and leaves
        // the stored number alone, so a shelf that shrank under a scrolled
        // view kept an offset the list no longer has — and `Up` then spent one
        // press per phantom row bringing it back down, swallowing the key each
        // time. With the keyboard repeating, what that looks like is the view
        // frozen under a held arrow.
        let last_offset = total.saturating_sub(SHELF_LINES);
        let at = self
            .shelves
            .get(&which)
            .copied()
            .unwrap_or(0)
            .min(last_offset);
        if at != self.shelves.get(&which).copied().unwrap_or(0) {
            self.shelves.insert(which, at);
        }
        if step > 0 && self.cursor == *last && at + SHELF_LINES < total {
            self.shelves.insert(which, at + 1);
            return true;
        }
        if step < 0 && self.cursor == *first && at > 0 {
            self.shelves.insert(which, at - 1);
            return true;
        }
        false
    }

    /// Every shelf on screen: where it starts, how many lines it draws, how
    /// many rows it holds and where it is scrolled to.
    ///
    /// ASKED BY BOTH DOORS, so each can draw the shelf's own bar with the same
    /// [`ronoctua_design::Scroll`] the region and the manager already use.
    #[must_use]
    pub fn shelves(&self, rows: &[Row]) -> Vec<Band> {
        let mut out: Vec<Band> = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let Some((which, total)) = row.shelf else {
                continue;
            };
            match out.last_mut() {
                // CONTIGUOUS, and not merely the same shelf: two runs of one
                // shelf with anything between them are two bands, and merging
                // them would give a band whose `lines` covers rows it does not
                // own. Nothing produces that today; a band is cheap insurance.
                Some(band) if band.shelf == which && band.first + band.lines == i => {
                    band.lines += 1;
                }
                _ => out.push(Band {
                    shelf: which,
                    first: i,
                    lines: 1,
                    total,
                    offset: self.shelves.get(&which).copied().unwrap_or(0),
                }),
            }
        }
        out
    }

    /// Scrolls one shelf, from a door's drag of its bar.
    ///
    /// Clamped by the shelf itself when it next draws (`Builder::shelf`), so a
    /// door may hand in whatever its arithmetic produced.
    pub fn put_shelf(&mut self, which: Shelf, offset: usize) {
        self.shelves.insert(which, offset);
    }

    /// The line the region should put at the top, if the view asked for one.
    ///
    /// TAKEN, not read: it answers once and then the cursor is in charge —
    /// see the field's own comment.
    pub fn take_anchor(&mut self) -> Option<usize> {
        self.anchor.take()
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
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Network> {
        match self.rows(facts, columns, texts).get(self.cursor)? {
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
        // THE HINT TEACHES THE LEVEL. A sheet that went on offering `s` to
        // scan and `f` to forget would teach six keys that do nothing where
        // you are — which is what the owner photographed on the radio.
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(self.level))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        crate::chrome::hint_of(&here, texts)
    }

    /// A Tor setting's value, as it was typed in the footer's minibuffer.
    ///
    /// The DOOR collects it — the footer is the only place anything is typed
    /// in this system — and hands it back here, because what to do with it is
    /// the view's to decide and both doors must decide it the same way.
    #[must_use]
    pub fn tor_typed(&mut self, facts: &Facts<'_>, id: &str, values: Vec<String>) -> Errand {
        let Some(setting) = ronoctua_tunnel::options::by_id(id) else {
            return Errand::Nothing;
        };
        if setting.fixed {
            return Errand::Nothing;
        }
        self.tor_put(facts, setting, values)
    }

    /// What a key does to one Tor setting.
    ///
    /// ONE FUNCTION FOR `RET` AND BOTH `S-<arrows>`, because the difference
    /// between them is one boolean and nothing else. Splitting it was how the
    /// Bluetooth view ended up with a `RET` that turned a ring one way and an
    /// arrow that turned a different ring.
    fn tor_key(&mut self, facts: &Facts<'_>, id: &'static str, forwards: bool) -> Errand {
        let Some(setting) = ronoctua_tunnel::options::by_id(id) else {
            // A ROW NAMING A SETTING THE TABLE DOES NOT HOLD is a programming
            // error, not a state: the rows are built from the same table.
            debug_assert!(false, "a row named `{id}`, which is not in the table");
            return Errand::Nothing;
        };
        if setting.fixed {
            return Errand::Nothing;
        }
        let lines = self.tor_lines(facts);
        match setting.turned(&lines, forwards) {
            Some(value) => self.tor_put(facts, setting, vec![value]),
            // TYPED, NOT TURNED. The door decides where the typing happens.
            None => Errand::TorEdit(id),
        }
    }

    /// Tor's settings as the sheet has them.
    ///
    /// The fragment as it was read, with whatever has been turned since laid
    /// over it. See [`View::tor_edits`] for why the view holds a copy.
    fn tor_lines(&self, facts: &Facts<'_>) -> ronoctua_tunnel::tor::fragment::Lines {
        self.tor_edits
            .clone()
            .unwrap_or_else(|| facts.tunnels.settings.clone())
    }

    /// Puts a value into the sheet's copy and asks for it to be written.
    fn tor_put(
        &mut self,
        facts: &Facts<'_>,
        setting: &'static ronoctua_tunnel::options::Setting,
        values: Vec<String>,
    ) -> Errand {
        let mut lines = self.tor_lines(facts);
        setting.put(&mut lines, values);
        self.tor_edits = Some(lines.clone());
        Errand::TorConfigure(Box::new(lines))
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
    ///
    /// Answers whether the cursor was ALREADY on that row: that is the second
    /// click, which the door turns into `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(|r| !r.heading) {
            let again = self.cursor == n;
            self.cursor = n;
            return again;
        }
        false
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
    ///
    /// THE WIDTH IS THE DOOR'S. A key acts on the row under the cursor and the
    /// cursor is an index into the rows that were DRAWN, so the two have to be
    /// the same list — and since a note wraps (`chrome::note_lines`), how many
    /// rows there are is a function of the width.
    #[must_use]
    pub fn key(
        &mut self,
        press: Press,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let net = facts.net;
        let rows = self.rows(facts, columns, texts);
        self.settle(&rows);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        let disabled = rows.get(self.cursor).is_some_and(|r| r.disabled);
        match press {
            Press::Up | Press::Down => {
                let step: isize = if press == Press::Down { 1 } else { -1 };
                // AT THE EDGE OF A SHELF, THE SHELF MOVES. A shelf draws a
                // fixed number of rows out of however many it holds, so
                // walking off its end has to bring the next one INTO it —
                // otherwise the rows the ceiling hides are unreachable by
                // keyboard, which is half a list.
                if self.scrolled_the_shelf(&rows, step) {
                    return Errand::Nothing;
                }
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
            Press::Section | Press::SectionBack => {
                // TO THE SECTION, AND THE SECTION TO THE TOP. Nine sections in
                // one list is a lot of `<down>`, and landing on a row whose
                // title is off the screen above it is landing nowhere: the
                // anchor is what makes `TAB` show the section rather than one
                // row of it.
                let titles: Vec<usize> = rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.heading && !r.fields.is_empty())
                    .map(|(i, _)| i)
                    .collect();
                let Some(count) = std::num::NonZeroUsize::new(titles.len()) else {
                    return Errand::Nothing;
                };
                let here = Self::title_above(&rows, self.cursor)
                    .and_then(|t| titles.iter().position(|i| *i == t));
                let step: isize = if press == Press::Section { 1 } else { -1 };
                // A RING, like every other `TAB` in this system.
                // WITH NO TITLE ABOVE THE CURSOR, the ring starts at the end
                // it is walking towards — the first section going forwards,
                // the last going back. `map_or(0, …)` sent both to the first.
                let next = here.map_or(if step > 0 { 0 } else { count.get() - 1 }, |a| {
                    let n = isize::try_from(count.get()).unwrap_or(1);
                    usize::try_from((isize::try_from(a).unwrap_or(0) + step).rem_euclid(n))
                        .unwrap_or(0)
                });
                let title = titles[next];
                self.anchor = Some(title);
                // THE CURSOR GOES TO THE FIRST ROW OF THE SECTION, never to
                // its title: `RET` on a title does nothing, which the power
                // view's first screen proofs already showed.
                if let Some(i) = rows.iter().skip(title).position(|r| !r.heading) {
                    self.cursor = title + i;
                }
                Errand::Nothing
            }
            // ESCAPE WALKS BACK ONE LEVEL, and a file waiting to be brought
            // in is a level: closing the whole view on it would throw away
            // the reading without saying so.
            // ONE LEVEL BACK, and only then the view. It is the rule every
            // levelled view in this system follows, and the reason `Esc` is in
            // the sheet's hint: without it the only way out of thirty-four
            // settings would be to close the whole thing.
            // `S-<arrows>` CHANGE THE VALUE. Nothing else in this view has
            // one to change, so they are inert on the list — which is better
            // than borrowing them for something, since a key that means two
            // things is a key nobody trusts.
            Press::More | Press::Less => match target {
                Target::TorSetting(id) => self.tor_key(facts, id, press == Press::More),
                _ => Errand::Nothing,
            },
            Press::Escape if self.level == Level::Tor => {
                self.level = Level::List;
                self.cursor = 0;
                // WHAT WAS TURNED IS ALREADY WRITTEN. Each key sent its own
                // order, so leaving does not discard anything — and the copy
                // goes, because the next reading of the file is the truth.
                self.tor_edits = None;
                Errand::Nothing
            }
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
                // AN INFORMATIVE ROW COPIES ITSELF.
                //
                // `RET` here used to be the one key on the one row that did
                // nothing, in a list where half of what is drawn is a thing
                // somebody wants to paste. See `chrome::information_of`.
                Target::Nothing => rows
                    .get(self.cursor)
                    .and_then(|r| crate::chrome::information_of(&r.fields))
                    .map_or(Errand::Nothing, Errand::Copy),
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
                // INTO THE SHEET. The cursor starts at the top of it, and
                // not where it happened to be in the list.
                Target::TorSheet => {
                    self.level = Level::Tor;
                    self.cursor = 0;
                    self.tor_edits = None;
                    Errand::Nothing
                }
                Target::TorDrawer => {
                    self.drawer = !self.drawer;
                    Errand::Nothing
                }
                Target::TorAutostart(on) => Errand::TorAutostart(on),
                // `RET` TURNS WHAT CAN BE TURNED AND OPENS WHAT MUST BE TYPED.
                // A ring and a switch have somewhere to go; a bridge line has
                // to be typed, and the only place anything is typed in this
                // system is the footer's minibuffer.
                Target::TorSetting(id) => self.tor_key(facts, id, true),
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
            shelf: None,
            offsets: &self.shelves,
        };

        // TOR'S SHEET IS A SCREEN OF ITS OWN, and it returns here: everything
        // below is the LIST, and drawing both would be a sheet with ten
        // sections of network underneath it.
        if self.level == Level::Tor {
            self.tor_sheet_rows(&mut b, facts);
            return b.out;
        }

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
            // WHAT THE RADIO IS DOING, for the row of the network it is doing it
            // with: `active_one` says "this is the one in use", which is true
            // while the link is still authenticating and asking for an address.
            let radio_state = net
                .links
                .iter()
                .find(|l| l.medium == Medium::Radio)
                .map(|l| l.state);
            // THE NETWORKS IN RANGE, ON A SHELF. Every one of them is listed
            // — the daemon's reading is deduplicated by NAME, keeping the
            // strongest access point of each, and nothing is cut — and the
            // shelf is what stops the list from moving the page every time a
            // neighbour's router answers or goes quiet.
            //
            // THE EMPTY SENTENCE GOES INSIDE THE SHELF, and that is the whole
            // difference between a shelf that keeps its promise and one that
            // nearly does: with the sentence outside, nought networks drew one
            // line and one network drew six, so everything under the Wi-Fi
            // moved five lines every time a scan came back empty and refilled.
            // Measured, and the first version of the test asserted the jump
            // instead of forbidding it.
            b.shelf(Shelf::Wifi, |b| {
                if listed.is_empty() {
                    // THREE DIFFERENT EMPTIES, and not one generic "no
                    // networks": no radio, radio off and nothing in range have
                    // different fixes — the first has none, the second is one
                    // key, the third is a walk to another room. One sentence
                    // for all three would send the person looking in the wrong
                    // place.
                    b.note(if !has_radio(net) {
                        key::NETWORK_NO_ADAPTER
                    } else if net.radio {
                        key::NETWORK_EMPTY
                    } else {
                        key::NETWORK_RADIO_OFF
                    });
                }
                for one in listed {
                    Self::network_row(b, one, radio_state);
                }
            });

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

            // 3b. THE ONES THIS MACHINE REMEMBERS, which were nowhere at all.
            //
            // A saved network OUT OF RANGE existed in no list: the only place
            // `saved` appeared was as a word beside a network the radio could
            // see. So the answer to "which Wi-Fi does this machine know?" was
            // "the ones you happen to be near", and forgetting one you are not
            // near was impossible.
            //
            // AFTER the hotspot rows, and not between them and the Wi-Fi list:
            // a heading inserted in the middle of a section takes everything
            // below it, and "share this connection" read as part of the
            // remembered list.
            //
            // AND WITHOUT A RADIO TOO. The profiles are still there when the
            // adapter is out or its driver is reloading, and that is exactly
            // when somebody wants to look at them.
            b.heading(key::NETWORK_HEADING_REMEMBERED);
            let remembered: Vec<&Saved> = net
                .saved
                .iter()
                .filter(|p| p.kind == WIRELESS)
                // BY THE NETWORK IT JOINS, and never by its NAME. `id` is a
                // name somebody typed and NetworkManager lets it be anything:
                // a profile renamed to `home` still joins `NET_2G4A1`, and the
                // daemon calls a second profile for one network `MyWifi 1`.
                // Matched by `id`, the same network was listed twice — once in
                // range and once out of it, which is a lie about the machine.
                .filter(|p| {
                    let joins = p.ssid.as_deref().unwrap_or(&p.id);
                    !listed.iter().any(|n| n.ssid == joins)
                })
                .collect();
            b.shelf(Shelf::Remembered, |b| {
                if remembered.is_empty() {
                    b.note(key::NETWORK_NO_REMEMBERED);
                }
                for p in &remembered {
                    b.pair(
                        format!(
                            "{} {}",
                            tokens::icon::NETWORK_SECURED,
                            p.ssid.as_deref().unwrap_or(&p.id)
                        ),
                        texts.t(key::NETWORK_OUT_OF_RANGE),
                        Target::Saved(p.at.clone()),
                        // THE AEROPLANE AND THE RADIO BOTH REFUSE IT, and they
                        // are two independent properties: `airplane()` is the
                        // daemon's `NetworkingEnabled` and `radio` is its
                        // `WirelessEnabled`, so in the aeroplane the radio flag
                        // stays true. The row says so by being dim rather than
                        // by sending an order NetworkManager will refuse with a
                        // sentence nobody wrote.
                        flying || !net.radio || !has_radio(net),
                    );
                }
            });

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

        // 6. TOR — AND THIS ONE IS DRAWN WHETHER IT IS RUNNING OR NOT.
        //
        // It used to be `if let Some(scene)`, and the control socket only
        // answers when Tor is already up. So a stopped Tor had no heading, no
        // row and no key: there was no way in the whole shell to turn it on,
        // and the screen did not say the program existed. The owner reported
        // exactly that. What decides now is the UNIT's standing, which is a
        // different question and the one this section is about.
        if facts.tunnels.standing.actionable() || facts.tunnels.tor.is_some() {
            b.heading(key::NETWORK_HEADING_TOR);
            Self::tor_rows(&mut b, facts);
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
            b.fact(key::NETWORK_IMPORT_ADDRESS, address.clone());
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
            // ONE LABEL FOR TWO FAMILIES SAYS NOTHING. The tailnet hands out
            // a v4 and a v6, and two rows reading `address on the tailnet`
            // leave the reader to work out which is which from the shape of
            // the number.
            b.fact(
                if address.contains(':') {
                    key::NETWORK_FACT_ADDRESS_TAILNET6
                } else {
                    key::NETWORK_FACT_ADDRESS_TAILNET
                },
                address.clone(),
            );
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
        // ON A SHELF: a tailnet is every machine somebody owns, and a laptop
        // that wakes up moves the whole list.
        b.shelf(Shelf::Peers, |b| {
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
        });
    }

    /// Tor: how far it got, what it has, and the two things to do to it.
    /// Tor, in the LIST: what it is doing, the switch, and the way in.
    ///
    /// THE SWITCH COMES FROM THE UNIT AND THE WORDS FROM THE CIRCUIT. They are
    /// different facts, and reading one as the other has now been a defect
    /// twice. First the circuit was read as the switch, so `!established` sent
    /// `systemctl start` to a unit already up. The fix made the row
    /// stop-ONLY — a literal `false` — and that was the second defect: with
    /// the section itself only drawn when Tor answered, `Order::Tor(true)` was
    /// reachable from nowhere in the shell. `Standing` is the fact that was
    /// missing both times.
    fn tor_rows(b: &mut Builder<'_>, facts: &Facts<'_>) {
        use ronoctua_i18n::key;
        let standing = facts.tunnels.standing;
        let scene = facts.tunnels.tor.as_ref();

        // WHAT THE UNIT IS, IN ITS OWN WORD. Five standings, five sentences:
        // a machine with no Tor installed and a machine whose Tor is off are
        // not the same thing, and neither is a sandbox that cannot ask.
        let said = b.texts.t(match standing {
            ronoctua_tunnel::tor::Standing::Running => key::NETWORK_TOR_RUNNING,
            ronoctua_tunnel::tor::Standing::Stopped => key::NETWORK_TOR_STOPPED,
            ronoctua_tunnel::tor::Standing::Moving => key::NETWORK_TOR_MOVING,
            ronoctua_tunnel::tor::Standing::Absent => key::NETWORK_TOR_ABSENT,
            ronoctua_tunnel::tor::Standing::Unknown => key::NETWORK_TOR_UNKNOWN,
        });
        if standing.actionable() {
            b.pair(
                b.texts.t(key::NETWORK_TOR_STATE),
                said,
                Target::Tunnels(Switch::Tor, standing.starts()),
                // MOVING REFUSES. A unit halfway up takes a `start` and does
                // nothing with it, and the screen would show a key that
                // appeared to fail.
                standing == ronoctua_tunnel::tor::Standing::Moving,
            );
            // WHAT THE KEY WOULD DO, in words, under the state it would change.
            b.note(if standing.starts() {
                key::NETWORK_TOR_START
            } else {
                key::NETWORK_TOR_STOP
            });
        } else {
            // NOT INSTALLED, OR UNASKABLE. A row that acts would be a key
            // about a program that is not there.
            b.fact(key::NETWORK_TOR_STATE, said);
        }

        if let Some(t) = scene {
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
                // only string on this screen that is not ours, and translating
                // it is not possible: Tor writes forty of them and they change
                // with its releases. A sentence that says "starting, 45%"
                // beside it is what carries the meaning; the summary is the
                // detail, in the words the person will find in Tor's own log.
                b.pair(said, t.summary.clone(), Target::Nothing, false);
            } else {
                b.fact(key::NETWORK_TOR_STATE, b.texts.t(key::NETWORK_TOR_READY));
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
            b.fact(
                key::NETWORK_FACT_RECEIVED,
                readable_bytes(t.received, b.texts.decimal_mark()),
            );
            b.fact(
                key::NETWORK_FACT_SENT,
                readable_bytes(t.sent, b.texts.decimal_mark()),
            );
        }

        // THE SOCKS ADDRESS, and it comes from the FLAKE now. It used to be a
        // literal in the crate against a module that declared no `SOCKSPort`
        // at all: the two agreed because the nixpkgs default happened to be
        // the same number, and the day either moved this row would have named
        // a port nothing listened on.
        b.fact(
            key::NETWORK_FACT_SOCKS,
            ronoctua_tunnel::tor::SOCKS.to_owned(),
        );

        if scene.is_some() {
            b.pair(
                b.texts.t(key::NETWORK_TOR_IDENTITY),
                String::new(),
                Target::Tunnels(Switch::Identity, true),
                !scene.is_some_and(|t| t.established),
            );
        }

        // THE WAY INTO THE SHEET, and it is here rather than on the heading
        // because a heading is not a row the cursor stops on. It is offered
        // with Tor stopped too, and deliberately: bridges are configured
        // BEFORE the first start, since without them a censored network never
        // bootstraps at all.
        b.pair(
            b.texts.t(key::NETWORK_TOR_SETTINGS),
            String::new(),
            Target::TorSheet,
            !standing.actionable(),
        );
    }

    /// Tor's own sheet — the second screen of this view.
    ///
    /// `Esc` goes back to the list. Everything here writes into the torrc
    /// fragment this system owns; what `nix/modules/tor.nix` declares is drawn
    /// REFUSING, because the generated torrc reads `%include` first and
    /// overrides it — see `assets/tor-options.nix`.
    fn tor_sheet_rows(&self, b: &mut Builder<'_>, facts: &Facts<'_>) {
        use ronoctua_i18n::key;
        use ronoctua_tunnel::options::{self, Scope};
        let lines = self.tor_lines(facts);
        let standing = facts.tunnels.standing;

        b.heading(key::NETWORK_TOR_HEADING_SERVICE);
        let said = b.texts.t(match standing {
            ronoctua_tunnel::tor::Standing::Running => key::NETWORK_TOR_RUNNING,
            ronoctua_tunnel::tor::Standing::Stopped => key::NETWORK_TOR_STOPPED,
            ronoctua_tunnel::tor::Standing::Moving => key::NETWORK_TOR_MOVING,
            ronoctua_tunnel::tor::Standing::Absent => key::NETWORK_TOR_ABSENT,
            ronoctua_tunnel::tor::Standing::Unknown => key::NETWORK_TOR_UNKNOWN,
        });
        b.pair(
            b.texts.t(key::NETWORK_TOR_STATE),
            said,
            Target::Tunnels(Switch::Tor, standing.starts()),
            !standing.actionable() || standing == ronoctua_tunnel::tor::Standing::Moving,
        );
        // START IT WITH THE SESSION, and it is OURS rather than systemd's.
        // `systemctl enable` would need a third verb in the polkit rule, and
        // `tor-polkit-is-narrow` refuses one with a reason: `enable` survives
        // a reboot, which is a different kind of power from starting something
        // now. So the footer reads a preference when the session comes up.
        let autostart = facts.tunnels.autostart;
        b.pair(
            b.texts.t(key::NETWORK_TOR_AUTOSTART),
            // ON OR OFF, not what the unit is doing — the sibling of the
            // defect in `tor_setting_row`, and this one was PHOTOGRAPHED: the
            // sheet read "start it with the session   stopped", one row under
            // "the Tor service   stopped", which are two different facts
            // wearing one word.
            b.texts.t(if autostart {
                key::NETWORK_ON
            } else {
                key::NETWORK_OFF
            }),
            Target::TorAutostart(!autostart),
            false,
        );
        for setting in options::of_scope(Scope::Service) {
            self.tor_setting_row(b, setting, &lines);
        }

        for (heading, scope) in [
            (key::NETWORK_TOR_HEADING_PATH, Scope::Path),
            (key::NETWORK_TOR_HEADING_BRIDGES, Scope::Bridges),
            (key::NETWORK_TOR_HEADING_ONION, Scope::Onion),
        ] {
            let plain: Vec<_> = options::of_scope(scope).filter(|s| !s.advanced).collect();
            if plain.is_empty() {
                continue;
            }
            b.heading(heading);
            for setting in plain {
                self.tor_setting_row(b, setting, &lines);
            }
        }

        // THE DRAWER, and the chevron is its and nobody else's — the owner
        // refused one on a row that acts, saying it is what collapse and
        // expand use.
        b.drawer(key::NETWORK_TOR_HEADING_ADVANCED, self.drawer);
        if !self.drawer {
            return;
        }
        // ONE DRAWER FOR ALL OF THEM, whatever scope declared them. The
        // scopes are the sheet's SHAPE and `advanced` is how often a thing is
        // touched; four drawers of three rows each would be four chevrons to
        // open before finding anything.
        b.note(key::NETWORK_TOR_ADVANCED_ABOUT);
        for setting in options::SETTINGS.iter().filter(|s| s.advanced) {
            self.tor_setting_row(b, setting, &lines);
        }
    }

    /// One declared Tor setting's row.
    fn tor_setting_row(
        &self,
        b: &mut Builder<'_>,
        setting: &'static ronoctua_tunnel::options::Setting,
        lines: &ronoctua_tunnel::tor::fragment::Lines,
    ) {
        use ronoctua_options::Kind;
        let label = b.texts.t(setting.label);
        // WHAT THE FLAKE OWNS SAYS SO INSTEAD OF SAYING ITS VALUE. The value
        // in the fragment is not what Tor is using, because the generated
        // torrc overrides it — so printing it would be printing a number that
        // is not in force.
        if setting.fixed {
            b.pair(
                label,
                b.texts.t(ronoctua_i18n::key::NETWORK_TOR_FROM_FLAKE),
                Target::TorSetting(setting.id),
                true,
            );
            return;
        }
        let value = match setting.kind {
            // A SWITCH SAYS ON OR OFF, not what the SERVICE is doing. These
            // two read `network-tor-running`/`-stopped`, which belong to the
            // unit: fourteen declared switches drew "use bridges   running".
            //
            // AND THEY ARE THIS VIEW'S OWN WORDS, not a new pair: a second key
            // saying "on" in the same file is what
            // `labels-of-a-view-are-distinct` refuses, and it is right to —
            // two keys drift, and then one row says "on" while another says
            // "enabled" about the same kind of thing.
            Kind::Switch => b.texts.t(if setting.is_on(lines) {
                ronoctua_i18n::key::NETWORK_ON
            } else {
                ronoctua_i18n::key::NETWORK_OFF
            }),
            Kind::Ring(_) => setting
                .word(lines)
                .map(|k| b.texts.t(&k))
                .unwrap_or_default(),
            Kind::Number { unit, .. } => {
                let now = setting.one(lines).unwrap_or("").to_owned();
                match (now.is_empty(), unit) {
                    (true, _) => b.texts.t(ronoctua_i18n::key::NETWORK_TOR_FROM_FLAKE),
                    (false, Some(u)) => format!("{now} {}", b.texts.t(u)),
                    (false, None) => now,
                }
            }
            // A LIST SAYS HOW MANY, not all of them: three bridge lines are
            // sixty characters each and the row is one line. The count is what
            // a person checks at a glance, and `RET` is what shows them.
            Kind::List => {
                let held = setting.read(lines).map_or(0, Vec::len);
                held.to_string()
            }
            Kind::Text | Kind::Amount => setting.one(lines).unwrap_or("").to_owned(),
        };
        b.pair(label, value, Target::TorSetting(setting.id), false);
    }

    /// How many established sockets of ours are read before the rest become a
    /// sentence.
    ///
    /// A browser holds hundreds, and reading all of them into rows to throw
    /// most away is work for nothing. The SHELF is what decides how many are
    /// on screen; this is the ceiling on how many are counted at all, and the
    /// sentence under it says how many were left out.
    const ESTABLISHED: usize = 10;

    /// What is listening, and what of ours is talking.
    fn port_rows(b: &mut Builder<'_>, watch: &Watch) {
        use ronoctua_i18n::key;
        let listening: Vec<&Socket> = watch.sockets().iter().filter(|s| s.listening).collect();
        let talking: Vec<&Socket> = watch
            .sockets()
            .iter()
            .filter(|s| !s.listening && s.owner.is_some())
            .collect();
        let hidden = talking.len().saturating_sub(Self::ESTABLISHED);
        // ONE SHELF FOR BOTH HALVES, and that is what they are on screen: what
        // this machine answers on and what it is talking to run one into the
        // other with no heading between them. Two shelves of five made a block
        // of ten — twice the height of every other one, for a section that
        // reads as a single list.
        //
        // What a machine is listening on changes as programs come and go, and
        // what it is talking to changes faster still: a browser opens and
        // closes connections while nobody touches anything, and a list free to
        // grow moved everything under it.
        b.shelf(Shelf::Ports, |b| {
            if listening.is_empty() && talking.is_empty() {
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
        });
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
        // ON A SHELF, and this machine sits on it too: the neighbour table
        // answers a little differently every second — an entry goes stale, a
        // sweep fills it — and a list free to grow moved the two sections
        // under it every time.
        b.shelf(Shelf::Neighbours, |b| {
            // AND THIS MACHINE FIRST, not after the neighbours: the shelf shows
            // six rows and the seventh neighbour would push the machine's own
            // address off it — in a section whose stated point is that a list
            // of the network without the machine reading it has a hole in it.
            for address in &mine {
                b.pair(
                    address.clone(),
                    b.texts.t(key::NETWORK_LAN_THIS_MACHINE),
                    Target::Nothing,
                    false,
                );
            }
            // THE NOTE IS ABOUT THE NEIGHBOURS, and it goes inside the shelf
            // for the reason every empty sentence does — outside it, the
            // section moved by five lines the first time anybody answered.
            // It said "nobody has been spoken to yet" directly above a shelf
            // listing this machine, which is why it now waits for BOTH to be
            // empty.
            if fresh.is_empty() && mine.is_empty() {
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
        });
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
            b.note(key::NETWORK_NO_TRAFFIC);
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
            b.fact(
                key::NETWORK_FACT_RECEIVED_BOOT,
                readable_bytes(c.received, b.texts.decimal_mark()),
            );
            b.fact(
                key::NETWORK_FACT_SENT_BOOT,
                readable_bytes(c.sent, b.texts.decimal_mark()),
            );
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
                key::NETWORK_STATE_CONNECTING
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
    /// Which shelf is being filled, if any. The TOTAL is stamped on when the
    /// shelf closes, which is the first moment anybody knows it.
    shelf: Option<Shelf>,
    /// Where each shelf is scrolled to.
    offsets: &'a std::collections::BTreeMap<Shelf, usize>,
}

impl Builder<'_> {
    /// Fills a shelf, and closes it at exactly [`SHELF_LINES`] rows.
    ///
    /// # Why the rows are built and then CUT
    ///
    /// Because `focused` is decided by where a row lands in the final list,
    /// and a shelf that emitted only its visible slice would have to know that
    /// before it knew how many rows came before it. Building all of them into
    /// a side list and taking the window afterwards keeps one coordinate
    /// system: the cursor counts DRAWN rows, which is what the click, the
    /// scrollbar and both doors already count.
    ///
    /// AND IT ALWAYS CLOSES AT THE SAME HEIGHT, padding with air. That is the
    /// whole point: the rows under a shelf do not move when its content
    /// changes.
    fn shelf<F: FnOnce(&mut Self)>(&mut self, which: Shelf, fill: F) {
        let before = std::mem::take(&mut self.out);
        let was = self.shelf.replace(which);
        // AND THE ROWS INSIDE IT ARE THE CONTAINER'S WIDTH, not the surface's.
        //
        // A container has a right edge of its own and a bar sitting on it
        // (`chrome::shelf_box`), so a row that went on being cut at the
        // surface's edge would run its value out of the box and under that
        // bar. It is the same reason the manager's panels each cut to their
        // own section rather than to the window.
        let inner = crate::chrome::shelf_box(self.left, self.room).room;
        let full_room = std::mem::replace(&mut self.room, inner);
        // NO ROW INSIDE THE SHELF DECIDES IT IS FOCUSED.
        //
        // The focus is a COLOUR, not a flag — neither door reads `Row.focused`,
        // both read the fields — and the colour is baked where the row is
        // built, from `cursor == out.len()`. Inside here `out` is an empty side
        // list, so that index counts from zero and has nothing to do with where
        // the row will land. Left alone it painted the accent on whichever row
        // of the shelf happened to sit at the cursor's NUMBER, which for the
        // Wi-Fi shelf (starting around row twelve) meant the highlight simply
        // vanished when the cursor walked into it.
        //
        // A cursor no row can match, then, and the accent applied on the way
        // out — where the row's real position is known.
        let held_cursor = std::mem::replace(&mut self.cursor, usize::MAX);
        fill(self);
        self.cursor = held_cursor;
        self.room = full_room;
        let held = std::mem::replace(&mut self.out, before);
        self.shelf = was;
        let total = held.len();
        // NOTHING TO SCROLL, NOTHING TO PAD: a shelf that never fills is a
        // block of blank lines under a heading, which is worse than a short
        // list. It takes its ceiling only once it has reached it.
        let offset = if total > SHELF_LINES {
            (*self.offsets.get(&which).unwrap_or(&0)).min(total - SHELF_LINES)
        } else {
            0
        };
        let shown = held.into_iter().skip(offset).take(SHELF_LINES);
        let mut drawn = 0;
        for row in shown {
            // THE FOCUS IS DECIDED HERE, where the row's real position is
            // known — and it is applied to the FIELDS, because that is what a
            // door draws. `Row::focused` is carried for the tests.
            let focused = self.cursor == self.out.len();
            let mut row = row;
            if focused {
                for f in &mut row.fields {
                    f.color = tokens::color::STATE_ACCENT;
                }
            }
            row.focused = focused;
            row.shelf = Some((which, total));
            self.out.push(row);
            drawn += 1;
        }
        // A SHELF WITH ANYTHING ON IT IS ALWAYS ITS FULL HEIGHT. Padding only
        // up to what it holds is the defect back again: a Wi-Fi list going
        // from one network to six moves everything under it by five lines,
        // which is exactly what the owner reported. An EMPTY one draws
        // nothing at all — the section says why in a sentence of its own,
        // and a block of blank lines under a heading says nothing.
        let height = if total == 0 { 0 } else { SHELF_LINES };
        for _ in drawn..height {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                disabled: false,
                shelf: Some((which, total)),
            });
        }
    }

    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    /// `1,2 GB/s`, in the person's language.
    fn per_second(&self, bytes: u64) -> String {
        self.texts.t_with(
            ronoctua_i18n::key::NETWORK_PER_SECOND,
            &[("bytes", &readable_bytes(bytes, self.texts.decimal_mark()))],
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
        // A TITLE IS NEVER INSIDE A SHELF. The shelf clips and pads by
        // counting rows, and a title among them would be counted as content,
        // stamped with the shelf on the way out and then treated as PADDING by
        // the arrow at the shelf's edge — a section that scrolls its own name.
        debug_assert!(
            self.shelf.is_none(),
            "a section title was opened inside a shelf"
        );
        // AIR FIRST, and never before the first title. See `chrome::AIR`: a
        // section that starts immediately under the last row of the one above
        // does not start anywhere, and the owner read that off the screen.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                disabled: false,
                shelf: None,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                self.texts.t(key),
                tokens::color::TEXT_HEADING,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
            shelf: None,
        });
    }

    /// A sentence that stands for a whole section — "no networks in range".
    /// The drawer's own line: a chevron, and the word.
    ///
    /// THE CHEVRON IS THE DRAWER'S AND NOBODY ELSE'S. The owner refused one on
    /// a row that acts, saying it is what collapse and expand use; by that
    /// argument it belongs here, and this is the only place in this view that
    /// draws one.
    fn drawer(&mut self, key: &str, open: bool) {
        let focused = self.cursor == self.out.len();
        let glyph = if open {
            tokens::icon::COLLAPSE
        } else {
            tokens::icon::EXPAND
        };
        let colour = if focused {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_HEADING
        };
        // THE GLYPH IS ONE CELL AND THE WORD STARTS AFTER IT, with a space
        // between: a mark against the letters is the "glifo encostado na
        // borda" the owner has had to point out before.
        self.out.push(Row {
            fields: vec![
                Self::piece(self.left, glyph.to_string(), colour),
                Self::piece(
                    self.left + 2,
                    layout::elide_end(&self.texts.t(key), self.room.saturating_sub(2)),
                    colour,
                ),
            ],
            focused,
            heading: false,
            target: Target::TorDrawer,
            disabled: false,
            shelf: None,
        });
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    /// The same, for a sentence that carries a number.
    fn sentence(&mut self, text: String) {
        self.sentence_coloured(text, tokens::color::TEXT_DIM);
    }

    /// A sentence in a colour of its own — a warning that is not a row.
    ///
    /// # IT WRAPS, and it used to be cut
    ///
    /// One `Row` per line, through the one function the eight views of this
    /// shape share — see [`crate::chrome::note_lines`]. This view has the
    /// longest of these sentences in the system: what a VPN file's
    /// `AllowedIPs` and `DNS` lines will do to the machine is read BEFORE the
    /// `RET`, and an ellipsis took the half that says what happens.
    ///
    /// # AND EVERY LINE IS THE SAME KIND OF ROW THE NOTE WAS
    ///
    /// The argument `power.rs::sentence` writes out in full: `RET` on an
    /// informative row copies what it says, so a line of a note is a line
    /// somebody can take, and a section whose only content is a note would
    /// leave its heading with nothing under it.
    ///
    /// It is also what keeps a note inside a SHELF whole. `Builder::shelf`
    /// paints the accent on whichever row sits at the cursor's number, so the
    /// line the cursor is on is the line that lights — where rows the cursor
    /// skipped would have left half a sentence accent and half of it dim.
    fn sentence_coloured(&mut self, text: String, colour: tokens::Color) {
        for line in crate::chrome::note_lines(&text, self.room) {
            let focused = self.cursor == self.out.len();
            self.out.push(Row {
                fields: vec![Self::piece(
                    // A SENTENCE LINES UP WITH THE LABELS, which since the
                    // mark went is where the titles are too — a section's
                    // title and its rows share one left edge now.
                    self.left,
                    line,
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
                shelf: self.shelf.map(|which| (which, 0)),
            });
        }
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

    /// The one that draws them all — and the placement is
    /// [`crate::chrome::field_pair`], which is the same one the power and
    /// Bluetooth views draw with.
    ///
    /// IT USED TO BE WRITTEN HERE, and in each of the other two, and the three
    /// had already drifted: this one pushed a short value to the last cell of
    /// the surface, which on a wide window is a hundred and fifty columns from
    /// its label with nothing in between. See the comment on `value_column`.
    fn coloured(
        &mut self,
        label: String,
        value: String,
        target: Target,
        disabled: bool,
        colour: Option<tokens::Color>,
    ) {
        let focused = self.cursor == self.out.len();
        // A LIST ITEM READS THE OTHER WAY ROUND FROM A FACT, and only in the
        // VALUE.
        //
        // The owner asked for it: *"as listas, como redes WiFi ou Tailnet,
        // devem ter cores diferentes do resto"*. A fact's value is the answer
        // to the question its label asks, so it is the strong half; a list
        // item's is a description of a thing whose NAME is already on the
        // line, so it is the weak one.
        //
        // The LABEL is not in that trade at all any more — it says whether the
        // row is a door, in a hue no value wears. See `colors.nix`, `label`.
        let listed = self.shelf.is_some();
        let acts = target != Target::Nothing;
        let kind = match (disabled, acts, listed) {
            (true, _, _) => crate::chrome::Kind::Disabled,
            (false, true, false) => crate::chrome::Kind::Setting,
            (false, true, true) => crate::chrome::Kind::ListedSetting,
            (false, false, false) => crate::chrome::Kind::Fact,
            (false, false, true) => crate::chrome::Kind::ListedFact,
        };
        self.out.push(Row {
            fields: crate::chrome::tinted(
                crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
                colour,
                focused,
            ),
            focused,
            heading: false,
            target,
            disabled,
            shelf: self.shelf.map(|which| (which, 0)),
        });
    }
}

/// `44220000000` -> `44,2 GB`.
///
/// One decimal place, like the link's rate: the total transferred grows all
/// the time, and the second place would be a digit flickering without
/// informing anything.
///
/// THE SEPARATOR IS THE LANGUAGE'S and comes in from
/// [`ronoctua_i18n::Texts::decimal_mark`]. It was a comma written into the
/// format string, so an English screen said `44,2 GB` — which in English is
/// forty-four thousand two hundred gigabytes.
#[must_use]
pub fn readable_bytes(bytes: u64, mark: char) -> String {
    const K: u64 = 1_000;
    const M: u64 = K * 1_000;
    const G: u64 = M * 1_000;
    match bytes {
        0..=999 => format!("{bytes} B"),
        1_000..=999_999 => format!("{} kB", bytes / K),
        1_000_000..=999_999_999 => format!("{} MB", bytes / M),
        _ => {
            let tenth = (bytes + G / 20) / (G / 10);
            format!("{}{mark}{} GB", tenth / 10, tenth % 10)
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
            ssid: None,
        }];
        let facts = facts(&snap, &[]);
        for (notation, id, _) in OF_THE_VIEW {
            let Some(press) = crate::network_words::press_of(id) else {
                panic!("`{id}` is taught and resolves to no press");
            };
            // ON EVERY ROW, and it has to work on at least ONE. A row key is
            // honest about the row it is on — `RET` on a fact does nothing,
            // and should — so the property is "somewhere on this screen", not
            // "wherever the cursor happened to land".
            // ON THE LEVEL THE KEY BELONGS TO. `S-<right>` turns a Tor
            // setting and does nothing in the list, which is not a defect —
            // it is what `Where` says. Asking only about the list would fail
            // every key of the sheet, and asking about neither would let a
            // dead key through.
            let mut worked = false;
            for level in [Level::List, Level::Tor] {
                if !where_of(id).is_some_and(|w| w.shown(level)) {
                    continue;
                }
                let mut v = View::fresh();
                v.level = level;
                let rows = v.rows(&facts, 80, &t);
                for row in 0..rows.len() {
                    let mut here = v.clone();
                    here.clicked(row, &rows);
                    let was = here.cursor();
                    let did = here.key(press, &facts, DECIDING_WIDTH, &t);
                    worked |= did != Errand::Nothing || here.cursor() != was || here.level != level;
                }
            }
            assert!(
                worked,
                "`{notation}` ({id}) does nothing on any row of any level"
            );
        }
    }

    #[test]
    fn the_things_a_key_can_act_on_are_the_same_at_every_width() {
        // WHAT A KEY MAY NOT DEPEND ON.
        //
        // A key acts on the row under the cursor, and the cursor is an index
        // into the rows that were DRAWN — so `View::key` is given the DOOR's
        // width and decides against that same list. What this holds down is
        // the half that no width may change: WHICH things can be acted on, and
        // in what ORDER.
        //
        // The row COUNT is allowed to differ now, and that is not a weakening:
        // a note WRAPS (`chrome::note_lines`), so one sentence is a row on a
        // wide screen and three on a narrow one. It used to be cut instead,
        // which kept the count the same and took the end off every explanation
        // this view has. The FIELDS are allowed to differ too — that is what
        // eliding is.
        let t = texts();
        let snap = machine();
        let facts = facts(&snap, &[]);
        let want = |columns: u16| -> Vec<Target> {
            View::fresh()
                .rows(&facts, columns, &t)
                .into_iter()
                .map(|r| r.target)
                .filter(|t| *t != Target::Nothing)
                .collect()
        };
        let at_eighty = want(DECIDING_WIDTH);
        assert!(at_eighty.len() >= 5, "only {} acting rows", at_eighty.len());
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
                // THE AIR IS A HEADING WITH NO FIELDS (`chrome::AIR`), and
                // what it is under is the title it introduces.
                if !row.heading || row.fields.is_empty() {
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
            ssid: None,
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
        // THE HINT TEACHES THE LEVEL, and the count follows from that rather
        // than from the whole table: a piece is a key, a word and a separator
        // between them, so `3n - 1` for the `n` distinct ids that are shown
        // WHERE YOU ARE.
        let shown = |level: Level| {
            OF_THE_VIEW
                .iter()
                .filter(|(_, _, w)| w.shown(level))
                .map(|(_, id, _)| *id)
                .collect::<std::collections::BTreeSet<_>>()
        };
        let mut list = View::fresh();
        assert_eq!(list.hint(&t).len(), 3 * shown(Level::List).len() - 1);
        list.level = Level::Tor;
        assert_eq!(list.hint(&t).len(), 3 * shown(Level::Tor).len() - 1);
        // AND THE TWO ARE NOT THE SAME LINE. A sheet that went on teaching `s`
        // to scan and `f` to forget would be teaching keys that do nothing
        // where you are — which is what the owner photographed on the radio.
        assert_ne!(shown(Level::List), shown(Level::Tor));

        for (_, id, _) in OF_THE_VIEW {
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
        // EVERY FIELD OF THE ROW, and not the first: a row is a label and a
        // value and can carry a mark besides, so which index holds the padlock
        // is not something a test should be spelling out. (The comment here
        // used to name an `icon.row-acts` in front of the label; the owner
        // refused that mark and it exists in no token — the difference between
        // a row that acts and one that does not is COLOUR.)
        let label_of = |ssid: &str| {
            rows.iter()
                .find(|r| matches!(&r.target, Target::Wireless(n) if n.ssid == ssid))
                .map(|r| {
                    r.fields
                        .iter()
                        .map(|f| f.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
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
        assert!(words(&snap).contains(&t.t(ronoctua_i18n::key::NETWORK_STATE_CONNECTING)));
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
                ssid: None,
            },
            Saved {
                at: "/s/2".to_owned(),
                id: "a house".to_owned(),
                kind: "wireguard".to_owned(),
                ssid: None,
            },
            // AND A NETWORK IS NOT A TUNNEL: every profile the daemon has is
            // in the snapshot, and this section is about two of the words.
            Saved {
                at: "/s/3".to_owned(),
                id: "a café".to_owned(),
                kind: "802-11-wireless".to_owned(),
                ssid: Some("a café".to_owned()),
            },
        ];
        let rows = View::fresh().rows(&facts(&snap, &[]), DECIDING_WIDTH, &t);
        // THE TUNNELS' SECTION, and not every row that carries a profile: a
        // remembered Wi-Fi is a `Saved` too now (its own shelf, so that a
        // network one is not near can still be forgotten), and reading the
        // whole list would call that a tunnel.
        let title = t.t(ronoctua_i18n::key::NETWORK_HEADING_TUNNELS);
        let from = rows
            .iter()
            .position(|r| r.fields.first().is_some_and(|f| f.text == title))
            .expect("no tunnels section");
        let tunnels: Vec<&Target> = rows
            .iter()
            .skip(from + 1)
            .take_while(|r| !(r.heading && !r.fields.is_empty()))
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
        // AND THE WIRELESS PROFILE IS IN THE OTHER SECTION, not missing.
        assert!(
            rows.iter()
                .any(|r| r.target == Target::Saved("/s/3".to_owned())),
            "the remembered network went nowhere at all"
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
            ssid: None,
        }];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &[]), 80, &t);
        let i = rows
            .iter()
            .position(|r| r.target == Target::Saved("/s/1".to_owned()))
            .unwrap();
        assert!(rows[i].disabled);
        v.clicked(i, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Nothing
        );
    }

    #[test]
    fn forget_only_acts_on_what_can_be_forgotten() {
        let t = texts();
        let mut snap = machine();
        snap.saved = vec![Saved {
            at: "/s/1".to_owned(),
            id: "an office".to_owned(),
            kind: "vpn".to_owned(),
            ssid: None,
        }];
        let mut saved_one = network("a house", 80);
        saved_one.saved = true;
        let seen = vec![saved_one, network("a stranger", 40)];
        let mut v = View::fresh();
        let rows = v.rows(&facts(&snap, &seen), 80, &t);

        // ON A FACT, NOTHING. `f` over the way out has nothing to forget.
        assert_eq!(
            v.key(Press::Forget, &facts(&snap, &seen), DECIDING_WIDTH, &t),
            Errand::Nothing
        );

        let saved_row = rows
            .iter()
            .position(|r| matches!(&r.target, Target::Wireless(n) if n.saved))
            .unwrap();
        v.clicked(saved_row, &rows);
        assert_eq!(
            v.key(Press::Forget, &facts(&snap, &seen), DECIDING_WIDTH, &t),
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
            v.key(Press::Forget, &facts(&snap, &seen), DECIDING_WIDTH, &t),
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
            v.key(Press::Forget, &facts(&snap, &seen), DECIDING_WIDTH, &t),
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
                v.key(Press::Down, &facts(&snap, &seen), DECIDING_WIDTH, &t),
                Errand::Nothing
            );
            assert!(!rows[v.cursor()].heading, "it stopped on a heading");
        }
        assert_eq!(v.cursor(), rows.len() - 1, "it did not reach the end");
        for _ in 0..rows.len() * 2 {
            assert_eq!(
                v.key(Press::Up, &facts(&snap, &seen), DECIDING_WIDTH, &t),
                Errand::Nothing
            );
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
        assert_eq!(
            v.key(Press::Escape, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Close
        );
        assert_eq!(
            v.key(Press::Radio, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Radio(false)
        );
        snap.radio = false;
        assert_eq!(
            v.key(Press::Radio, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Radio(true)
        );
        assert_eq!(
            v.key(Press::Airplane, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Airplane(true)
        );
        assert_eq!(
            v.key(Press::Disconnect, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Disconnect(Medium::Wired)
        );
        assert_eq!(
            v.key(Press::Scan, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Scan
        );
        assert_eq!(
            v.key(Press::Facts, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Facts
        );
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
            v.focused_network(&facts(&snap, &seen), DECIDING_WIDTH, &t)
                .map(|n| n.ssid),
            Some("a café".to_owned())
        );
        assert_eq!(
            v.key(Press::Enter, &facts(&snap, &seen), DECIDING_WIDTH, &t),
            Errand::Connect(Box::new(seen[1].clone()))
        );
    }

    #[test]
    fn the_cursor_is_visible_wherever_it_stands_inside_a_shelf() {
        // THE FOCUS IS A COLOUR, not a flag: neither door reads
        // `Row::focused`, both read the fields. A shelf builds its rows into a
        // side list that starts at zero, so a row that baked its colour from
        // `cursor == out.len()` there painted the accent on whichever row of
        // the shelf carried the cursor's NUMBER — which for a shelf starting
        // at row twelve is none of them, and the highlight vanished.
        let t = texts();
        let snap = machine();
        let seen: Vec<Network> = (0..20)
            .map(|i| network(&format!("net {i}"), 90 - u8::try_from(i).unwrap_or(0)))
            .collect();
        let f = facts(&snap, &seen);
        let mut v = View::fresh();
        let rows = v.rows(&f, DECIDING_WIDTH, &t);
        let first = rows
            .iter()
            .position(|r| r.shelf.map(|(s, _)| s) == Some(Shelf::Wifi) && !r.heading)
            .expect("the shelf drew nothing");
        v.clicked(first, &rows);

        let accented = |v: &View| -> Vec<usize> {
            v.rows(&f, DECIDING_WIDTH, &t)
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    r.fields
                        .iter()
                        .any(|x| x.color == tokens::color::STATE_ACCENT)
                })
                .map(|(i, _)| i)
                .collect()
        };
        // Walk the whole shelf, including the part that only exists after it
        // scrolls, and ask every step which row is lit.
        for step in 0..seen.len() + 4 {
            let lit = accented(&v);
            assert_eq!(
                lit,
                vec![v.cursor()],
                "step {step}: the cursor is at {} and the accent is on {lit:?}",
                v.cursor()
            );
            let _ = v.key(Press::Down, &f, DECIDING_WIDTH, &t);
        }
    }

    #[test]
    fn a_list_item_and_a_fact_do_not_read_the_same() {
        // The owner asked for the difference twice, and the second time moved
        // it: a fact's ANSWER is the strong half and a list item's DESCRIPTION
        // is the weak one — the two VALUES, on the neutral ramp. The LABEL is
        // out of that trade entirely now; it says whether the row is a door,
        // in a hue no value wears.
        let t = texts();
        let snap = machine();
        let seen = vec![network("a house", 90)];
        let rows = View::fresh().rows(&facts(&snap, &seen), DECIDING_WIDTH, &t);
        let item = rows
            .iter()
            .find(|r| matches!(&r.target, Target::Wireless(n) if n.ssid == "a house"))
            .expect("no network row");
        let fact = rows
            .iter()
            .find(|r| {
                r.fields
                    .first()
                    .is_some_and(|f| f.text == t.t(ronoctua_i18n::key::NETWORK_FACT_BAND))
            })
            .expect("no fact row");
        assert_eq!(
            item.fields[1].color,
            tokens::color::TEXT_SECONDARY,
            "a list item's description is not the weak half: {:?}",
            item.fields
        );
        // A FACT IS A ROW ONE CANNOT ACT ON, so its answer is the STRONG step
        // with the colour taken out — the owner's third reading of this line:
        // *"mude a cor apenas dos valores que não são interativos, deixando
        // eles cinza"*.
        assert_eq!(
            fact.fields[1].color,
            tokens::color::TEXT_READ_STRONG,
            "a fact's answer is not the grey strong half: {:?}",
            fact.fields
        );
        // AND THE LABEL SAYS WHICH ROW IS A DOOR, everywhere in the view.
        assert_eq!(item.fields[0].color, tokens::color::TEXT_LABEL_ACTS);
        assert_eq!(fact.fields[0].color, tokens::color::TEXT_LABEL_FACT);
        // NO LABEL ANYWHERE WEARS A VALUE'S COLOUR, which is what the owner
        // asked for in those words: *"não quero que você utilize as mesmas
        // cores para as labels e para os valores"*.
        for row in rows.iter().filter(|r| !r.heading && r.fields.len() == 2) {
            let (label, value) = (row.fields[0].color, row.fields[1].color);
            if label == tokens::color::TEXT_DIM {
                // A REFUSING ROW TAKES BOTH HALVES: that is not a kind of row,
                // it is what is happening to one.
                assert_eq!(label, value);
                continue;
            }
            if label == tokens::color::STATE_ACCENT {
                // THE CURSOR IS NOT LIKE THAT, and this assertion said it was.
                // The accent used to take both halves, so a door and a fact
                // became one row at the moment the cursor was on them — which
                // is the moment the question is being asked. The label says
                // "the keyboard is here" and the value goes on saying whether
                // the answer can be changed.
                let acts = row.target != Target::Nothing && !row.disabled;
                assert_eq!(
                    value,
                    if acts {
                        tokens::color::TEXT_HERE_ACTS
                    } else {
                        tokens::color::TEXT_HERE_READ
                    },
                    "the focused row's answer does not say what kind of row it is: {:?}",
                    row.fields
                );
                continue;
            }
            assert!(
                label == tokens::color::TEXT_LABEL_ACTS || label == tokens::color::TEXT_LABEL_FACT,
                "a label outside the label family: {:?}",
                row.fields
            );
            assert_ne!(
                label, value,
                "a label wears its value's colour: {:?}",
                row.fields
            );
            // AND THE TWO HALVES AGREE ABOUT THE ROW: a value keeps its
            // colour only where the label says the row is a door.
            let grey =
                value == tokens::color::TEXT_READ_STRONG || value == tokens::color::TEXT_READ_WEAK;
            assert_eq!(
                grey,
                label == tokens::color::TEXT_LABEL_FACT,
                "the value's colour and the label's disagree: {:?}",
                row.fields
            );
        }
    }

    #[test]
    fn ret_on_an_informative_row_copies_its_value() {
        // The owner asked for it: *"ao interagir (clicar enter) com os itens
        // não interativos, os informativos, a info seja copiada para o
        // clipboard"*. `RET` there used to be the one key on the one row that
        // did nothing, in a list where half of what is drawn is an address, a
        // port or a name somebody wants to paste.
        let t = texts();
        let snap = machine();
        let rows = View::fresh().rows(&facts(&snap, &[]), DECIDING_WIDTH, &t);
        let (i, row) = rows
            .iter()
            .enumerate()
            .find(|(_, r)| r.target == Target::Nothing && !r.heading && r.fields.len() == 2)
            .expect("no informative row with a value");
        let value = row.fields[1].text.clone();
        let mut v = View::fresh();
        v.clicked(i, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Copy(value.clone()),
            "`RET` on an informative row did not ask for the copy"
        );
        // AND IT IS THE VALUE, NOT THE LINE. `192.168.1.5/24` is what goes
        // into a terminal; the label in front of it does not.
        assert!(
            !value.contains(&row.fields[0].text),
            "the label was copied with the value"
        );

        // A SENTENCE HAS NO VALUE, and there the whole of it is the
        // information — the same rule, and `chrome::information_of` is the one
        // place it lives.
        let sentence = rows
            .iter()
            .find(|r| r.target == Target::Nothing && !r.heading && r.fields.len() == 1)
            .expect("no sentence row");
        assert_eq!(
            crate::chrome::information_of(&sentence.fields),
            Some(sentence.fields[0].text.trim().to_owned())
        );
        // AND A ROW THAT ACTS IS UNTOUCHED: `RET` there still does its thing.
        let door = rows
            .iter()
            .position(|r| matches!(r.target, Target::Airplane(_)))
            .expect("no aeroplane row");
        let mut v = View::fresh();
        v.clicked(door, &rows);
        assert!(
            !matches!(
                v.key(Press::Enter, &facts(&snap, &[]), DECIDING_WIDTH, &t),
                Errand::Copy(_)
            ),
            "`RET` on a door copied instead of acting"
        );
    }

    #[test]
    fn a_shelf_is_the_same_height_whatever_it_holds() {
        // THE WHOLE PROPERTY. The owner read the absence off the screen: the
        // Wi-Fi list grew and shrank as routers answered, and everything under
        // it moved while nobody touched anything. A shelf that always draws
        // the same number of lines is what stops that, and the test is that
        // the rows AFTER it do not move.
        let t = texts();
        let snap = machine();
        let where_of = |seen: &[Network]| {
            let rows = View::fresh().rows(&facts(&snap, seen), DECIDING_WIDTH, &t);
            let title = t.t(ronoctua_i18n::key::NETWORK_HEADING_TUNNELS);
            rows.iter()
                .position(|r| r.fields.first().is_some_and(|f| f.text == title))
                .expect("no tunnels section")
        };
        // EVERY COUNT, INCLUDING NOUGHT. The first version of this asserted
        // that nought and one were DIFFERENT — it proved the jump instead of
        // forbidding it, and a review caught that: a scan coming back empty
        // and refilling moved everything under the Wi-Fi by five lines, which
        // is the report this whole shelf exists to answer. The empty sentence
        // lives inside the shelf now, so the height never changes.
        let counts: Vec<Vec<Network>> = [0usize, 1, 2, 5, 6, 7, 30]
            .into_iter()
            .map(|n| {
                (0..n)
                    .map(|i| network(&format!("net {i}"), 90 - u8::try_from(i).unwrap_or(0)))
                    .collect()
            })
            .collect();
        let places: Vec<usize> = counts.iter().map(|c| where_of(c)).collect();
        assert!(
            places.windows(2).all(|w| w[0] == w[1]),
            "the section under the Wi-Fi moved with the list: {places:?} for \
             0, 1, 2, 5, 6, 7 and 30 networks"
        );
    }

    #[test]
    fn nothing_inside_a_container_is_drawn_outside_it() {
        // A CONTAINER HAS A RIGHT EDGE AND A BAR ON IT (`chrome::shelf_box`),
        // so a row that went on being cut at the surface's edge would run its
        // value out of the box and under that bar. A wide surface is where it
        // shows: at 200 columns the row's own room is nearly two hundred cells
        // and the box is a third of that.
        let t = texts();
        let mut snap = machine();
        // TWO SHELVES WITH ROWS IN THEM, and not one: the Wi-Fi list and the
        // remembered one. The other three are empty here and draw their own
        // sentence, which is a shelf row too and is measured with the rest.
        snap.saved = (0..4)
            .map(|i| Saved {
                at: format!("/s/{i}"),
                id: format!("a remembered network with a name long enough to be cut, number {i}"),
                kind: WIRELESS.to_owned(),
                ssid: None,
            })
            .collect();
        let seen: Vec<Network> = (0..12)
            .map(|i| {
                network(
                    &format!("a network with a name long enough to be cut, number {i}"),
                    90 - u8::try_from(i).unwrap_or(0),
                )
            })
            .collect();
        for columns in [60u16, 100, 200] {
            let rows = View::fresh().rows(&facts(&snap, &seen), columns, &t);
            let (left, room) = extent(columns);
            let shelf = crate::chrome::shelf_box(left, room);
            // IN THE SURFACE'S CELLS, which is where the ground and the bar
            // are: a door draws a row's cell `c` at `FIELDS_LEAD + c`.
            let text_ends =
                shelf.from + shelf.cells - crate::chrome::SHELF_PAD - crate::chrome::BAR_CELLS;
            let mut measured = 0;
            let mut which: std::collections::BTreeSet<Shelf> = std::collections::BTreeSet::new();
            for row in rows.iter().filter(|r| r.shelf.is_some()) {
                if let Some((s, _)) = row.shelf {
                    which.insert(s);
                }
                for f in &row.fields {
                    let at = crate::chrome::FIELDS_LEAD + f.at;
                    measured += 1;
                    assert!(
                        at >= shelf.from,
                        "a row began left of its container at {columns} columns"
                    );
                    assert!(
                        at + layout::width(&f.text) <= text_ends,
                        "at {columns} columns a row ran past its container: \
                         {:?} ends at {}, the box's text stops at {text_ends}",
                        f.text,
                        at + layout::width(&f.text)
                    );
                }
            }
            // AND IT DID NOT PASS BY MEASURING NOTHING. A test whose loop
            // never runs is green for the wrong reason, and this one is one
            // fixture change away from that.
            assert!(
                measured > 20 && which.len() >= 4,
                "at {columns} columns only {measured} pieces in {} containers were measured",
                which.len()
            );
        }
    }

    #[test]
    fn every_network_is_reachable_by_keyboard_through_the_shelf() {
        // A CEILING THAT HIDES ROWS THE KEYBOARD CANNOT REACH is half a list.
        // Walking down has to bring the next one INTO the shelf.
        let t = texts();
        let snap = machine();
        let seen: Vec<Network> = (0..20)
            .map(|i| network(&format!("net {i}"), 90 - u8::try_from(i).unwrap_or(0)))
            .collect();
        let f = facts(&snap, &seen);
        let mut v = View::fresh();
        let rows = v.rows(&f, DECIDING_WIDTH, &t);
        let band = v
            .shelves(&rows)
            .into_iter()
            .find(|b| b.shelf == Shelf::Wifi)
            .expect("no Wi-Fi shelf");
        assert_eq!(band.lines, SHELF_LINES);
        assert_eq!(band.total, seen.len());

        // Walk to the shelf's first row, then all the way down it.
        v = View::fresh();
        let first = rows
            .iter()
            .position(|r| r.shelf.map(|(s, _)| s) == Some(Shelf::Wifi) && !r.heading)
            .expect("the shelf drew nothing");
        v.clicked(first, &rows);
        let mut names = std::collections::BTreeSet::new();
        for _ in 0..seen.len() * 2 {
            let rows = v.rows(&f, DECIDING_WIDTH, &t);
            if let Some(Target::Wireless(n)) = rows.get(v.cursor()).map(|r| r.target.clone()) {
                names.insert(n.ssid.clone());
            }
            let _ = v.key(Press::Down, &f, DECIDING_WIDTH, &t);
        }
        assert_eq!(
            names.len(),
            seen.len(),
            "the keyboard reached {} of {} networks",
            names.len(),
            seen.len()
        );
    }

    #[test]
    fn a_remembered_network_out_of_range_is_listed_and_can_be_forgotten() {
        // IT WAS NOWHERE. `saved` was a word beside a network the radio could
        // see, so "which Wi-Fi does this machine know?" answered "the ones you
        // are near", and forgetting one you are not near was impossible.
        let t = texts();
        let mut snap = machine();
        snap.saved = vec![Saved {
            at: "/s/9".to_owned(),
            id: "the office".to_owned(),
            kind: "802-11-wireless".to_owned(),
            ssid: Some("the office".to_owned()),
        }];
        let rows = View::fresh().rows(&facts(&snap, &[]), DECIDING_WIDTH, &t);
        let row = rows
            .iter()
            .find(|r| r.target == Target::Saved("/s/9".to_owned()))
            .expect("the remembered network is in no list");
        assert_eq!(row.shelf.map(|(s, _)| s), Some(Shelf::Remembered));
        assert!(
            row.fields.iter().any(|f| f.text.contains("the office")),
            "the row does not say which network it is"
        );

        // AND A PROFILE WHOSE NAME IS NOT THE NETWORK IS NOT LISTED TWICE.
        //
        // `id` is a name somebody typed and NetworkManager lets it be
        // anything: a profile renamed to `home` still joins `NET_2G4A1`, and
        // the daemon calls a second profile for one network `MyWifi 1`.
        // Matched by name, the same network appeared as in range AND as out
        // of it.
        snap.saved = vec![Saved {
            at: "/s/9".to_owned(),
            id: "home".to_owned(),
            kind: "802-11-wireless".to_owned(),
            ssid: Some("a house".to_owned()),
        }];
        let seen = vec![network("a house", 90)];
        let rows = View::fresh().rows(&facts(&snap, &seen), DECIDING_WIDTH, &t);
        assert!(
            !rows
                .iter()
                .any(|r| r.target == Target::Saved("/s/9".to_owned())),
            "a network in range was listed again as out of range"
        );
    }

    #[test]
    fn tab_walks_the_sections_in_a_ring_and_puts_each_title_at_the_top() {
        // NINE SECTIONS IN ONE LIST is a lot of `<down>`, and the owner asked
        // for the ring. What makes it useful is the anchor: landing on a row
        // whose title is off the screen above it is landing nowhere.
        let t = texts();
        let snap = machine();
        let f = facts(&snap, &[]);
        let mut v = View::fresh();
        let rows = v.rows(&f, DECIDING_WIDTH, &t);
        let titles: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.heading && !r.fields.is_empty())
            .map(|(i, _)| i)
            .collect();
        assert!(titles.len() >= 4, "too few sections to walk: {titles:?}");

        let mut seen = Vec::new();
        for _ in 0..titles.len() {
            assert_eq!(
                v.key(Press::Section, &f, DECIDING_WIDTH, &t),
                Errand::Nothing
            );
            let top = v.take_anchor().expect("TAB did not ask for a title");
            assert!(titles.contains(&top), "{top} is not a section's title");
            assert!(
                v.cursor() > top && !rows[v.cursor()].heading,
                "the cursor landed on a heading, where RET does nothing"
            );
            seen.push(top);
        }
        // A RING: every section once, and back where it started.
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), titles.len(), "the ring skipped a section");
        assert_eq!(
            v.key(Press::Section, &f, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert_eq!(v.take_anchor(), Some(seen[0]), "the ring did not close");

        // AND BACKWARDS IS THE OTHER WAY ROUND.
        assert_eq!(
            v.key(Press::SectionBack, &f, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert_eq!(v.take_anchor(), Some(*seen.last().unwrap()));
    }

    #[test]
    fn the_anchor_answers_once_and_then_the_cursor_is_in_charge() {
        // Taken, not read: a region that kept scrolling to the anchor would
        // refuse to follow the arrows afterwards.
        let t = texts();
        let snap = machine();
        let f = facts(&snap, &[]);
        let mut v = View::fresh();
        let _ = v.key(Press::Section, &f, DECIDING_WIDTH, &t);
        assert!(v.take_anchor().is_some());
        assert!(v.take_anchor().is_none(), "the anchor answered twice");
        let _ = v.key(Press::Down, &f, DECIDING_WIDTH, &t);
        assert!(v.take_anchor().is_none(), "an arrow asked for a title");
    }

    #[test]
    fn a_mark_opens_the_view_on_the_row_it_was_about() {
        let t = texts();
        let mut snap = machine();
        snap.active = vec![tunnel("an office", "/a/1")];
        let want = Target::Tunnel("/a/1".to_owned());
        let v = View::fresh_at(&facts(&snap, &[]), &want, DECIDING_WIDTH, &t);
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
        // THE SEPARATOR IS THE LANGUAGE'S: asked of the bundle, never written
        // down, or this passes or fails with whoever runs it.
        assert!(said(&format!("44{}2 GB", t.decimal_mark())));
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
            standing: ronoctua_tunnel::tor::Standing::Running,
            ..ronoctua_tunnel::Scene::default()
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
            v.key(Press::Enter, &facts, DECIDING_WIDTH, &t),
            Errand::Tunnel(ronoctua_tunnel::Order::ExitNode(Some("n3".to_owned())))
        );

        // TOR'S ROW ASKS THE UNIT, not the circuit. That distinction has now
        // been a defect twice: first the circuit WAS the switch, so
        // `!established` sent `start` to a unit already up; then the fix made
        // the row stop-only with a literal `false`, and `Order::Tor(true)`
        // became reachable from nowhere in the shell — which is what the
        // owner reported. `Standing::Running` is what makes this row say stop.
        let stop = target(&Target::Tunnels(Switch::Tor, false)).unwrap();
        v.clicked(stop, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts, DECIDING_WIDTH, &t),
            Errand::Tunnel(ronoctua_tunnel::Order::Tor(false))
        );
        // AND THE IDENTITY IS REFUSED while there is no circuit.
        let identity = target(&Target::Tunnels(Switch::Identity, true)).unwrap();
        assert!(rows[identity].disabled, "a new identity with no circuit");
        v.clicked(identity, &rows);
        assert_eq!(
            v.key(Press::Enter, &facts, DECIDING_WIDTH, &t),
            Errand::Nothing
        );

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
            ..ronoctua_tunnel::Scene::default()
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
            v.key(Press::Enter, &facts, DECIDING_WIDTH, &t),
            Errand::Tunnel(ronoctua_tunnel::Order::ExitNode(None)),
            "a second press on the node in use takes it off"
        );
    }

    #[test]
    fn a_tor_switch_says_on_or_off_and_never_what_the_unit_is_doing() {
        // `Kind::Switch` RESOLVED TO THE UNIT'S WORDS. Fourteen declared
        // switches — `use-bridges`, `strict-nodes`, `safe-socks` and the rest
        // — drew `network-tor-running` / `-stopped`, so the sheet read "use
        // bridges   running", which is a sentence about a different thing.
        let t = texts();
        let running = t.t(ronoctua_i18n::key::NETWORK_TOR_RUNNING);
        let stopped = t.t(ronoctua_i18n::key::NETWORK_TOR_STOPPED);
        let mut lines = ronoctua_tunnel::tor::fragment::Lines::default();
        let mut switches = 0;
        for setting in ronoctua_tunnel::options::SETTINGS {
            if matches!(setting.kind, ronoctua_options::Kind::Switch) && !setting.fixed {
                let on = setting.turned(&lines, true).expect("a switch always turns");
                setting.put(&mut lines, vec![on]);
                switches += 1;
            }
        }
        assert!(switches > 0, "no switch to measure");
        let tunnels = ronoctua_tunnel::Scene {
            settings: lines,
            ..ronoctua_tunnel::Scene::default()
        };
        let snap = Snapshot::default();
        let facts = Facts {
            net: &snap,
            seen: &[],
            watch: &Watch::default(),
            tunnels: &tunnels,
        };
        let mut v = View::fresh();
        v.level = Level::Tor;
        let rows = v.rows(&facts, 80, &t);
        let mut measured = 0;
        for row in &rows {
            let Target::TorSetting(id) = &row.target else {
                continue;
            };
            let Some(setting) = ronoctua_tunnel::options::SETTINGS
                .iter()
                .find(|s| s.id == *id)
            else {
                continue;
            };
            if !matches!(setting.kind, ronoctua_options::Kind::Switch) || setting.fixed {
                continue;
            }
            measured += 1;
            for f in &row.fields {
                assert_ne!(f.text, running, "`{id}` answers with the unit's word");
                assert_ne!(f.text, stopped, "`{id}` answers with the unit's word");
            }
        }
        assert!(measured > 0, "no switch was drawn, so nothing was measured");

        // AND THE AUTOSTART ROW, which is not a declared setting and had the
        // same defect — photographed, one line under the service's own row:
        // "start it with the session   stopped".
        let autostart = rows
            .iter()
            .find(|r| matches!(r.target, Target::TorAutostart(_)))
            .expect("the sheet drew no autostart row");
        for f in &autostart.fields {
            assert_ne!(f.text, running, "autostart answers with the unit's word");
            assert_ne!(f.text, stopped, "autostart answers with the unit's word");
        }
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
            v.key(Press::Enter, &facts(&snap, &[]), DECIDING_WIDTH, &t),
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
            v.key(Press::Escape, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert!(!v.bringing());
        assert_eq!(
            v.key(Press::Escape, &facts(&snap, &[]), DECIDING_WIDTH, &t),
            Errand::Close
        );
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
        assert_eq!(readable_bytes(0, ','), "0 B");
        assert_eq!(readable_bytes(70_123, ','), "70 kB");
        // THE SEPARATOR IS THE LANGUAGE'S, and both are asserted: the same
        // number came out `44,2 GB` on an English screen until the mark
        // started coming in from the bundle.
        // pt-BR: the comma below is the Portuguese separator, on purpose.
        assert_eq!(readable_bytes(44_220_000_000, ','), "44,2 GB");
        assert_eq!(readable_bytes(44_220_000_000, '.'), "44.2 GB");
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
