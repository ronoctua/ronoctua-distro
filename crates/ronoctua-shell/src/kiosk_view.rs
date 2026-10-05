//! Everything the kiosk can do, in the footer's region — the modeline's `…`,
//! `LEADER w B` on a kiosk, and the `,` group.
//!
//! # One table, three doors
//!
//! [`ROOT`] is the grid this view opens on — the window menu's grammar: a
//! key and a label per cell, a click or the key runs it — and it is ALSO what
//! the `,` group is generated from ([`local_bindings`]), the way `LEADER w`
//! is generated from the window menu. What the modeline's pointer reaches is
//! a command of that table or of one of this view's lists, which
//! `footer-mouse-reaches-nothing-new` holds it to: the three doors are one
//! set of deeds.
//!
//! # Only in the footer
//!
//! There is no window door for this view, and that is the owner's decision:
//! a window would take the focus from the kiosk it controls, and every
//! command here acts on the kiosk's tab in front.
//!
//! # Four levels
//!
//! The grid, the grid of zoom steps, and two LISTS — the tabs and the
//! extensions — whose cursor keeps the TAB'S or the EXTENSION'S id, never the
//! row's index: a list that Chrome reorders under the cursor keeps the
//! cursor on the same tab.

use ronoctua_browser::wire;
use ronoctua_i18n::{Texts, key};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit as FilesHit, Ink};
use crate::window_menu::Generated;

/// Where the view is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The grid.
    Root,
    /// Every tab, one per row.
    Tabs,
    /// Every extension, pinned first.
    Extensions,
    /// The zoom's steps.
    Zoom,
}

/// One cell of a grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The key that chooses it, in the keymap's notation.
    pub key: &'static str,
    /// What it says.
    pub label: &'static str,
    /// What it says instead when what it switches is ON — a toggle is drawn
    /// on the side it will go to, the window menu's rule.
    pub when_on: Option<&'static str>,
    /// The command it runs — and the one `,` runs under the same key.
    pub command: &'static str,
    /// The level of this view it opens, when it is a door to one.
    pub opens: Option<Level>,
    /// Whether the view stays open after it runs: a step is clicked several
    /// times in a row.
    pub stays: bool,
}

/// A cell that runs a command.
const fn run(key: &'static str, label: &'static str, command: &'static str, stays: bool) -> Cell {
    Cell {
        key,
        label,
        when_on: None,
        command,
        opens: None,
        stays,
    }
}

/// A cell that switches something, labelled by which way it goes.
const fn switch(
    key: &'static str,
    off: &'static str,
    on: &'static str,
    command: &'static str,
) -> Cell {
    Cell {
        key,
        label: off,
        when_on: Some(on),
        command,
        opens: None,
        stays: true,
    }
}

/// A cell that opens a level — and, from outside the view, the command that
/// opens the view there.
const fn door(key: &'static str, label: &'static str, command: &'static str, to: Level) -> Cell {
    Cell {
        key,
        label,
        when_on: None,
        command,
        opens: Some(to),
        stays: true,
    }
}

