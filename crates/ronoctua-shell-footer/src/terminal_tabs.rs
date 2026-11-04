//! A GROUP OF TERMINALS ON THE MODELINE — its tabs, as the kiosk's are
//! Chrome's (`crate::kiosk`), while its window is the one in focus.
//!
//! # What it claims
//!
//! The focused window whose process is the viewer of two terminals or more:
//! a group (`ronoctua_terminals::state::TerminalState::viewer`). One window
//! per viewer process — the rule `terminal-show` already finds a terminal's
//! window by. With one terminal there is no group, and the modeline says
//! what it says of any window: a modeline at rest says nothing.
//!
//! # What it draws, and what answers
//!
//! A tab per terminal, in the group's order, named as the census names it
//! (its program's title, else the program): the one in front in the line's
//! first tone, a terminal that asks for the person in caution's, the rest
//! quieter — never the accent, which on this line says where the keyboard
//! is. The window keeps its thin lines across its top (the owner, 2026-11-03:
//! a group out of focus, or on another screen, still shows it is one).
//!
//! Every answer is a command of the registry (`crate::adapt`): a click brings
//! a terminal to the front (`terminal-show`), the wheel walks the group from
//! the one in front, and the middle button ends one — ASKED FIRST, `no`
//! first, on the background terminals' view (`terminals-end`, the view's
//! `d`): it hangs up a shell and whatever runs in it, which no tab of
//! Chrome's closing is.

use ronoctua_compositor::Window;
use ronoctua_i18n::{Arg, Texts};
use ronoctua_render::layout::{self, Segment, Shrink, Side};
use ronoctua_terminals::TerminalId;
use ronoctua_terminals::state::TerminalState;
use ronoctua_tokens as tokens;

use crate::adapt::{Act, Button, ModelineAdapter, Strip};
use crate::key;

/// The adapter's id: its switch in `modeline.json`.
pub const ADAPTER: &str = "terminals";

/// How many terminals make a group — the window's strip draws from the same
/// count (`ronoctua_shell_terminal::terminal_strip::FEWEST`).
const FEWEST: usize = 2;

/// The tabs' pool — `Segment::id` is `&'static str`, so the tabs are named by
/// their place, as the kiosk's are (`crate::kiosk::TABS`). Sixteen around
/// the one in front; the rest are counted.
pub const TABS: [&str; 16] = [
    "terminal-tab-1",
    "terminal-tab-2",
    "terminal-tab-3",
    "terminal-tab-4",
    "terminal-tab-5",
    "terminal-tab-6",
    "terminal-tab-7",
    "terminal-tab-8",
    "terminal-tab-9",
    "terminal-tab-10",
    "terminal-tab-11",
    "terminal-tab-12",
    "terminal-tab-13",
    "terminal-tab-14",
    "terminal-tab-15",
    "terminal-tab-16",
];

/// The counter before the tabs that are drawn.
pub const BEFORE: &str = "terminal-tabs-before";
/// The counter after them.
pub const AFTER: &str = "terminal-tabs-after";

/// What brings a terminal to the front: its window focused, the terminal in
/// front in it (`surface/terminals.rs`).
const SHOW: &str = "terminal-show";

/// What ends one, asked first (`ronoctua_shell_terminal::terminals_region`).
const END: &str = "terminals-end";

/// The tab in front never elides below this many cells: fewer is not a word.
const FRONT_FLOOR: u16 = 6;

/// The terminals on either side of the one in front, which the wheel and
/// the counters step to — the group's ends meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Around {
    previous: TerminalId,
    next: TerminalId,
}

/// What a segment of the group's modeline is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A terminal's tab.
    Tab {
        id: TerminalId,
        front: bool,
        asking: bool,
        title: String,
        around: Around,
    },
    /// The counter before the drawn tabs, with how many it counts.
    Before(usize, Around),
    /// The counter after them.
    After(usize, Around),
}

/// The group's modeline, over the footer's census of the held terminals.
pub struct Adapter<'a> {
    terminals: &'a [TerminalState],
}

