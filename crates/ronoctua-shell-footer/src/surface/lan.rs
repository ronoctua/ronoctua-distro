//! THE WAY OUT, AS THE FOOTER KEEPS IT FOR THE SESSION — not a view.
//!
//! NetworkManager's reading and the two tunnels' are the footer's for the
//! whole session: the modeline draws three marks from the first and two from
//! the second with no view open, a hotspot's password file has to go when the
//! hotspot does whether anybody is looking or not, and a network or a tunnel
//! coming and going is said on the minibuffer's line. The network VIEW is a
//! region of its own crate (`ronoctua-shell-network`), told
//! every reading — and the line to the tunnels' thread — as the footer hears
//! them.

use super::*;

/// The network view's id, as the footer ranks it (`PRECEDENCE`) — the
/// region the binary registers under it, hosted by its id alone.
const NETWORK: &str = "network";

impl Footer {
    /// A fresh reading of the way out.
    ///
    /// Repaints only when one of the three MARKS changed, or the view is
    /// open: NetworkManager talks constantly while a link settles, and a frame
    /// for a reading that draws the same pixels is a frame wasted.
    pub(super) fn net_arrived(&mut self, s: ronoctua_network::Snapshot) {
        // WHAT THE FOOTER HEARD, told to every region of the slot host first:
        // the network view follows the rows that are left.
        self.hosted_heard::<ronoctua_network::Snapshot>(&s);
        let before = (
            self.net.adrift(),
            self.net.airplane(),
            self.net.tunnel().is_some(),
        );
        // A NETWORK, A TUNNEL OR THE AEROPLANE COMING AND GOING is said —
        // after the repaint below, once the modeline draws the new reading.
        let said = self.events.network(&s);
        self.net = s;
        // THE HOTSPOT'S PASSWORD FILE follows the hotspot, view open or not:
        // its end takes the file away (`link_words::sharing::follow`).
        crate::link_words::sharing::follow(&mut self.hotspot_serving, &self.net);
        let after = (
            self.net.adrift(),
            self.net.airplane(),
            self.net.tunnel().is_some(),
        );
        if before != after || self.regions.is_open(NETWORK) {
            // AND THE REGION IS ONLY CLAMPED — see `settle_region`: nothing an
            // arrival does is a reason to move the page.
            self.settle_region();
            self.apply_geometry(self.interactivity());
            self.draw_and_paint();
        }
        self.said_now(said);
    }

    /// A fresh reading of the two tunnels, or the end of an order.
    ///
    /// Repaints only when one of the two MARKS changed, or the view is open:
    /// the thread reads every ten seconds and most readings draw the same
    /// pixels.
    pub(super) fn tunnel_event(&mut self, e: ronoctua_tunnel::Event) {
        // WHAT THE FOOTER HEARD, told to every region of the slot host first.
        self.hosted_heard::<ronoctua_tunnel::Event>(&e);
        match e {
            ronoctua_tunnel::Event::Scene(scene) => {
                let before = (self.tunnels.on_tailnet(), self.tunnels.through_tor());
                // THE TAILNET OR TOR COMING AND GOING is said; the thread's
                // first scene is only the baseline.
                let said = self.events.tunnels(&scene);
                self.tunnels = *scene;
                let after = (self.tunnels.on_tailnet(), self.tunnels.through_tor());
                if before != after || self.regions.is_open(NETWORK) {
                    // THE CURSOR FOLLOWS THE ROWS THAT ARE LEFT (the view did,
                    // as it heard the scene): a tailnet going down takes its
                    // whole section away. The region is only clamped.
                    self.settle_region();
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
                self.said_now(said);
            }
            // A REFUSAL IS A SENTENCE, and the daemon's own words are what
            // says why: there are no error names here worth a table of their
            // own, and a silent failure would be a switch that does nothing.
            // THE THREAD IS THE FOOTER'S, and so is the sentence — said once,
            // whichever door's view gave the order.
            ronoctua_tunnel::Event::Done { refused, .. } => {
                if let Some(r) = refused {
                    let said = crate::link_words::tunnel_refused(&r, &self.texts);
                    self.echo_message(said);
                    self.draw_and_paint();
                }
            }
            // THE LOGIN IS A PAGE IN A BROWSER, which is the only place a
            // tailnet's login can happen: the daemon hands back an address
            // and whoever is at the machine has to open it.
            ronoctua_tunnel::Event::Login(url) => {
                self.open_foreign(crate::link_words::OPEN, std::slice::from_ref(&url), None);
                let said = self
                    .texts
                    .t_with(crate::key::ECHO_NETWORK_LOGIN, &[("url", &url)]);
                self.echo_message(said);
                self.draw_and_paint();
            }
        }
    }
}