/// THE GRID, and the `,` group, key for key.
///
/// `h l r` are a browser's three, on vi's sideways keys; `[ ]` walk the tabs
/// and `{ }` carry the tab in front with them (Chrome's own `Ctrl`+`Shift`
/// pair, one key further). The zoom is `= - 0`, Chrome's own. `k` is the
/// bars, `a` the address line, and the capital changes what the kiosk IS:
/// `R`, back to an ordinary window. The modeline's adaptation was `M` here
/// until it moved to the footer's own settings (`LEADER s r`,
/// `crate::footer_settings`, 2026-09-30): it is a setting of the footer, and
/// this grid is where a person acts on the page.
pub const ROOT: &[Cell] = &[
    run("h", key::KIOSK_MENU_BACK, "kiosk-back", true),
    run("l", key::KIOSK_MENU_FORWARD, "kiosk-forward", true),
    run("r", key::KIOSK_MENU_RELOAD, "kiosk-reload", true),
    run("n", key::KIOSK_MENU_NEW_TAB, "kiosk-new-tab", false),
    run("e", key::KIOSK_MENU_ADDRESS, "kiosk-address", false),
    door("t", key::KIOSK_MENU_TABS, "kiosk-tabs", Level::Tabs),
    run(
        "[",
        key::KIOSK_MENU_TAB_PREVIOUS,
        "kiosk-tab-previous",
        true,
    ),
    run("]", key::KIOSK_MENU_TAB_NEXT, "kiosk-tab-next", true),
    run("{", key::KIOSK_MENU_TAB_LEFT, "kiosk-tab-left", true),
    run("}", key::KIOSK_MENU_TAB_RIGHT, "kiosk-tab-right", true),
    run("w", key::KIOSK_MENU_TAB_CLOSE, "kiosk-tab-close", true),
    door(
        "x",
        key::KIOSK_MENU_EXTENSIONS,
        "kiosk-extensions",
        Level::Extensions,
    ),
    door("z", key::KIOSK_MENU_ZOOM, "kiosk-zoom", Level::Zoom),
    run("=", key::KIOSK_MENU_ZOOM_IN, "kiosk-zoom-in", true),
    run("-", key::KIOSK_MENU_ZOOM_OUT, "kiosk-zoom-out", true),
    run("0", key::KIOSK_MENU_ZOOM_RESET, "kiosk-zoom-reset", true),
    switch(
        "k",
        key::KIOSK_MENU_SHOW_CHROME,
        key::KIOSK_MENU_HIDE_CHROME,
        "kiosk-chrome-ui",
    ),
    switch(
        "a",
        key::KIOSK_MENU_ADDRESS_SHOW,
        key::KIOSK_MENU_ADDRESS_HIDE,
        "kiosk-address-line",
    ),
    run("R", key::KIOSK_MENU_RELEASE, "kiosk-release", false),
];

/// The zoom's steps — the grid's three, on their own.
const ZOOM: &[Cell] = &[
    run("-", key::KIOSK_MENU_ZOOM_OUT, "kiosk-zoom-out", true),
    run("=", key::KIOSK_MENU_ZOOM_IN, "kiosk-zoom-in", true),
    run("0", key::KIOSK_MENU_ZOOM_RESET, "kiosk-zoom-reset", true),
];

/// The tabs' keys. The hint line is GENERATED from it (`chrome::hint_of`).
/// `K` and `J` carry the tab under the cursor earlier and later — the window
/// menu's `H L K J`, on the list's axis.
pub const OF_THE_TABS: &[(&str, &str)] = &[
    ("<down>", "kiosk-list-down"),
    ("<up>", "kiosk-list-up"),
    ("RET", "kiosk-list-enter"),
    ("K", "kiosk-tab-earlier"),
    ("J", "kiosk-tab-later"),
    ("d", "kiosk-tab-close"),
    ("n", "kiosk-new-tab"),
];

/// The extensions' keys, likewise.
pub const OF_THE_EXTENSIONS: &[(&str, &str)] = &[
    ("<down>", "kiosk-list-down"),
    ("<up>", "kiosk-list-up"),
    ("RET", "kiosk-list-enter"),
    ("o", "kiosk-extension-options"),
    ("i", "kiosk-extension-details"),
];

/// Every command this view's keys reach — for `leader::reachable`, which is
/// what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    ROOT.iter()
        .map(|c| c.command)
        .chain(OF_THE_TABS.iter().map(|(_, id)| *id))
        .chain(OF_THE_EXTENSIONS.iter().map(|(_, id)| *id))
}

/// THE `,` GROUP while a kiosk has the focus, generated from [`ROOT`]:
/// every cell a sequence under `,` with the cell's own key. Built once.
#[must_use]
pub fn local_bindings() -> &'static [Generated] {
    static ONCE: std::sync::OnceLock<Vec<Generated>> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        ROOT.iter()
            .map(|c| {
                (
                    format!("{} {}", ronoctua_protocol::LOCAL_PREFIX, c.key),
                    c.command,
                    None,
                )
            })
            .collect()
    })
}

/// What the grid's switches are switched to now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    /// Chrome's bars are showing.
    pub bars_shown: bool,
    /// The address line is up.
    pub address_line: bool,
}

impl Facts {
    /// Whether the thing a switching cell switches is ON.
    fn on(self, command: &str) -> bool {
        match command {
            "kiosk-chrome-ui" => self.bars_shown,
            "kiosk-address-line" => self.address_line,
            _ => false,
        }
    }
}

/// A cell, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its key.
    pub key: &'static str,
    /// Its label.
    pub label: String,
    /// The whole sentence, for the pointer resting on it.
    pub about: String,
    /// The command it runs.
    pub command: &'static str,
}

