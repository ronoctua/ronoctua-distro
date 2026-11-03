//! A WINDOW'S GROUP: the terminals one window holds, in the strip's order,
//! and the one it shows in front — the owner's exception to invariant 4
//! (2026-11-02), drawn by `crate::terminal_strip`.
//!
//! # Who knows the group
//!
//! The terminals do: every holder's state says the group its window shows it
//! in, its place there, and whether it is in front and seen
//! (`ronoctua_terminals::state::TerminalState`). A window says them as it
//! attaches (the group and the place) and as they change (`Place` — in
//! front, seen while the window holds the keyboard), and a group exists
//! only while some holder carries its id. So a window that dies leaves its
//! whole group in the background, each holder knowing it on its own (its
//! window's socket closed), and a window opened on them later shows them in
//! the same order.
//!
//! # A place is a number with room on both sides
//!
//! A new terminal goes right after the one in front, and the holders hear
//! their place only when a window attaches them. So places are spaced
//! ([`ORDER_STEP`]) and a new one takes the middle of the gap it lands in
//! ([`Group::order_after_front`]) — no other holder has to be told anything.
//! A gap that closed (sixteen halvings in one spot) is renumbered here, and
//! each holder hears its new number with the next `Place` the window tells
//! it ([`Group::places`]).
//!
//! # What this module is not
//!
//! It draws nothing and talks to no holder: it is the bookkeeping the
//! window (`crate::terminal`) keeps and tests hold — which one is in front,
//! where a newcomer goes, who takes the front when the one there leaves.

use std::sync::Arc;

use ronoctua_terminals::TerminalId;
use ronoctua_terminals::state::TerminalState;
use ronoctua_terminals::wire::{self, Place};

use crate::terminal_link::{Link, Mirrored, Reach};

/// The room between two places: a place for every terminal, spaced, so a
/// newcomer always has a middle to take.
pub const ORDER_STEP: u32 = 1 << 16;

/// ONE TERMINAL THE WINDOW HOLDS: its attachment, its mirror and what the
/// window last told it and heard of it.
pub struct Held {
    /// The attachment's number in this window — every attach, a reattach
    /// included, is a new one, and what an older one's reader still says is
    /// not this one's.
    pub generation: u64,
    /// The terminal.
    pub id: TerminalId,
    /// Its place in the group.
    pub order: u32,
    /// Its mirror.
    pub mirrored: Arc<Mirrored>,
    /// Where it is — `None` for one held only in a test or by the camera.
    pub reach: Option<Reach>,
    /// The attachment.
    pub link: Option<Link>,
    /// Its state, as the holder said it last.
    pub state: Option<Box<TerminalState>>,
    /// The size last asked of it.
    pub sent_size: Option<wire::Size>,
    /// The colours last sent.
    pub sent_colors: Option<wire::Colors>,
    /// Where it was told it is shown.
    pub told: Option<Place>,
    /// Its `front_asked` as last heard — `None` before the first state, which
    /// is where counting starts: a terminal is fronted when it MOVES.
    pub front_asked: Option<u64>,
    /// Something is selected on its mirror.
    pub selected: bool,
}

impl Held {
    /// A terminal held, nothing told it yet.
    #[must_use]
    pub const fn new(generation: u64, id: TerminalId, order: u32, mirrored: Arc<Mirrored>) -> Self {
        Self {
            generation,
            id,
            order,
            mirrored,
            reach: None,
            link: None,
            state: None,
            sent_size: None,
            sent_colors: None,
            told: None,
            front_asked: None,
            selected: false,
        }
    }

    /// Says something to its holder, when it has one.
    pub fn send(&self, frame: wire::ToHolder) {
        if let Some(link) = &self.link {
            link.send(frame);
        }
    }

    /// Whether it lives in the window's own process — and so cannot leave
    /// it for another window, nor have its group's new ones held.
    #[must_use]
    pub fn in_process(&self) -> bool {
        self.reach.as_ref().is_some_and(Reach::in_process)
    }

    /// What its program called it, plain; empty when it said nothing.
    #[must_use]
    pub fn title(&self) -> &str {
        self.state.as_deref().map_or("", |s| s.title.as_str())
    }

