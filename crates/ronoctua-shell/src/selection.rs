//! The compositor's selection, as far as this footer can know it.
//!
//! # Why a shadow
//!
//! Selecting is marking windows to act on them together — bring them all here,
//! turn them into a trail. The owner asked for a mark on the modeline while
//! something is selected, and for a menu behind it. The compositor keeps the
//! selection in a field (`container->selected`) that NO reply and NO event
//! writes out, and its Lua API does not read it either — measured in the pinned
//! source. So the footer keeps its own copy, written by the deeds as they go
//! through `run_with`: the five selection chords go through our binary for this
//! (`actions::chords`, the `Insert` family), so a key and a click write the same
//! copy.
//!
//! # What it cannot see
//!
//! A selection made any other way — a script, `scrollmsg` in a terminal — and
//! the compositor's own choice to mark a whole COLUMN rather than the window in
//! some layout modes (`layout_selection_toggle` climbs to the parent). The mark
//! says "something is selected" by the copy, and the menu behind it acts on
//! whatever the compositor really has.

use std::collections::BTreeSet;

/// What this footer knows is selected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    /// The windows toggled on, by id.
    windows: BTreeSet<u64>,
    /// Whether a whole workspace was selected, whose windows this copy never
    /// learnt one by one.
    workspace_wide: bool,
}

impl Selection {
    /// Nothing selected — the `Default`, spelt for a `const` context.
    pub const EMPTY: Self = Self {
        windows: BTreeSet::new(),
        workspace_wide: false,
    };

    /// Whether anything is selected, as far as the copy knows.
    #[must_use]
    pub fn any(&self) -> bool {
        self.workspace_wide || !self.windows.is_empty()
    }

    /// How many windows, when the copy can count them — a whole workspace it
    /// cannot.
    #[must_use]
    pub fn count(&self) -> Option<usize> {
        (!self.workspace_wide).then_some(self.windows.len())
    }

    /// Whether this window is one the copy has as selected.
    #[must_use]
    pub fn has(&self, id: u64) -> bool {
        self.windows.contains(&id)
    }

    /// A selection deed ran — `focused` is the window it acted on. Answers
    /// whether the copy changed.
    pub fn deed(&mut self, id: &str, focused: Option<u64>) -> bool {
        let before = self.clone();
        match id {
            "wm-select" => {
                if let Some(w) = focused
                    && !self.windows.remove(&w)
                {
                    self.windows.insert(w);
                }
            }
            "wm-select-workspace" => self.workspace_wide = true,
            // THE THREE THAT END IT: clearing, bringing the marked windows
            // here, and turning them into a trail all leave nothing marked.
            "wm-select-none" | "wm-select-move" | "wm-select-to-trail" => {
                *self = Self::default();
            }
            // A TRAIL'S WINDOWS BECOME THE SELECTION, and which they are the
            // copy cannot know — as with a whole workspace.
            "wm-trail-to-selection" => self.workspace_wide = true,
            _ => {}
        }
        *self != before
    }

    /// A window closed: it is selected no more.
    pub fn closed(&mut self, id: u64) -> bool {
        self.windows.remove(&id)
    }

    /// Whether this deed is one the copy follows.
    #[must_use]
    pub fn follows(id: &str) -> bool {
        matches!(
            id,
            "wm-select"
                | "wm-select-workspace"
                | "wm-select-none"
                | "wm-select-move"
                | "wm-select-to-trail"
                | "wm-trail-to-selection"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_toggle_flips_the_focused_window() {
        let mut s = Selection::default();
        assert!(s.deed("wm-select", Some(3)));
        assert_eq!(s.count(), Some(1));
        assert!(s.has(3));
        assert!(s.deed("wm-select", Some(3)));
        assert!(!s.any());
        assert!(
            !s.deed("wm-select", None),
            "nothing focused, nothing toggled"
        );
    }

    #[test]
    fn a_workspace_cannot_be_counted_and_the_three_ends_clear() {
        for end in ["wm-select-none", "wm-select-move", "wm-select-to-trail"] {
            let mut s = Selection::default();
            s.deed("wm-select", Some(1));
            s.deed("wm-select-workspace", Some(1));
            assert!(s.any());
            assert_eq!(s.count(), None);
            assert!(s.deed(end, None));
            assert!(!s.any(), "{end} left something selected");
        }
    }

    #[test]
    fn a_closed_window_leaves_the_selection() {
        let mut s = Selection::default();
        s.deed("wm-select", Some(5));
        assert!(s.closed(5));
        assert!(!s.any());
        assert!(!s.closed(5));
    }

    #[test]
    fn only_the_selection_deeds_are_followed() {
        assert!(Selection::follows("wm-select-move"));
        assert!(!Selection::follows("wm-float"));
    }
}
