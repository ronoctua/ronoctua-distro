//! The keyboards — `LEADER s k`.
//!
//! # What this is for
//!
//! The compositor has ONE keyboard configuration for every keyboard attached
//! to the machine: one layout, one repeat rate, no way to name a device. That
//! is niri's shape, and invariant 3 keeps it out of reach. So "this keyboard,
//! not that one" lives below the compositor, in `keyd` — and this is where a
//! person says it.
//!
//! # What it can and cannot promise, and it says so on screen
//!
//! REMAPPING is per keyboard, because keyd matches devices by
//! `vendor:product`. The LAYOUT, the compose key and the repeat rate are not:
//! the compositor applies one of each to everything, and a screen that offered
//! them here would be offering a per-keyboard setting that is not one. They
//! stay where they are (`nix/keyboard-xkb.nix`, and `LEADER s d` for the
//! text), and this buffer is about the one thing that really is per device.
//!
//! # Invariant 11, and how the vocabulary gets here
//!
//! This component holds no table of ours. The keyboards, the keys it may
//! offer and what they may become all ARRIVE, as `input::keyboards`, each one
//! carrying keyd's own word and the text key that names it. The buffer draws
//! the text keys and hands the keyd words back — it translates nothing, reads
//! nothing and writes nothing.

use std::cell::RefCell;

use ronoctua_design::guest::Wit;
use ronoctua_design::types::{
    Binding, Board, Boards, BufferState, Capability, Command, Context, Descriptor, Effect, Guest,
    Icon, Input, Remap, Role, Run, Screen, Scroll, Tone,
};

/// The text keys this buffer draws. Every one of them is owed by both
/// languages, and `buffer-keys-have-text` is what says so.
mod key {
    /// The heading of the list.
    pub const TITLE: &str = "keyboard-title";
    /// The heading of one keyboard's own page.
    pub const ONE: &str = "keyboard-one";
    /// No keyboard at all — which on a laptop means `/proc` said nothing.
    pub const NONE: &str = "keyboard-none";
    /// This keyboard is part of the machine.
    pub const BUILT_IN: &str = "keyboard-built-in";
    /// This keyboard was plugged or paired in.
    pub const ATTACHED: &str = "keyboard-attached";
    /// How many keys somebody has changed on it.
    pub const CHANGED: &str = "keyboard-changed";
    /// Nothing has been changed on it.
    pub const UNTOUCHED: &str = "keyboard-untouched";
    /// The column of the keys themselves.
    pub const THE_KEY: &str = "keyboard-the-key";
    /// The column that says what a key does now.
    pub const BECOMES: &str = "keyboard-becomes";
    /// What a key does when nobody has changed it.
    ///
    /// SEPARATE FROM THE HEADING, and it was not: one key served as both, so
    /// the header row drew the VALUE — a line with no label and the word
    /// `itself` sitting in the value column, which the owner photographed.
    pub const UNCHANGED: &str = "keyboard-unchanged";
    /// Held one way, tapped the other.
    pub const HOLDS: &str = "keyboard-holds";
    /// What is global and cannot be per keyboard, said out loud.
    pub const GLOBAL: &str = "keyboard-global";
    /// Two keyboards of one model answer to one rule.
    pub const SAME_MODEL: &str = "keyboard-same-model";
    /// This key is the leader, and moving it costs the leader.
    pub const IS_THE_LEADER: &str = "keyboard-is-the-leader";
}

/// The keys this buffer binds, all under `m` — the host refuses any other
/// prefix at load.
const KEYMAP: &[(&str, &str)] = &[
    ("m n", "keyboard-next"),
    ("m p", "keyboard-previous"),
    ("m l", "keyboard-list"),
];

/// Which screen is open.
// NOT `Copy` any more: the level carries the keyboard's ID, and a `String`
// is not copyable. Everything that reads it borrows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Level {
    /// Every keyboard the machine has.
    #[default]
    List,
    /// One keyboard, and every key it offers.
    ///
    /// THE ID AND NOT THE INDEX. The list is the kernel's, sorted by that same
    /// id, and it changes under the cursor: a keyboard plugged in or a battery
    /// running out moves every row after it. Held as an index, the screen would
    /// go on saying one keyboard's name while the arrows remapped another —
    /// which is the transfer queue's lesson, in `AGENTS.md`.
    One(String),
}