impl<'a> Adapter<'a> {
    /// Over the census, as the footer last read it.
    #[must_use]
    pub const fn new(terminals: &'a [TerminalState]) -> Self {
        Self { terminals }
    }

    /// The group `w` shows, in its order: the terminals its process is the
    /// viewer of and whose program still runs — none when it is not one.
    fn group_of(&self, w: &Window) -> Vec<&'a TerminalState> {
        let Some(pid) = w.pid.and_then(|p| u32::try_from(p).ok()) else {
            return Vec::new();
        };
        let mut group: Vec<&TerminalState> = self
            .terminals
            .iter()
            .filter(|t| t.viewer == Some(pid) && t.ended.is_none())
            .collect();
        group.sort_by(|a, b| (a.order, a.started_s, &a.id).cmp(&(b.order, b.started_s, &b.id)));
        if group.len() < FEWEST {
            group.clear();
        }
        group
    }
}

/// The place of the one in front, the first while none says it is.
fn front_of(group: &[&TerminalState]) -> usize {
    group.iter().position(|t| t.front).unwrap_or(0)
}

/// Which of the group's tabs have a place in the pool: `(first, how many)`,
/// centred on the one in front.
fn pooled(group: &[&TerminalState]) -> (usize, usize) {
    let n = group.len();
    let len = n.min(TABS.len());
    let first = front_of(group).saturating_sub(len / 2).min(n - len);
    (first, len)
}

/// The terminals either side of the one in front.
fn around(group: &[&TerminalState]) -> Around {
    let at = front_of(group);
    let n = group.len();
    Around {
        previous: group[(at + n - 1) % n].id.clone(),
        next: group[(at + 1) % n].id.clone(),
    }
}

/// How a terminal goes by on its tab: its program's title, else its program.
fn name_of(t: &TerminalState) -> &str {
    if t.title.trim().is_empty() {
        &t.program
    } else {
        &t.title
    }
}

/// The counter's text for `n` tabs.
fn counter(n: usize, texts: &Texts) -> String {
    texts.t_args(
        key::TERMINAL_TABS_MORE,
        &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
    )
}