/// What the footer does after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again.
    Redraw,
    /// Close the view.
    Close,
    /// Run a command of the registry — and close the view unless it stays.
    Run {
        /// The command.
        id: &'static str,
        /// Its argument.
        arg: Option<String>,
        /// Whether the view stays.
        stays: bool,
    },
}

/// A key of a list, as the list reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Act on the row.
    Enter,
    /// The tab under the cursor, one place earlier.
    Earlier,
    /// One place later.
    Later,
    /// Close the tab under the cursor.
    Close,
    /// A new tab, asking where.
    NewTab,
    /// The extension's options.
    Options,
    /// The extension's entry in Chrome's manager.
    Details,
    /// Up a level, or closed.
    Escape,
}

/// A key of a list, by its WHOLE notation — the one reader the keyboard and
/// a click on the hint both go through.
#[must_use]
pub fn press_of_key(level: Level, key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let table = match level {
        Level::Tabs => OF_THE_TABS,
        Level::Extensions => OF_THE_EXTENSIONS,
        Level::Root | Level::Zoom => return None,
    };
    let notation = key.notation();
    let id = table
        .iter()
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    press_of(id)
}

/// A list's command, as its press — what `LEADER :` running it by name
/// presses on the open view.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "kiosk-list-down" => Press::Down,
        "kiosk-list-up" => Press::Up,
        "kiosk-list-enter" => Press::Enter,
        "kiosk-tab-earlier" => Press::Earlier,
        "kiosk-tab-later" => Press::Later,
        "kiosk-tab-close" => Press::Close,
        "kiosk-new-tab" => Press::NewTab,
        "kiosk-extension-options" => Press::Options,
        "kiosk-extension-details" => Press::Details,
        _ => return None,
    })
}

/// What a row of a list is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A heading, a note: the cursor skips it.
    Nothing,
    /// A tab, by Chrome's id.
    Tab(i64),
    /// An extension, by its id.
    Extension(String),
}

/// One row of a list.
pub struct Row {
    /// The pieces, placed.
    pub fields: Vec<Field>,
    /// The cursor does not stand here.
    pub heading: bool,
    /// What it is.
    pub target: Target,
}

/// What a list is drawn from: the kiosk's tabs and its profile's extensions,
/// when Chrome is connected, and our extension's id to leave out.
#[derive(Debug, Clone, Copy)]
pub struct Reading<'a> {
    /// The kiosk, when Chrome said it.
    pub kiosk: Option<&'a wire::Kiosk>,
    /// Every extension of its profile.
    pub extensions: &'a [wire::Extension],
    /// Ours.
    pub ours: Option<&'a str>,
}

/// The view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// The compositor's window it is about.
    pub window: u64,
    /// Where it is.
    pub level: Level,
    /// The list's cursor, by what it stands on.
    cursor: Target,
}

impl View {
    /// The view about `window`, at `level`.
    #[must_use]
    pub const fn fresh(window: u64, level: Level) -> Self {
        Self {
            window,
            level,
            cursor: Target::Nothing,
        }
    }

    /// Goes to `level` — the tabs' cursor on `tab`, when one is named.
    pub fn go(&mut self, level: Level, tab: Option<i64>) {
        if self.level != level {
            self.cursor = Target::Nothing;
        }
        self.level = level;
        if let Some(t) = tab {
            self.cursor = Target::Tab(t);
        }
    }

    /// Whether the level on screen is a grid — which pages, like the
    /// leader's — rather than a list, which scrolls.
    #[must_use]
    pub const fn is_a_grid(&self) -> bool {
        matches!(self.level, Level::Root | Level::Zoom)
    }

    /// The cells of a grid level, as they are drawn; none on a list.
    #[must_use]
    pub fn entries(&self, facts: Facts, texts: &Texts) -> Vec<Entry> {
        let cells = match self.level {
            Level::Root => ROOT,
            Level::Zoom => ZOOM,
            Level::Tabs | Level::Extensions => return Vec::new(),
        };
        cells
            .iter()
            .map(|c| Entry {
                key: c.key,
                label: texts.t(match c.when_on {
                    Some(on) if facts.on(c.command) => on,
                    _ => c.label,
                }),
                about: texts.t(&format!("cmd-{}", c.command)),
                command: c.command,
            })
            .collect()
    }