/// Everything this buffer holds.
#[derive(Default)]
struct State {
    /// The whole picture, as the host last sent it.
    ///
    /// NEVER assembled here out of changes: the host reads the kernel and the
    /// person's file and sends both, which is the rule `rack` and `queue`
    /// already follow. A buffer that kept its own copy would be a second place
    /// where the truth lives, and the two would disagree the first time a
    /// keyboard was unplugged.
    boards: Option<Boards>,
    level: Level,
    /// Which row the cursor is on, within whichever level is open.
    cursor: usize,
    /// The first row of the list that is drawn.
    offset: usize,
    /// Whether the window still owes the cursor a move.
    ///
    /// A KEY asks for it; the wheel does not. The wheel scrolls the page and
    /// leaves the cursor where it is — which is what every window in the shell
    /// does — and a window that followed unconditionally would put it back on
    /// the next frame.
    follows: bool,
    /// The cell the pointer is on, if it is on one.
    hover: Option<(u16, u16)>,
    /// WHERE THE LIST WAS DRAWN, remembered where it was measured.
    ///
    /// The first line depends on the heading's own arithmetic, which depends
    /// on the window's height — so it cannot be recomputed from nothing, and
    /// a click resolved by a second guess lands on a different row from the
    /// band. It is the footer's `region_can_spare_a_row` idiom: what the
    /// drawing learned, written down where it learned it.
    list_top: u16,
    /// How many rows of it are on the screen.
    list_lines: u16,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

impl State {
    /// How many rows the open level has.
    fn rows(&self) -> usize {
        let Some(b) = self.boards.as_ref() else {
            return 0;
        };
        match &self.level {
            Level::List => b.boards.len(),
            Level::One(_) => b.keys.len(),
        }
    }

    /// The keyboard the cursor is about, on either level.
    fn board(&self) -> Option<&Board> {
        let b = self.boards.as_ref()?;
        match &self.level {
            Level::List => b.boards.get(self.cursor),
            Level::One(id) => b.boards.iter().find(|k| k.id == *id),
        }
    }

