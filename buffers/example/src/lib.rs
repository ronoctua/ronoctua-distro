//! Several keyboards in one app — the REFERENCE for the apps to come.
//!
//! It exists to show one thing working end to end: in Independent mode every
//! keyboard is its own seat, the host says whose keys come next
//! (`Input::KeyboardSeat`), and a buffer that keeps a caret per seat
//! (`ronoctua_design::Carets`) lets two people move two selections and fill
//! two fields AT ONCE, each drawn in their own cursor's colour
//! (`Role::selection_of`). A buffer that ignores the seat treats every
//! keyboard as one — which is what every other buffer does, and is right for
//! them.
//!
//! # What it is made of
//!
//! One list a caret walks: six items, then the form's two fields. Up and down
//! move the caret of the keyboard that pressed them; a character goes into the
//! field under THAT keyboard's caret; a click moves the caret of the cursor
//! that is resting where it clicked — the pointer and the keyboard of one seat
//! are one hand.
//!
//! # What this file does NOT import
//!
//! No Wayland, no token value, no translated text — invariant 11. Every word
//! on the screen goes out as a key.

use std::cell::RefCell;

use ronoctua_design::guest::Wit;
use ronoctua_design::types::{
    Binding, BufferState, Capability, Command, Context, Descriptor, Effect, Guest, Icon, Input,
    Role, Run, Screen, Tone,
};
use ronoctua_design::{Carets, Hovers};

/// The text keys this buffer draws — owed by both languages.
mod key {
    pub const TITLE: &str = "example-title";
    pub const NOTE: &str = "example-note";
    pub const LIST: &str = "example-list";
    pub const FORM: &str = "example-form";
    pub const EMPTY: &str = "example-empty";
    /// The six items and the two fields, in the order the caret walks them.
    pub const ITEMS: [&str; 6] = [
        "example-item-one",
        "example-item-two",
        "example-item-three",
        "example-item-four",
        "example-item-five",
        "example-item-six",
    ];
    pub const FIELDS: [&str; 2] = ["example-field-name", "example-field-note"];
}

/// The keys this buffer binds, under `m`.
const KEYMAP: &[(&str, &str)] = &[("m c", "example-clear")];

/// How many rows the caret walks: the items, then the fields.
const ROWS: usize = key::ITEMS.len() + key::FIELDS.len();

#[derive(Default)]
struct State {
    /// Where every keyboard is — a row, per seat.
    carets: Carets<usize>,
    /// What the form holds: one value per field, whoever typed it.
    values: [String; 2],
    /// Where every pointer rests.
    hover: Hovers,
    /// The screen line each row was drawn on, remembered where it was drawn:
    /// a click is resolved against the drawing, never a second guess.
    lines: Vec<(u16, usize)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

impl State {
    /// The row a screen line is, if it is one.
    fn row_at(&self, line: u16) -> Option<usize> {
        self.lines.iter().find(|(l, _)| *l == line).map(|(_, r)| *r)
    }

    /// The band row `row` is drawn on, and whose: a keyboard's caret first
    /// (the first to type wins a shared row), then a resting pointer.
    fn band_of(&self, row: usize, line: u16) -> Option<Role> {
        let many = self.carets.many();
        if let Some((seat, _)) = self.carets.all().find(|(_, r)| **r == row) {
            return Some(if many {
                Role::selection_of(seat)
            } else {
                Role::Band
            });
        }
        self.hover.first(|(_, l)| l == line).map(Role::hover_of)
    }

    fn draw(&mut self, ctx: &Context) -> Vec<Command> {
        let mut screen = Screen::<Wit>::of(ctx);
        let columns = screen.columns();
        let second = (columns / 3).max(12);
        let mut line = screen.heading(Icon::Keyboard, key::TITLE);
        // A SENTENCE, WRAPPED BY THE HOST: it is the only side that has the
        // translated words, and cut at the width it would lose its end —
        // which is where the point is.
        screen.paragraph(
            line,
            2,
            Run::label(key::NOTE)
                .quiet()
                .within(columns.saturating_sub(1)),
        );
        line += 3;
        self.lines.clear();
        screen.put(line, Run::label(key::LIST).dim());
        line += 1;
        for (n, item) in key::ITEMS.iter().enumerate() {
            if let Some(role) = self.band_of(n, line) {
                screen.ground_in(line, 0, columns, role);
            }
            screen.put(line, Run::label(item).within(columns));
            self.lines.push((line, n));
            line += 1;
        }
        line += 1;
        screen.put(line, Run::label(key::FORM).dim());
        line += 1;
        for (f, field) in key::FIELDS.iter().enumerate() {
            let n = key::ITEMS.len() + f;
            if let Some(role) = self.band_of(n, line) {
                screen.ground_in(line, 0, columns, role);
            }
            screen.put(line, Run::label(field).within(second.saturating_sub(1)));
            let value = &self.values[f];
            let run = if value.is_empty() {
                Run::label(key::EMPTY).dim()
            } else {
                Run::datum(value)
            };
            screen.put(line, run.at(second).within(columns.saturating_sub(second)));
            self.lines.push((line, n));
            line += 1;
        }
        screen.finish_wit()
    }