    /// Chooses cell `n` of a grid — by its key or a click.
    pub fn choose(&mut self, n: usize) -> Errand {
        let cells = match self.level {
            Level::Root => ROOT,
            Level::Zoom => ZOOM,
            Level::Tabs | Level::Extensions => return Errand::Redraw,
        };
        let Some(c) = cells.get(n) else {
            return Errand::Redraw;
        };
        match c.opens {
            Some(level) => {
                self.level = level;
                self.cursor = Target::Nothing;
                Errand::Redraw
            }
            None => Errand::Run {
                id: c.command,
                arg: None,
                stays: c.stays,
            },
        }
    }

    /// The key with this notation, when a cell of the grid has it.
    pub fn pressed(&mut self, notation: &str) -> Errand {
        let cells = match self.level {
            Level::Root => ROOT,
            Level::Zoom => ZOOM,
            Level::Tabs | Level::Extensions => return Errand::Redraw,
        };
        match cells.iter().position(|c| c.key == notation) {
            Some(n) => self.choose(n),
            None => Errand::Redraw,
        }
    }

    /// `Esc`: up to the grid, and from the grid, closed.
    pub fn back(&mut self) -> Errand {
        if self.level == Level::Root {
            return Errand::Close;
        }
        self.level = Level::Root;
        self.cursor = Target::Nothing;
        Errand::Redraw
    }