    /// Takes in a new state: `true` when its `front_asked` MOVED since the
    /// last one (something asked for it to be put in front — the footer's
    /// `terminal-show`, an agent's mark); the first state only starts the
    /// count.
    pub fn heard(&mut self, state: Box<TerminalState>) -> bool {
        let asked = state.front_asked;
        let moved = self.front_asked.is_some_and(|seen| asked > seen);
        self.front_asked = Some(asked);
        self.state = Some(state);
        moved
    }
}

/// A terminal on its way: launched, not attached yet — and the place it
/// was promised.
#[derive(Clone)]
pub struct Pending {
    /// The attachment's number.
    pub generation: u64,
    /// Its place.
    pub order: u32,
    /// It goes in front when it arrives.
    pub front: bool,
    /// The mirror its reader feeds.
    pub mirrored: Arc<Mirrored>,
    /// The size it was started at.
    pub size: wire::Size,
    /// The colours it was started with.
    pub colors: wire::Colors,
    /// It is a NEW terminal (`C-T`), and not one that ran already: one
    /// that could not start is said on the line.
    pub new: bool,
}

/// What a terminal leaving did to the group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    /// It was not in the group.
    Stranger,
    /// It went from behind: the one in front stays.
    Behind,
    /// It was the one in front: its neighbour is now.
    Front,
    /// It was the last: nothing is held, and nothing is on its way.
    Last,
}

/// THE GROUP: the terminals held, in the strip's order, the one in front,
/// and those on their way.
#[derive(Default)]
pub struct Group {
    held: Vec<Held>,
    front: usize,
    pending: Vec<Pending>,
}

