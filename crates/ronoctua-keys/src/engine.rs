//! One keyboard's remaps — pure, and the whole of the rule.
//!
//! # What it is fed and what it answers
//!
//! The kernel's key events of ONE physical keyboard go in (`code`, and
//! `value` 1 press, 0 release, 2 autorepeat); what comes out is the events the
//! keyboard's virtual twin should emit, or [`Out::Leader`]. Nothing here
//! touches a device, so a year of typing fits in a unit test.
//!
//! # keyd's meaning, kept
//!
//! The vocabulary is keyd's (`assets/keyboards.nix`), and Together mode hands
//! it to keyd itself — so the same word has to MEAN the same thing in both
//! modes, or switching modes changes what a key does:
//!
//! - a plain word maps the key: `capslock = esc`;
//! - `noop` swallows it;
//! - `overload(hold, tap)` is `hold` while held and `tap` when tapped — and a
//!   TAP is a press and release with no other key pressed in between. keyd
//!   decides it the same way, which is what makes `control` held with a
//!   letter a chord and not an escape.
//!
//! # The leader is an event here, never a key
//!
//! Together mode's leader is keyd turning a tap of `leftmeta` into F13, which
//! the compositor binds. Here the tap of the keyboard's leader key comes out
//! as [`Out::Leader`] and nothing is emitted: the footer is told over a
//! socket which keyboard asked, which is the one thing F13 could not say.
//!
//! # A key is released as what it was pressed as
//!
//! What a press emitted is remembered until its release, so a remap changed
//! while a key is down — the screen turns a ring under a held key — releases
//! the key that went down, never a different one. A modifier left pressed in
//! the compositor is the "no shortcut works" that has three causes on one
//! screen (`AGENTS.md`).

use std::collections::BTreeMap;

use crate::codes;

/// What one key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Becomes this key.
    Key(u16),
    /// Does nothing.
    Noop,
    /// `hold` while held with another key, `tap` when tapped alone.
    Overload {
        /// The key held.
        hold: u16,
        /// What a tap is.
        tap: Tap,
    },
}

/// What a tap of an overloaded key is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tap {
    /// This key, pressed and released.
    Key(u16),
    /// The leader.
    Leader,
}

impl Action {
    /// keyd's word, as an action — `esc`, `noop`, `overload(control, esc)`.
    ///
    /// A tap of F13 IS the leader: that is what keyd's leader rule says in
    /// keyd's words, and here it is said as what it means.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim();
        if word == "noop" {
            return Some(Self::Noop);
        }
        if let Some(inside) = word
            .strip_prefix("overload(")
            .and_then(|w| w.strip_suffix(')'))
        {
            let (hold, tap) = inside.split_once(',')?;
            let hold = codes::of(hold.trim())?;
            let tap = codes::of(tap.trim())?;
            let tap = if tap == codes::F13 {
                Tap::Leader
            } else {
                Tap::Key(tap)
            };
            return Some(Self::Overload { hold, tap });
        }
        codes::of(word).map(Self::Key)
    }
}

/// What the virtual keyboard should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    /// Emit this key event.
    Key {
        /// The kernel's key code.
        code: u16,
        /// 1 press, 0 release, 2 autorepeat.
        value: i32,
    },
    /// The leader was tapped.
    Leader,
}

/// What a key that is down is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Down {
    /// It emitted this key — or nothing, when it was swallowed.
    Plain(Option<u16>),
    /// An overload with no other key since its press.
    Waiting { hold: u16, tap: Tap },
    /// An overload that became its hold key.
    Holding(u16),
}

/// One keyboard's rules and what is down on it.
#[derive(Debug, Clone, Default)]
pub struct Engine {
    rules: BTreeMap<u16, Action>,
    down: BTreeMap<u16, Down>,
}

