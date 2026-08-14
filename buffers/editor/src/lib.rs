//! The file editor — section 7.9's buffer, and the one `LEADER f f` opens.
//!
//! # It has no filesystem, and does not need one
//!
//! The content arrives through [`Input::Paste`] and leaves through
//! [`Effect::Save`]. The HOST reads and writes; this component never learns a
//! path and never opens a file — invariant 11 holds exactly as it does for the
//! clipboard, where the buffer hands text over and the host decides where it
//! goes.
//!
//! Giving a buffer a preopened directory would have been a new capability on the
//! boundary, and it would have bought nothing: the two directions already
//! existed.
//!
//! # What it is NOT
//!
//! It is not Emacs, and it does not try to be. There is an Emacs on this system,
//! in daemon mode, and it is what one reaches for to write code. This exists so
//! that `LEADER f f` stops promising something that does not happen — a shortcut
//! that does nothing is worse than an absent one, and it was that which made
//! every combination look broken.

// The workspace's lints (`unsafe_code`, `missing_docs`) hit the code
// `wit_bindgen::generate!` emits: FFI glue with `export_name`, `unsafe fn` and
// modules with no docs. None of that is ours and none of it is optional — it is
// the target's shape.
#![allow(unsafe_code, missing_docs)]

use std::cell::RefCell;

wit_bindgen::generate!({
    world: "buffer",
    path: "../../crates/ronoctua-protocol/wit",
    generate_all,
});

use exports::ronoctua::buffer::view::Guest;
use ronoctua::buffer::types::{
    Binding, Color, Command, Context, Descriptor, Effect, Input, LineCommand, Scale, Text,
    TextCommand,
};

/// The buffer's whole state.
struct State {
    /// The text, split by line. Splitting once on load and keeping it that way
    /// is what makes drawing a screenful cost a slice and not a scan of the
    /// whole file.
    lines: Vec<String>,
    /// Where the caret is: line, then column.
    line: usize,
    column: usize,
    /// The first VISIBLE line. Scrolling is this number, and nothing else.
    top: usize,
    /// Has it changed since the last save?
    ///
    /// It is what the modeline shows and what makes saving an unchanged buffer
    /// a no-op instead of a pointless write.
    dirty: bool,
    /// The last save's verdict, when there was one.
    ///
    /// `None` = nothing tried yet. The host answers through `Input::Copied`,
    /// which is the SAME way back the clipboard already used — a buffer showing
    /// an optimistic "saved!" would lie exactly when it matters.
    saved: Option<bool>,
}

impl State {
    const fn fresh() -> Self {
        Self {
            lines: Vec::new(),
            line: 0,
            column: 0,
            top: 0,
            dirty: false,
            saved: None,
        }
    }

    /// The current line, or an empty one when the file is.
    fn current(&mut self) -> &mut String {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        &mut self.lines[self.line]
    }

    /// Keeps the caret visible after a move.
    ///
    /// It runs on every movement and not only on scrolling, because the caret
    /// leaving the screen is not a scroll — it is the buffer showing the wrong
    /// place.
    fn follow(&mut self, height: usize) {
        if self.line < self.top {
            self.top = self.line;
        } else if height > 0 && self.line >= self.top + height {
            self.top = self.line + 1 - height;
        }
    }

    /// The whole text, as the host will write it.
    ///
    /// A trailing newline, always: POSIX says a text file's last line ends with
    /// one, and its absence is what makes `git diff` say "\ No newline at end of
    /// file" on a file nobody touched.
    fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }
}