    /// The rows of a list level, at this width; none on a grid.
    #[must_use]
    pub fn rows(&self, r: Reading<'_>, columns: u16, texts: &Texts) -> Vec<Row> {
        let (left, room) = crate::power::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.stood_on(r),
            texts,
        };
        match self.level {
            Level::Root | Level::Zoom => return Vec::new(),
            Level::Tabs => {
                let Some(k) = r.kiosk else {
                    b.note(key::ECHO_KIOSK_OFFLINE);
                    return b.out;
                };
                for t in &k.tabs {
                    let host = host_of(&t.url);
                    let value = if t.active {
                        texts.t_with(key::KIOSK_TAB_FRONT, &[("host", host)])
                    } else {
                        host.to_owned()
                    };
                    b.pair(title_of(t).to_owned(), value, Target::Tab(t.id));
                }
            }
            // OFFLINE, THE LIST IS NOT EMPTY — IT IS UNKNOWN. Chrome said
            // nothing, so "no extension besides ours" would be a false
            // sentence; the refusal is a sentence of its own, as the tabs'.
            Level::Extensions if r.kiosk.is_none() => b.note(key::ECHO_KIOSK_OFFLINE),
            Level::Extensions => {
                let theirs: Vec<&wire::Extension> = r
                    .extensions
                    .iter()
                    .filter(|e| Some(e.id.as_str()) != r.ours)
                    .collect();
                let pinned = crate::kiosk::pinned(r.extensions, r.ours);
                let others: Vec<&wire::Extension> = theirs
                    .iter()
                    .copied()
                    .filter(|e| e.enabled && e.pinned_at.is_none())
                    .collect();
                let off: Vec<&wire::Extension> =
                    theirs.iter().copied().filter(|e| !e.enabled).collect();
                if theirs.is_empty() {
                    b.note(key::KIOSK_NO_EXTENSIONS);
                }
                for (heading, list) in [
                    (key::KIOSK_HEADING_PINNED, pinned),
                    (key::KIOSK_HEADING_OTHERS, others),
                    (key::KIOSK_HEADING_DISABLED, off),
                ] {
                    if list.is_empty() {
                        continue;
                    }
                    b.heading(heading);
                    for e in list {
                        b.pair(
                            e.name.clone(),
                            e.version.clone(),
                            Target::Extension(e.id.clone()),
                        );
                    }
                }
            }
        }
        b.out
    }

    /// What the cursor stands on: what it was put on, when that is still
    /// there — else the tab in front, or the first extension.
    fn stood_on(&self, r: Reading<'_>) -> Target {
        match (self.level, &self.cursor) {
            (Level::Tabs, Target::Tab(id))
                if r.kiosk.is_some_and(|k| k.tabs.iter().any(|t| t.id == *id)) =>
            {
                self.cursor.clone()
            }
            (Level::Tabs, _) => r
                .kiosk
                .and_then(|k| crate::kiosk::active(k).or_else(|| k.tabs.first()))
                .map_or(Target::Nothing, |t| Target::Tab(t.id)),
            (Level::Extensions, Target::Extension(id))
                if r.extensions
                    .iter()
                    .any(|e| e.id == *id && Some(id.as_str()) != r.ours) =>
            {
                self.cursor.clone()
            }
            (Level::Extensions, _) => {
                // THE FIRST ROW THE LIST DRAWS, which is the first pinned one.
                let pinned = crate::kiosk::pinned(r.extensions, r.ours);
                pinned
                    .first()
                    .copied()
                    .or_else(|| {
                        r.extensions
                            .iter()
                            .find(|e| e.enabled && Some(e.id.as_str()) != r.ours)
                    })
                    .or_else(|| r.extensions.iter().find(|e| Some(e.id.as_str()) != r.ours))
                    .map_or(Target::Nothing, |e| Target::Extension(e.id.clone()))
            }
            (Level::Root | Level::Zoom, _) => Target::Nothing,
        }
    }

    /// Where the cursor's row is drawn, so the page follows it; `None` on a
    /// grid, which has no cursor.
    #[must_use]
    pub fn drawn_cursor(&self, r: Reading<'_>, rows: &[Row]) -> Option<usize> {
        if self.is_a_grid() {
            return None;
        }
        let at = self.stood_on(r);
        rows.iter().position(|row| row.target == at)
    }

    /// A click on row `n` of a list: the cursor goes there, and whether it was
    /// ALREADY there — the second click, which is `RET` (invariant 19).
    pub fn clicked(&mut self, r: Reading<'_>, n: usize, rows: &[Row]) -> bool {
        let Some(row) = rows.get(n).filter(|row| !row.heading) else {
            return false;
        };
        let again = self.stood_on(r) == row.target;
        self.cursor = row.target.clone();
        again
    }

    /// A key of a list.
    pub fn press(&mut self, p: Press, r: Reading<'_>, rows: &[Row]) -> Errand {
        let at = self.stood_on(r);
        match p {
            Press::Escape => self.back(),
            Press::Up | Press::Down => {
                let here = rows.iter().position(|row| row.target == at);
                let n = rows.len();
                let mut i = here.unwrap_or(0);
                for _ in 0..n {
                    i = if p == Press::Down {
                        (i + 1) % n.max(1)
                    } else {
                        i.checked_sub(1).unwrap_or(n.saturating_sub(1))
                    };
                    if rows.get(i).is_some_and(|row| !row.heading) {
                        self.cursor = rows[i].target.clone();
                        break;
                    }
                }
                Errand::Redraw
            }
            Press::NewTab => Errand::Run {
                id: "kiosk-new-tab",
                arg: None,
                stays: false,
            },
            _ => {
                self.cursor = at.clone();
                acted(p, &at, r)
            }
        }
    }

    /// The hint line: the grid names the tab it acts on, the zoom says where
    /// it is, and a list teaches its keys.
    #[must_use]
    pub fn hint(&self, r: Reading<'_>, texts: &Texts) -> Vec<crate::chrome::Piece> {
        match self.level {
            Level::Root => r
                .kiosk
                .and_then(crate::kiosk::active)
                .map(|t| crate::chrome::Piece::prose(title_of(t)))
                .into_iter()
                .collect(),
            // WHERE THE ZOOM IS, when Chrome said it — and when it did not,
            // that it did not: a default percentage would be a number
            // nobody measured.
            Level::Zoom => {
                let said = match r.kiosk.map(crate::kiosk::active) {
                    Some(Some(t)) => texts.t_args(
                        key::KIOSK_ZOOM_PERCENT,
                        &[(
                            "percent",
                            ronoctua_i18n::Arg::Number(i64::from(t.zoom_percent)),
                        )],
                    ),
                    Some(None) => texts.t(key::ECHO_KIOSK_NO_TAB),
                    None => texts.t(key::ECHO_KIOSK_OFFLINE),
                };
                vec![crate::chrome::Piece::prose(said)]
            }
            Level::Tabs => crate::chrome::hint_of(OF_THE_TABS, texts),
            Level::Extensions => crate::chrome::hint_of(OF_THE_EXTENSIONS, texts),
        }
    }

    /// The argument a list's command takes by name when this view is open —
    /// the row under the cursor — so `LEADER :` and the `,` group act on
    /// what the list shows.
    #[must_use]
    pub fn argument_for(&self, id: &str, r: Reading<'_>) -> Option<String> {
        match (self.level, id, self.stood_on(r)) {
            (Level::Tabs, "kiosk-tab-close" | "kiosk-tab", Target::Tab(t)) => Some(t.to_string()),
            (
                Level::Extensions,
                "kiosk-extension" | "kiosk-extension-options" | "kiosk-extension-details",
                Target::Extension(e),
            ) => Some(e),
            _ => None,
        }
    }
}

