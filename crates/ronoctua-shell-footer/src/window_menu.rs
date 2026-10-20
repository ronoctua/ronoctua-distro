//! Everything that can be done to the focused window, as a grid of controls —
//! the modeline's app name, clicked.
//!
//! # Why it exists
//!
//! Because the pointer had no way to do any of it. Every item here was already
//! a command with a key (`LEADER w …`, `LEADER TAB m 3`, a chord of the
//! compositor's), and a person with a hand on the mouse had to let go of it to
//! float a window. The owner asked for the whole of it behind one click, with a
//! submenu where an item is a choice among many — the workspace to send the
//! window to is ten items, not one.
//!
//! # It is the leader's grid, and the leader's grammar
//!
//! One key and one label per cell, columns filling the width, a group marked
//! with the `+` and drawn in the group's tone. A key or a click runs a cell;
//! a group replaces the grid with its own. What differs is the way back: `Esc`
//! goes up ONE level here, because a submenu is a place a person chose to go.
//!
//! # A toggle is drawn on one side
//!
//! The compositor's floating, fullscreen and sticky are toggles, and a menu that
//! said "toggle floating" would make the reader work out which way it goes. The
//! window's state comes in with it (`ronoctua_compositor::Window`), so the item
//! says what it WILL do: "Float" on a tiled window, "Back into the layout" on a
//! floating one, and "Stick" only where sticking means something.
//!
//! # What runs, runs on the window the menu was opened for
//!
//! The deeds act on the FOCUSED window, and the footer's keyboard grab does not
//! move the compositor's focus — so the menu closes first and the deed then
//! reaches the window whose name was clicked.

use ronoctua_compositor::{Trails, Window, Workspace};

use crate::key;
use crate::selection::Selection;
use ronoctua_i18n::Texts;

/// The nine workspaces the menu offers, on the keys that reach them — nine,
/// the owner's rule, and never a tenth.
const WORKSPACES: [(u32, &str); 9] = [
    (1, "1"),
    (2, "2"),
    (3, "3"),
    (4, "4"),
    (5, "5"),
    (6, "6"),
    (7, "7"),
    (8, "8"),
    (9, "9"),
];

/// Where in the menu the grid is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Everything, and the doors to the rest.
    Root,
    /// The workspace to send the window to — and whether the view goes with
    /// it (`follow`) or stays where it is.
    SendTo {
        /// Whether the view follows the window there.
        follow: bool,
    },
    /// Where in the layout, or on which screen.
    Move,
    /// Against which edge, or in the middle.
    Align,
    /// Its width and height.
    Size,
    /// A width, as a share of the screen.
    Width,
    /// A height, as a share of the screen.
    Height,
    /// How big what is INSIDE it is drawn.
    Content,
    /// How it looks: opacity, border, shadow, dimming.
    Look,
    /// What it may do to the machine: keep the screen awake, tear, take the
    /// shortcuts.
    Behaviour,
    /// The three kinds of fullscreen the compositor has.
    Fullscreen,
    /// What the drawer holds — every window put away, and the two ways the
    /// compositor has of taking one out.
    Drawer,
    /// The trails: this window on the active one or off it, walking it, and
    /// making, choosing, emptying and deleting trails.
    Trail,
    /// The windows selected to be acted on together, and what can be done to
    /// them.
    Selection,
    /// A question about something that cannot be undone.
    Confirm(Danger),
}

/// What a question is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Danger {
    /// The window's process, killed — whatever it had not saved is lost.
    Kill,
    /// Every other window, asked to close.
    CloseOthers,
    /// The trail in use, deleted — the windows stay, the list of them goes.
    DeleteTrail,
}

/// What choosing a cell does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Does {
    /// Runs a command of the registry.
    Run(&'static str),
    /// Runs a command of the registry with its argument.
    RunWith(&'static str, String),
    /// Opens a submenu.
    Enter(Level),
    /// Kills the window's process.
    Kill,
    /// Takes this window out of the drawer.
    TakeOut(u64),
    /// Puts this window, which the drawer let out, back in it.
    PutBack(u64),
    /// Nothing — the question's "no", which only goes back.
    Back,
}

/// One cell of the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The key that chooses it, in the keymap's notation.
    pub key: &'static str,
    /// What it says.
    pub label: String,
    /// The whole sentence, for the pointer resting on it.
    pub about: Option<String>,
    /// What choosing it does.
    pub does: Does,
    /// Whether the menu STAYS OPEN after it runs: a step — a size, a zoom, a
    /// move — is something a person clicks several times in a row, and a menu
    /// that closed after each would be opened again for each.
    pub stays: bool,
    /// Whether it acts on the menu's window, focused first
    /// (`Compositor::deed_on`). Not for walking a trail: that moves the focus
    /// itself, and focusing the menu's window first would walk from the same
    /// place every time.
    pub aimed: bool,
    /// A group's label, as the KEY it is written with — what the leader's
    /// group of the same name is labelled with (`leader_bindings`).
    pub names: Option<&'static str>,
}

impl Entry {
    /// The same cell, leaving the menu open after it runs.
    #[must_use]
    pub const fn stays(mut self) -> Self {
        self.stays = true;
        self
    }

    /// The same cell, acting on whatever the compositor has focused.
    #[must_use]
    pub const fn loose(mut self) -> Self {
        self.aimed = false;
        self
    }

    /// Whether it opens a submenu rather than doing something.
    #[must_use]
    pub const fn is_group(&self) -> bool {
        matches!(self.does, Does::Enter(_))
    }
}

/// What the footer does after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing but drawing again.
    Redraw,
    /// A click PICKED a cell and ran nothing ([`View::clicked`]): drawn again,
    /// on the page it is on — a new level's first page is `Redraw`'s.
    Picked,
    /// Close the menu.
    Close,
    /// Run this command — on the window `on` names, focused first, when it
    /// names one — and close the menu unless it `stays`.
    Run {
        /// The command.
        id: &'static str,
        /// Its argument.
        arg: Option<String>,
        /// The window it acts on.
        on: Option<u64>,
        /// Whether the menu stays open.
        stays: bool,
    },
    /// Close the menu, then kill this process.
    Kill(i32),
    /// Close the menu, then take this window out of the drawer.
    TakeOut(u64),
}

/// The menu, about one window — or about the drawer, which is the one level
/// that needs no window: what it lists is what no focus can reach.
#[derive(Debug, Clone)]
pub struct View {
    /// The window it was opened for, as the compositor described it then.
    pub window: Option<Window>,
    /// Every window put away in the drawer.
    drawer: Vec<Window>,
    /// The trails, as the compositor last said them.
    trails: Trails,
    /// What this footer knows is selected (`crate::selection`).
    selection: Selection,
    /// Every workspace there is.
    workspaces: Vec<Workspace>,
    /// How many screens there are: moving to another one needs another one.
    screens: usize,
    /// Whether the window is a Chrome kiosk (`crate::kiosk`): its own view
    /// is one cell away, which is what reaches it when the modeline is not
    /// adapted to it.
    kiosk: bool,
    /// Where the grid is.
    pub level: Level,
    /// The cell a click picked — or, on a question, its `no`, where the cursor
    /// is born ([`View::picked`]).
    aim: crate::aim::Aim<usize>,
    /// Whether the level on screen appeared UNDER THE HAND that clicked.
    ///
    /// A click on a group, on the question's door or on its `no` replaces the
    /// grid under the pointer, and the second press of a double click landed
    /// on whatever cell came up there — and ran it. So a level a CLICK opened
    /// (or the menu, opened by a click on a line of the region) picks a cell
    /// on one click and runs it on a click on the pick ([`View::clicked`]).
    ///
    /// A level a KEY opened, and the menu opened from the modeline — it grows
    /// above the line that was clicked, never under the pointer — run a cell
    /// on its click, as a cell of the leader does: the grid has no cursor, and
    /// what a cell does is written on it.
    under_the_hand: bool,
}

impl View {
    /// The menu about `window`, at its first level.
    #[must_use]
    pub const fn fresh(window: Window, workspaces: Vec<Workspace>, screens: usize) -> Self {
        Self {
            window: Some(window),
            drawer: Vec::new(),
            trails: Trails {
                count: 0,
                active: None,
                marked: 0,
            },
            selection: Selection::EMPTY,
            workspaces,
            screens,
            kiosk: false,
            level: Level::Root,
            aim: crate::aim::Aim::none(),
            under_the_hand: false,
        }
    }