thread_local! {
    /// `RefCell` and not `Cell`: the state is not `Copy`, and a component is
    /// single-threaded by construction.
    static STATE: RefCell<State> = const { RefCell::new(State::fresh()) };
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "editor".into(),
            mode: "editor".into(),
            // The keymap as DATA, and only under `SPC m` — the host refuses at
            // load whoever binds under a global prefix (section 7.9).
            //
            // The FIRST key comes with `Ctrl` — `Ctrl+m`, then `s`. It is the
            // host that decides this and not us (invariant 11), and the reason
            // is on its side: with the first key plain, every `m` typed into
            // the file was swallowed.
            keymap: vec![Binding {
                keys: "m s".into(),
                command: "editor-save".into(),
            }],
            commands: vec!["editor-save".into()],
            segments: vec![],
            // Neither a PTY nor pixels: it is text, and text is the node's own
            // primitive.
            capabilities: vec![],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        // `with_borrow_mut`, because rendering is where the scroll is decided.
        //
        // The caret moves in `event`, which does NOT know the height — the
        // number of lines comes in `Context`, and only here. Following the caret
        // in `event` would mean the buffer guessing its own size, and the guess
        // would be wrong on the first resize.
        STATE.with_borrow_mut(|s| {
            s.follow(usize::from(ctx.lines).saturating_sub(2));
            let mut out = Vec::new();

            // THE STATUS LINE FIRST, and it is one line on purpose: the footer
            // is the system's modeline (invariant 14), and a second full-width
            // bar in here would be a competing one.
            let status = match (s.dirty, s.saved) {
                (_, Some(false)) => "NAO SALVOU".to_owned(),
                (true, _) => format!("{}:{}  *", s.line + 1, s.column + 1),
                (false, _) => format!("{}:{}", s.line + 1, s.column + 1),
            };
            out.push(Command::Text(TextCommand {
                line: 0,
                column: 0,
                content: Text::Data(status),
                color: if s.saved == Some(false) {
                    Color::StateError
                } else {
                    Color::TextSecondary
                },
                scale: Scale::Normal,
                max_width: ctx.columns,
            }));
            out.push(Command::Rule(LineCommand {
                line: 1,
                color: Color::RuleDefault,
            }));

            // The body. Two lines are the status and the rule; what is left is
            // the window into the file.
            let height = usize::from(ctx.lines).saturating_sub(2);
            for (n, content) in s.lines.iter().skip(s.top).take(height).enumerate() {
                out.push(Command::Text(TextCommand {
                    line: u16::try_from(n + 2).unwrap_or(u16::MAX),
                    column: 0,
                    // DATA, never a key: a file's content has no translation,
                    // and running it through the text keys would be the host
                    // trying to translate someone's source code.
                    content: Text::Data(content.clone()),
                    color: Color::TextPrimary,
                    scale: Scale::Normal,
                    max_width: ctx.columns,
                }));
            }
            out
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        STATE.with_borrow_mut(|s| match e {
            // THE CONTENT ARRIVES HERE. It is the same door the clipboard uses,
            // and that is the whole reason this buffer needs no filesystem.
            Input::Paste(text) => {
                s.lines = text.lines().map(str::to_owned).collect();
                if s.lines.is_empty() {
                    s.lines.push(String::new());
                }
                s.line = 0;
                s.column = 0;
                s.top = 0;
                s.dirty = false;
                s.saved = None;
                vec![Effect::Redraw]
            }

            // The host's verdict on the last save. See `State::saved`.
            Input::Copied(ok) => {
                s.saved = Some(ok);
                if ok {
                    s.dirty = false;
                }
                vec![Effect::Redraw]
            }

            // `SPC m s`, resolved by the host from the keymap above.
            Input::Command(id) if id == "editor-save" => {
                if s.dirty {
                    vec![Effect::Save(s.text())]
                } else {
                    // Saving an unchanged buffer would rewrite the file and
                    // move its mtime, which is what a build system watches.
                    vec![]
                }
            }
            Input::Command(_) => vec![],

            Input::Character(c) => {
                let column = s.column;
                let at = s
                    .current()
                    .char_indices()
                    .nth(column)
                    .map_or_else(|| s.lines[s.line].len(), |(i, _)| i);
                s.lines[s.line].insert(at, c);
                s.column += 1;
                s.dirty = true;
                s.saved = None;
                vec![Effect::Redraw]
            }

            Input::Confirm => {
                let column = s.column;
                let at = s
                    .current()
                    .char_indices()
                    .nth(column)
                    .map_or_else(|| s.lines[s.line].len(), |(i, _)| i);
                let rest = s.lines[s.line].split_off(at);
                s.lines.insert(s.line + 1, rest);
                s.line += 1;
                s.column = 0;
                s.dirty = true;
                s.saved = None;
                vec![Effect::Redraw]
            }

            Input::Delete => {
                if s.column > 0 {
                    let column = s.column;
                    let at = s
                        .current()
                        .char_indices()
                        .nth(column - 1)
                        .map_or(0, |(i, _)| i);
                    s.lines[s.line].remove(at);
                    s.column -= 1;
                } else if s.line > 0 {
                    // Joining with the line above, which is what Backspace at
                    // column zero means everywhere.
                    let tail = s.lines.remove(s.line);
                    s.line -= 1;
                    s.column = s.lines[s.line].chars().count();
                    s.lines[s.line].push_str(&tail);
                } else {
                    return vec![];
                }
                s.dirty = true;
                s.saved = None;
                vec![Effect::Redraw]
            }

            Input::Up => {
                if s.line > 0 {
                    s.line -= 1;
                    s.column = s.column.min(s.lines[s.line].chars().count());
                }
                vec![Effect::Redraw]
            }
            Input::Down => {
                if s.line + 1 < s.lines.len() {
                    s.line += 1;
                    s.column = s.column.min(s.lines[s.line].chars().count());
                }
                vec![Effect::Redraw]
            }
            Input::Left => {
                if s.column > 0 {
                    s.column -= 1;
                }
                vec![Effect::Redraw]
            }
            Input::Right => {
                let width = s.lines.get(s.line).map_or(0, |l| l.chars().count());
                if s.column < width {
                    s.column += 1;
                }
                vec![Effect::Redraw]
            }

            // `Cancel` does NOT close, and does not discard.
            //
            // `Effect::Close` exists, but tying it to the key one presses by
            // reflex to leave anything at all would DESTROY unsaved work. Here
            // it only clears the last save's verdict, which is transient state.
            Input::Cancel => {
                s.saved = None;
                vec![Effect::Redraw]
            }

            _ => vec![],
        })
    }
}

export!(Component);