/// What a key does to the row under the cursor.
fn acted(p: Press, at: &Target, r: Reading<'_>) -> Errand {
    match (p, at) {
        (Press::Enter, Target::Tab(id)) => Errand::Run {
            id: "kiosk-tab",
            arg: Some(id.to_string()),
            stays: false,
        },
        (Press::Close, Target::Tab(id)) => Errand::Run {
            id: "kiosk-tab-close",
            arg: Some(id.to_string()),
            stays: true,
        },
        (Press::Earlier | Press::Later, Target::Tab(id)) => {
            let Some(t) = r.kiosk.and_then(|k| k.tabs.iter().find(|t| t.id == *id)) else {
                return Errand::Redraw;
            };
            let last = r.kiosk.map_or(0, |k| {
                u32::try_from(k.tabs.len().saturating_sub(1)).unwrap_or(0)
            });
            let to = if p == Press::Earlier {
                t.index.checked_sub(1)
            } else {
                (t.index < last).then_some(t.index + 1)
            };
            to.map_or(Errand::Redraw, |index| Errand::Run {
                id: "kiosk-tab-move",
                arg: Some(format!("{id} {index}")),
                stays: true,
            })
        }
        (Press::Enter, Target::Extension(e)) => Errand::Run {
            id: "kiosk-extension",
            arg: Some(e.clone()),
            stays: false,
        },
        (Press::Options, Target::Extension(e)) => Errand::Run {
            id: "kiosk-extension-options",
            arg: Some(e.clone()),
            stays: false,
        },
        (Press::Details, Target::Extension(e)) => Errand::Run {
            id: "kiosk-extension-details",
            arg: Some(e.clone()),
            stays: false,
        },
        _ => Errand::Redraw,
    }
}

/// A tab's title, or its address before it has one.
fn title_of(t: &wire::Tab) -> &str {
    if t.title.trim().is_empty() {
        &t.url
    } else {
        &t.title
    }
}

/// The host of an address, without its scheme.
fn host_of(address: &str) -> &str {
    let (head, _) = ronoctua_browser::url::split_for_the_line(address);
    head.split_once("://").map_or(head, |(_, host)| host)
}

