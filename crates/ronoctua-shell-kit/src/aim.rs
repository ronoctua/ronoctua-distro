//! Where a click lands on what a person did not put under the pointer
//! themselves — a line that appeared by itself, or a grid a click just opened
//! under the hand that clicked.
//!
//! # The rule
//!
//! THE FIRST CLICK PICKS, AND ONLY A CLICK ON THE PERSON'S PICK RUNS IT
//! (invariant 19, and "a question that appears by itself draws `no` first").
//! A card that rises on the band, a countdown on the brain's strip, or a
//! submenu that replaced the grid under the hand is something the person may
//! be clicking at by accident: the second press of a double click, or a click
//! meant for the window the band just covered. One press there acts on
//! nothing anybody chose.
//!
//! # And the cursor the screen put there
//!
//! A question draws its `no` first with the cursor on it ([`Aim::born`]). That
//! cursor is the SCREEN's, not the person's, and a click on it only makes it
//! theirs: two clicks, the price of the position not deciding — the rule the
//! file manager's `Picker::aimed` already keeps for its questions.

/// What the cursor is on, and whether a person put it there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aim<T> {
    /// What the cursor is on, if anything.
    on: Option<T>,
    /// Whether a PERSON put it there — a click, or a key that moved it.
    aimed: bool,
}

impl<T> Default for Aim<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T> Aim<T> {
    /// Nothing picked yet.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            on: None,
            aimed: false,
        }
    }

    /// A cursor born on `on`: where the screen put it, so a click there only
    /// picks it.
    #[must_use]
    pub const fn born(on: T) -> Self {
        Self {
            on: Some(on),
            aimed: false,
        }
    }

    /// What the cursor is on.
    #[must_use]
    pub const fn on(&self) -> Option<&T> {
        self.on.as_ref()
    }
}

impl<T: PartialEq> Aim<T> {
    /// A click on `at`. `true` when it runs — the person's own pick, clicked
    /// again; otherwise `at` becomes the pick and nothing runs.
    ///
    /// THE PICK STAYS after it runs: a step a person clicks several times in
    /// a row (a zoom, a size) runs on every click after the first.
    pub fn clicked(&mut self, at: T) -> bool {
        if self.aimed && self.on.as_ref() == Some(&at) {
            return true;
        }
        self.on = Some(at);
        self.aimed = true;
        false
    }

    /// A key put the cursor on `at` — a person's gesture, so a click there
    /// runs it.
    pub fn moved(&mut self, at: T) {
        self.on = Some(at);
        self.aimed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::Aim;

    /// FAILS IF: the first click on something runs it, or a click on the
    /// pick does not.
    #[test]
    fn the_first_click_picks_and_the_second_runs() {
        let mut aim = Aim::none();
        assert!(!aim.clicked(3), "the first click ran it");
        assert_eq!(aim.on(), Some(&3));
        assert!(aim.clicked(3), "a click on the pick did not run it");
        // AND IT STAYS: a step is clicked several times in a row.
        assert!(aim.clicked(3));
        // ANOTHER ONE IS PICKED, and nothing runs.
        assert!(!aim.clicked(4));
        assert_eq!(aim.on(), Some(&4));
    }

    /// FAILS IF: the cursor the screen put on a question's `no` is taken for
    /// the person's — a click there would answer what nobody chose.
    #[test]
    fn a_cursor_born_there_is_picked_before_it_runs() {
        let mut aim = Aim::born("no");
        assert_eq!(aim.on(), Some(&"no"));
        assert!(!aim.clicked("no"), "the born cursor answered one click");
        assert!(aim.clicked("no"));
        // A KEY THAT MOVED IT is a person's gesture: the click runs.
        let mut aim = Aim::born("no");
        aim.moved("yes");
        assert!(aim.clicked("yes"));
    }
}