    /// The menu at the drawer, with no window to be about — the drawer's mark
    /// on the modeline, clicked, or nothing focused when the menu was asked for.
    #[must_use]
    pub fn of_the_drawer(drawer: Vec<Window>) -> Self {
        Self::alone(Level::Drawer).with_drawer(drawer)
    }

    /// The menu at a level that needs no window — the drawer, the trails, the
    /// selection — opened from its mark on the modeline.
    #[must_use]
    pub const fn alone(level: Level) -> Self {
        Self {
            window: None,
            drawer: Vec::new(),
            trails: Trails {
                count: 0,
                active: None,
                marked: 0,
            },
            selection: Selection::EMPTY,
            workspaces: Vec::new(),
            screens: 0,
            kiosk: false,
            level,
            aim: crate::aim::Aim::none(),
            under_the_hand: false,
        }
    }

    /// The same menu, knowing the trails.
    #[must_use]
    pub const fn with_trails(mut self, trails: Trails) -> Self {
        self.trails = trails;
        self
    }

    /// The same menu, knowing what is selected.
    #[must_use]
    pub fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = selection;
        self
    }

    /// The same menu, knowing what the drawer holds.
    #[must_use]
    pub fn with_drawer(mut self, drawer: Vec<Window>) -> Self {
        self.drawer = drawer;
        self
    }

    /// The same menu, opened by a click on a line of the region — under the
    /// hand that clicked, when `yes` ([`View::clicked`]).
    #[must_use]
    pub const fn opened_under_the_hand(mut self, yes: bool) -> Self {
        self.under_the_hand = yes;
        self
    }

    /// Goes to `level` — opened from its mark, or by a click on a line of
    /// the region when `under_the_hand`.
    pub fn go(&mut self, level: Level, under_the_hand: bool) {
        let was = self.level;
        self.level = level;
        self.landed(was);
        self.under_the_hand = under_the_hand;
    }

    /// The cell the cursor is on, if the grid has one here: a click's pick,
    /// or the question's `no`.
    #[must_use]
    pub fn picked(&self) -> Option<usize> {
        self.aim.on().copied()
    }

    /// The grid moved from `was` to the level on screen: what was picked
    /// there is not a cell of this one. A QUESTION is born with the cursor on
    /// its `no`, the first cell — the screen's, so its first click picks it.
    fn landed(&mut self, was: Level) {
        if self.level == was {
            return;
        }
        self.aim = match self.level {
            Level::Confirm(_) => crate::aim::Aim::born(0),
            _ => crate::aim::Aim::none(),
        };
        self.under_the_hand = false;
    }

    /// Whether a click on this level picks before it runs: under the hand,
    /// or a question about what cannot be undone, wherever it was opened.
    const fn picks(&self) -> bool {
        self.under_the_hand || matches!(self.level, Level::Confirm(_))
    }

    /// A CLICK on cell `n`: on a level that [`picks`](Self::picks), the
    /// first click picks it and only a click on the pick runs it; elsewhere
    /// it runs, as its key does. A level the click opens is under the hand.
    pub fn clicked(&mut self, n: usize, texts: &Texts) -> Errand {
        if self.picks() && !self.aim.clicked(n) {
            return Errand::Picked;
        }
        let was = self.level;
        let errand = self.choose(n, texts);
        if self.level != was {
            self.under_the_hand = true;
        }
        errand
    }

    /// The same menu, knowing whether its window is a Chrome kiosk.
    #[must_use]
    pub const fn with_kiosk(mut self, kiosk: bool) -> Self {
        self.kiosk = kiosk;
        self
    }

    /// The number of the workspace in front, which is the one the window is on.
    fn current(&self) -> Option<u32> {
        self.workspaces
            .iter()
            .find(|w| w.focused)
            .and_then(|w| w.number)
    }

    /// The cells of the level the menu is at.
    #[must_use]
    pub fn entries(&self, texts: &Texts) -> Vec<Entry> {
        let floating = self.window.as_ref().is_some_and(|w| w.floating);
        // THE SHORT LABEL IS THE MENU'S, the sentence is the registry's: the
        // `cmd-<id>` of a deed is a whole sentence and would make every column
        // as wide as the longest one.
        let run = |k: &'static str, label: &str, id: &'static str| Entry {
            key: k,
            label: texts.t(label),
            about: Some(texts.t(&format!("cmd-{id}"))),
            does: Does::Run(id),
            stays: false,
            aimed: true,
            names: None,
        };
        let group = |k: &'static str, label: &'static str, to: Level| Entry {
            key: k,
            label: texts.t(label),
            about: None,
            does: Does::Enter(to),
            stays: false,
            aimed: true,
            names: Some(label),
        };
        let with = |k: &'static str, label: &str, id: &'static str, arg: &str| Entry {
            key: k,
            label: texts.t(label),
            about: Some(texts.t(&format!("cmd-{id}"))),
            does: Does::RunWith(id, arg.to_owned()),
            stays: false,
            aimed: true,
            names: None,
        };
        match self.level {
            // THE ROOT IS `LEADER w`, key for key (`leader_bindings`): what
            // that group already had keeps its letter — `f`, `t`, `m`, `c`,
            // `d`, `H L K J` — and what the menu adds takes a letter
            // `w` did not use. `h l k j` (focus), `a`/`e` (in and out of a
            // column, which "move" also has), `w` (switch), `n` (next),
            // `s` (the buffer's state) and `o` (this menu) stay the group's
            // own and are never a cell's.
            Level::Root => {
                let Some(w) = self.window.as_ref() else {
                    return Vec::new();
                };
                let mut out = vec![if w.fullscreen {
                    run("f", key::WINDOW_MENU_UNFULLSCREEN, "window-screen-full")
                } else {
                    run("f", key::WINDOW_MENU_FULLSCREEN, "window-screen-full")
                }];
                out.push(if w.floating {
                    run("t", key::WINDOW_MENU_TILE, "window-float")
                } else {
                    run("t", key::WINDOW_MENU_FLOAT, "window-float")
                });
                // WHAT ONLY A LAID-OUT WINDOW HAS — measured in the pinned
                // source: `fit_size` refuses a floating container, and `align`
                // refuses a floating one or one in fullscreen. A floating
                // window is centred by position instead, which is a different
                // deed, on `C`: one key meaning two deeds would be two leader
                // sequences on one key.
                if w.floating {
                    // STICKING IS A FLOATING WINDOW'S, and so is the floating
                    // mode: a tiled one is laid out on its workspace.
                    out.push(if w.sticky {
                        run("S", key::WINDOW_MENU_UNSTICK, "wm-stick")
                    } else {
                        run("S", key::WINDOW_MENU_STICK, "wm-stick")
                    });
                    out.push(run("C", key::WINDOW_MENU_CENTRE, "wm-float-centre"));
                } else if !w.fullscreen {
                    out.extend([
                        run("m", key::WINDOW_MENU_MAXIMISE, "window-maximize"),
                        run("c", key::WINDOW_MENU_CENTRE, "window-center"),
                    ]);
                }
                // MOVING IS A STEP, on the group's own `H L K J`; a floating
                // window moves by a few pixels on the same keys.
                out.extend([
                    run("H", key::WINDOW_MENU_LEFT, "window-move-left").stays(),
                    run("L", key::WINDOW_MENU_RIGHT, "window-move-right").stays(),
                    run("K", key::WINDOW_MENU_UP, "window-move-up").stays(),
                    run("J", key::WINDOW_MENU_DOWN, "window-move-down").stays(),
                ]);
                // THE DRAWER: this window into it (or back, when it came from
                // there), out of it for good, and what it holds.
                out.push(run(
                    "u",
                    if w.drawer {
                        key::WINDOW_MENU_PUT_BACK
                    } else {
                        key::WINDOW_MENU_PUT_AWAY
                    },
                    "wm-scratchpad-put",
                ));
                if w.drawer {
                    // OUT FOR GOOD: the compositor lets a window go from the
                    // drawer when it stops floating.
                    out.push(run("U", key::WINDOW_MENU_TAKE_OUT, "wm-float-off"));
                }
                if !self.drawer.is_empty() {
                    out.push(group("g", key::WINDOW_MENU_DRAWER, Level::Drawer));
                }
                out.extend([
                    group(
                        "p",
                        key::WINDOW_MENU_SEND_TO,
                        Level::SendTo { follow: false },
                    ),
                    group(
                        "P",
                        key::WINDOW_MENU_TAKE_TO,
                        Level::SendTo { follow: true },
                    ),
                    group("M", key::WINDOW_MENU_MOVE, Level::Move),
                ]);
                if !w.floating && !w.fullscreen {
                    out.push(group("A", key::WINDOW_MENU_ALIGN, Level::Align));
                }
                out.extend([
                    group("z", key::WINDOW_MENU_SIZE, Level::Size),
                    group("i", key::WINDOW_MENU_CONTENT, Level::Content),
                    group("x", key::WINDOW_MENU_LOOK, Level::Look),
                    group("b", key::WINDOW_MENU_BEHAVIOUR, Level::Behaviour),
                    group("F", key::WINDOW_MENU_FULLSCREEN_KINDS, Level::Fullscreen),
                    group("y", key::WINDOW_MENU_TRAIL, Level::Trail),
                    group("v", key::WINDOW_MENU_SELECTION, Level::Selection),
                    run("r", key::WINDOW_MENU_RESIZE, "wm-mode-resize"),
                ]);
                if w.floating {
                    out.push(run("R", key::WINDOW_MENU_FLOAT_KEYS, "wm-mode-floating"));
                }
                // A KIOSK'S OWN VIEW (`crate::kiosk_view`), on `B` for
                // browser — `K` is the group's move up. It is how the pointer
                // reaches the kiosk's toolbar when the modeline is not adapted
                // to it, and the leader walks it into `LEADER w B`.
                if self.kiosk {
                    out.push(run("B", key::WINDOW_MENU_KIOSK, "kiosk-more"));
                }
                out.extend([
                    run("d", key::WINDOW_MENU_CLOSE, "buffer-close"),
                    Entry {
                        key: "D",
                        label: texts.t(key::WINDOW_MENU_CLOSE_OTHERS),
                        about: Some(texts.t(crate::key::CMD_WM_CLOSE_OTHERS)),
                        does: Does::Enter(Level::Confirm(Danger::CloseOthers)),
                        stays: false,
                        aimed: true,
                        names: None,
                    },
                ]);
                // THE PROCESS, and only when there is one to name: a window
                // without a pid has nothing this could send a signal to.
                if w.pid.is_some() {
                    out.push(Entry {
                        key: "X",
                        label: texts.t(key::WINDOW_MENU_KILL),
                        about: Some(texts.t(key::ABOUT_WINDOW_MENU_KILL)),
                        does: Does::Enter(Level::Confirm(Danger::Kill)),
                        stays: false,
                        aimed: true,
                        names: None,
                    });
                }
                out
            }
            Level::SendTo { follow } => {
                // TWO DEEDS, and the difference is the view: one sends the
                // window away and leaves you where you are, the other takes
                // you with it.
                let id = if follow {
                    "workspace-move"
                } else {
                    "wm-workspace-send"
                };
                let current = self.current();
                let mut out: Vec<Entry> = WORKSPACES
                    .iter()
                    .filter(|(n, _)| Some(*n) != current)
                    .map(|&(n, k)| {
                        let busy = self
                            .workspaces
                            .iter()
                            .any(|s| s.number == Some(n) && s.occupied);
                        let number = n.to_string();
                        Entry {
                            key: k,
                            label: texts.t_with(
                                if busy {
                                    key::WINDOW_MENU_WORKSPACE
                                } else {
                                    key::WINDOW_MENU_WORKSPACE_EMPTY
                                },
                                &[("n", &number)],
                            ),
                            about: Some(texts.t(&format!("cmd-{id}"))),
                            does: Does::RunWith(id, number),
                            stays: false,
                            aimed: true,
                            names: None,
                        }
                    })
                    .collect();
                // THE NEIGHBOURS ARE DEEDS THAT FOLLOW, and the compositor has
                // no pair that stays: offered only where the view goes too.
                if follow {
                    out.extend([
                        run("n", key::WINDOW_MENU_NEXT, "wm-workspace-move-next"),
                        run("p", key::WINDOW_MENU_PREVIOUS, "wm-workspace-move-previous"),
                    ]);
                }
                out
            }
            Level::Move => {
                // A FLOATING WINDOW MOVES BY PIXELS, the floating mode's step:
                // its left, right, up and down are nudges (measured — `move
                // left` on a floating container moves it by an amount), and
                // the column's beginning, end, pins and way out refuse it.
                // Every one a STEP, so the menu stays for the next click.
                // THE PLAIN FOUR ARE ON THE ROOT (`H L K J`); here is the rest.
                // A floating window moves by a bigger step, the floating
                // mode's; the column's own moves refuse it.
                let mut out = if floating {
                    let by = crate::actions::NUDGE_STEP;
                    vec![
                        with("h", key::WINDOW_MENU_NUDGE_LEFT, "wm-nudge-left", by).stays(),
                        with("l", key::WINDOW_MENU_NUDGE_RIGHT, "wm-nudge-right", by).stays(),
                        with("k", key::WINDOW_MENU_NUDGE_UP, "wm-nudge-up", by).stays(),
                        with("j", key::WINDOW_MENU_NUDGE_DOWN, "wm-nudge-down", by).stays(),
                    ]
                } else {
                    vec![
                        run("H", key::WINDOW_MENU_OUT_LEFT, "wm-move-out-left").stays(),
                        run("L", key::WINDOW_MENU_OUT_RIGHT, "wm-move-out-right").stays(),
                        run("K", key::WINDOW_MENU_OUT_UP, "wm-move-out-up").stays(),
                        run("J", key::WINDOW_MENU_OUT_DOWN, "wm-move-out-down").stays(),
                        run("b", key::WINDOW_MENU_BEGINNING, "wm-move-beginning"),
                        run("e", key::WINDOW_MENU_END, "wm-move-end"),
                        run("B", key::WINDOW_MENU_OUT_BEGINNING, "wm-move-out-beginning"),
                        run("E", key::WINDOW_MENU_OUT_END, "wm-move-out-end"),
                        run("p", key::WINDOW_MENU_PIN_BEGINNING, "wm-pin-beginning"),
                        run("P", key::WINDOW_MENU_PIN_END, "wm-pin-end"),
                    ]
                };
                // ANOTHER SCREEN ONLY WHERE THERE IS ONE: with a single output
                // the two items would be controls that do nothing.
                if self.screens > 1 {
                    out.extend([
                        run("[", key::WINDOW_MENU_SCREEN_LEFT, "wm-move-screen-left"),
                        run("]", key::WINDOW_MENU_SCREEN_RIGHT, "wm-move-screen-right"),
                    ]);
                }
                out
            }
            Level::Align => vec![
                run("h", key::WINDOW_MENU_ALIGN_LEFT, "wm-align-left"),
                run("l", key::WINDOW_MENU_ALIGN_RIGHT, "wm-align-right"),
                run("c", key::WINDOW_MENU_CENTRE, "wm-align-centre"),
                run("k", key::WINDOW_MENU_ALIGN_TOP, "wm-align-top"),
                run("j", key::WINDOW_MENU_ALIGN_BOTTOM, "wm-align-bottom"),
                run("m", key::WINDOW_MENU_ALIGN_MIDDLE, "wm-align-middle"),
                run("a", key::WINDOW_MENU_ALIGN_AUTO, "wm-align-auto"),
            ],
            // EVERY SIZE IS A STEP, and the menu stays for the next click. The
            // full-size toggle is the LAYOUT's (`toggle_size` sizes the tiled
            // windows), so a floating window is not offered it.
            Level::Size => {
                let by = crate::actions::RESIZE_STEP;
                let mut out: Vec<Entry> = vec![
                    group("w", key::WINDOW_MENU_WIDTH, Level::Width),
                    group("v", key::WINDOW_MENU_HEIGHT, Level::Height),
                    run("n", key::WINDOW_MENU_WIDTH_NEXT, "wm-width-next"),
                    run("N", key::WINDOW_MENU_WIDTH_PREVIOUS, "wm-width-previous"),
                    run("m", key::WINDOW_MENU_HEIGHT_NEXT, "wm-height-next"),
                    run("M", key::WINDOW_MENU_HEIGHT_PREVIOUS, "wm-height-previous"),
                    with("l", key::WINDOW_MENU_WIDER, "wm-resize-wider", by),
                    with("h", key::WINDOW_MENU_NARROWER, "wm-resize-narrower", by),
                    with("j", key::WINDOW_MENU_TALLER, "wm-resize-taller", by),
                    with("k", key::WINDOW_MENU_SHORTER, "wm-resize-shorter", by),
                    with("L", key::WINDOW_MENU_GROW_RIGHT, "wm-resize-grow-right", by),
                    with("H", key::WINDOW_MENU_GROW_LEFT, "wm-resize-grow-left", by),
                    with("J", key::WINDOW_MENU_GROW_DOWN, "wm-resize-grow-down", by),
                    with("K", key::WINDOW_MENU_GROW_UP, "wm-resize-grow-up", by),
                    with(
                        "C-l",
                        key::WINDOW_MENU_SHRINK_RIGHT,
                        "wm-resize-shrink-right",
                        by,
                    ),
                    with(
                        "C-h",
                        key::WINDOW_MENU_SHRINK_LEFT,
                        "wm-resize-shrink-left",
                        by,
                    ),
                    with(
                        "C-j",
                        key::WINDOW_MENU_SHRINK_DOWN,
                        "wm-resize-shrink-down",
                        by,
                    ),
                    with("C-k", key::WINDOW_MENU_SHRINK_UP, "wm-resize-shrink-up", by),
                ]
                .into_iter()
                .map(|e| if e.is_group() { e } else { e.stays() })
                .collect();
                if !floating {
                    out.extend([
                        run("f", key::WINDOW_MENU_TOGGLE_SIZE, "wm-toggle-size-this"),
                        run("r", key::WINDOW_MENU_RESET_SIZE, "wm-toggle-size-reset"),
                    ]);
                }
                out
            }
            // THE TWELVE SHARES OF THE SIZE MODES, on their own keys.
            Level::Width | Level::Height => {
                let id = if self.level == Level::Width {
                    "wm-width-set"
                } else {
                    "wm-height-set"
                };
                crate::actions::FRACTIONS
                    .iter()
                    .map(|&(k, _, share)| {
                        let percent = share
                            .parse::<f64>()
                            .map_or(0, |f| (f * 100.0).round() as i64);
                        Entry {
                            key: k,
                            label: texts.t_args(
                                key::WINDOW_MENU_SHARE,
                                &[("percent", ronoctua_i18n::Arg::Number(percent))],
                            ),
                            about: Some(texts.t(&format!("cmd-{id}"))),
                            does: Does::RunWith(id, share.to_owned()),
                            stays: true,
                            aimed: true,
                            names: None,
                        }
                    })
                    .collect()
            }
            // A ZOOM IS A STEP, and so is opacity: the menu stays.
            Level::Content => vec![
                run("=", key::WINDOW_MENU_CONTENT_BIGGER, "wm-content-bigger").stays(),
                run("-", key::WINDOW_MENU_CONTENT_SMALLER, "wm-content-smaller").stays(),
                run("0", key::WINDOW_MENU_CONTENT_RESET, "wm-content-reset").stays(),
            ],
            Level::Look => vec![
                run("=", key::WINDOW_MENU_OPACITY_MORE, "wm-opacity-more").stays(),
                run("-", key::WINDOW_MENU_OPACITY_LESS, "wm-opacity-less").stays(),
                run("0", key::WINDOW_MENU_OPACITY_FULL, "wm-opacity-full").stays(),
                run("n", key::WINDOW_MENU_BORDER_NONE, "wm-border-none"),
                run("p", key::WINDOW_MENU_BORDER_PIXEL, "wm-border-pixel"),
                run("t", key::WINDOW_MENU_BORDER_NORMAL, "wm-border-normal"),
                run("c", key::WINDOW_MENU_BORDER_CSD, "wm-border-csd"),
                run("s", key::WINDOW_MENU_SHADOW_ON, "wm-shadow-on"),
                run("S", key::WINDOW_MENU_SHADOW_OFF, "wm-shadow-off"),
                run("d", key::WINDOW_MENU_DIM_ON, "wm-dim-on"),
                run("D", key::WINDOW_MENU_DIM_OFF, "wm-dim-off"),
            ],
            Level::Behaviour => vec![
                run("v", key::WINDOW_MENU_AWAKE_VISIBLE, "wm-awake-visible"),
                run("f", key::WINDOW_MENU_AWAKE_FOCUS, "wm-awake-focus"),
                run(
                    "F",
                    key::WINDOW_MENU_AWAKE_FULLSCREEN,
                    "wm-awake-fullscreen",
                ),
                run("o", key::WINDOW_MENU_AWAKE_OPEN, "wm-awake-open"),
                run("n", key::WINDOW_MENU_AWAKE_NONE, "wm-awake-none"),
                run("t", key::WINDOW_MENU_TEARING_ON, "wm-tearing-on"),
                run("T", key::WINDOW_MENU_TEARING_OFF, "wm-tearing-off"),
                run("k", key::WINDOW_MENU_KEEP_SHORTCUTS, "wm-keep-shortcuts"),
                run("c", key::WINDOW_MENU_CALM, "wm-calm"),
            ],
            // EVERY WINDOW PUT AWAY, by its name, and then the compositor's
            // two: the next one out, and a jump by letter.
            // EVERY WINDOW OF THE DRAWER, by its name: the one put away comes
            // out, the one out goes back. Then the compositor's two.
            Level::Drawer => {
                const KEYS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
                let mut out: Vec<Entry> = self
                    .drawer
                    .iter()
                    .zip(KEYS)
                    .map(|(w, k)| {
                        let name = match (&w.app_id, &w.title) {
                            (Some(app), Some(title)) => texts.t_with(
                                key::WINDOW_MENU_DRAWER_ONE,
                                &[("app", app), ("title", title)],
                            ),
                            (Some(one), None) | (None, Some(one)) => one.clone(),
                            (None, None) => w.id.to_string(),
                        };
                        Entry {
                            key: k,
                            label: if w.hidden {
                                name
                            } else {
                                texts.t_with(key::WINDOW_MENU_DRAWER_OUT, &[("name", &name)])
                            },
                            about: Some(texts.t(if w.hidden {
                                key::ABOUT_WINDOW_MENU_TAKE_OUT_THIS
                            } else {
                                key::ABOUT_WINDOW_MENU_PUT_BACK_THIS
                            })),
                            does: if w.hidden {
                                Does::TakeOut(w.id)
                            } else {
                                Does::PutBack(w.id)
                            },
                            stays: false,
                            aimed: false,
                            names: None,
                        }
                    })
                    .collect();
                out.extend([
                    run("n", key::WINDOW_MENU_DRAWER_NEXT, "wm-scratchpad-show").loose(),
                    run("j", key::WINDOW_MENU_DRAWER_JUMP, "wm-scratchpad-jump").loose(),
                ]);
                out
            }
            Level::Trail => self.trail_entries(texts),
            Level::Selection => self.selection_entries(texts),
            Level::Fullscreen => vec![
                run("l", key::WINDOW_MENU_FULL_LAYOUT, "wm-full-layout"),
                run(
                    "a",
                    key::WINDOW_MENU_FULL_APPLICATION,
                    "wm-full-application",
                ),
                run("g", key::WINDOW_MENU_FULL_GLOBAL, "wm-full-global"),
            ],
            // "NO" FIRST, and it is also what `Esc` means: the question opened
            // because somebody clicked, and the next click must not be the one
            // that loses their work.
            Level::Confirm(danger) => vec![
                Entry {
                    key: "n",
                    label: texts.t(key::WINDOW_MENU_NO),
                    about: None,
                    does: Does::Back,
                    stays: false,
                    aimed: true,
                    names: None,
                },
                Entry {
                    key: "y",
                    label: texts.t(match danger {
                        Danger::Kill => key::WINDOW_MENU_YES_KILL,
                        Danger::CloseOthers => key::WINDOW_MENU_YES_CLOSE_OTHERS,
                        Danger::DeleteTrail => key::WINDOW_MENU_YES_DELETE_TRAIL,
                    }),
                    about: None,
                    does: match danger {
                        Danger::Kill => Does::Kill,
                        Danger::CloseOthers => Does::Run("wm-close-others"),
                        Danger::DeleteTrail => Does::Run("wm-trail-delete"),
                    },
                    stays: false,
                    aimed: true,
                    names: None,
                },
            ],
        }
    }

    /// The trails' cells: what exists is what is offered — walking a trail
    /// needs windows on it, choosing needs two trails, and only a window can be
    /// put on one.
    fn trail_entries(&self, texts: &Texts) -> Vec<Entry> {
        let run = |k: &'static str, label: &str, id: &'static str| Entry {
            key: k,
            label: texts.t(label),
            about: Some(texts.t(&format!("cmd-{id}"))),
            does: Does::Run(id),
            stays: false,
            aimed: true,
            names: None,
        };
        let t = self.trails;
        let mut out = Vec::new();
        if let Some(w) = self.window.as_ref() {
            out.push(run(
                "t",
                if w.trailmark {
                    key::WINDOW_MENU_TRAIL_OFF
                } else {
                    key::WINDOW_MENU_TRAIL_ON
                },
                "wm-trailmark",
            ));
        }
        if t.marked > 0 {
            out.extend([
                run("n", key::WINDOW_MENU_TRAIL_NEXT_WINDOW, "wm-trailmark-next")
                    .loose()
                    .stays(),
                run(
                    "p",
                    key::WINDOW_MENU_TRAIL_PREVIOUS_WINDOW,
                    "wm-trailmark-previous",
                )
                .loose()
                .stays(),
                run("j", key::WINDOW_MENU_DRAWER_JUMP, "wm-trailmark-jump").loose(),
                run(
                    "s",
                    key::WINDOW_MENU_TRAIL_TO_SELECTION,
                    "wm-trail-to-selection",
                )
                .loose(),
            ]);
        }
        out.push(run("N", key::WINDOW_MENU_TRAIL_NEW, "wm-trail-new").loose());
        // CHOOSING NEEDS TWO, and the compositor counts trails from zero —
        // `trail number 0` is the first — while a person counts from one.
        if t.count > 1 {
            const KEYS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
            for (n, k) in (0..t.count).zip(KEYS) {
                let shown = (n + 1).to_string();
                out.push(Entry {
                    key: k,
                    label: texts.t_with(
                        if t.active == Some(n) {
                            key::WINDOW_MENU_TRAIL_IN_USE
                        } else {
                            key::WINDOW_MENU_TRAIL_NUMBER
                        },
                        &[("n", &shown)],
                    ),
                    about: Some(texts.t(crate::key::CMD_WM_TRAIL_NUMBER)),
                    does: Does::RunWith("wm-trail-number", n.to_string()),
                    stays: true,
                    aimed: false,
                    names: None,
                });
            }
            out.extend([
                run("]", key::WINDOW_MENU_TRAIL_NEXT, "wm-trail-next")
                    .loose()
                    .stays(),
                run("[", key::WINDOW_MENU_TRAIL_PREVIOUS, "wm-trail-previous")
                    .loose()
                    .stays(),
            ]);
        }
        if t.count > 0 {
            out.push(run("c", key::WINDOW_MENU_TRAIL_CLEAR, "wm-trail-clear").loose());
            out.push(Entry {
                key: "D",
                label: texts.t(key::WINDOW_MENU_TRAIL_DELETE),
                about: Some(texts.t(crate::key::CMD_WM_TRAIL_DELETE)),
                does: Does::Enter(Level::Confirm(Danger::DeleteTrail)),
                stays: false,
                aimed: true,
                names: None,
            });
        }
        out
    }

    /// The selection's cells: what can be done to the windows marked, and
    /// this window in or out of them.
    fn selection_entries(&self, texts: &Texts) -> Vec<Entry> {
        let run = |k: &'static str, label: &str, id: &'static str| Entry {
            key: k,
            label: texts.t(label),
            about: Some(texts.t(&format!("cmd-{id}"))),
            does: Does::Run(id),
            stays: false,
            aimed: true,
            names: None,
        };
        let mut out = Vec::new();
        if let Some(w) = self.window.as_ref() {
            out.push(run(
                "v",
                if self.selection.has(w.id) {
                    key::WINDOW_MENU_UNSELECT
                } else {
                    key::WINDOW_MENU_SELECT
                },
                "wm-select",
            ));
        }
        out.push(run(
            "w",
            key::WINDOW_MENU_SELECT_WORKSPACE,
            "wm-select-workspace",
        ));
        if self.selection.any() {
            out.extend([
                run("h", key::WINDOW_MENU_SELECT_MOVE, "wm-select-move"),
                run("t", key::WINDOW_MENU_SELECT_TO_TRAIL, "wm-select-to-trail"),
                run("c", key::WINDOW_MENU_SELECT_NONE, "wm-select-none"),
            ]);
        }
        out
    }

    /// Chooses cell `n` of the level on screen — by a click or by its key.
    pub fn choose(&mut self, n: usize, texts: &Texts) -> Errand {
        let Some(e) = self.entries(texts).into_iter().nth(n) else {
            return Errand::Redraw;
        };
        let on = e
            .aimed
            .then(|| self.window.as_ref().map(|w| w.id))
            .flatten();
        let was = self.level;
        let errand = match e.does {
            Does::Run(id) => Errand::Run {
                id,
                arg: None,
                on,
                stays: e.stays,
            },
            Does::RunWith(id, arg) => Errand::Run {
                id,
                arg: Some(arg),
                on,
                stays: e.stays,
            },
            Does::PutBack(id) => Errand::Run {
                id: "wm-scratchpad-put",
                arg: None,
                on: Some(id),
                stays: false,
            },
            Does::Enter(level) => {
                self.level = level;
                Errand::Redraw
            }
            Does::Kill => self
                .window
                .as_ref()
                .and_then(|w| w.pid)
                .map_or(Errand::Close, Errand::Kill),
            Does::TakeOut(id) => Errand::TakeOut(id),
            Does::Back => self.back(),
        };
        self.landed(was);
        errand
    }

    /// The key with this notation, if a cell of the level has it — and
    /// `RET`, the cell the cursor is on: what the click on the pick does.
    pub fn pressed(&mut self, notation: &str, texts: &Texts) -> Errand {
        if notation == "RET" {
            return match self.picked() {
                Some(n) => self.choose(n, texts),
                None => Errand::Redraw,
            };
        }
        match self.entries(texts).iter().position(|e| e.key == notation) {
            Some(n) => self.choose(n, texts),
            None => Errand::Redraw,
        }
    }

    /// `Esc`: up one level, and from the first one, closed.
    pub fn back(&mut self) -> Errand {
        let was = self.level;
        let errand = self.went_back();
        self.landed(was);
        errand
    }

    /// Where `Esc` goes, from the level on screen.
    fn went_back(&mut self) -> Errand {
        match self.level {
            Level::Root => Errand::Close,
            // A LEVEL OPENED ON ITS OWN, from its mark, has nowhere to go up
            // to — except the question it asked, which goes back to it.
            Level::Drawer | Level::Trail | Level::Selection if self.window.is_none() => {
                Errand::Close
            }
            Level::Confirm(Danger::DeleteTrail) => {
                self.level = Level::Trail;
                Errand::Redraw
            }
            // A SHARE WAS CHOSEN FROM THE SIZES, and that is where it goes back.
            Level::Width | Level::Height => {
                self.level = Level::Size;
                Errand::Redraw
            }
            _ => {
                self.level = Level::Root;
                Errand::Redraw
            }
        }
    }
}