    /// What this key becomes on this keyboard, as keyd's word.
    ///
    /// Empty is "leave it alone", which is the ABSENCE of a rule — the same
    /// answer for a keyboard nobody has touched and for a key nobody has
    /// touched on a keyboard that was.
    fn becomes(&self, board: &Board, keyd: &str) -> String {
        board
            .decided
            .iter()
            .find(|(k, _)| k == keyd)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    /// The choice one step around the ring from where this key is.
    ///
    /// A RING AND NOT A LIST TO PICK FROM: the choices are nine, the way back
    /// is the first of them, and cycling is one key rather than a second
    /// screen. It is the shape the visibility timeout already has.
    fn ring(&self, now: &str, forward: bool) -> String {
        let Some(b) = self.boards.as_ref() else {
            return String::new();
        };
        if b.choices.is_empty() {
            return String::new();
        }
        let at = b.choices.iter().position(|c| c.keyd == now).unwrap_or(0);
        let len = b.choices.len();
        let next = if forward {
            (at + 1) % len
        } else {
            (at + len - 1) % len
        };
        b.choices[next].keyd.clone()
    }

    /// The remap the cursor's key would become, one step around the ring.
    fn turn(&self, forward: bool) -> Option<Remap> {
        let Level::One(_) = &self.level else {
            return None;
        };
        let b = self.boards.as_ref()?;
        let board = self.board()?;
        let word = b.keys.get(self.cursor)?;
        let now = self.becomes(board, &word.keyd);
        Some(Remap {
            device: board.id.clone(),
            key: word.keyd.clone(),
            becomes: self.ring(&now, forward),
        })
    }

    /// How many rows of the list are shown, and the first of them.
    ///
    /// ONE arithmetic: the drawing walks this window and the pointer is
    /// resolved against the same two numbers. Written twice, a click lands on
    /// the row the band is not on.
    fn window(&mut self, top: u16, height: u16) -> (usize, usize) {
        // WHAT THE LEVEL DRAWS UNDER ITS LIST, which the list may not take:
        // the list level ends with the sentence about what this screen cannot
        // do, and a keyboard's own page ends with the technical word under the
        // cursor and the warning about two of a model.
        let after = match self.level {
            Level::List => 2,
            Level::One(_) => 4,
        };
        let room = usize::from(height.saturating_sub(top + after).max(1));
        let rows = self.rows();
        if self.follows {
            if self.cursor < self.offset {
                self.offset = self.cursor;
            } else if self.cursor >= self.offset + room {
                self.offset = self.cursor + 1 - room;
            }
        }
        self.offset = self.offset.min(rows.saturating_sub(room.min(rows)));
        (self.offset, room)
    }

    /// Which row of the list a line of the screen is, if it is one.
    fn row_at(&self, line: u16) -> Option<usize> {
        if line < self.list_top || line >= self.list_top + self.list_lines {
            return None;
        }
        let n = self.offset + usize::from(line - self.list_top);
        (n < self.rows()).then_some(n)
    }

    /// What `RET` does where the cursor is.
    ///
    /// Its own function because the POINTER does it too: a second click on a
    /// row is the row's own key, which is the rule the file manager already
    /// has written down and the one the owner asked to be absolute.
    fn confirm(&mut self) -> Vec<Effect> {
        match self.level {
            Level::List if self.rows() > 0 => {
                let Some(id) = self.board().map(|k| k.id.clone()) else {
                    return Vec::new();
                };
                self.level = Level::One(id);
                self.cursor = 0;
                self.offset = 0;
                vec![Effect::Redraw]
            }
            Level::List => Vec::new(),
            Level::One(_) => self.turn(true).map(Effect::Remap).into_iter().collect(),
        }
    }

    /// A click on a row: the first SELECTS, the second does what `RET` does.
    fn reach(&mut self, n: usize) -> Vec<Effect> {
        if self.cursor == n {
            return self.confirm();
        }
        self.cursor = n;
        self.follows = true;
        vec![Effect::Redraw]
    }

    /// Which row of the list the pointer is on, if it is on one.
    fn hovered_row(&self) -> Option<usize> {
        let (_, line) = self.hover?;
        self.row_at(line)
    }

    /// THE BAR, in the lane kept for it, when the list does not fit.
    ///
    /// A list one cannot see the length of is the defect
    /// `every-list-window-has-a-bar` names on the host's side; a buffer owes
    /// the same thing, and a list with no bar and no wheel is a list whose end
    /// the mouse cannot reach at all.
    /// THE BAND AND THE WINDOW ARE ONE NUMBER, and it is how many rows were
    /// DRAWN. The host says so out loud when they differ — a thumb against a
    /// track the band does not have — and with a list shorter than the room it
    /// is the difference between two and thirty-five.
    fn draw_bar(&self, screen: &mut Screen<Wit>, columns: u16, total: usize) {
        let shown = usize::from(self.list_lines);
        screen.bar(
            self.list_top,
            self.list_lines,
            columns.saturating_sub(1),
            Scroll {
                total,
                lines: shown,
                offset: self.offset,
                top: 0,
            },
        );
    }

    /// Keeps the cursor on a row that exists — the list changes under it when
    /// a keyboard is unplugged, and a cursor past the end focuses nothing.
    fn settle(&mut self) {
        let rows = self.rows();
        if rows == 0 {
            self.cursor = 0;
        } else if self.cursor >= rows {
            self.cursor = rows - 1;
        }
        // AND A LEVEL ABOUT A KEYBOARD THAT WENT AWAY GOES BACK TO THE LIST.
        // A page of keys for a keyboard nobody can see is a page where every
        // key would be refused, one at a time, with nothing saying why.
        if let Level::One(id) = &self.level
            && self
                .boards
                .as_ref()
                .is_none_or(|b| !b.boards.iter().any(|k| k.id == *id))
        {
            self.level = Level::List;
            self.cursor = 0;
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "keyboard".into(),
            mode: "keyboard".into(),
            keymap: KEYMAP
                .iter()
                .map(|(keys, command)| Binding {
                    keys: (*keys).into(),
                    command: (*command).into(),
                })
                .collect(),
            // TWO STATES, and `normal` first because that is where `Esc`
            // returns to. The second says the screen is about ONE keyboard,
            // which is the thing a person needs to know before pressing an
            // arrow that changes what a key does.
            states: vec![
                BufferState {
                    id: "normal".into(),
                    letter: 'n',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "keyboard-editing".into(),
                    letter: 'e',
                    tone: Tone::Accent,
                },
            ],
            commands: KEYMAP.iter().map(|(_, c)| (*c).into()).collect(),
            segments: vec![],
            // THE ONE IT NEEDS, and it is its own for a reason the protocol
            // spells out: what this reaches is the file a daemon between the
            // keyboard and everything else reads.
            capabilities: vec![Capability::Keyboard],
            phrases: vec![],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| s.draw(&ctx))
    }

    fn event(e: Input) -> Vec<Effect> {
        STATE.with_borrow_mut(|s| s.act(e))
    }
}

impl State {
    /// THE WHOLE FRAME, and the only place it is laid out.
    ///
    /// `render` calls it and so do the tests — a test that assembled its own
    /// frame would be proving a screen nobody is shown.
    #[allow(clippy::too_many_lines)]
    fn draw(&mut self, ctx: &Context) -> Vec<Command> {
        {
            let s = self;
            let mut screen = Screen::<Wit>::of(ctx);
            let columns = screen.columns();
            // A CELL IS KEPT FOR THE BAR, and every band stops before it. The
            // band is full-bleed and the bar is drawn after, so without this
            // the control sits on top of the selected row — which is what the
            // owner reported of the brain, in those words.
            let body = columns.saturating_sub(1);
            // WHERE THE SECOND COLUMN STARTS. A third of the width, floored so
            // a narrow window still leaves the name something to be read in.
            let second = (columns / 3).max(12);

            let heading = match s.level {
                Level::List => key::TITLE,
                Level::One(_) => key::ONE,
            };
            let mut line = screen.heading(Icon::Keyboard, heading);

            // CLONED, because the drawing asks the state where its window is
            // and that answer is remembered where it is measured — see
            // `State::list_top`. A borrow held across the walk would make the
            // question unaskable, and the second place to ask it from is the
            // place the click and the band come to disagree.
            let Some(b) = s.boards.clone() else {
                return screen.finish_wit();
            };

            // WHAT WENT WRONG, FIRST AND IN THE LOUD TONE. A refusal nobody
            // sees is a key that did nothing.
            if !b.trouble.is_empty() {
                // `put` RETURNS THE HEIGHT, not the next line — the row does
                // not move, because a row of columns is several runs at one
                // line. Assigning it here set `line` to 1 and the list then
                // drew over the sentence.
                screen.put(line, Run::label(&b.trouble).accent());
                line += 2;
            }

            match s.level {
                Level::List => {
                    if b.boards.is_empty() {
                        screen.put(line, Run::label(key::NONE).dim());
                        return screen.finish_wit();
                    }
                    let (from, room) = s.window(line, ctx.lines);
                    s.list_top = line;
                    let shown = room.min(b.boards.len().saturating_sub(from));
                    s.list_lines = u16::try_from(shown).unwrap_or(u16::MAX);
                    for (n, board) in b.boards.iter().enumerate().skip(from).take(room) {
                        let here = n == s.cursor;
                        if here {
                            screen.ground_in(line, 0, body, Role::Band);
                        } else if s.hovered_row() == Some(n) {
                            // THE ROW UNDER THE POINTER, in a tone of its own:
                            // the cursor is where the KEYBOARD is and the hover
                            // is where a click would land, and every window in
                            // the shell answers the two separately.
                            screen.ground_in(line, 0, body, Role::Hover);
                        }
                        let name = Run::datum(&board.name).within(second.saturating_sub(1));
                        screen.put(line, if here { name.accent() } else { name });
                        let where_from = if board.built_in {
                            key::BUILT_IN
                        } else {
                            key::ATTACHED
                        };
                        // EVERY COLUMN CARRIES ITS BUDGET. Without one a run
                        // is "as far as there is", and at 46 columns `part of
                        // this machine` reached across the count beside it —
                        // the driver refused it, on line 3, by name.
                        screen.put(
                            line,
                            Run::label(where_from)
                                .at(second)
                                .within(second.saturating_sub(1))
                                .quiet(),
                        );
                        // HOW MANY KEYS SOMEBODY MOVED, as a number and then
                        // the word for it. A count cannot go inside a text key
                        // — the buffer sends a key and the host resolves it,
                        // with no arguments to put a number in (invariant 11)
                        // — so the two are two runs on one line.
                        let changed = board.decided.len();
                        // AND THE LAST COLUMN STOPS BEFORE THE BAR'S LANE: a
                        // field measured against the whole width reaches the
                        // cell the control is in, and no bar draws over
                        // content nor content over a bar.
                        let last = body.saturating_sub(second * 2);
                        if changed == 0 {
                            screen.put(
                                line,
                                Run::label(key::UNTOUCHED).dim().at(second * 2).within(last),
                            );
                        } else {
                            screen.put(line, Run::datum(&changed.to_string()).at(second * 2));
                            screen.put(
                                line,
                                Run::label(key::CHANGED)
                                    .quiet()
                                    .at(second * 2 + 3)
                                    .within(last.saturating_sub(3)),
                            );
                        }
                        line += 1;
                    }
                    s.draw_bar(&mut screen, columns, b.boards.len());
                    // AND WHAT THIS SCREEN CANNOT DO, said rather than left to
                    // be discovered. The layout and the repeat rate are the
                    // compositor's, one of each for every keyboard there is —
                    // a page that stayed quiet about it would read as a page
                    // that had simply forgotten them.
                    line += 1;
                    screen.put(line, Run::label(key::GLOBAL).dim());
                }
                Level::One(_) => {
                    let Some(board) = s.board().cloned() else {
                        return screen.finish_wit();
                    };
                    screen.put(line, Run::datum(&board.name).quiet());
                    line += 2;
                    // A HEADER HAS TWO HALVES, like the rows under it. With
                    // only the right one it read as a row whose label had gone
                    // missing.
                    screen.put(
                        line,
                        Run::label(key::THE_KEY)
                            .within(second.saturating_sub(1))
                            .dim(),
                    );
                    screen.put(
                        line,
                        Run::label(key::BECOMES)
                            .at(second)
                            .within(second.saturating_sub(1))
                            .dim(),
                    );
                    line += 1;
                    let (from, room) = s.window(line, ctx.lines);
                    s.list_top = line;
                    let shown = room.min(b.keys.len().saturating_sub(from));
                    s.list_lines = u16::try_from(shown).unwrap_or(u16::MAX);
                    for (n, word) in b.keys.iter().enumerate().skip(from).take(room) {
                        let here = n == s.cursor;
                        if here {
                            screen.ground_in(line, 0, body, Role::Band);
                        } else if s.hovered_row() == Some(n) {
                            screen.ground_in(line, 0, body, Role::Hover);
                        }
                        let label = Run::label(&word.text).within(second.saturating_sub(1));
                        screen.put(line, if here { label.accent() } else { label });
                        let now = s.becomes(&board, &word.keyd);
                        let said = b
                            .choices
                            .iter()
                            .find(|c| c.keyd == now)
                            .map_or((key::UNCHANGED, false), |c| (c.text.as_str(), c.holds));
                        screen.put(
                            line,
                            Run::label(said.0)
                                .at(second)
                                .within(second.saturating_sub(1)),
                        );
                        if said.1 {
                            screen.put(
                                line,
                                Run::label(key::HOLDS)
                                    .at(second * 2)
                                    .within(body.saturating_sub(second * 2))
                                    .dim(),
                            );
                        }
                        line += 1;
                    }
                    s.draw_bar(&mut screen, columns, b.keys.len());
                    // THE TECHNICAL WORD, UNDER THE CURSOR AND NOWHERE ELSE.
                    //
                    // keyd's own name for the key and for what it becomes. It
                    // is the format buffer's register: a name nobody can search
                    // for is a name nobody can fix, and a column of them would
                    // be a screen of jargon.
                    // AND THE ONE KEY THAT COSTS SOMETHING TO MOVE.
                    //
                    // Every file the shell renders claims the leader's key,
                    // and a rule written after it wins — so a person who
                    // remaps it gets what they asked for and loses the key
                    // that opens everything, on that keyboard. Said BEFORE the
                    // arrow, on the row, rather than discovered afterwards.
                    if b.keys.get(s.cursor).is_some_and(|w| w.keyd == b.leader) {
                        line += 1;
                        screen.put(line, Run::label(key::IS_THE_LEADER).accent());
                    }
                    if let Some(word) = b.keys.get(s.cursor) {
                        let now = s.becomes(&board, &word.keyd);
                        let shown = if now.is_empty() {
                            word.keyd.clone()
                        } else {
                            format!("{} = {now}", word.keyd)
                        };
                        line += 1;
                        screen.put(line, Run::datum(&shown).dim().small());
                    }
                    // AND THE WARNING THAT TWO OF A MODEL ARE ONE RULE. keyd
                    // matches on `vendor:product`, which two identical
                    // keyboards share; saying so is cheaper than a person
                    // discovering it.
                    if b.boards.iter().filter(|o| o.id == board.id).count() > 1 {
                        line += 1;
                        screen.put(line, Run::label(key::SAME_MODEL).dim());
                    }
                }
            }
            screen.finish_wit()
        }
    }