    fn act(&mut self, e: Input) -> Vec<Effect> {
        match e {
            Input::KeyboardSeat(seat) => {
                self.carets.seat(seat);
                vec![Effect::Redraw]
            }
            Input::KeyboardLeft(seat) => {
                self.carets.left(seat);
                vec![Effect::Redraw]
            }
            Input::Down => {
                let r = self.carets.here();
                *r = (*r + 1).min(ROWS - 1);
                vec![Effect::Redraw]
            }
            Input::Up => {
                let r = self.carets.here();
                *r = r.saturating_sub(1);
                vec![Effect::Redraw]
            }
            // A CHARACTER GOES INTO THE FIELD UNDER THIS KEYBOARD'S CARET, and
            // nowhere when the caret is on an item.
            Input::Character(c) => {
                let row = *self.carets.here();
                match row.checked_sub(key::ITEMS.len()) {
                    Some(f) if f < key::FIELDS.len() => {
                        self.values[f].push(c);
                        vec![Effect::Redraw]
                    }
                    _ => Vec::new(),
                }
            }
            Input::Delete => {
                let row = *self.carets.here();
                match row.checked_sub(key::ITEMS.len()) {
                    Some(f) if f < key::FIELDS.len() => {
                        self.values[f].pop();
                        vec![Effect::Redraw]
                    }
                    _ => Vec::new(),
                }
            }
            // `RET` walks on, as a form's `Tab` does.
            Input::Confirm => {
                let r = self.carets.here();
                *r = (*r + 1) % ROWS;
                vec![Effect::Redraw]
            }
            // A CLICK MOVES THE CARET OF THE HAND THAT CLICKED: the cursor
            // resting on that line is the seat's, and its keyboard's caret goes
            // there. With no hand resting there, the keyboard typing now moves.
            Input::Click(p) => {
                let Some(row) = self.row_at(p.line) else {
                    return Vec::new();
                };
                let seat = self
                    .hover
                    .first(|(_, l)| l == p.line)
                    .unwrap_or_else(|| self.carets.now());
                *self.carets.of(seat) = row;
                vec![Effect::Redraw]
            }
            Input::Hover(p) => {
                if self.hover.set(p.seat, (p.at.column, p.at.line)) {
                    vec![Effect::Redraw]
                } else {
                    Vec::new()
                }
            }
            Input::Unhover(seat) => {
                if self.hover.clear(seat) {
                    vec![Effect::Redraw]
                } else {
                    Vec::new()
                }
            }
            Input::Command(id) if id == "example-clear" => {
                self.values = Default::default();
                vec![Effect::Redraw]
            }
            _ => Vec::new(),
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "example".into(),
            mode: "example".into(),
            keymap: KEYMAP
                .iter()
                .map(|(keys, command)| Binding {
                    keys: (*keys).into(),
                    command: (*command).into(),
                })
                .collect(),
            // ONE STATE: there is nothing here but the list and the form.
            states: vec![BufferState {
                id: "normal".into(),
                letter: 'n',
                tone: Tone::Standard,
            }],
            commands: KEYMAP.iter().map(|(_, c)| (*c).into()).collect(),
            segments: vec![],
            // NO CAPABILITY: whose keys come next is not a privilege, it is
            // what every buffer is told.
            capabilities: Vec::<Capability>::new(),
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

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_design::types::{Heights, Pointed, Position};

    fn context() -> Context {
        Context {
            lines: 30,
            columns: 80,
            heights: Heights {
                small: 1,
                base: 1,
                normal: 1,
                large: 2,
                huge: 2,
            },
        }
    }

    /// FAILS IF: two keyboards move one selection, or type into one field.
    #[test]
    fn two_keyboards_move_two_carets_and_fill_two_fields() {
        let mut s = State::default();
        let _ = s.draw(&context());
        // Keyboard of seat 0 goes to the first field.
        let _ = s.act(Input::KeyboardSeat(0));
        for _ in 0..key::ITEMS.len() {
            let _ = s.act(Input::Down);
        }
        // Keyboard of seat 3 goes to the second.
        let _ = s.act(Input::KeyboardSeat(3));
        for _ in 0..=key::ITEMS.len() {
            let _ = s.act(Input::Down);
        }
        // They type at once, interleaved.
        for (seat, c) in [(0, 'a'), (3, 'x'), (0, 'b'), (3, 'y')] {
            let _ = s.act(Input::KeyboardSeat(seat));
            let _ = s.act(Input::Character(c));
        }
        assert_eq!(s.values[0], "ab");
        assert_eq!(s.values[1], "xy");
        assert!(s.carets.many());
    }

    /// FAILS IF: a click moves some other hand's caret.
    #[test]
    fn a_click_moves_the_caret_of_the_hand_that_clicked() {
        let mut s = State::default();
        let _ = s.draw(&context());
        let _ = s.act(Input::KeyboardSeat(0));
        let _ = s.act(Input::KeyboardSeat(2));
        let (line, row) = s.lines[3];
        let _ = s.act(Input::Hover(Pointed {
            at: Position { column: 2, line },
            seat: 2,
        }));
        let _ = s.act(Input::Click(Position { column: 2, line }));
        let carets: Vec<(u16, usize)> = s.carets.all().map(|(k, r)| (k, *r)).collect();
        assert!(carets.contains(&(2, row)), "seat 2's caret: {carets:?}");
        assert!(
            carets.contains(&(0, 0)),
            "seat 0's did not move: {carets:?}"
        );
    }
}