/// The leader's group the window menu IS — `LEADER w`, the window group.
pub const LEADER_PREFIX: &str = "w";

/// A sequence of the leader the menu generates: `(sequence, command,
/// argument)`.
pub type Generated = (String, &'static str, Option<&'static str>);

/// The generated sequences and the groups they sit in — `(sequence, label key)`.
pub type Leader = (Vec<Generated>, Vec<(String, &'static str)>);

/// The leader's `w` group, GENERATED from this menu: every cell a sequence
/// with the cell's own key, every submenu a group labelled as the submenu is.
///
/// # Why generated
///
/// Because the owner asked for every item here to have a leader sequence,
/// and a second table written by hand beside the menu would be the menu's
/// keys told twice — the first rebinding would make the two disagree, and
/// the hover would teach a key that does something else. Walked from the
/// menu, what the leader offers IS what the menu offers.
///
/// Both states are walked — a tiled window and a floating one, with a drawer,
/// a selection and nine trails — so an item only one of them shows still has
/// its sequence; the menu's keys are chosen so the two states never put two
/// deeds on one key. A question becomes the sequence of its "yes" — typing
/// three keys is the deliberate act the question exists to ask for — and
/// "force quit" has none, because it kills a process and is no deed of the
/// compositor's. Computed once: the tree is built at start, and the tests
/// build it many times.
#[must_use]
pub fn leader_bindings() -> &'static Leader {
    static ONCE: std::sync::OnceLock<Leader> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        // THE LABELS ARE NOT READ — a cell's key and deed are what matter —
        // but `entries` builds them, and it needs a language to build them in.
        let texts = Texts::fresh("en-GB").expect("the base language");
        let mut binds: Vec<Generated> = Vec::new();
        // `w` ITSELF is the leader's own group, labelled there.
        let mut groups = Vec::new();
        for floating in [false, true] {
            let mut w = Window {
                id: 1,
                title: None,
                app_id: None,
                pid: Some(1),
                is_focused: true,
                floating,
                fullscreen: false,
                sticky: false,
                drawer: floating,
                hidden: false,
                trailmark: false,
                rect: None,
            };
            w.sticky = false;
            let mut selection = Selection::default();
            selection.deed("wm-select", Some(1));
            // AND AS A KIOSK, so its one cell has its sequence too.
            let mut v = View::fresh(w.clone(), Vec::new(), 2)
                .with_kiosk(true)
                .with_drawer(vec![w])
                .with_trails(Trails {
                    count: 9,
                    active: Some(0),
                    marked: 2,
                })
                .with_selection(selection);
            walk(
                &mut v,
                Level::Root,
                LEADER_PREFIX,
                &texts,
                &mut binds,
                &mut groups,
            );
        }
        (binds, groups)
    })
}

