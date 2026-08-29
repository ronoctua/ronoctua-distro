//! The network view — the seventh of the expanded region.
//!
//! # What it is, and what it is not
//!
//! It is a **list**, with the same gesture as the other six: it opens, one navigates with the
//! arrows, one acts with a key. It is not a control panel and has no chrome
//! of its own (invariant 14) — the footer's region is the same.
//!
//! # Why the read is synchronous, and why that does not repeat the app
//! launcher's mistake
//!
//! The launcher scans the XDG on a thread because reading hundreds of `.desktop`
//! files from disk costs tens of milliseconds. Here there is no disk: the
//! NetworkManager keeps in memory the list of access points the radio
//! has already seen, and returning it is one trip to the bus — microseconds.
//!
//! What DOES take time is the radio's scan itself, and that is **not** waited for: the
//! `RequestScan` returns at once and the result appears in NetworkManager's
//! list a few seconds later. The `r` key asks for the scan and
//! re-reads what is already there; whatever the radio finds afterwards comes in on the next read.
//! Blocking until the radio answers would freeze the whole footer for seconds —
//! which is exactly the `systemd-run --scope` trap recorded in
//! CLAUDE.md.

use ronoctua_network::{Link, Manager, Medium, Network, State};

/// The view's state while it is open.
pub struct ViewState {
    /// The conversation with NetworkManager. `None` when there is no bus —
    /// the view opens anyway and says why, instead of not opening.
    manager: Option<Manager>,
    /// The networks in range, already grouped by name.
    networks: Vec<Network>,
    /// Cable and radio, with each one's state.
    links: Vec<Link>,
    /// Is the radio on?
    radio: bool,
    /// The focused row.
    cursor: usize,
}

/// What a row of the view represents.
///
/// Two variants and not a single list of networks: the cable is not a choosable
/// network, and showing it as if it were would give a row on which `Enter` makes
/// no sense. Whoever draws asks the type before offering the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A cable link, with its state.
    Wired {
        /// `enp1s0`.
        interface: String,
        /// Connected, connecting or not.
        state: State,
        /// The negotiated rate, in kb/s.
        rate_kbps: u32,
    },
    /// A wireless network in range.
    Wireless(Network),
}

impl ViewState {
    /// Opens the view, reading what NetworkManager already knows.
    #[must_use]
    pub fn fresh_one() -> Self {
        let manager = Manager::fresh().ok();
        let mut v = Self {
            manager,
            networks: Vec::new(),
            links: Vec::new(),
            radio: false,
            cursor: 0,
        };
        v.reread();
        v
    }

    /// Re-reads NetworkManager's state without asking for a new scan.
    pub fn reread(&mut self) {
        let Some(g) = &self.manager else { return };
        let radio = g.radio_on().unwrap_or(false);
        let links = g.links().unwrap_or_default();
        let networks = g.networks().unwrap_or_default();
        self.apply_to(radio, links, networks);
    }

    /// Stores what came from the bus.
    ///
    /// Separated from [`Self::reread`] on purpose: this is where the decision lives — what
    /// the list shows and where the cursor lands — and it has to be exercisable
    /// without a actual NetworkManager running. A test that only called
    /// `reread` with no bus would pass through none of these lines, and
    /// would pass even with the rule inverted.
    fn apply_to(&mut self, radio: bool, links: Vec<Link>, networks: Vec<Network>) {
        self.radio = radio;
        self.links = links;
        // A radio switched off lists NO network at all. Showing what the last
        // scan saw would offer a connection that cannot happen, and the
        // key would fail looking like a fault.
        self.networks = if radio { networks } else { Vec::new() };
        let ceiling = self.lines().len().saturating_sub(1);
        self.cursor = self.cursor.min(ceiling);
    }

    /// Asks the radio to scan and re-reads what there ALREADY is.
    ///
    /// `RequestScan` returns at once: what the radio finds anew arrives seconds
    /// later, and it is `Footer::watch_scans` — a thread on `LastScan` — that
    /// brings the list up to date without anyone pressing anything.
    pub fn scan(&mut self) {
        if let Some(g) = &self.manager {
            let _ = g.scan();
        }
        self.reread();
    }