/// Builds the rows — `geo.rs`'s builder, and the same reasons.
struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: Target,
    texts: &'a Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, colour: tokens::Color, room: u16) -> Field {
        Field {
            at,
            text: layout::elide_end(&text, room),
            color: colour,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    fn heading(&mut self, key: &str) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                heading: true,
                target: Target::Nothing,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                self.texts.t(key),
                tokens::color::TEXT_HEADING,
                self.room,
            )],
            heading: true,
            target: Target::Nothing,
        });
    }

    /// A sentence: IT WRAPS (`chrome::note_lines`).
    fn note(&mut self, key: &str) {
        for line in crate::chrome::note_lines(&self.texts.t(key), self.room) {
            self.out.push(Row {
                fields: vec![Self::piece(
                    self.left,
                    line,
                    tokens::color::TEXT_DIM,
                    self.room,
                )],
                heading: true,
                target: Target::Nothing,
            });
        }
    }

    fn pair(&mut self, label: String, value: String, target: Target) {
        let focused = self.cursor == target;
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                crate::chrome::Kind::ListedSetting,
                focused,
            ),
            heading: false,
            target,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> Texts {
        Texts::fresh("en-GB").expect("the base language")
    }

    fn tab(id: i64, index: u32, active: bool) -> wire::Tab {
        wire::Tab {
            id,
            index,
            active,
            title: format!("Tab {id}"),
            url: format!("https://t{id}.example/x"),
            zoom_percent: 100,
            ..wire::Tab::default()
        }
    }

    fn kiosk(tabs: Vec<wire::Tab>) -> wire::Kiosk {
        wire::Kiosk {
            window: 45,
            nonce: "n".into(),
            tabs,
            ..wire::Kiosk::default()
        }
    }

    /// FAILS IF: the `,` group and the grid say different things — a key of
    /// one that runs something else in the other.
    #[test]
    fn the_local_rows_are_the_grid() {
        let rows = local_bindings();
        assert_eq!(rows.len(), ROOT.len());
        for (row, cell) in rows.iter().zip(ROOT) {
            assert_eq!(row.0, format!(", {}", cell.key));
            assert_eq!(row.1, cell.command);
        }
    }

    /// FAILS IF: two cells share a key, or a key's notation does not read
    /// back as itself — a cell no key would ever reach.
    #[test]
    fn every_key_is_unique_and_parses() {
        for cells in [ROOT, ZOOM] {
            let keys: Vec<&str> = cells.iter().map(|c| c.key).collect();
            for (i, k) in keys.iter().enumerate() {
                assert!(!keys[..i].contains(k), "`{k}` twice");
                let parsed = ronoctua_keymap::Key::parse(k).expect("the notation parses");
                assert_eq!(
                    parsed.notation(),
                    *k,
                    "`{k}` reads back as another notation"
                );
            }
        }
    }

    /// FAILS IF: a command of the grid or of a list is not in the registry,
    /// so its key would lead nowhere.
    #[test]
    fn every_command_is_registered() {
        let (_, registry) = crate::leader::default_of().expect("the default keymap");
        for id in reached() {
            assert!(registry.search(id).is_some(), "`{id}` is not a command");
        }
    }

    /// FAILS IF: the tabs' cursor follows the row instead of the tab when
    /// Chrome reorders them.
    #[test]
    fn the_cursor_keeps_the_tab_across_a_reorder() {
        let t = texts();
        let mut v = View::fresh(7, Level::Tabs);
        v.go(Level::Tabs, Some(2));
        let before = kiosk(vec![tab(1, 0, true), tab(2, 1, false), tab(3, 2, false)]);
        let r = Reading {
            kiosk: Some(&before),
            extensions: &[],
            ours: None,
        };
        let rows = v.rows(r, 80, &t);
        assert_eq!(v.drawn_cursor(r, &rows), Some(1));
        let after = kiosk(vec![tab(2, 0, false), tab(1, 1, true), tab(3, 2, false)]);
        let r = Reading {
            kiosk: Some(&after),
            ..r
        };
        let rows = v.rows(r, 80, &t);
        assert_eq!(
            v.drawn_cursor(r, &rows),
            Some(0),
            "the cursor went with tab 2"
        );
        v.press(Press::Down, r, &rows);
        assert_eq!(v.argument_for("kiosk-tab-close", r).as_deref(), Some("1"));
    }

    /// FAILS IF: `K` and `J` do not move the tab under the cursor, or the
    /// list's `RET` does not bring it to the front.
    #[test]
    fn k_and_j_move_the_tab_under_the_cursor() {
        let t = texts();
        let k = kiosk(vec![tab(1, 0, true), tab(2, 1, false), tab(3, 2, false)]);
        let r = Reading {
            kiosk: Some(&k),
            extensions: &[],
            ours: None,
        };
        let mut v = View::fresh(7, Level::Tabs);
        v.go(Level::Tabs, Some(2));
        let rows = v.rows(r, 80, &t);
        let key = |s: &str| ronoctua_keymap::Key::parse(s).expect("parses");
        let earlier = press_of_key(Level::Tabs, key("K")).expect("K is a key here");
        assert_eq!(
            v.press(earlier, r, &rows),
            Errand::Run {
                id: "kiosk-tab-move",
                arg: Some("2 0".into()),
                stays: true
            }
        );
        let later = press_of_key(Level::Tabs, key("J")).expect("J is a key here");
        assert_eq!(
            v.press(later, r, &rows),
            Errand::Run {
                id: "kiosk-tab-move",
                arg: Some("2 2".into()),
                stays: true
            }
        );
        assert_eq!(
            v.press(Press::Enter, r, &rows),
            Errand::Run {
                id: "kiosk-tab",
                arg: Some("2".into()),
                stays: false
            }
        );
    }

    /// FAILS IF: `Esc` does not go up to the grid, and from there close; or
    /// a door of the grid does not open its level.
    #[test]
    fn esc_goes_up_then_closes() {
        let mut v = View::fresh(7, Level::Root);
        assert_eq!(v.pressed("t"), Errand::Redraw);
        assert_eq!(v.level, Level::Tabs);
        assert_eq!(v.back(), Errand::Redraw);
        assert_eq!(v.level, Level::Root);
        assert_eq!(v.back(), Errand::Close);
        assert_eq!(
            v.pressed("h"),
            Errand::Run {
                id: "kiosk-back",
                arg: None,
                stays: true
            }
        );
    }

    /// FAILS IF: a switch is drawn on the side it already is, or two labels
    /// of one level say the same thing.
    #[test]
    fn a_switch_says_which_way_it_goes() {
        let t = texts();
        let v = View::fresh(7, Level::Root);
        let hidden = v.entries(Facts::default(), &t);
        let shown = v.entries(
            Facts {
                bars_shown: true,
                ..Facts::default()
            },
            &t,
        );
        let k = |e: &[Entry]| e.iter().find(|e| e.key == "k").map(|e| e.label.clone());
        assert_eq!(k(&hidden), Some(t.t(key::KIOSK_MENU_SHOW_CHROME)));
        assert_eq!(k(&shown), Some(t.t(key::KIOSK_MENU_HIDE_CHROME)));
        let labels: Vec<&String> = hidden.iter().map(|e| &e.label).collect();
        for (i, l) in labels.iter().enumerate() {
            assert!(!labels[..i].contains(l), "{l} twice");
        }
    }

    /// FAILS IF: the extensions list offers ours, or does not open on the
    /// first pinned one.
    #[test]
    fn the_extensions_leave_ours_out() {
        let t = texts();
        let ours = "abcdefghijklmnopabcdefghijklmnop";
        let e = |id: &str, pinned: Option<u32>, enabled: bool| wire::Extension {
            id: id.into(),
            name: id.into(),
            enabled,
            pinned_at: pinned,
            ..wire::Extension::default()
        };
        let all = [
            e(ours, Some(0), true),
            e("b", None, true),
            e("c", Some(3), true),
            e("d", None, false),
        ];
        let connected = kiosk(vec![tab(1, 0, true)]);
        let r = Reading {
            kiosk: Some(&connected),
            extensions: &all,
            ours: Some(ours),
        };
        let v = View::fresh(7, Level::Extensions);
        let rows = v.rows(r, 80, &t);
        assert!(
            !rows
                .iter()
                .any(|row| row.target == Target::Extension(ours.into()))
        );
        let at = v.drawn_cursor(r, &rows).expect("a cursor");
        assert_eq!(rows[at].target, Target::Extension("c".into()));
    }

    /// The words of a list's rows, joined.
    fn said(rows: &[Row]) -> String {
        rows.iter()
            .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// FAILS IF: with Chrome not connected, the extensions' list says this
    /// Chrome has none, or the zoom's level says a percentage nobody
    /// measured — an empty panel standing in for a refused one.
    #[test]
    fn offline_the_lists_say_so_and_invent_nothing() {
        let t = texts();
        let offline = Reading {
            kiosk: None,
            extensions: &[],
            ours: None,
        };
        let rows = View::fresh(7, Level::Extensions).rows(offline, 80, &t);
        let words = said(&rows);
        assert!(
            words.contains(&t.t(key::ECHO_KIOSK_OFFLINE)),
            "the refusal is said: {words}"
        );
        assert!(
            !words.contains(&t.t(key::KIOSK_NO_EXTENSIONS)),
            "\"no extension besides ours\" is not known offline: {words}"
        );
        let hint = View::fresh(7, Level::Zoom).hint(offline, &t);
        let hint: String = hint.iter().map(|p| p.text.clone()).collect();
        assert_eq!(hint, t.t(key::ECHO_KIOSK_OFFLINE));
        // CONNECTED, the zoom says where it is, and an empty list says so.
        let connected = kiosk(vec![wire::Tab {
            zoom_percent: 125,
            ..tab(1, 0, true)
        }]);
        let online = Reading {
            kiosk: Some(&connected),
            ..offline
        };
        let hint = View::fresh(7, Level::Zoom).hint(online, &t);
        let hint: String = hint.iter().map(|p| p.text.clone()).collect();
        assert!(hint.contains("125"), "{hint}");
        let rows = View::fresh(7, Level::Extensions).rows(online, 80, &t);
        assert!(said(&rows).contains(&t.t(key::KIOSK_NO_EXTENSIONS)));
    }
}