/// One level of [`leader_bindings`], and the levels under it.
fn walk(
    v: &mut View,
    level: Level,
    prefix: &str,
    texts: &Texts,
    binds: &mut Vec<Generated>,
    groups: &mut Vec<(String, &'static str)>,
) {
    v.level = level;
    for e in v.entries(texts) {
        let seq = format!("{prefix} {}", e.key);
        let (id, arg) = match e.does {
            Does::Run(id) => (id, None),
            Does::RunWith(id, arg) => (id, Some(interned(arg))),
            Does::Enter(Level::Confirm(Danger::CloseOthers)) => ("wm-close-others", None),
            Does::Enter(Level::Confirm(Danger::DeleteTrail)) => ("wm-trail-delete", None),
            Does::Enter(Level::Confirm(Danger::Kill)) => continue,
            Does::Enter(to) => {
                if !groups.iter().any(|(s, _)| *s == seq)
                    && let Some(label) = e.names
                {
                    groups.push((seq.clone(), label));
                }
                walk(v, to, &seq, texts, binds, groups);
                v.level = level;
                continue;
            }
            Does::Kill | Does::TakeOut(_) | Does::PutBack(_) | Does::Back => continue,
        };
        // THE FIRST STATE TO SAY A SEQUENCE KEEPS IT — and the second says
        // the same deed, or the menu's keys are wrong (the test below).
        if !binds.iter().any(|(s, _, _)| *s == seq) {
            binds.push((seq, id, arg));
        }
    }
}

/// An argument as the `&'static str` a binding holds. Every argument this
/// menu gives is one of a closed few — a workspace, a share, a step, a trail
/// — and [`leader_bindings`] runs once, so each is leaked once.
fn interned(arg: String) -> &'static str {
    Box::leak(arg.into_boxed_str())
}