impl Group {
    /// How many are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Nothing is held and nothing is on its way — the window has nothing
    /// left to show.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.held.is_empty() && self.pending.is_empty()
    }

    /// The terminals, in order.
    pub fn iter(&self) -> impl Iterator<Item = &Held> {
        self.held.iter()
    }

    /// The terminals, in order, to change.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Held> {
        self.held.iter_mut()
    }

    /// Where the one in front is, in the strip's order.
    #[must_use]
    pub const fn front_index(&self) -> usize {
        self.front
    }

    /// The one in front.
    #[must_use]
    pub fn front(&self) -> Option<&Held> {
        self.held.get(self.front)
    }

    /// The one in front, to change.
    pub fn front_mut(&mut self) -> Option<&mut Held> {
        self.held.get_mut(self.front)
    }

    /// Terminal `index` of the strip.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Held> {
        self.held.get(index)
    }

    /// Terminal `index` of the strip, to change.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Held> {
        self.held.get_mut(index)
    }

    /// Where the attachment `generation` is.
    #[must_use]
    pub fn index_of(&self, generation: u64) -> Option<usize> {
        self.held.iter().position(|h| h.generation == generation)
    }

    /// The attachment `generation`, to change.
    pub fn by_generation(&mut self, generation: u64) -> Option<&mut Held> {
        self.held.iter_mut().find(|h| h.generation == generation)
    }

    /// The terminals on their way, how many.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Whether the attachment `generation` is on its way: launched, its
    /// launch not back yet.
    #[must_use]
    pub fn is_pending(&self, generation: u64) -> bool {
        self.pending.iter().any(|p| p.generation == generation)
    }

    /// Promises a place to a terminal on its way.
    pub fn expect(&mut self, pending: Pending) {
        self.pending.push(pending);
    }

    /// The terminal on its way as `generation` arrived — or could not: its
    /// promise, taken.
    pub fn arrived(&mut self, generation: u64) -> Option<Pending> {
        let at = self
            .pending
            .iter()
            .position(|p| p.generation == generation)?;
        Some(self.pending.remove(at))
    }

    /// A PLACE RIGHT AFTER THE ONE IN FRONT: the middle between its place
    /// and the next one taken — held or promised — or a step after it when
    /// none is. A gap with no middle left is renumbered (see the head of
    /// the module) and asked again.
    pub fn order_after_front(&mut self) -> u32 {
        if let Some(order) = self.middle_after_front() {
            return order;
        }
        tracing::debug!("the group's places ran out of room; renumbered");
        // EVERY PLACE, the held and the promised, in the order they are —
        // what is on its way keeps its place among the others.
        let mut places: Vec<(u32, bool, usize)> = self
            .held
            .iter()
            .enumerate()
            .map(|(i, h)| (h.order, true, i))
            .chain(
                self.pending
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.order, false, i)),
            )
            .collect();
        places.sort_by_key(|&(order, held, i)| (order, !held, i));
        for (n, (_, held, i)) in places.into_iter().enumerate() {
            if held {
                self.held[i].order = step(n);
            } else {
                self.pending[i].order = step(n);
            }
        }
        let after = self.front().map_or(0, |h| h.order);
        self.middle_after_front().unwrap_or(after + 1)
    }

    fn middle_after_front(&self) -> Option<u32> {
        let Some(front) = self.front() else {
            return Some(ORDER_STEP);
        };
        let after = front.order;
        let next = self
            .held
            .iter()
            .map(|h| h.order)
            .chain(self.pending.iter().map(|p| p.order))
            .filter(|&o| o > after)
            .min();
        match next {
            None => after.checked_add(ORDER_STEP),
            Some(next) if next - after >= 2 => Some(after + (next - after) / 2),
            Some(_) => None,
        }
    }

    /// Takes `held` in at its place in the order, after any of the same
    /// place — and in front when `front` says so or nothing else is.
    /// Answers where it went.
    pub fn take_in(&mut self, held: Held, front: bool) -> usize {
        let at = self
            .held
            .iter()
            .position(|h| h.order > held.order)
            .unwrap_or(self.held.len());
        let was_empty = self.held.is_empty();
        self.held.insert(at, held);
        if was_empty || front {
            self.front = at;
        } else if at <= self.front {
            // THE ONE IN FRONT moved one along: it is still the one shown.
            self.front += 1;
        }
        at
    }

    /// Puts terminal `index` in front. `true` when that changed anything.
    pub fn front_to(&mut self, index: usize) -> bool {
        if index >= self.held.len() || index == self.front {
            return false;
        }
        self.front = index;
        true
    }

    /// The next terminal (`by` 1) or the previous one (`by` -1) in front,
    /// round the end of the strip. `true` when that changed anything.
    pub fn step(&mut self, by: isize) -> bool {
        let n = self.held.len();
        if n < 2 {
            return false;
        }
        let n = isize::try_from(n).unwrap_or(isize::MAX);
        let at = isize::try_from(self.front).unwrap_or(0);
        let next = usize::try_from((at + by).rem_euclid(n)).unwrap_or(0);
        self.front_to(next)
    }

    /// The attachment `generation` leaves the group — its program ended,
    /// another window took it, its holder went away. The one in front when
    /// it was is its right-hand neighbour, or the left-hand one at the end
    /// of the strip. Answers what that did, and the one that left.
    pub fn leave(&mut self, generation: u64) -> (Left, Option<Held>) {
        let Some(at) = self.index_of(generation) else {
            return (Left::Stranger, None);
        };
        let held = self.held.remove(at);
        let left = if self.held.is_empty() {
            self.front = 0;
            if self.pending.is_empty() {
                Left::Last
            } else {
                Left::Front
            }
        } else if at == self.front {
            self.front = at.min(self.held.len() - 1);
            Left::Front
        } else {
            if at < self.front {
                self.front -= 1;
            }
            Left::Behind
        };
        (left, Some(held))
    }

    /// WHERE EACH TERMINAL IS SHOWN, for a window that holds the keyboard
    /// or not: in front, and seen when in front of a focused window — every
    /// one whose answer differs from what it was told last, with its index.
    #[must_use]
    pub fn places(&self, focused: bool) -> Vec<(usize, Place)> {
        self.held
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                // ITS NUMBER TOO: a gap that closed renumbers the group
                // here, and each holder hears its new place with the next
                // `Place` — not only when a window next attaches it.
                let place = Place {
                    front: i == self.front,
                    seen: i == self.front && focused,
                    order: Some(h.order),
                };
                (h.told != Some(place)).then_some((i, place))
            })
            .collect()
    }

    /// Records that terminal `index` was told `place`.
    pub fn told(&mut self, index: usize, place: Place) {
        if let Some(h) = self.held.get_mut(index) {
            h.told = Some(place);
        }
    }
}

