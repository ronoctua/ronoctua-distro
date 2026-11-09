//! A WINDOW'S TABS ON THE MODELINE — the screens it shows one at a time, as
//! the window publishes them (`door::Content::tabs`), while it is in focus.
//!
//! # What it claims
//!
//! The focused window whose process published two tabs or more on its socket
//! (`ipc::buffer_tabs`, read with its shortcuts, and again when it says they
//! changed). The owner's ask (2026-11-09): the icons' window draws its three
//! sets nowhere a hand can reach with its header hidden, and the footer's
//! view of it has them on its first row — so the modeline is their row while
//! the window is in front, as it is a group of terminals'
//! (`crate::terminal_tabs`). It names no window: what a tab is, and what it
//! is called, is the window's.
//!
//! # What it draws, and what answers
//!
//! A tab per screen, in the window's order: the one shown in the line's first
//! tone, the rest quieter — never the accent, which on this line says where
//! the keyboard is. A click on a tab, and the wheel stepping from the one
//! shown, run the tab's command IN THE WINDOW ([`Act::Window`]): one of the
//! shortcuts it publishes, by the one door `LEADER Alt` runs them through, so
//! the mouse reaches nothing the keyboard does not. Under the pointer the
//! minibuffer says what the window says of that shortcut.

use ronoctua_compositor::Window;
use ronoctua_i18n::Texts;
use ronoctua_render::layout::{self, Segment, Shrink, Side};
use ronoctua_shell_kit::ipc::{Shortcut, WindowTab};
use ronoctua_tokens as tokens;

use crate::adapt::{Act, Button, ModelineAdapter, Strip};

/// The adapter's id: its switch in `modeline.json`.
pub const ADAPTER: &str = "tabs";

/// How many tabs make a row of them: one is no choice.
const FEWEST: usize = 2;

/// The tabs' pool — `Segment::id` is `&'static str`, so the tabs are named by
/// their place, as the terminals' are (`crate::terminal_tabs::TABS`). A
/// window with more draws the first nine.
pub const TABS: [&str; 9] = [
    "window-tab-1",
    "window-tab-2",
    "window-tab-3",
    "window-tab-4",
    "window-tab-5",
    "window-tab-6",
    "window-tab-7",
    "window-tab-8",
    "window-tab-9",
];

/// The longest a tab's name is drawn, in cells.
const NAME_CELLS: u16 = 24;

/// What a tab's segment is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The window's command that shows it.
    pub command: String,
    /// Its name.
    pub name: String,
    /// Whether it is the one shown.
    pub current: bool,
    /// The commands of the tabs either side of the one shown, which the
    /// wheel steps to — the row's ends meet.
    pub previous: String,
    /// The next one's.
    pub next: String,
}

/// The tabs of the window in focus, as it published them.
pub struct Adapter<'a> {
    tabs: &'a [WindowTab],
    shortcuts: &'a [Shortcut],
    pid: Option<u32>,
}

impl<'a> Adapter<'a> {
    /// Over what the window of process `pid` published: its tabs, and its
    /// shortcuts, whose sentences say what a tab does.
    #[must_use]
    pub const fn new(tabs: &'a [WindowTab], shortcuts: &'a [Shortcut], pid: Option<u32>) -> Self {
        Self {
            tabs,
            shortcuts,
            pid,
        }
    }

    /// The tabs drawn, when `w` is the window that published them.
    fn drawn(&self, w: &Window) -> &'a [WindowTab] {
        let published = w.pid.and_then(|p| u32::try_from(p).ok());
        if published.is_none() || published != self.pid || self.tabs.len() < FEWEST {
            return &[];
        }
        &self.tabs[..self.tabs.len().min(TABS.len())]
    }
}