/// The key that does what a cell does, outside the menu: the leader's
/// sequence when there is one (`LEADER w f`), and the compositor's chord when
/// there is not (`SUPER + S-z`) — written the way which-key and the manual
/// write them, and read from the SAME tables, so a rebinding moves this too.
/// `None` for what only the menu reaches.
///
/// The sequence is `bindings::taught_keys`' — from the leader's DEFAULT set,
/// the one every sequence starts in, as every sentence that teaches a key
/// reads it. `w` folds in no set, so the menu's own cells read the same as
/// they always did; a deed whose key is under a folded group comes out with
/// the tap that reaches it (`CTRL …`).
#[must_use]
pub fn taught(trees: &crate::bindings::Trees, does: &Does) -> Option<String> {
    let (id, arg) = match does {
        Does::Run(id) => (*id, None),
        Does::RunWith(id, arg) => (*id, Some(arg.as_str())),
        _ => return None,
    };
    if let Some(keys) = crate::bindings::taught_keys(trees, id, arg) {
        return Some(format!("{} {keys}", crate::bindings::label()));
    }
    // THE CHORDS, built once: the region is drawn every frame, and the
    // table is four hundred binds spelt out.
    static CHORDS: std::sync::OnceLock<Vec<(&'static str, &'static str)>> =
        std::sync::OnceLock::new();
    let chords = CHORDS.get_or_init(|| {
        let nested = crate::actions::nested();
        crate::actions::full_chords()
            .iter()
            .map(|c| (c.command, c.shows_for(nested)))
            .collect()
    });
    arg.is_none()
        .then(|| chords.iter().find(|(command, _)| *command == id))
        .flatten()
        .map(|(_, shows)| format!("{} + {shows}", crate::bindings::chord_label()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> Texts {
        Texts::fresh("en-GB").expect("the base language")
    }

    fn window(floating: bool, sticky: bool, fullscreen: bool) -> Window {
        Window {
            id: 1,
            title: Some("a title".to_owned()),
            app_id: Some("an.app".to_owned()),
            pid: Some(4242),
            is_focused: true,
            floating,
            fullscreen,
            sticky,
            drawer: false,
            hidden: false,
            trailmark: false,
            rect: None,
        }
    }

    fn spaces(current: u32) -> Vec<Workspace> {
        (1..=3)
            .map(|n| Workspace {
                name: n.to_string(),
                number: Some(n),
                focused: n == current,
                occupied: n != 3,
                visible: n == current,
                output: Some("eDP-1".to_owned()),
            })
            .collect()
    }

    fn ids(v: &View) -> Vec<Does> {
        v.entries(&texts()).into_iter().map(|e| e.does).collect()
    }

    /// A toggle is offered on the side it will go to, and sticking only where
    /// it means something.
    #[test]
    fn a_toggle_says_which_way_it_goes() {
        let t = texts();
        let tiled = View::fresh(window(false, false, false), spaces(1), 1);
        let labels: Vec<String> = tiled.entries(&t).into_iter().map(|e| e.label).collect();
        assert!(labels.contains(&t.t(key::WINDOW_MENU_FLOAT)));
        assert!(!labels.contains(&t.t(key::WINDOW_MENU_TILE)));
        assert!(!ids(&tiled).contains(&Does::Run("wm-stick")));

        let floating = View::fresh(window(true, true, false), spaces(1), 1);
        let labels: Vec<String> = floating.entries(&t).into_iter().map(|e| e.label).collect();
        assert!(labels.contains(&t.t(key::WINDOW_MENU_TILE)));
        assert!(labels.contains(&t.t(key::WINDOW_MENU_UNSTICK)));
        assert!(!labels.contains(&t.t(key::WINDOW_MENU_FLOAT)));

        let full = View::fresh(window(false, false, true), spaces(1), 1);
        assert_eq!(
            full.entries(&t)[0].label,
            t.t(key::WINDOW_MENU_UNFULLSCREEN)
        );
    }

    /// The window is not offered the workspace it is already on, and every
    /// other one sends it there.
    #[test]
    fn the_workspaces_are_every_other_one() {
        let mut v = View::fresh(window(false, false, false), spaces(2), 1);
        let t = texts();
        assert_eq!(v.pressed("P", &t), Errand::Redraw);
        assert_eq!(v.level, Level::SendTo { follow: true });
        let all = ids(&v);
        assert!(!all.contains(&Does::RunWith("workspace-move", "2".into())));
        assert!(all.contains(&Does::RunWith("workspace-move", "9".into())));
        assert!(
            !all.contains(&Does::RunWith("workspace-move", "10".into())),
            "a tenth"
        );
        assert_eq!(
            v.pressed("3", &t),
            Errand::Run {
                id: "workspace-move",
                arg: Some("3".into()),
                on: Some(1),
                stays: false,
            }
        );
        // AND SENDING WITHOUT FOLLOWING is the other deed.
        v.back();
        v.pressed("p", &t);
        assert_eq!(
            v.pressed("3", &t),
            Errand::Run {
                id: "wm-workspace-send",
                arg: Some("3".into()),
                on: Some(1),
                stays: false,
            }
        );
    }

    /// `Esc` goes up one level, and closes from the first.
    #[test]
    fn esc_goes_up_then_closes() {
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        let t = texts();
        v.pressed("M", &t);
        assert_eq!(v.level, Level::Move);
        assert_eq!(v.back(), Errand::Redraw);
        assert_eq!(v.level, Level::Root);
        assert_eq!(v.back(), Errand::Close);
    }

    /// Killing asks first, and the question's first cell is "no".
    #[test]
    fn killing_asks_and_no_comes_first() {
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        let t = texts();
        assert_eq!(v.pressed("X", &t), Errand::Redraw);
        assert_eq!(v.level, Level::Confirm(Danger::Kill));
        assert_eq!(v.choose(0, &t), Errand::Redraw);
        assert_eq!(v.level, Level::Root);
        v.pressed("X", &t);
        assert_eq!(v.pressed("y", &t), Errand::Kill(4242));
    }

    /// The index of the cell with this key, on the level on screen.
    fn cell(v: &View, key: &str) -> usize {
        v.entries(&texts())
            .iter()
            .position(|e| e.key == key)
            .unwrap_or_else(|| panic!("no cell `{key}` on {:?}", v.level))
    }

    /// FAILS IF: the double click on a group runs the cell that came up
    /// under the hand, or a cell picked there does not run on a click on the
    /// pick — or a cell of the menu opened from the modeline stops running
    /// on its click.
    #[test]
    fn a_level_a_click_opened_picks_before_it_runs() {
        let t = texts();
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        // FROM THE MODELINE, the menu grows above the line clicked: a cell
        // runs on its click, and a group opens on its click.
        let group = cell(&v, "M");
        assert_eq!(v.clicked(group, &t), Errand::Redraw);
        assert_eq!(v.level, Level::Move);
        // THE SECOND PRESS OF THE DOUBLE CLICK lands on what came up there:
        // it picks, and runs nothing.
        assert_eq!(v.clicked(group, &t), Errand::Picked);
        assert_eq!(v.level, Level::Move);
        assert_eq!(v.picked(), Some(group));
        // A CLICK ON ANOTHER CELL moves the pick, and runs nothing either.
        assert_eq!(v.clicked(0, &t), Errand::Picked);
        assert_eq!(v.picked(), Some(0));
        // THE CLICK ON THE PICK runs it — and `RET` is that click.
        let ran = v.clone().clicked(0, &t);
        assert!(matches!(ran, Errand::Run { .. }), "{ran:?}");
        assert_eq!(v.pressed("RET", &t), ran);
        // A KEY-OPENED LEVEL runs a cell on its click.
        let mut keyed = View::fresh(window(false, false, false), spaces(1), 1);
        keyed.pressed("M", &t);
        assert!(matches!(keyed.clicked(0, &t), Errand::Run { .. }));
        // AND THE MENU OPENED BY A CLICK ON A LINE OF THE REGION is under the
        // hand from its first level.
        let mut under =
            View::fresh(window(false, false, false), spaces(1), 1).opened_under_the_hand(true);
        let float = cell(&under, "f");
        assert_eq!(under.clicked(float, &t), Errand::Picked);
        assert!(matches!(under.clicked(float, &t), Errand::Run { .. }));
    }

    /// FAILS IF: the question about killing the window is not born with the
    /// cursor on "no", answers on one click — wherever it was opened — or
    /// the click that answered "no" lets the next press run a cell of the
    /// level it went back to.
    #[test]
    fn the_question_picks_before_it_answers() {
        let t = texts();
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        // OPENED BY A KEY, and still two clicks: it cannot be undone.
        v.pressed("X", &t);
        assert_eq!(v.picked(), Some(0), "the cursor is not born on no");
        assert_eq!(v.clicked(1, &t), Errand::Picked, "one click killed it");
        assert_eq!(v.clicked(1, &t), Errand::Kill(4242));
        // "NO" IS BORN UNDER THE CURSOR, and the screen's: two clicks too.
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        v.pressed("X", &t);
        assert_eq!(v.clicked(0, &t), Errand::Picked);
        assert_eq!(v.clicked(0, &t), Errand::Redraw);
        assert_eq!(v.level, Level::Root);
        // AND THE ROOT IT WENT BACK TO CAME UP UNDER THE HAND.
        assert_eq!(v.clicked(0, &t), Errand::Picked);
        // `RET` on the question is its cursor's answer: "no".
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        v.pressed("X", &t);
        assert_eq!(v.pressed("RET", &t), Errand::Redraw);
        assert_eq!(v.level, Level::Root);
    }

    /// Another screen is offered only where there is one.
    #[test]
    fn another_screen_only_with_two() {
        let mut one = View::fresh(window(false, false, false), spaces(1), 1);
        let mut two = View::fresh(window(false, false, false), spaces(1), 2);
        let t = texts();
        one.pressed("M", &t);
        two.pressed("M", &t);
        assert!(!ids(&one).contains(&Does::Run("wm-move-screen-left")));
        assert!(ids(&two).contains(&Does::Run("wm-move-screen-left")));
    }

    /// Every cell of every level has a key of its own, so a key never picks
    /// the wrong one — and every notation parses, so it can be typed.
    #[test]
    fn every_key_is_unique_and_parses() {
        let t = texts();
        for level in [
            Level::Root,
            Level::SendTo { follow: false },
            Level::SendTo { follow: true },
            Level::Move,
            Level::Align,
            Level::Size,
            Level::Width,
            Level::Height,
            Level::Content,
            Level::Look,
            Level::Behaviour,
            Level::Fullscreen,
            Level::Drawer,
            Level::Trail,
            Level::Selection,
            Level::Confirm(Danger::Kill),
        ] {
            let mut sel = Selection::default();
            sel.deed("wm-select", Some(1));
            let mut v = View::fresh(window(true, false, false), spaces(1), 2)
                .with_drawer(vec![window(true, false, false)])
                .with_trails(Trails {
                    count: 9,
                    active: Some(0),
                    marked: 2,
                })
                .with_selection(sel);
            v.level = level;
            let keys: Vec<&str> = v.entries(&t).iter().map(|e| e.key).collect();
            for (i, k) in keys.iter().enumerate() {
                assert!(!keys[..i].contains(k), "{level:?}: `{k}` twice");
                // AND IT READS BACK AS ITSELF: the footer finds the cell by
                // the notation of the key typed, so a spelling that parses
                // into a key whose notation is another string is a cell no
                // key ever reaches.
                let parsed = ronoctua_keymap::Key::parse(k).expect("the notation parses");
                assert_eq!(
                    parsed.notation(),
                    *k,
                    "`{k}` reads back as another notation"
                );
            }
        }
    }

    /// The drawer lists what is put away, takes out the one chosen, and is
    /// offered at the root only while it holds something.
    #[test]
    fn the_drawer_lists_what_it_holds_and_takes_one_out() {
        let t = texts();
        let mut away = window(true, false, false);
        away.id = 9;
        away.drawer = true;
        away.hidden = true;
        let empty = View::fresh(window(false, false, false), spaces(1), 1);
        assert!(!ids(&empty).contains(&Does::Enter(Level::Drawer)));
        let mut v =
            View::fresh(window(false, false, false), spaces(1), 1).with_drawer(vec![away.clone()]);
        assert!(ids(&v).contains(&Does::Enter(Level::Drawer)));
        v.pressed("g", &t);
        assert_eq!(v.pressed("1", &t), Errand::TakeOut(9));

        // OPENED FROM THE MARK, with nothing focused: `Esc` closes.
        let mut alone = View::of_the_drawer(vec![away]);
        assert_eq!(alone.level, Level::Drawer);
        assert_eq!(alone.back(), Errand::Close);
    }

    /// A window the drawer let out goes back, and can leave it for good.
    #[test]
    fn a_window_out_of_the_drawer_goes_back_or_leaves_it() {
        let t = texts();
        let mut out = window(true, false, false);
        out.drawer = true;
        let v = View::fresh(out, spaces(1), 1);
        let labels: Vec<String> = v.entries(&t).into_iter().map(|e| e.label).collect();
        assert!(labels.contains(&t.t(key::WINDOW_MENU_PUT_BACK)));
        assert!(!labels.contains(&t.t(key::WINDOW_MENU_PUT_AWAY)));
        assert!(ids(&v).contains(&Does::Run("wm-float-off")));
    }

    /// Trails are offered by what exists: choosing needs two, walking needs
    /// windows on it, and the compositor counts them from zero.
    #[test]
    fn the_trails_offer_what_exists() {
        let t = texts();
        let none = View::fresh(window(false, false, false), spaces(1), 1);
        let mut v = none.with_trails(Trails {
            count: 0,
            active: None,
            marked: 0,
        });
        v.level = Level::Trail;
        let d = ids(&v);
        assert!(d.contains(&Does::Run("wm-trailmark")));
        assert!(d.contains(&Does::Run("wm-trail-new")));
        assert!(!d.contains(&Does::Run("wm-trailmark-next")));
        assert!(!d.contains(&Does::Run("wm-trail-clear")));

        let mut two = View::fresh(window(false, false, false), spaces(1), 1).with_trails(Trails {
            count: 2,
            active: Some(1),
            marked: 3,
        });
        two.level = Level::Trail;
        assert_eq!(
            two.pressed("1", &t),
            Errand::Run {
                id: "wm-trail-number",
                arg: Some("0".into()),
                on: None,
                stays: true,
            },
            "a person's first trail is the compositor's zeroth"
        );
        // DELETING ASKS, and `Esc` from the question goes back to the trails.
        two.level = Level::Trail;
        two.pressed("D", &t);
        assert_eq!(two.level, Level::Confirm(Danger::DeleteTrail));
        two.back();
        assert_eq!(two.level, Level::Trail);
    }

    /// The selection offers acting on it only while something is selected,
    /// and says whether this window is in it.
    #[test]
    fn the_selection_acts_only_on_something() {
        let t = texts();
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        v.level = Level::Selection;
        assert!(!ids(&v).contains(&Does::Run("wm-select-move")));
        let mut sel = Selection::default();
        sel.deed("wm-select", Some(1));
        let mut v = View::fresh(window(false, false, false), spaces(1), 1).with_selection(sel);
        v.level = Level::Selection;
        assert!(ids(&v).contains(&Does::Run("wm-select-move")));
        let unselect = t.t(key::WINDOW_MENU_UNSELECT);
        assert!(v.entries(&t).into_iter().any(|e| e.label == unselect));
        let mut alone = View::alone(Level::Selection);
        assert_eq!(alone.back(), Errand::Close);
    }

    /// A floating window is offered only what works on one — measured:
    /// `fit_size` and `align` refuse it — and is centred by position.
    #[test]
    fn a_floating_window_is_offered_what_works_on_it() {
        let floating = View::fresh(window(true, false, false), spaces(1), 1);
        let d = ids(&floating);
        assert!(!d.contains(&Does::Run("window-maximize")));
        assert!(!d.contains(&Does::Enter(Level::Align)));
        assert!(d.contains(&Does::Run("wm-float-centre")));
        let mut v = floating;
        v.level = Level::Move;
        assert!(!ids(&v).contains(&Does::Run("wm-move-beginning")));
        let tiled = View::fresh(window(false, false, false), spaces(1), 1);
        assert!(ids(&tiled).contains(&Does::Run("window-maximize")));
    }

    /// A step keeps the menu up and acts on the menu's window; walking a trail
    /// acts on the focus, which it moves itself.
    #[test]
    fn a_step_stays_and_a_walk_is_loose() {
        let t = texts();
        let mut v = View::fresh(window(false, false, false), spaces(1), 1);
        v.level = Level::Content;
        assert_eq!(
            v.pressed("=", &t),
            Errand::Run {
                id: "wm-content-bigger",
                arg: None,
                on: Some(1),
                stays: true,
            }
        );
        let mut w = View::fresh(window(false, false, false), spaces(1), 1).with_trails(Trails {
            count: 1,
            active: Some(0),
            marked: 2,
        });
        w.level = Level::Trail;
        assert_eq!(
            w.pressed("n", &t),
            Errand::Run {
                id: "wm-trailmark-next",
                arg: None,
                on: None,
                stays: true,
            }
        );
    }

    /// A window the drawer let out is listed with the ones put away, and
    /// choosing it puts it back.
    #[test]
    fn the_drawer_lists_what_it_let_out_too() {
        let t = texts();
        let mut out = window(true, false, false);
        out.id = 8;
        out.drawer = true;
        let mut v = View::of_the_drawer(vec![out]);
        assert_eq!(
            v.pressed("1", &t),
            Errand::Run {
                id: "wm-scratchpad-put",
                arg: None,
                on: Some(8),
                stays: false,
            }
        );
    }

    /// A cell teaches the key that does the same outside the menu: the
    /// leader's sequence, or the compositor's chord where there is none, and
    /// nothing where only the menu reaches.
    #[test]
    fn a_cell_teaches_its_key_outside_the_menu() {
        let (tree, _) =
            crate::bindings::assemble(&[&crate::leader::BASE]).expect("the footer's keymap");
        // `w` FOLDS IN NO SET, so the menu's cells teach the same keys in the
        // default set as they do in the full tree.
        assert_eq!(
            taught(&tree, &Does::Run("window-screen-full")),
            tree.full
                .sequence_of("window-screen-full", None)
                .map(|s| format!(
                    "{} {}",
                    crate::bindings::label(),
                    ronoctua_keymap::notation(&s)
                ))
        );
        let full = taught(&tree, &Does::Run("window-screen-full")).expect("a key");
        assert!(full.ends_with(" w f"), "{full}");
        assert!(full.starts_with(&crate::bindings::label()), "{full}");
        // THE MENU'S KEYS ARE THE LEADER'S under `w` — the chord is taught
        // only for what no sequence reaches.
        let away = taught(&tree, &Does::Run("wm-scratchpad-put")).expect("a key");
        assert!(away.ends_with(" w u"), "{away}");
        assert_eq!(taught(&tree, &Does::Run("wm-workspace-send")), None);
        let three = taught(&tree, &Does::RunWith("workspace-move", "3".into())).expect("a key");
        // TWO SEQUENCES OF THREE KEYS (`TAB m 3`, `w P 3`), and either is true.
        assert!(three.ends_with(" 3"), "{three}");
    }

    /// EVERY CELL OF EVERY LEVEL HAS A LEADER SEQUENCE, the owner's request:
    /// the menu is the pointer's way to a deed, and the keyboard's way to the
    /// same deed is the leader. A cell with none is a feature only the mouse
    /// reaches, which is invariant 19 read backwards.
    #[test]
    fn every_cell_has_a_leader_sequence() {
        let t = texts();
        let (tree, _) =
            crate::bindings::assemble(&[&crate::leader::BASE]).expect("the footer's keymap");
        let mut missing = Vec::new();
        for level in [
            Level::Root,
            Level::SendTo { follow: false },
            Level::SendTo { follow: true },
            Level::Move,
            Level::Align,
            Level::Size,
            Level::Width,
            Level::Height,
            Level::Content,
            Level::Look,
            Level::Behaviour,
            Level::Fullscreen,
            Level::Drawer,
            Level::Trail,
            Level::Selection,
        ] {
            for floating in [false, true] {
                let mut sel = Selection::default();
                sel.deed("wm-select", Some(1));
                let mut w = window(floating, false, false);
                w.drawer = floating;
                let mut v = View::fresh(w, spaces(1), 2)
                    .with_drawer(vec![window(true, false, false)])
                    .with_trails(Trails {
                        count: 3,
                        active: Some(0),
                        marked: 2,
                    })
                    .with_selection(sel);
                v.level = level;
                for e in v.entries(&t) {
                    let (id, arg) = match &e.does {
                        Does::Run(id) => (*id, None),
                        Does::RunWith(id, arg) => (*id, Some(arg.clone())),
                        _ => continue,
                    };
                    if tree.full.sequence_of(id, arg.as_deref()).is_none() {
                        let name = match arg {
                            Some(a) => format!("{id} {a}"),
                            None => id.to_owned(),
                        };
                        if !missing.contains(&name) {
                            missing.push(name);
                        }
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "no leader sequence for:\n{}",
            missing.join("\n")
        );
    }

    /// A kiosk's window offers its view on `B`, no other window does, and the
    /// leader has the same cell as `LEADER w B`.
    #[test]
    fn a_kiosk_offers_its_view_on_b() {
        let t = texts();
        let plain = View::fresh(window(false, false, false), spaces(1), 1);
        assert!(!ids(&plain).contains(&Does::Run("kiosk-more")));
        let kiosk = View::fresh(window(false, false, false), spaces(1), 1).with_kiosk(true);
        assert!(ids(&kiosk).contains(&Does::Run("kiosk-more")));
        let keys: Vec<&str> = kiosk.entries(&t).iter().map(|e| e.key).collect();
        assert_eq!(keys.iter().filter(|k| **k == "B").count(), 1);
        let (binds, _) = leader_bindings();
        assert!(
            binds
                .iter()
                .any(|(s, id, arg)| s == "w B" && *id == "kiosk-more" && arg.is_none()),
            "LEADER w B is not generated"
        );
    }

    /// THE TWO STATES NEVER PUT TWO DEEDS ON ONE KEY — which is what lets
    /// `leader_bindings` walk both and keep the first.
    #[test]
    fn a_key_means_one_deed_in_every_state() {
        let t = texts();
        let mut seen: Vec<(String, String)> = Vec::new();
        for floating in [false, true] {
            let mut w = window(floating, false, false);
            w.drawer = floating;
            let mut v = View::fresh(w, spaces(1), 2);
            for level in [Level::Root, Level::Move, Level::Size, Level::Align] {
                v.level = level;
                for e in v.entries(&t) {
                    let deed = format!("{:?}", e.does);
                    let at = format!("{level:?} {}", e.key);
                    if let Some((_, before)) = seen.iter().find(|(k, _)| *k == at) {
                        assert_eq!(*before, deed, "{at} means two deeds");
                    } else {
                        seen.push((at, deed));
                    }
                }
            }
        }
    }
}