/// The `n`-th place of a group laid out afresh — what a window opened on a
/// group (`--show`) gives each, in the order it was given them.
#[must_use]
pub fn step(n: usize) -> u32 {
    u32::try_from(n + 1)
        .unwrap_or(u32::MAX)
        .saturating_mul(ORDER_STEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(generation: u64, order: u32) -> Held {
        Held::new(
            generation,
            TerminalId::fresh().unwrap(),
            order,
            Mirrored::new().unwrap(),
        )
    }

    fn group_of(orders: &[u32]) -> Group {
        let mut g = Group::default();
        for (n, &o) in orders.iter().enumerate() {
            g.take_in(held(n as u64, o), false);
        }
        g
    }

    fn generations(g: &Group) -> Vec<u64> {
        g.iter().map(|h| h.generation).collect()
    }

    #[test]
    fn a_new_terminal_goes_right_after_the_one_in_front() {
        // FAILS IF a new terminal lands at the end of the strip instead of
        // beside the one the person was in, two quick ones land anywhere but
        // right after it, or the one in front stops being the one shown when
        // a newcomer is taken in before it.
        let mut g = group_of(&[step(0), step(1), step(2)]);
        assert_eq!(g.front_index(), 0, "the first taken in is in front");
        let o = g.order_after_front();
        assert!(step(0) < o && o < step(1), "{o}");
        g.take_in(held(10, o), false);
        assert_eq!(generations(&g), [0, 10, 1, 2]);
        assert_eq!(g.front().map(|h| h.generation), Some(0));
        // A SECOND, promised before the first arrived, takes the middle
        // nearer the front — and is right after it too.
        let first = g.order_after_front();
        g.expect(Pending {
            generation: 11,
            order: first,
            front: false,
            mirrored: Mirrored::new().unwrap(),
            size: wire::Size {
                columns: 1,
                rows: 1,
                cell_width_px: 1,
                cell_height_px: 1,
            },
            colors: crate::terminal_sheet::Palette::of_the_page().wire(),
            new: true,
        });
        let second = g.order_after_front();
        assert!(step(0) < second && second < first, "{second} {first}");
        // ONE IN FRONT OF THE FRONT keeps the front where it was.
        g.take_in(held(12, 0), false);
        assert_eq!(g.front().map(|h| h.generation), Some(0));
        // THE END OF THE STRIP takes a step after it.
        assert!(g.front_to(g.len() - 1));
        let last = g.order_after_front();
        assert_eq!(last, step(2) + ORDER_STEP);
    }

    #[test]
    fn a_gap_with_no_middle_is_renumbered_and_keeps_the_order() {
        // FAILS IF a new terminal's place equals a neighbour's (two at one
        // place come back from the background in either order), or the
        // strip's order changes when the places are renumbered.
        let mut g = group_of(&[5, 6]);
        assert_eq!(g.front_index(), 0);
        let o = g.order_after_front();
        let orders: Vec<u32> = g.iter().map(|h| h.order).collect();
        assert!(orders[0] < o && o < orders[1], "{orders:?} {o}");
        assert_eq!(generations(&g), [0, 1]);
    }

    #[test]
    fn the_one_that_leaves_hands_the_front_to_its_neighbour_and_the_last_closes() {
        // FAILS IF the front is left on a terminal that is gone (keys sent
        // nowhere), a terminal that leaves from behind moves the front, or
        // the window is not told when the last one left — and is told while
        // one is still on its way.
        let mut g = group_of(&[step(0), step(1), step(2)]);
        assert!(g.front_to(1));
        assert_eq!(g.leave(0).0, Left::Behind);
        assert_eq!(g.front().map(|h| h.generation), Some(1));
        assert_eq!(g.leave(1).0, Left::Front);
        assert_eq!(
            g.front().map(|h| h.generation),
            Some(2),
            "the right-hand one"
        );
        let (left, gone) = g.leave(2);
        assert_eq!(left, Left::Last);
        assert_eq!(gone.map(|h| h.generation), Some(2));
        assert!(g.is_done());
        assert_eq!(g.leave(2).0, Left::Stranger);
        // AT THE END OF THE STRIP, the left-hand one takes it.
        let mut g = group_of(&[step(0), step(1)]);
        assert!(g.front_to(1));
        assert_eq!(g.leave(1).0, Left::Front);
        assert_eq!(g.front().map(|h| h.generation), Some(0));
        // ONE STILL ON ITS WAY: not the last.
        g.expect(Pending {
            generation: 9,
            order: step(5),
            front: true,
            mirrored: Mirrored::new().unwrap(),
            size: wire::Size {
                columns: 1,
                rows: 1,
                cell_width_px: 1,
                cell_height_px: 1,
            },
            colors: crate::terminal_sheet::Palette::of_the_page().wire(),
            new: true,
        });
        assert_eq!(g.leave(0).0, Left::Front);
        assert!(!g.is_done());
    }

    #[test]
    fn the_walk_goes_round_the_strip_and_a_lone_terminal_stays() {
        // FAILS IF next past the last is not the first (or previous before
        // the first not the last), or a window of one terminal reports a
        // change it did not make.
        let mut g = group_of(&[step(0), step(1), step(2)]);
        assert!(g.step(1) && g.front_index() == 1);
        assert!(g.step(1) && g.step(1) && g.front_index() == 0);
        assert!(g.step(-1) && g.front_index() == 2);
        let mut one = group_of(&[step(0)]);
        assert!(!one.step(1) && !one.step(-1));
        assert!(!one.front_to(0), "already in front");
    }

    #[test]
    fn a_place_is_told_once_and_seen_follows_the_keyboard() {
        // FAILS IF a terminal is told where it is on every frame (a state
        // file rewritten for nothing), the one in front of a window without
        // the keyboard is said to be seen (its call for attention cleared
        // with nobody looking), or the one behind is said to be in front.
        let mut g = group_of(&[step(0), step(1)]);
        let places = g.places(false);
        assert_eq!(places.len(), 2);
        assert_eq!(
            places[0],
            (
                0,
                Place {
                    front: true,
                    seen: false,
                    order: Some(step(0)),
                }
            )
        );
        assert_eq!(
            places[1],
            (
                1,
                Place {
                    front: false,
                    seen: false,
                    order: Some(step(1)),
                }
            )
        );
        for (i, p) in places {
            g.told(i, p);
        }
        assert!(g.places(false).is_empty(), "told twice");
        assert_eq!(
            g.places(true),
            vec![(
                0,
                Place {
                    front: true,
                    seen: true,
                    order: Some(step(0)),
                }
            )]
        );
        g.front_to(1);
        let moved = g.places(false);
        assert_eq!(
            moved,
            vec![
                (
                    0,
                    Place {
                        front: false,
                        seen: false,
                        order: Some(step(0)),
                    }
                ),
                (
                    1,
                    Place {
                        front: true,
                        seen: false,
                        order: Some(step(1)),
                    }
                )
            ]
        );
    }

    #[test]
    fn a_renumbered_group_tells_each_holder_its_new_place() {
        // FAILS IF a gap that closed renumbers the group in the window alone:
        // the holders would keep their old numbers, and a window opened on
        // the group once this one is gone (`--show`) would lay it out in an
        // order nobody chose — or a holder whose place did not move is told
        // again.
        let mut g = group_of(&[step(0), step(0) + 1]);
        for (i, p) in g.places(true) {
            g.told(i, p);
        }
        // AFTER THE FRONT, between two places one apart: no middle, so the
        // whole group is numbered afresh.
        let o = g.order_after_front();
        let orders: Vec<u32> = g.iter().map(|h| h.order).collect();
        assert!(orders[0] < o && o < orders[1], "{orders:?} {o}");
        let told = g.places(true);
        assert!(
            told.iter().all(|(i, p)| p.order == Some(orders[*i])),
            "{told:?} {orders:?}"
        );
        assert!(
            told.iter().any(|(i, _)| *i == 1),
            "the holder whose number moved was not told: {told:?}"
        );
        for (i, p) in told {
            g.told(i, p);
        }
        assert!(g.places(true).is_empty(), "told twice");
    }

    #[test]
    fn front_asked_fronts_only_when_it_moves() {
        // FAILS IF the first state a window hears fronts its terminal (every
        // terminal of a restored group fighting for the front), or a request
        // to front it — the count moving — is not heard.
        let mut h = held(0, step(0));
        let mut state = state();
        state.front_asked = 4;
        assert!(
            !h.heard(Box::new(state.clone())),
            "the first state only counts"
        );
        assert!(!h.heard(Box::new(state.clone())), "nothing moved");
        state.front_asked = 5;
        assert!(h.heard(Box::new(state)));
    }

    pub(crate) fn state() -> TerminalState {
        TerminalState {
            protocol: ronoctua_terminals::PROTOCOL,
            id: TerminalId::fresh().unwrap(),
            group: ronoctua_terminals::GroupId::fresh().unwrap(),
            order: 0,
            viewer: Some(1),
            front: true,
            seen: false,
            title: String::new(),
            program: "sh".to_owned(),
            cwd: None,
            started_s: 0,
            alone_since_s: None,
            attention: ronoctua_terminals::state::Attention::default(),
            notice: None,
            command: None,
            front_asked: 0,
            ended: None,
            statuses: Vec::new(),
            progress: None,
        }
    }
}
