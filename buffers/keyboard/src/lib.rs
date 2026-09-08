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
    Icon, Input, Remap, Run, Screen, Tone,
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
        STATE.with_borrow(|s| {
            let mut screen = Screen::<Wit>::of(&ctx);
            let columns = screen.columns();
            // WHERE THE SECOND COLUMN STARTS. A third of the width, floored so
            // a narrow window still leaves the name something to be read in.
            let second = (columns / 3).max(12);

            let heading = match s.level {
                Level::List => key::TITLE,
                Level::One(_) => key::ONE,
            };
            let mut line = screen.heading(Icon::Keyboard, heading);

            let Some(b) = s.boards.as_ref() else {
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
                    for (n, board) in b.boards.iter().enumerate() {
                        let here = n == s.cursor;
                        if here {
                            screen.band(line);
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
                        let last = columns.saturating_sub(second * 2);
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
                    // AND WHAT THIS SCREEN CANNOT DO, said rather than left to
                    // be discovered. The layout and the repeat rate are the
                    // compositor's, one of each for every keyboard there is —
                    // a page that stayed quiet about it would read as a page
                    // that had simply forgotten them.
                    line += 1;
                    screen.put(line, Run::label(key::GLOBAL).dim());
                }
                Level::One(_) => {
                    let Some(board) = s.board() else {
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
                    for (n, word) in b.keys.iter().enumerate() {
                        let here = n == s.cursor;
                        if here {
                            screen.band(line);
                        }
                        let label = Run::label(&word.text).within(second.saturating_sub(1));
                        screen.put(line, if here { label.accent() } else { label });
                        let now = s.becomes(board, &word.keyd);
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
                                    .within(columns.saturating_sub(second * 2))
                                    .dim(),
                            );
                        }
                        line += 1;
                    }
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
                        let now = s.becomes(board, &word.keyd);
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
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        STATE.with_borrow_mut(|s| {
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
                    vec![Effect::Redraw]
                }
                Input::Up => {
                    s.cursor = s.cursor.saturating_sub(1);
                    vec![Effect::Redraw]
                }
                // `RET` OPENS A KEYBOARD, and on a key it is one step round the
                // ring — the same thing the right arrow does, because a row
                // with an order has one obvious meaning for `RET`.
                Input::Confirm | Input::Right => match s.level {
                    Level::List if s.rows() > 0 => {
                        let Some(id) = s.board().map(|k| k.id.clone()) else {
                            return vec![];
                        };
                        s.level = Level::One(id);
                        s.cursor = 0;
                        vec![Effect::Redraw]
                    }
                    Level::List => vec![],
                    Level::One(_) => s.turn(true).map(Effect::Remap).into_iter().collect(),
                },
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
        })
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);