    /// One input, answered.
    fn act(&mut self, e: Input) -> Vec<Effect> {
        {
            let s = self;
            // WHAT LEVEL THIS STARTED ON, and it is read for EVERY input.
            //
            // It used to be read only for a key, which left two paths that
            // moved the level and said nothing: `enter-state`, which the host
            // sends when somebody taps `Esc` or picks a state under `SPC w s`
            // — the driver caught that one — and `settle`, which sends the
            // page of a keyboard that has been unplugged back to the list. A
            // level that moves without reporting is a modeline naming a screen
            // nobody is on.
            let was = s.level.clone();
            let effects = match e {
                Input::Keyboards(b) => {
                    s.boards = Some(b);
                    s.settle();
                    vec![Effect::Redraw]
                }
                // THE HOST'S HALF OF THE CONTRACT: it names a state the
                // descriptor declared, and the buffer goes there and says so.
                // Silence here is a state nobody can reach, with the modeline
                // going on offering it.
                Input::EnterState(id) => {
                    s.level = if id == "keyboard-editing" {
                        Level::One(s.board().map(|k| k.id.clone()).unwrap_or_default())
                    } else {
                        Level::List
                    };
                    s.cursor = 0;
                    vec![Effect::Redraw]
                }
                Input::Down => {
                    if s.cursor + 1 < s.rows() {
                        s.cursor += 1;
                    }
                    // A PERSON MOVED IT, so the window owes it a move.
                    s.follows = true;
                    vec![Effect::Redraw]
                }
                Input::Up => {
                    s.cursor = s.cursor.saturating_sub(1);
                    s.follows = true;
                    vec![Effect::Redraw]
                }
                // THE POINTER, and all four of its questions. A screen only
                // the keyboard can reach is half a screen — the rule is
                // absolute, and this buffer had no click at all.
                Input::Click(p) => match s.row_at(p.line) {
                    Some(n) => s.reach(n),
                    None => Vec::new(),
                },
                Input::Hover(p) => {
                    let now = Some((p.column, p.line));
                    if s.hover == now {
                        return Vec::new();
                    }
                    s.hover = now;
                    vec![Effect::Redraw]
                }
                Input::Unhover => {
                    if s.hover.is_none() {
                        return Vec::new();
                    }
                    s.hover = None;
                    vec![Effect::Redraw]
                }
                // THE WHEEL SCROLLS THE WINDOW and leaves the cursor where it
                // is: this page explains whatever the cursor is on, and a
                // wheel that moved it would change the explanation while
                // somebody is reading it.
                Input::Wheel(w) => {
                    let rows = s.rows();
                    let room = usize::from(s.list_lines.max(1));
                    let most = rows.saturating_sub(room);
                    s.offset = if w.lines < 0 {
                        s.offset.saturating_sub(w.lines.unsigned_abs() as usize)
                    } else {
                        (s.offset + w.lines as usize).min(most)
                    };
                    s.follows = false;
                    vec![Effect::Redraw]
                }
                // AND THE BAR IS DRAGGED. The host built the thumb and owns
                // the inverse; this says which list the row belongs to, which
                // is the half only this side knows.
                Input::Scrolled(d) => {
                    let rows = s.rows();
                    let room = usize::from(s.list_lines.max(1));
                    s.offset = (d.first as usize).min(rows.saturating_sub(room.min(rows)));
                    s.follows = false;
                    vec![Effect::Redraw]
                }
                // `RET` OPENS A KEYBOARD, and on a key it is one step round the
                // ring — the same thing the right arrow does, because a row
                // with an order has one obvious meaning for `RET`.
                Input::Confirm | Input::Right => s.confirm(),
                Input::Left => match s.level {
                    Level::One(_) => s.turn(false).map(Effect::Remap).into_iter().collect(),
                    Level::List => vec![],
                },
                // `Esc` GOES BACK ONE LEVEL and never closes: the key one
                // presses by reflex to leave anything at all must not destroy
                // the buffer.
                Input::Cancel => match &s.level {
                    // AND IT COMES BACK TO THE KEYBOARD IT WAS ON, found by
                    // id: the list may have moved while somebody was inside.
                    Level::One(id) => {
                        let at = s
                            .boards
                            .as_ref()
                            .and_then(|b| b.boards.iter().position(|k| k.id == *id));
                        s.level = Level::List;
                        s.cursor = at.unwrap_or(0);
                        vec![Effect::Redraw]
                    }
                    Level::List => vec![],
                },
                Input::Command(id) => match id.as_str() {
                    "keyboard-next" => {
                        if s.cursor + 1 < s.rows() {
                            s.cursor += 1;
                        }
                        vec![Effect::Redraw]
                    }
                    "keyboard-previous" => {
                        s.cursor = s.cursor.saturating_sub(1);
                        vec![Effect::Redraw]
                    }
                    "keyboard-list" => {
                        s.level = Level::List;
                        s.cursor = 0;
                        vec![Effect::Redraw]
                    }
                    _ => vec![],
                },
                _ => vec![],
            };
            // THE STATE IS REPORTED ONCE, and only when it CHANGED: the host
            // learns where the buffer is from this and from nothing else, and
            // saying it every frame would be a modeline that redraws itself
            // for no reason.
            let now = match &s.level {
                Level::List => "normal",
                Level::One(_) => "keyboard-editing",
            };
            let before = match &was {
                Level::List => "normal",
                Level::One(_) => "keyboard-editing",
            };
            // REPORTED WHEN THE NAME CHANGES, and not when the level does:
            // moving from one keyboard to another is the same state, and
            // saying so every time would be a modeline redrawing itself for
            // nothing.
            if now == before {
                return effects;
            }
            let mut out = effects;
            out.push(Effect::StateChanged(now.into()));
            out
        }
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::{Level, State};
    use ronoctua_design::types::{
        Board, Boards, Context, Effect, Heights, Input, Position, WheelEvent, Word,
    };

    fn a_context(lines: u16, columns: u16) -> Context {
        Context {
            lines,
            columns,
            heights: Heights {
                small: 1,
                base: 1,
                normal: 1,
                large: 2,
                huge: 2,
            },
        }
    }

    fn a_board(n: usize) -> Board {
        Board {
            id: format!("0001:000{n}"),
            name: format!("keyboard {n}"),
            built_in: n == 0,
            decided: Vec::new(),
        }
    }

    fn a_word(n: usize) -> Word {
        Word {
            keyd: format!("k{n}"),
            text: format!("key {n}"),
            holds: false,
        }
    }

    /// Nine keyboards and nine keys, which is more than any window here shows:
    /// a list that FITS proves nothing about a window.
    fn a_state() -> State {
        State {
            boards: Some(Boards {
                keys: (0..9).map(a_word).collect(),
                choices: vec![a_word(0), a_word(1)],
                boards: (0..9).map(a_board).collect(),
                trouble: String::new(),
                leader: "k0".to_owned(),
            }),
            ..State::default()
        }
    }

    /// Draws the frame the way `render` does, so the state learns where its
    /// list went — which is what the pointer is then resolved against.
    fn drawn(s: &mut State, lines: u16, columns: u16) {
        let _ = s.draw(&a_context(lines, columns));
    }

    fn click(s: &mut State, line: u16) -> Vec<Effect> {
        s.act(Input::Click(Position { column: 2, line }))
    }

    /// FAILS IF: a row the keyboard can reach cannot be reached with the
    /// pointer.
    ///
    /// This buffer had no `Input::Click` arm at all: every row of it was
    /// keyboard-only, and the rule is absolute — what the keyboard reaches,
    /// the pointer reaches.
    #[test]
    fn every_drawn_row_is_reachable_by_the_pointer() {
        for lines in [10_u16, 16, 30] {
            for scrolled in [0_i16, 2] {
                let mut s = a_state();
                drawn(&mut s, lines, 80);
                // AND FROM A SCROLLED WINDOW TOO. With the list at its top,
                // "the row on screen line N" and "row N of the list" are the
                // same number, so a click that forgot the window would pass —
                // which is the defect, not the test.
                let _ = s.act(Input::Wheel(WheelEvent {
                    at: Position { column: 2, line: 5 },
                    lines: scrolled,
                }));
                drawn(&mut s, lines, 80);
                assert!(s.list_lines > 0, "at {lines} lines the list drew nothing");
                for row in 0..s.list_lines {
                    let line = s.list_top + row;
                    let want = s.offset + usize::from(row);
                    s.cursor = usize::MAX; // somewhere no row is
                    click(&mut s, line);
                    assert_eq!(
                        s.cursor, want,
                        "at {lines} lines scrolled by {scrolled}, a click on screen line \
                     {line} chose row {}",
                        s.cursor
                    );
                }
                // AND A LINE ABOVE THE LIST IS NOT A ROW OF IT.
                if s.list_top > 0 {
                    let above = s.list_top - 1;
                    s.cursor = 3;
                    click(&mut s, above);
                    assert_eq!(s.cursor, 3, "the heading answered as a row");
                }
            }
        }
    }

    /// FAILS IF: the second click on a row does not do what `RET` does.
    #[test]
    fn a_second_click_opens_the_keyboard() {
        let mut s = a_state();
        drawn(&mut s, 20, 80);
        let line = s.list_top + 1;
        click(&mut s, line);
        assert!(matches!(s.level, Level::List), "one click opened a page");
        click(&mut s, line);
        assert!(
            matches!(s.level, Level::One(_)),
            "the second click did not open the keyboard"
        );
    }

    /// FAILS IF: the wheel moves the cursor, or the window cannot reach the
    /// end of a list that does not fit.
    #[test]
    fn the_wheel_moves_the_window_and_not_the_cursor() {
        let mut s = a_state();
        drawn(&mut s, 10, 80);
        let room = usize::from(s.list_lines);
        assert!(room < 9, "the window has to be smaller than the list");
        let before = s.cursor;
        let _ = s.act(Input::Wheel(WheelEvent {
            at: Position { column: 2, line: 5 },
            lines: 3,
        }));
        assert_eq!(s.cursor, before, "the wheel moved the cursor");
        assert_eq!(s.offset, 3, "the wheel did not move the window");
    }
}