    /// Is there a bus to talk to NetworkManager on?
    #[must_use]
    pub const fn available(&self) -> bool {
        self.manager.is_some()
    }

    /// Is the radio on?
    #[must_use]
    pub const fn radio_on(&self) -> bool {
        self.radio
    }

    /// Does this machine have a radio?
    ///
    /// Telling "there is no radio" from "the radio is off" matters: the first
    /// cannot be fixed by a key, and offering `w` there would be promising what
    /// does not happen.
    #[must_use]
    pub fn tem_radio(&self) -> bool {
        self.links.iter().any(|e| e.medium == Medium::Radio)
    }

    /// The rows, in the order they appear.
    ///
    /// The cable comes first, always: when it is connected it is how
    /// the machine is on the network, and burying it under the Wi-Fi list would hide
    /// the answer to the question the view exists to answer.
    #[must_use]
    pub fn lines(&self) -> Vec<Line> {
        let mut output: Vec<Line> = self
            .links
            .iter()
            .filter(|e| e.medium == Medium::Wired)
            .map(|e| Line::Wired {
                interface: e.interface.clone(),
                state: e.state,
                rate_kbps: e.rate_kbps,
            })
            .collect();
        output.extend(self.networks.iter().cloned().map(Line::Wireless));
        output
    }

    /// The active link, for the command that shows the rate.
    ///
    /// The cable beats the radio when both are connected — it is where
    /// the traffic goes, and it is its rate that answers "why is this
    /// slow?".
    #[must_use]
    pub fn active(&self) -> Option<&Link> {
        self.links
            .iter()
            .filter(|e| e.state == State::Connected)
            .min_by_key(|e| u8::from(e.medium == Medium::Radio))
    }

    /// The focused row.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the focus, without leaving the ends.
    ///
    /// It does not wrap, for the same reason as the region's scrolling: going back to the start
    /// after the end makes the eye lose its place.
    pub fn move_by(&mut self, step: i32) {
        let total = self.lines().len();
        if total == 0 {
            self.cursor = 0;
            return;
        }
        let fresh = i64::from(step) + i64::try_from(self.cursor).unwrap_or(0);
        self.cursor =
            usize::try_from(fresh.clamp(0, i64::try_from(total - 1).unwrap_or(0))).unwrap_or(0);
    }

    /// Puts the focus on a row — it is what the pointer uses.
    pub fn focus_on(&mut self, i: usize) {
        self.cursor = i.min(self.lines().len().saturating_sub(1));
    }

    /// The network under the focus, when the row is a network.
    #[must_use]
    pub fn focused_network(&self) -> Option<Network> {
        match self.lines().into_iter().nth(self.cursor) {
            Some(Line::Wireless(r)) => Some(r),
            _ => None,
        }
    }

    /// Turns the radio on or off, and re-reads.
    ///
    /// It returns the new state, or `None` when the bus refused — polkit
    /// may deny, and in that case the screen must not say it turned on.
    pub fn toggle_radio(&mut self) -> Option<bool> {
        let fresh = self.manager.as_ref()?.toggle_radio().ok()?;
        self.reread();
        Some(fresh)
    }

    /// Connects to the focused network.
    ///
    /// `remember` decides the `psk-flags`: with it, the password is sealed into the TPM and the
    /// profile does not store it; without it, nothing is persisted anywhere.
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
        let g = self
            .manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no NetworkManager"))?;
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
        let g = self
            .manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no NetworkManager"))?;
        g.forget(ssid)?;
        self.reread();
        Ok(())
    }

    /// Disconnects the given medium.
    ///
    /// # Errors
    ///
    /// There is no device of that medium, or the bus refused.
    pub fn disconnect(&mut self, medium: Medium) -> anyhow::Result<()> {
        let g = self
            .manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no NetworkManager"))?;
        g.disconnect(medium)?;
        self.reread();
        Ok(())
    }
}