impl Engine {
    /// A keyboard whose `leader` key is the leader, with `remaps` over it.
    ///
    /// The remaps are keyd's words by keyd's key names — `keyboards.json`'s
    /// own shape — and a remap of the leader key WINS, as it does in the files
    /// keyd reads: somebody who remapped that key meant it.
    #[must_use]
    pub fn new<'a>(leader: &str, remaps: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut rules = BTreeMap::new();
        if let (Some(key), Some(action)) = (
            codes::of(leader),
            Action::parse(ronoctua_keyboards::LEADER.1),
        ) {
            rules.insert(key, action);
        }
        for (key, word) in remaps {
            if word.is_empty() {
                continue;
            }
            if let (Some(key), Some(action)) = (codes::of(key), Action::parse(word)) {
                rules.insert(key, action);
            }
        }
        Self {
            rules,
            down: BTreeMap::new(),
        }
    }

    /// One event of the physical keyboard.
    pub fn feed(&mut self, code: u16, value: i32) -> Vec<Out> {
        let mut out = Vec::new();
        match value {
            1 => {
                // ANOTHER KEY WENT DOWN: every overload still waiting is
                // held, and its hold key goes down FIRST — `control` then the
                // letter, which is the chord the person made.
                for d in self.down.values_mut() {
                    if let Down::Waiting { hold, .. } = *d {
                        out.push(Out::Key {
                            code: hold,
                            value: 1,
                        });
                        *d = Down::Holding(hold);
                    }
                }
                let now = match self.rules.get(&code).copied() {
                    None => Down::Plain(Some(code)),
                    Some(Action::Key(k)) => Down::Plain(Some(k)),
                    Some(Action::Noop) => Down::Plain(None),
                    Some(Action::Overload { hold, tap }) => Down::Waiting { hold, tap },
                };
                if let Down::Plain(Some(k)) = now {
                    out.push(Out::Key { code: k, value: 1 });
                }
                self.down.insert(code, now);
            }
            0 => match self.down.remove(&code) {
                Some(Down::Plain(Some(k)) | Down::Holding(k)) => {
                    out.push(Out::Key { code: k, value: 0 });
                }
                Some(Down::Waiting { tap, .. }) => match tap {
                    Tap::Key(k) => {
                        out.push(Out::Key { code: k, value: 1 });
                        out.push(Out::Key { code: k, value: 0 });
                    }
                    Tap::Leader => out.push(Out::Leader),
                },
                Some(Down::Plain(None)) => {}
                // A RELEASE OF A KEY PRESSED BEFORE THE GRAB: it went down to
                // the compositor as itself, and it comes up as itself.
                None => out.push(Out::Key { code, value: 0 }),
            },
            // A WAITING OVERLOAD DOES NOT REPEAT: holding `control` alone is
            // not a stream of escapes.
            2 => {
                if let Some(Down::Plain(Some(k)) | Down::Holding(k)) = self.down.get(&code) {
                    out.push(Out::Key { code: *k, value: 2 });
                }
            }
            _ => {}
        }
        out
    }

    /// Everything down, released — what a keyboard that is let go of, or
    /// diverted, must say so nothing stays pressed in the compositor.
    pub fn let_go(&mut self) -> Vec<Out> {
        let out = self
            .down
            .values()
            .filter_map(|d| match *d {
                Down::Plain(Some(k)) | Down::Holding(k) => Some(Out::Key { code: k, value: 0 }),
                _ => None,
            })
            .collect();
        self.down.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u16 = 30;
    const CAPS: u16 = 58;
    const ESC: u16 = 1;
    const CTRL: u16 = 29;

    fn key(code: u16, value: i32) -> Out {
        Out::Key { code, value }
    }

    #[test]
    fn a_key_nobody_touched_is_itself() {
        let mut e = Engine::new("leftmeta", []);
        assert_eq!(e.feed(A, 1), vec![key(A, 1)]);
        assert_eq!(e.feed(A, 2), vec![key(A, 2)]);
        assert_eq!(e.feed(A, 0), vec![key(A, 0)]);
    }

    #[test]
    fn a_plain_remap_is_the_other_key() {
        let mut e = Engine::new("leftmeta", [("capslock", "esc")]);
        assert_eq!(e.feed(CAPS, 1), vec![key(ESC, 1)]);
        assert_eq!(e.feed(CAPS, 0), vec![key(ESC, 0)]);
    }

    #[test]
    fn noop_swallows_the_key() {
        let mut e = Engine::new("leftmeta", [("insert", "noop")]);
        assert!(e.feed(110, 1).is_empty());
        assert!(e.feed(110, 0).is_empty());
    }

    #[test]
    fn an_overload_tapped_alone_is_its_tap() {
        let mut e = Engine::new("leftmeta", [("capslock", "overload(control, esc)")]);
        assert!(e.feed(CAPS, 1).is_empty());
        assert!(
            e.feed(CAPS, 2).is_empty(),
            "a waiting overload does not repeat"
        );
        assert_eq!(e.feed(CAPS, 0), vec![key(ESC, 1), key(ESC, 0)]);
    }

    #[test]
    fn an_overload_held_with_a_letter_is_a_chord() {
        let mut e = Engine::new("leftmeta", [("capslock", "overload(control, esc)")]);
        assert!(e.feed(CAPS, 1).is_empty());
        assert_eq!(e.feed(A, 1), vec![key(CTRL, 1), key(A, 1)]);
        assert_eq!(e.feed(A, 0), vec![key(A, 0)]);
        assert_eq!(
            e.feed(CAPS, 0),
            vec![key(CTRL, 0)],
            "no escape after a chord"
        );
    }

    #[test]
    fn a_tap_of_the_leader_key_is_the_leader() {
        let mut e = Engine::new("leftmeta", []);
        assert!(e.feed(codes::LEFTMETA, 1).is_empty());
        assert_eq!(e.feed(codes::LEFTMETA, 0), vec![Out::Leader]);
    }

    #[test]
    fn the_leader_key_held_is_meta() {
        let mut e = Engine::new("leftmeta", []);
        e.feed(codes::LEFTMETA, 1);
        assert_eq!(e.feed(A, 1), vec![key(codes::LEFTMETA, 1), key(A, 1)]);
        e.feed(A, 0);
        assert_eq!(e.feed(codes::LEFTMETA, 0), vec![key(codes::LEFTMETA, 0)]);
    }

    #[test]
    fn each_keyboard_can_have_its_own_leader_key() {
        let mut e = Engine::new("rightalt", []);
        assert_eq!(e.feed(codes::LEFTMETA, 1), vec![key(codes::LEFTMETA, 1)]);
        e.feed(codes::LEFTMETA, 0);
        assert!(e.feed(100, 1).is_empty());
        assert_eq!(e.feed(100, 0), vec![Out::Leader]);
    }

    #[test]
    fn a_remap_of_the_leader_key_wins() {
        let mut e = Engine::new("leftmeta", [("leftmeta", "esc")]);
        assert_eq!(e.feed(codes::LEFTMETA, 1), vec![key(ESC, 1)]);
    }

    #[test]
    fn a_key_is_released_as_what_it_was_pressed_as() {
        let mut e = Engine::new("leftmeta", [("capslock", "esc")]);
        e.feed(CAPS, 1);
        // The ring turned under the held key.
        let mut changed = Engine::new("leftmeta", [("capslock", "backspace")]);
        changed.down.clone_from(&e.down);
        assert_eq!(changed.feed(CAPS, 0), vec![key(ESC, 0)]);
    }

    #[test]
    fn letting_go_releases_everything_down() {
        let mut e = Engine::new("leftmeta", [("capslock", "overload(control, esc)")]);
        e.feed(CAPS, 1);
        e.feed(A, 1);
        let mut released = e.let_go();
        released.sort_by_key(|o| match o {
            Out::Key { code, .. } => *code,
            Out::Leader => 0,
        });
        assert_eq!(released, vec![key(CTRL, 0), key(A, 0)]);
        assert!(
            e.feed(A, 0) == vec![key(A, 0)],
            "an unknown release goes through"
        );
    }
}