impl ModelineAdapter for Adapter<'_> {
    type Target = Target;

    fn id(&self) -> &'static str {
        ADAPTER
    }

    fn claims(&self, w: &Window) -> bool {
        !self.drawn(w).is_empty()
    }

    fn segments(&self, w: &Window, _texts: &Texts) -> Vec<(Segment, Target)> {
        let tabs = self.drawn(w);
        let n = tabs.len();
        if n == 0 {
            return Vec::new();
        }
        let shown = tabs.iter().position(|t| t.current).unwrap_or(0);
        let previous = tabs[(shown + n - 1) % n].command.clone();
        let next = tabs[(shown + 1) % n].command.clone();
        tabs.iter()
            .enumerate()
            .map(|(i, t)| {
                // THE ONE SHOWN STAYS; the farthest from it go first.
                let distance = u8::try_from(i.abs_diff(shown)).unwrap_or(u8::MAX);
                let seg = Segment {
                    id: TABS[i],
                    text: layout::elide_end(&t.name, NAME_CELLS),
                    priority: if i == shown {
                        5
                    } else {
                        20u8.saturating_add(distance)
                    },
                    min_cols: 1,
                    side: Side::Left,
                    shrink: if i == shown {
                        Shrink::AtTheEnd
                    } else {
                        Shrink::Cut
                    },
                    removable: i != shown,
                    place: None,
                };
                let target = Target {
                    command: t.command.clone(),
                    name: t.name.clone(),
                    current: i == shown,
                    previous: previous.clone(),
                    next: next.clone(),
                };
                (seg, target)
            })
            .collect()
    }

    fn strip(&self, _: &Window) -> Option<Strip> {
        None
    }

    fn colour(&self, t: &Target) -> Option<tokens::Color> {
        Some(if t.current {
            tokens::color::TEXT_PRIMARY
        } else {
            tokens::color::TEXT_SECONDARY
        })
    }

    fn clicked(&self, t: &Target, b: Button) -> Option<Act> {
        match b {
            Button::Left => Some(Act::Window(t.command.clone())),
            Button::Right | Button::Middle => None,
        }
    }

    fn wheeled(&self, t: &Target, lines: i64) -> Option<Act> {
        // DOWN IS THE NEXT ONE — the terminals' tabs' convention.
        match lines.signum() {
            1 => Some(Act::Window(t.next.clone())),
            -1 => Some(Act::Window(t.previous.clone())),
            _ => None,
        }
    }

    fn draggable(&self, _: &Target) -> bool {
        false
    }

    fn dropped(&self, _: &Target, _: &Target) -> Option<Act> {
        None
    }

    /// WHAT THE WINDOW SAYS OF THE TAB'S SHORTCUT — the sentence it publishes
    /// beside it — and its name where it says none.
    fn hint(&self, t: &Target, _texts: &Texts) -> Option<String> {
        let said = self
            .shortcuts
            .iter()
            .find(|s| s.command == t.command)
            .and_then(|s| s.sentence.clone());
        Some(said.unwrap_or_else(|| t.name.clone()))
    }

    fn counted(&self, t: &Target, _: usize) -> Target {
        t.clone()
    }

    /// NONE OF ITS OWN: the window publishes its keys itself.
    fn local_bindings(&self) -> &'static [crate::window_menu::Generated] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapt::Adapted;

    const PID: u32 = 4242;

    fn tab(command: &str, name: &str, current: bool) -> WindowTab {
        WindowTab {
            command: command.to_owned(),
            name: name.to_owned(),
            current,
        }
    }

    fn window(pid: u32) -> Window {
        Window {
            id: 7,
            title: Some("*Ícones*".to_owned()),
            app_id: Some("br.ronoctua.icons".to_owned()),
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

    /// FAILS IF: the window's tabs are not drawn in its order, the one shown
    /// in the first tone and never removed; a click and the wheel do not run
    /// the window's own command; or the line is claimed for a window with one
    /// tab, or for another window than the one that published them.
    #[test]
    fn a_windows_tabs_are_its_own_and_run_its_commands() {
        let tabs = [
            tab("icons-tab-nerd-fonts", "Nerd Fonts", false),
            tab("icons-tab-lucide", "Lucide", true),
            tab("icons-tab-unicode", "Unicode", false),
        ];
        let shortcuts = [Shortcut {
            keys: "2".to_owned(),
            command: "icons-tab-lucide".to_owned(),
            words: Some("Lucide".to_owned()),
            sentence: Some("Shows the Lucide set.".to_owned()),
        }];
        let texts = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let a = Adapter::new(&tabs, &shortcuts, Some(PID));
        let w = window(PID);
        assert!(a.claims(&w));
        assert!(
            !a.claims(&window(PID + 1)),
            "another window's tabs were drawn"
        );
        assert!(
            !Adapter::new(&tabs[..1], &shortcuts, Some(PID)).claims(&w),
            "one tab is no row"
        );
        let segs = a.segments(&w, &texts);
        assert_eq!(
            segs.iter()
                .map(|(s, _)| s.text.as_str())
                .collect::<Vec<_>>(),
            ["Nerd Fonts", "Lucide", "Unicode"]
        );
        let (shown_seg, shown) = &segs[1];
        assert!(!shown_seg.removable && shown.current);
        assert_eq!(a.colour(shown), Some(tokens::color::TEXT_PRIMARY));
        assert_eq!(a.colour(&segs[0].1), Some(tokens::color::TEXT_SECONDARY));
        assert_eq!(
            a.clicked(&segs[2].1, Button::Left),
            Some(Act::Window("icons-tab-unicode".to_owned()))
        );
        assert_eq!(
            a.wheeled(shown, 1),
            Some(Act::Window("icons-tab-unicode".to_owned()))
        );
        assert_eq!(
            a.wheeled(shown, -1),
            Some(Act::Window("icons-tab-nerd-fonts".to_owned()))
        );
        assert_eq!(
            a.hint(shown, &texts).as_deref(),
            Some("Shows the Lucide set.")
        );
        assert_eq!(a.hint(&segs[0].1, &texts).as_deref(), Some("Nerd Fonts"));
        // AND THROUGH THE ONE ENUM, as the footer asks it.
        let line = Adapted::Tabs(a).line(Vec::new(), &w, &texts);
        assert_eq!(line.targets.len(), 3);
    }
}