/// `44220000000` -> `44,2 GB`.
///
/// One decimal place, like the link's rate: the total transferred grows all
/// the time, and the second place would be a digit flickering without informing anything.
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
/// Four bands and not a percentage: the exact number wobbles every
/// second and nobody decides anything with "67%". What one decides with signal is
/// "can I connect from here?", and four steps answer that.
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
    use ronoctua_network::Band;

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

    fn link_info(medium: Medium, state: State, rate: u32) -> Link {
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
            signal: 0,
            band: Band::Other,
        }
    }

    /// A view built by hand, with no bus — it is what allows testing the list's
    /// RULES without a actual NetworkManager running.
    fn view(networks: Vec<Network>, links: Vec<Link>) -> ViewState {
        ViewState {
            manager: None,
            networks,
            links,
            radio: true,
            cursor: 0,
        }
    }

    #[test]
    fn the_wire_comes_before_the_networks() {
        // When the cable is connected, it is how the machine is on the
        // network. Burying it under the Wi-Fi list would hide the answer to the
        // question the view exists to answer.
        let v = view(
            vec![network("casa", 90)],
            vec![link_info(Medium::Wired, State::Connected, 1_000_000)],
        );
        assert!(matches!(v.lines().first(), Some(Line::Wired { .. })));
        assert!(matches!(v.lines().get(1), Some(Line::Wireless(_))));
    }

    #[test]
    fn the_wire_beats_the_radio_on_the_active_link() {
        // With both connected, the traffic goes over the cable. Showing the radio's
        // rate would answer "why is this slow?" wrongly.
        let v = view(
            vec![],
            vec![
                link_info(Medium::Radio, State::Connected, 400_000),
                link_info(Medium::Wired, State::Connected, 1_000_000),
            ],
        );
        assert_eq!(v.active().map(|e| e.medium), Some(Medium::Wired));
    }

    #[test]
    fn the_cursor_does_not_leave_the_list() {
        let mut v = view(vec![network("a", 50), network("b", 40)], vec![]);
        v.move_by(-1);
        assert_eq!(v.cursor(), 0, "it went above the first one");
        v.move_by(50);
        assert_eq!(v.cursor(), 1, "it went past the last one");
    }

    #[test]
    fn the_wired_row_is_not_a_network_to_connect_to() {
        // `Enter` over the cable has nothing to do. If `focused_network`
        // returned something here, the key would try to connect to a
        // non-existent network and the error would show up as a connection failure.
        let v = view(
            vec![network("casa", 90)],
            vec![link_info(Medium::Wired, State::Connected, 1_000_000)],
        );
        assert_eq!(v.cursor(), 0);
        assert!(v.focused_network().is_none());
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
    }

    #[test]
    fn a_radio_switched_off_lists_no_network_from_the_old_scan() {
        // Showing what the last scan saw, with the radio off,
        // would offer a connection that cannot happen — the key would fail
        // looking like a radio fault.
        //
        // The test goes through `apply_to`, which is where the bus actually
        // enters. Inverting the rule there makes it flag.
        let mut v = view(vec![], vec![]);
        v.apply_to(false, vec![], vec![network("casa", 90)]);
        assert!(
            v.lines().is_empty(),
            "it listed a network with the radio off"
        );

        v.apply_to(true, vec![], vec![network("casa", 90)]);
        assert_eq!(
            v.lines().len(),
            1,
            "with the radio on the network goes away"
        );
    }

    #[test]
    fn the_cursor_steps_back_when_the_list_shrinks() {
        // The focused network disappears between two reads — the neighbour switched the
        // router off. Without the step back, the cursor would point past the end and the
        // next key would act on nothing.
        let mut v = view(vec![], vec![]);
        v.apply_to(
            true,
            vec![],
            vec![network("a", 90), network("b", 80), network("c", 70)],
        );
        v.focus_on(2);
        assert_eq!(v.cursor(), 2);
        v.apply_to(true, vec![], vec![network("a", 90)]);
        assert_eq!(v.cursor(), 0, "the cursor ended up outside the list");
    }
}
