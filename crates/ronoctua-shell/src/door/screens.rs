//! WHICH SCREENS a door knows, and which of them its surface is on.
//!
//! # Why a door asks its own connection
//!
//! Because the compositor's IPC is the column's business, not every door's,
//! and because the answer is already on the connection: every door binds
//! `wl_output` (`OutputState`), which describes each screen — its connector
//! (`eDP-1`, `DP-2`: the `name` of version 4) and a description a person can
//! read — as it is plugged in, changed or taken away, with no question asked
//! on the loop. And `wl_surface` says which of them the surface is on
//! (`enter` and `leave`). Both are core protocol (invariant 3).
//!
//! # A door that does not know is on every screen
//!
//! A compositor older than version 4 names no connector, and a surface is on
//! no screen until its first `enter`. Then the door's screen is UNKNOWN, and
//! what is routed to a screen is shown there anyway
//! (`ronoctua_player::own::Screens::shows_on`): a picture that vanished
//! because a compositor was slow to say where it was would be a defect with
//! no symptom but an empty window.
//!
//! Generic over the output so the bookkeeping is tested without a
//! compositor; a door keeps `Screens<wl_output::WlOutput>`.
//!
//! What a window reads of it (`all`, `on`, `here`) is not read by any window
//! yet: the player's routing reads it once the daemon sends lanes with their
//! screens (the second protocol). Those three are allowed until then.

/// A screen, as a door's connection has had it described.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen<O> {
    /// Its connector (`eDP-1`), when the compositor names one.
    pub connector: Option<String>,
    /// What the compositor calls it for a person (the make, the model).
    pub description: Option<String>,
    /// The output itself — what a surface is put on
    /// (`xdg_toplevel.set_fullscreen`, a layer surface).
    pub output: O,
}

/// Every screen a door knows, and the ones its surface is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screens<O> {
    /// Every screen described, in the order they were.
    all: Vec<Screen<O>>,
    /// The screens the surface is on, in the order it entered them.
    on: Vec<O>,
}

impl<O> Default for Screens<O> {
    fn default() -> Self {
        Self {
            all: Vec::new(),
            on: Vec::new(),
        }
    }
}

impl<O: PartialEq + Clone> Screens<O> {
    /// A screen described, or described again: `true` when what a door
    /// shows of it changed — a screen new, or its name or its description.
    pub fn described(
        &mut self,
        output: O,
        connector: Option<String>,
        description: Option<String>,
    ) -> bool {
        match self.all.iter_mut().find(|s| s.output == output) {
            Some(s) if s.connector == connector && s.description == description => false,
            Some(s) => {
                s.connector = connector;
                s.description = description;
                true
            }
            None => {
                self.all.push(Screen {
                    connector,
                    description,
                    output,
                });
                true
            }
        }
    }

    /// A screen taken away: it is forgotten, and the surface is no longer on
    /// it. `true` when it was known.
    pub fn gone(&mut self, output: &O) -> bool {
        let before = self.all.len();
        self.all.retain(|s| &s.output != output);
        self.on.retain(|o| o != output);
        self.all.len() != before
    }

    /// The surface entered a screen. `true` when it was not on it already.
    pub fn entered(&mut self, output: O) -> bool {
        if self.on.contains(&output) {
            return false;
        }
        self.on.push(output);
        true
    }

    /// The surface left a screen. `true` when it was on it.
    pub fn left(&mut self, output: &O) -> bool {
        let before = self.on.len();
        self.on.retain(|o| o != output);
        self.on.len() != before
    }

    /// The surface is a new one, on no screen until it is told: what the old
    /// one was on says nothing about it. `true` when it was on one.
    pub fn left_every_screen(&mut self) -> bool {
        let was_on = !self.on.is_empty();
        self.on.clear();
        was_on
    }

    /// Every screen described.
    #[must_use]
    #[allow(dead_code)]
    pub fn all(&self) -> &[Screen<O>] {
        &self.all
    }

    /// The screen whose connector is `connector` — `None` until the
    /// compositor has described it, and then [`crate::door::Content`]'s
    /// `screens_changed` is the moment to ask again.
    #[must_use]
    pub fn named(&self, connector: &str) -> Option<&O> {
        self.all
            .iter()
            .find(|s| s.connector.as_deref() == Some(connector))
            .map(|s| &s.output)
    }

    /// The connectors of the screens the surface is on, in the order it
    /// entered them. A screen with no name is left out: EMPTY IS UNKNOWN,
    /// and unknown counts as every screen.
    #[must_use]
    #[allow(dead_code)]
    pub fn on(&self) -> Vec<&str> {
        self.on
            .iter()
            .filter_map(|o| self.all.iter().find(|s| &s.output == o))
            .filter_map(|s| s.connector.as_deref())
            .collect()
    }

    /// The screen the surface is on — the first it entered that has a name;
    /// `None` is unknown, which counts as every screen.
    #[must_use]
    #[allow(dead_code)]
    pub fn here(&self) -> Option<&str> {
        self.on().first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::Screens;

    fn name(s: &str) -> Option<String> {
        Some(s.to_owned())
    }

    /// FAILS IF: a screen described twice is two, a change of its name is not
    /// news, or one taken away is kept.
    #[test]
    fn a_screen_is_described_once_and_forgotten_when_it_goes() {
        let mut s = Screens::<u32>::default();
        assert!(s.described(7, name("eDP-1"), name("Built-in")));
        assert!(
            !s.described(7, name("eDP-1"), name("Built-in")),
            "the same words are not news"
        );
        assert!(s.described(8, None, None));
        assert!(
            s.described(8, name("DP-2"), name("A wide monitor")),
            "a name that came later is"
        );
        assert_eq!(s.all().len(), 2);
        assert_eq!(s.named("DP-2"), Some(&8));
        assert_eq!(s.named("HDMI-A-1"), None);
        assert!(s.gone(&8));
        assert!(!s.gone(&8));
        assert_eq!(s.named("DP-2"), None);
    }

    /// FAILS IF: which screens the surface is on stops following `enter` and
    /// `leave`, a screen that went keeps the surface on it, or a nameless
    /// screen reads as a known one instead of as every screen.
    #[test]
    fn the_surface_is_on_what_it_entered_and_unknown_is_nothing_named() {
        let mut s = Screens::<u32>::default();
        s.described(1, name("eDP-1"), None);
        s.described(2, name("DP-2"), None);
        s.described(3, None, None);
        assert_eq!(s.here(), None, "on no screen until the first enter");
        assert!(s.entered(2));
        assert!(!s.entered(2));
        assert!(s.entered(1));
        assert_eq!(s.on(), vec!["DP-2", "eDP-1"]);
        assert_eq!(s.here(), Some("DP-2"));
        assert!(s.left(&2));
        assert!(!s.left(&2));
        assert_eq!(s.here(), Some("eDP-1"));
        // A SCREEN WITH NO NAME is unknown, never a name made up for it.
        assert!(s.left_every_screen(), "it was on one");
        assert!(!s.left_every_screen(), "and now on none");
        assert!(s.entered(3));
        assert!(s.on().is_empty());
        assert_eq!(s.here(), None);
        // AND ONE THAT GOES takes the surface off it.
        assert!(s.entered(1));
        assert!(s.gone(&1));
        assert!(s.on().is_empty());
    }
}