impl ModelineAdapter for Adapter<'_> {
    type Target = Target;

    fn id(&self) -> &'static str {
        ADAPTER
    }

    fn claims(&self, w: &Window) -> bool {
        !self.group_of(w).is_empty()
    }

    fn segments(&self, w: &Window, texts: &Texts) -> Vec<(Segment, Target)> {
        let group = self.group_of(w);
        if group.is_empty() {
            return Vec::new();
        }
        let near = around(&group);
        let (first, len) = pooled(&group);
        let front = front_of(&group);
        // THE COUNTERS are written as the widest they could be; the second
        // pass of `adapt::resolve` writes the real number over the ones that
        // survive.
        let widest = counter(group.len(), texts);
        let counter_seg = |id: &'static str| Segment {
            id,
            text: widest.clone(),
            priority: 7,
            min_cols: 1,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
            place: None,
        };
        let mut out = vec![(counter_seg(BEFORE), Target::Before(0, near.clone()))];
        for (slot, (i, t)) in group.iter().enumerate().skip(first).take(len).enumerate() {
            let target = Target::Tab {
                id: t.id.clone(),
                front: i == front,
                asking: t.asks_attention(),
                title: name_of(t).to_owned(),
                around: near.clone(),
            };
            let seg = if i == front {
                Segment {
                    id: TABS[slot],
                    text: layout::elide_end(name_of(t), tokens::space::TERMINAL_TAB_FRONTCELLS),
                    priority: 5,
                    min_cols: FRONT_FLOOR,
                    side: Side::Left,
                    shrink: Shrink::AtTheEnd,
                    removable: false,
                    place: None,
                }
            } else {
                // FARTHEST FIRST: the rung grows with the distance from the
                // one in front, so what survives is the tabs around it, and
                // the counters count what is outside them.
                let distance = u8::try_from(i.abs_diff(front)).unwrap_or(u8::MAX);
                Segment {
                    id: TABS[slot],
                    text: layout::elide_end(name_of(t), tokens::space::TERMINAL_TAB_CELLS),
                    priority: 20u8.saturating_add(distance),
                    min_cols: 1,
                    side: Side::Left,
                    shrink: Shrink::Cut,
                    removable: true,
                    place: None,
                }
            };
            out.push((seg, target));
        }
        out.push((counter_seg(AFTER), Target::After(0, near)));
        out
    }

    fn strip(&self, w: &Window) -> Option<Strip> {
        let group = self.group_of(w);
        if group.is_empty() {
            return None;
        }
        let (first, len) = pooled(&group);
        Some(Strip {
            before: BEFORE,
            after: AFTER,
            members: TABS[..len].to_vec(),
            beyond: (first, group.len() - first - len),
            more: key::TERMINAL_TABS_MORE,
        })
    }

    fn colour(&self, t: &Target) -> Option<tokens::Color> {
        // NEVER THE ACCENT, which on this line says where the keyboard is —
        // the window's strip keeps the same three tones.
        Some(match t {
            Target::Tab { asking: true, .. } => tokens::color::STATE_CAUTION,
            Target::Tab { front: true, .. } => tokens::color::TEXT_PRIMARY,
            _ => tokens::color::TEXT_SECONDARY,
        })
    }

    fn clicked(&self, t: &Target, b: Button) -> Option<Act> {
        match (t, b) {
            (Target::Tab { id, .. }, Button::Left) => Some(Act::RunWith(SHOW, id.to_string())),
            // THE MIDDLE BUTTON ENDS IT, as it closes a browser's tab — asked
            // first, `no` first: a shell and what runs in it go.
            (Target::Tab { id, .. }, Button::Middle) => Some(Act::RunWith(END, id.to_string())),
            (Target::Before(_, near), Button::Left) => {
                Some(Act::RunWith(SHOW, near.previous.to_string()))
            }
            (Target::After(_, near), Button::Left) => {
                Some(Act::RunWith(SHOW, near.next.to_string()))
            }
            _ => None,
        }
    }

    fn wheeled(&self, t: &Target, lines: i64) -> Option<Act> {
        let near = match t {
            Target::Tab { around, .. } | Target::Before(_, around) | Target::After(_, around) => {
                around
            }
        };
        // DOWN IS THE NEXT ONE — the kiosk's tabs' convention, and the
        // window's strip's.
        match lines.signum() {
            1 => Some(Act::RunWith(SHOW, near.next.to_string())),
            -1 => Some(Act::RunWith(SHOW, near.previous.to_string())),
            _ => None,
        }
    }

    fn draggable(&self, _: &Target) -> bool {
        false
    }

    fn dropped(&self, _: &Target, _: &Target) -> Option<Act> {
        None
    }

    fn hint(&self, t: &Target, texts: &Texts) -> Option<String> {
        let count = |n: usize| Arg::Number(i64::try_from(n).unwrap_or(i64::MAX));
        Some(match t {
            Target::Tab {
                front: true, title, ..
            } => texts.t_with(key::HINT_TERMINAL_TAB_FRONT, &[("title", title)]),
            Target::Tab { title, .. } => texts.t_with(key::HINT_TERMINAL_TAB, &[("title", title)]),
            Target::Before(n, _) => {
                texts.t_args(key::HINT_TERMINAL_TABS_BEFORE, &[("count", count(*n))])
            }
            Target::After(n, _) => {
                texts.t_args(key::HINT_TERMINAL_TABS_AFTER, &[("count", count(*n))])
            }
        })
    }

    fn counted(&self, t: &Target, n: usize) -> Target {
        match t {
            Target::Before(_, near) => Target::Before(n, near.clone()),
            Target::After(_, near) => Target::After(n, near.clone()),
            other => other.clone(),
        }
    }

    /// NONE OF ITS OWN: the window publishes its keys itself (`LEADER Alt`),
    /// and the footer asks it for them as it asks any window.
    fn local_bindings(&self) -> &'static [crate::window_menu::Generated] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapt::{Adapted, Line};
    use ronoctua_terminals::GroupId;
    use ronoctua_terminals::state::{Attention, Ended};

    const VIEWER: u32 = 4242;

    fn id(n: u8) -> TerminalId {
        format!("{n:016x}").parse().expect("an id")
    }

    fn terminal(n: u8, order: u32, viewer: Option<u32>, front: bool, title: &str) -> TerminalState {
        TerminalState {
            protocol: ronoctua_terminals::PROTOCOL,
            id: id(n),
            group: "00000000000000aa".parse::<GroupId>().expect("a group"),
            order,
            viewer,
            front,
            seen: false,
            title: title.to_owned(),
            program: "zsh".to_owned(),
            cwd: None,
            started_s: 1_790_000_000,
            alone_since_s: None,
            attention: Attention::default(),
            notice: None,
            command: None,
            front_asked: 0,
            ended: None,
            statuses: Vec::new(),
            progress: None,
        }
    }

    fn window(pid: u32) -> Window {
        Window {
            id: 7,
            title: Some("zsh".to_owned()),
            app_id: Some("br.ronoctua.terminal".to_owned()),
            pid: Some(i32::try_from(pid).expect("a pid")),
            is_focused: true,
            floating: false,
            fullscreen: false,
            sticky: false,
            drawer: false,
            hidden: false,
            trailmark: false,
            rect: None,
        }
    }

    fn texts() -> Texts {
        Texts::fresh("pt-BR").expect("the texts")
    }

    fn tabs_of(targets: &[(&'static str, crate::adapt::Target)]) -> Vec<Target> {
        targets
            .iter()
            .filter_map(|(_, t)| match t {
                crate::adapt::Target::Terminals(t @ Target::Tab { .. }) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    /// FAILS IF: one terminal alone, a group of another window, or a group
    /// whose terminals ended, takes the modeline — a modeline at rest says
    /// nothing, and the tabs are the focused window's group's only.
    #[test]
    fn only_a_group_in_the_focused_window_is_claimed() {
        let alone = [terminal(1, 0, Some(VIEWER), true, "a")];
        assert!(
            !Adapter::new(&alone).claims(&window(VIEWER)),
            "one is no group"
        );
        let theirs = [
            terminal(1, 0, Some(VIEWER + 1), true, "a"),
            terminal(2, 1, Some(VIEWER + 1), false, "b"),
        ];
        assert!(
            !Adapter::new(&theirs).claims(&window(VIEWER)),
            "another window's group"
        );
        let mut ended = terminal(2, 1, Some(VIEWER), false, "b");
        ended.ended = Some(Ended { code: 0, at_s: 0 });
        let one_left = [terminal(1, 0, Some(VIEWER), true, "a"), ended];
        assert!(
            !Adapter::new(&one_left).claims(&window(VIEWER)),
            "an ended one counts"
        );
        let ours = [
            terminal(1, 0, Some(VIEWER), true, "a"),
            terminal(2, 1, Some(VIEWER), false, "b"),
            terminal(3, 0, None, false, "away"),
        ];
        assert!(Adapter::new(&ours).claims(&window(VIEWER)));
    }

    /// FAILS IF: the tabs are not the group's in its order, named as the
    /// census names them, the window's title stays beside them, or the one
    /// in front is not the one tab the line may not lose.
    #[test]
    fn the_tabs_are_the_group_in_its_order() {
        let census = [
            terminal(3, 2, Some(VIEWER), false, ""),
            terminal(1, 0, Some(VIEWER), false, "~/main"),
            terminal(2, 1, Some(VIEWER), true, "cargo test"),
        ];
        let a = Adapted::Terminals(Adapter::new(&census));
        let base = vec![
            Segment {
                id: "buffer",
                text: "cargo test".to_owned(),
                priority: 4,
                min_cols: 8,
                side: Side::Left,
                shrink: Shrink::InTheMiddle,
                removable: false,
                place: None,
            },
            Segment {
                id: "mode",
                text: "Terminal".to_owned(),
                priority: 6,
                min_cols: 1,
                side: Side::Left,
                shrink: Shrink::Cut,
                removable: true,
                place: None,
            },
        ];
        let t = texts();
        let line: Line = a.line(base, &window(VIEWER), &t);
        assert!(
            line.segments.iter().all(|s| s.id != "buffer"),
            "the title stayed"
        );
        let tabs = tabs_of(&line.targets);
        let names: Vec<&str> = tabs
            .iter()
            .map(|t| match t {
                Target::Tab { title, .. } => title.as_str(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            names,
            ["~/main", "cargo test", "zsh"],
            "the order or the names"
        );
        let front: Vec<&Segment> = line
            .segments
            .iter()
            .filter(|s| TABS.contains(&s.id) && !s.removable)
            .collect();
        assert_eq!(front.len(), 1, "one tab may not go: {front:?}");
        assert_eq!(front[0].text, "cargo test");
        // AND THE LINE, laid out narrow, keeps the one in front and counts
        // the rest.
        let (placed, drawn) = crate::adapt::resolve(30, &line, Some(&a), &t);
        assert!(
            placed.iter().any(|p| p.id == front[0].id),
            "the front tab went"
        );
        assert!(
            drawn
                .iter()
                .all(|(_, t)| matches!(t, crate::adapt::Target::Terminals(_))),
            "a target that is not the group's"
        );
    }

    /// FAILS IF: a tab's click, its middle button, the wheel or a counter
    /// runs anything but the two commands a key reaches too — or the middle
    /// button ends without asking, or the wheel does not step from the one
    /// in front, round the group's ends.
    #[test]
    fn the_pointer_runs_the_two_commands_with_the_terminal() {
        let census = [
            terminal(1, 0, Some(VIEWER), true, "a"),
            terminal(2, 1, Some(VIEWER), false, "b"),
            terminal(3, 2, Some(VIEWER), false, "c"),
        ];
        let a = Adapter::new(&census);
        let segments = a.segments(&window(VIEWER), &texts());
        let tab = |n: u8| {
            segments
                .iter()
                .map(|(_, t)| t)
                .find(|t| matches!(t, Target::Tab { id: i, .. } if *i == id(n)))
                .expect("its tab")
                .clone()
        };
        let show = |n: u8| Some(Act::RunWith(SHOW, id(n).to_string()));
        assert_eq!(a.clicked(&tab(2), Button::Left), show(2));
        assert_eq!(
            a.clicked(&tab(3), Button::Middle),
            Some(Act::RunWith("terminals-end", id(3).to_string())),
            "the middle button does not ask to end it"
        );
        assert_eq!(a.clicked(&tab(2), Button::Right), None);
        assert_eq!(
            a.wheeled(&tab(3), 1),
            show(2),
            "down is the next of the front"
        );
        assert_eq!(
            a.wheeled(&tab(2), -1),
            show(3),
            "up from the first is the last"
        );
        assert_eq!(a.wheeled(&tab(2), 0), None);
        let before = segments
            .iter()
            .find(|(s, _)| s.id == BEFORE)
            .map(|(_, t)| t.clone())
            .expect("the counter");
        assert_eq!(a.clicked(&before, Button::Left), show(3));
        assert!(!a.draggable(&tab(2)));
    }

    /// FAILS IF: the tab of a terminal that asks for the person is not in
    /// caution's tone, or the line's accent comes to the tabs.
    #[test]
    fn a_tab_that_asks_is_in_caution() {
        let mut asking = terminal(2, 1, Some(VIEWER), false, "b");
        asking.attention.bell = 1;
        let census = [terminal(1, 0, Some(VIEWER), true, "a"), asking];
        let a = Adapter::new(&census);
        let tones: Vec<tokens::Color> = a
            .segments(&window(VIEWER), &texts())
            .iter()
            .filter_map(|(_, t)| {
                matches!(t, Target::Tab { .. })
                    .then(|| a.colour(t))
                    .flatten()
            })
            .collect();
        assert_eq!(
            tones,
            [tokens::color::TEXT_PRIMARY, tokens::color::STATE_CAUTION]
        );
    }
}
