//! THE CALCULATOR'S KEYPAD: which keys, where, and what each one types.
//!
//! # One answer for the drawing and the click
//!
//! A key is a CARD (`space.calc.*`): a ground and a legend, placed on the
//! text's grid and inset by `keyAir` pixels so two neighbours read as two.
//! [`Card::rect`] is the ground's rectangle, and it is the one thing the
//! window paints AND the one thing a press is tested against ([`Keypad::at`])
//! — a control is grabbed where it is drawn (`AGENTS.md`), so the air between
//! two cards is nobody's, and a finger on it presses nothing.
//!
//! # The two phases of the size
//!
//! A key PREFERS `keyCells` × `keyLines`, grows with a roomy window up to
//! `keyMaxCells` × `keyMaxLines`, and shrinks with a short one — every key
//! alike. It never shrinks under its LEGEND: the least a key is is the widest
//! legend of its keypad in ANY declared language plus a cell of air on each
//! side ([`least_cells`]), because a width chosen by the English words cuts
//! `arcsen` in Portuguese, and three keys reading `ar…` are three keys nobody
//! can tell apart. So the scientific keypad, which cannot shrink its keys,
//! changes SHAPE instead ([`arrange`]): its two blocks side by side, then one
//! over the other, and — narrower than that — only the arithmetic, with a
//! sentence saying why (the functions are still typed by name). Only the
//! arithmetic ever shrinks past its legends, in a window too narrow for
//! anything else: "nothing leaks" is the last phase.
//!
//! In the footer a key is one line, at least as wide as its legend and a
//! cell ([`least_cells`] with one cell of air), drawn as a field of its row
//! (`crate::calc`), because the footer's region is lines of text.
//!
//! # What a key types
//!
//! The engine's own notation (`ronoctua_calc`): `×`, `÷`, `−`, a function's
//! name and its parenthesis, the decimal mark of the language. The LEGEND is
//! a text key (`calc-key-<id>`) — the sine is `sen` in Portuguese — and what a
//! function's key types is its legend, because the engine understands every
//! language's words at once (`assets/units.nix`, `functions`).

use ronoctua_calc::units::{Constant, Function};
use ronoctua_tokens as tokens;

/// The four operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    /// `+`.
    Add,
    /// `−`.
    Subtract,
    /// `×`.
    Multiply,
    /// `÷`.
    Divide,
}

/// One key of the keypad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// A digit, 0 to 9.
    Digit(u8),
    /// The language's decimal mark.
    Mark,
    /// An operation.
    Op(Op),
    /// `(`.
    Open,
    /// `)`.
    Close,
    /// `%`.
    Percent,
    /// `^` — the power of what follows.
    Power,
    /// `²`.
    Square,
    /// `^-1`, the reciprocal.
    Reciprocal,
    /// `!`.
    Factorial,
    /// `°`, an angle in degrees (`sin(30°)`).
    Degree,
    /// A function, and its parenthesis.
    Function(Function),
    /// A constant.
    Constant(Constant),
    /// Takes the last character back (`DEL`).
    Back,
    /// Empties the expression (`C-DEL`).
    Clear,
    /// Keeps the answer in the history (`RET`).
    Equals,
}

/// What a key does to the expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    /// Types this at the end.
    Type(String),
    /// Takes the last character back.
    Back,
    /// Empties it.
    Clear,
    /// Keeps the answer.
    Equals,
}

/// How a key's ground reads: what types a number, what does something to
/// one, and the one key that ends the sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A digit or the mark — the keys the eye hunts for.
    Number,
    /// An operation, a function, a correction.
    Action,
    /// `=`.
    Answer,
}

impl Button {
    /// Its key's tail — `calc-key-<id>` is its legend, `about-calc-key-<id>`
    /// is not needed: a key's legend is what it types.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Digit(_) => "digit",
            Self::Mark => "mark",
            Self::Op(Op::Add) => "add",
            Self::Op(Op::Subtract) => "subtract",
            Self::Op(Op::Multiply) => "multiply",
            Self::Op(Op::Divide) => "divide",
            Self::Open => "open",
            Self::Close => "close",
            Self::Percent => "percent",
            Self::Power => "power",
            Self::Square => "square",
            Self::Reciprocal => "reciprocal",
            Self::Factorial => "factorial",
            Self::Degree => "degree",
            Self::Function(f) => function_id(f),
            Self::Constant(Constant::Pi) => "pi",
            Self::Constant(Constant::E) => "e",
            Self::Constant(Constant::Tau) => "tau",
            Self::Back => "back",
            Self::Clear => "clear",
            Self::Equals => "equals",
        }
    }

    /// What it reads: a digit, the mark, the back key's glyph, or its key.
    #[must_use]
    pub fn legend(self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Digit(d) => char::from(b'0' + d.min(9)).to_string(),
            Self::Mark => texts.decimal_mark().to_string(),
            Self::Back => tokens::icon::CALC_BACKSPACE.to_string(),
            other => texts.t(&format!("calc-key-{}", other.id())),
        }
    }

    /// What pressing it does.
    #[must_use]
    pub fn deed(self, texts: &ronoctua_i18n::Texts) -> Deed {
        let typed = |s: &str| Deed::Type(s.to_owned());
        match self {
            Self::Digit(_) | Self::Mark => Deed::Type(self.legend(texts)),
            // THE ENGINE'S OWN SIGNS, the same in every language: the legend
            // may be dressed (`x²`), what is typed is what `ronoctua_calc`
            // reads.
            Self::Op(Op::Add) => typed("+"),
            Self::Op(Op::Subtract) => typed("−"),
            Self::Op(Op::Multiply) => typed("×"),
            Self::Op(Op::Divide) => typed("÷"),
            Self::Open => typed("("),
            Self::Close => typed(")"),
            Self::Percent => typed("%"),
            Self::Power => typed("^"),
            Self::Square => typed("²"),
            Self::Reciprocal => typed("^-1"),
            Self::Factorial => typed("!"),
            Self::Degree => typed("°"),
            // A FUNCTION TYPES ITS NAME AND ITS PARENTHESIS: the engine reads
            // `sqrt 9/4` as the root of 9, over 4, and a key that typed the
            // bare name would leave that to be guessed.
            Self::Function(_) => Deed::Type(format!("{}(", self.legend(texts))),
            Self::Constant(_) => Deed::Type(self.legend(texts)),
            Self::Back => Deed::Back,
            Self::Clear => Deed::Clear,
            Self::Equals => Deed::Equals,
        }
    }

    /// How its ground reads.
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Digit(_) | Self::Mark => Tone::Number,
            Self::Equals => Tone::Answer,
            _ => Tone::Action,
        }
    }
}

/// A function's key tail.
const fn function_id(f: Function) -> &'static str {
    match f {
        Function::Sqrt => "sqrt",
        Function::Cbrt => "cbrt",
        Function::Abs => "abs",
        Function::Floor => "floor",
        Function::Ceil => "ceil",
        Function::Round => "round",
        Function::Exp => "exp",
        Function::Ln => "ln",
        Function::Log => "log",
        Function::Log2 => "log2",
        Function::Sin => "sin",
        Function::Cos => "cos",
        Function::Tan => "tan",
        Function::Asin => "asin",
        Function::Acos => "acos",
        Function::Atan => "atan",
        Function::Sinh => "sinh",
        Function::Cosh => "cosh",
        Function::Tanh => "tanh",
    }
}

/// A key's place on the keypad's grid: its column and row, and how many
/// columns it spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// The key.
    pub button: Button,
    /// Its column, from the keypad's left.
    pub column: u16,
    /// Its row, from the keypad's top.
    pub row: u16,
    /// How many columns wide.
    pub span: u16,
}

/// The ARITHMETIC half, the whole keypad of Basic: four columns, five rows,
/// the digits where a telephone does not put them and every calculator does.
const BASIC: [[Option<Button>; 4]; 5] = {
    use Button as B;
    [
        [
            Some(B::Clear),
            Some(B::Back),
            Some(B::Percent),
            Some(B::Op(Op::Divide)),
        ],
        [
            Some(B::Digit(7)),
            Some(B::Digit(8)),
            Some(B::Digit(9)),
            Some(B::Op(Op::Multiply)),
        ],
        [
            Some(B::Digit(4)),
            Some(B::Digit(5)),
            Some(B::Digit(6)),
            Some(B::Op(Op::Subtract)),
        ],
        [
            Some(B::Digit(1)),
            Some(B::Digit(2)),
            Some(B::Digit(3)),
            Some(B::Op(Op::Add)),
        ],
        // `0` TAKES TWO COLUMNS (the `None` beside it is its second half):
        // it is the most pressed digit, and the widest key is the easiest to
        // hit.
        [Some(B::Digit(0)), None, Some(B::Mark), Some(B::Equals)],
    ]
};

/// The SCIENTIFIC half, four more columns on the left of the arithmetic.
const SCIENTIFIC: [[Button; 4]; 5] = {
    use Button as B;
    [
        [
            B::Function(Function::Sin),
            B::Function(Function::Cos),
            B::Function(Function::Tan),
            B::Constant(Constant::Pi),
        ],
        [
            B::Function(Function::Asin),
            B::Function(Function::Acos),
            B::Function(Function::Atan),
            B::Constant(Constant::E),
        ],
        [
            B::Function(Function::Ln),
            B::Function(Function::Log),
            B::Function(Function::Sqrt),
            B::Function(Function::Cbrt),
        ],
        [B::Square, B::Power, B::Reciprocal, B::Factorial],
        [B::Open, B::Close, B::Degree, B::Function(Function::Exp)],
    ]
};

/// How many rows a block of keys has — the arithmetic's, and the functions'.
pub const ROWS: u16 = 5;

/// How many columns a block of keys has.
pub const BLOCK_COLUMNS: u16 = 4;

/// How a keypad is laid out in the room it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Arrangement {
    /// The arithmetic, Basic's keypad.
    #[default]
    Basic,
    /// The functions on the left of the arithmetic: Scientific in a room
    /// with space for eight keys a row.
    Beside,
    /// The functions OVER the arithmetic: Scientific in a room with space
    /// for four.
    Stacked,
    /// Scientific in a room narrower than four keys of its legends: the
    /// arithmetic alone, and a sentence saying why the functions' keys are
    /// not there — they are typed by name.
    Narrow,
}

impl Arrangement {
    /// How many keys a row has.
    #[must_use]
    pub const fn columns(self) -> u16 {
        match self {
            Self::Beside => 2 * BLOCK_COLUMNS,
            Self::Basic | Self::Stacked | Self::Narrow => BLOCK_COLUMNS,
        }
    }

    /// How many rows of keys.
    #[must_use]
    pub const fn rows(self) -> u16 {
        match self {
            Self::Stacked => 2 * ROWS,
            Self::Basic | Self::Beside | Self::Narrow => ROWS,
        }
    }

    /// Whether the functions' block is on the keypad.
    #[must_use]
    pub const fn has_functions(self) -> bool {
        matches!(self, Self::Beside | Self::Stacked)
    }
}

/// Every key of a keypad, placed: the arithmetic alone, or with the
/// functions' block on its left or over it.
#[must_use]
pub fn plan(arrangement: Arrangement) -> Vec<Place> {
    let (shift, drop) = match arrangement {
        Arrangement::Beside => (BLOCK_COLUMNS, 0),
        Arrangement::Stacked => (0, ROWS),
        Arrangement::Basic | Arrangement::Narrow => (0, 0),
    };
    let mut out = Vec::new();
    if arrangement.has_functions() {
        for (row, keys) in (0u16..).zip(SCIENTIFIC.iter()) {
            for (column, b) in (0u16..).zip(keys.iter()) {
                out.push(Place {
                    button: *b,
                    column,
                    row,
                    span: 1,
                });
            }
        }
    }
    for (row, keys) in (0u16..).zip(BASIC.iter()) {
        for (column, b) in (0u16..).zip(keys.iter()) {
            let Some(button) = *b else { continue };
            // A KEY SPANS the `None`s after it.
            let span = 1 + u16::try_from(
                keys[usize::from(column) + 1..]
                    .iter()
                    .take_while(|k| k.is_none())
                    .count(),
            )
            .unwrap_or(0);
            out.push(Place {
                button,
                column: shift + column,
                row: drop + row,
                span,
            });
        }
    }
    out
}

/// The widest legend of a keypad's keys, in cells, over EVERY declared
/// language — the column a key reserves is the longest translation's
/// (`AGENTS.md`), worked out once per process.
#[must_use]
pub fn widest_legend(functions: bool) -> u16 {
    static WIDEST: std::sync::LazyLock<(u16, u16)> = std::sync::LazyLock::new(|| {
        let mut widest = (1, 1);
        for language in ronoctua_i18n::LANGUAGES {
            let Some(texts) = ronoctua_i18n::Texts::fresh(language) else {
                continue;
            };
            for p in plan(Arrangement::Beside) {
                let w = ronoctua_render::layout::width(&p.button.legend(&texts));
                if SCIENTIFIC.iter().flatten().any(|b| *b == p.button) {
                    widest.1 = widest.1.max(w);
                } else {
                    widest.0 = widest.0.max(w);
                }
            }
        }
        (widest.0, widest.0.max(widest.1))
    });
    if functions { WIDEST.1 } else { WIDEST.0 }
}

/// The least a key of a keypad is: its widest legend and `pad` cells — two
/// in a window, a cell of air on each side of the legend inside a card whose
/// ground is inset from its cells; one in the footer, where a key is a field
/// and the next begins a cell after its legend.
#[must_use]
pub fn least_cells(functions: bool, pad: u16) -> u16 {
    widest_legend(functions) + pad
}

/// How a mode's keypad is laid out in `room` cells, when every key is to be
/// at least `least` wide — the scientific keypad's three shapes, widest
/// first.
#[must_use]
pub const fn arrange(scientific: bool, room: u16, least: u16) -> Arrangement {
    if !scientific {
        Arrangement::Basic
    } else if room >= 2 * BLOCK_COLUMNS * least {
        Arrangement::Beside
    } else if room >= BLOCK_COLUMNS * least {
        Arrangement::Stacked
    } else {
        Arrangement::Narrow
    }
}

/// The least a key of the window's keypad is: `keyMinCells`, or its legends
/// and air if they are wider.
#[must_use]
pub fn window_least(functions: bool) -> u16 {
    tokens::space::CALC_KEYMINCELLS.max(least_cells(functions, 2))
}

/// The least a key of the footer's keypad is: its legends and a cell.
#[must_use]
pub fn footer_least(functions: bool) -> u16 {
    least_cells(functions, 1)
}

/// The arrangement a mode takes in the window, in `room` cells.
#[must_use]
pub fn window_arrangement(scientific: bool, room: u16) -> Arrangement {
    arrange(scientific, room, window_least(true))
}

/// A key laid out on the window's grid: the cells and lines it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    /// The key.
    pub button: Button,
    /// Its first column, from the surface's left edge.
    pub column: u16,
    /// Its first line, from the surface's top.
    pub line: u16,
    /// How many cells wide.
    pub cells: u16,
    /// How many lines tall.
    pub lines: u16,
}

/// A rectangle in logical pixels: `(x, y, width, height)`.
pub type Rect = crate::chrome::Rect;

impl Card {
    /// Its GROUND, in pixels — what is painted and what a press is tested
    /// against, both: the cells it takes, less `keyAir` on every side.
    #[must_use]
    pub fn rect(&self, frame: crate::chrome::Frame) -> Rect {
        let air = f32::from(tokens::space::CALC_KEYAIR);
        let (x, y) = (frame.x_of(self.column), frame.y_of(self.line));
        (
            x + air,
            y + air,
            f32::from(self.cells)
                .mul_add(frame.cell(), -2.0 * air)
                .max(0.0),
            f32::from(self.lines)
                .mul_add(frame.line(), -2.0 * air)
                .max(0.0),
        )
    }

    /// Whether the point `(x, y)` is on its ground.
    #[must_use]
    pub fn holds(&self, frame: crate::chrome::Frame, x: f32, y: f32) -> bool {
        let (left, top, w, h) = self.rect(frame);
        x >= left && x < left + w && y >= top && y < top + h
    }
}

/// A keypad laid out: its cards, and the room it took.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Keypad {
    /// Every key.
    pub cards: Vec<Card>,
    /// How many cells wide it is.
    pub cells: u16,
    /// How many lines tall.
    pub lines: u16,
    /// How its keys are laid out.
    pub arrangement: Arrangement,
}

impl Keypad {
    /// The key whose ground holds the point — `None` on the air between two.
    #[must_use]
    pub fn at(&self, frame: crate::chrome::Frame, x: f32, y: f32) -> Option<Button> {
        self.cards
            .iter()
            .find(|c| c.holds(frame, x, y))
            .map(|c| c.button)
    }
}

/// How wide a key is when the keypad has `room` cells for `columns` keys:
/// its share of the room, up to `keyMaxCells` — and never past the room, so
/// under `keyMinCells` only where nothing else would fit (the last phase).
#[must_use]
pub fn key_cells(columns: u16, room: u16) -> u16 {
    (room / columns.max(1)).clamp(1, tokens::space::CALC_KEYMAXCELLS)
}

/// How tall a key is when the keypad has `room` lines for `rows` rows: its
/// share, up to `keyMaxLines`, never under one.
#[must_use]
pub fn key_lines(rows: u16, room: u16) -> u16 {
    (room / rows.max(1)).clamp(1, tokens::space::CALC_KEYMAXLINES)
}

/// The keypad of `arrangement` at `(left, top)` — surface columns and lines —
/// in a room `room_cells` wide and `room_lines` tall.
#[must_use]
pub fn keypad(
    arrangement: Arrangement,
    left: u16,
    top: u16,
    room_cells: u16,
    room_lines: u16,
) -> Keypad {
    let columns = arrangement.columns();
    let rows = arrangement.rows();
    let w = key_cells(columns, room_cells);
    let h = key_lines(rows, room_lines);
    let cards = plan(arrangement)
        .into_iter()
        .map(|p| Card {
            button: p.button,
            column: left + p.column * w,
            line: top + p.row * h,
            cells: p.span * w,
            lines: h,
        })
        .collect();
    Keypad {
        cards,
        cells: columns * w,
        lines: rows * h,
        arrangement,
    }
}

/// A card's legend as it is drawn: cut to the cells its ground leaves with
/// a cell of air on each side, and the surface column it begins at — in the
/// middle of the card. Painted by the window, and asked by the test that the
/// legends drawn are inside their grounds and told apart.
#[must_use]
pub fn legend_of(card: &Card, texts: &ronoctua_i18n::Texts) -> (String, u16) {
    let legend = ronoctua_render::layout::elide_end(
        &card.button.legend(texts),
        card.cells.saturating_sub(2).max(1),
    );
    let wide = ronoctua_render::layout::width(&legend);
    (legend, card.column + card.cells.saturating_sub(wide) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The functions the scientific keypad offers, in its order.
    const FUNCTIONS: [Function; 11] = [
        Function::Sin,
        Function::Cos,
        Function::Tan,
        Function::Asin,
        Function::Acos,
        Function::Atan,
        Function::Ln,
        Function::Log,
        Function::Sqrt,
        Function::Cbrt,
        Function::Exp,
    ];

    fn frame() -> crate::chrome::Frame {
        crate::chrome::Frame::of(1200.0, 900.0, 0)
    }

    /// FAILS IF: the cards drawn and the cards pressed are two answers — a
    /// press in the middle of every card finds that card, and a press on the
    /// air between two finds none.
    #[test]
    fn a_key_is_pressed_where_it_is_drawn() {
        let f = frame();
        for arrangement in [
            Arrangement::Basic,
            Arrangement::Beside,
            Arrangement::Stacked,
            Arrangement::Narrow,
        ] {
            let pad = keypad(arrangement, 2, 4, 200, 40);
            for card in &pad.cards {
                let (x, y, w, h) = card.rect(f);
                assert!(w > 0.0 && h > 0.0, "{card:?} has no ground");
                assert_eq!(pad.at(f, x + w / 2.0, y + h / 2.0), Some(card.button));
                // THE AIR IS NOBODY'S: just outside the ground's left edge.
                if tokens::space::CALC_KEYAIR > 0 {
                    assert_eq!(pad.at(f, x - 0.5, y + h / 2.0), None, "{card:?}");
                }
            }
        }
    }

    /// FAILS IF: two cards overlap, or the keypad leaves the room it was
    /// given — at every width down to a window a third of a small screen
    /// wide, where the scientific keypad changes shape instead of running off
    /// the edge.
    #[test]
    fn the_keys_tile_and_nothing_leaks() {
        for scientific in [false, true] {
            for room in (16u16..=140).step_by(3) {
                let arrangement = window_arrangement(scientific, room);
                let pad = keypad(arrangement, 0, 0, room, 30);
                assert!(
                    pad.cells <= room,
                    "{arrangement:?} took {} cells of {room}",
                    pad.cells
                );
                assert!(pad.lines <= 30, "{arrangement:?} took {} lines", pad.lines);
                for card in &pad.cards {
                    assert!(card.column + card.cells <= room, "{card:?} leaks at {room}");
                }
                for a in &pad.cards {
                    for b in &pad.cards {
                        if a == b {
                            continue;
                        }
                        let apart = a.column + a.cells <= b.column
                            || b.column + b.cells <= a.column
                            || a.line + a.lines <= b.line
                            || b.line + b.lines <= a.line;
                        assert!(apart, "{a:?} and {b:?} overlap at {room}");
                    }
                }
                // EVERY MODE KEEPS ITS `=`: the arithmetic is on every shape.
                assert!(
                    pad.cards.iter().any(|c| c.button == Button::Equals),
                    "no `=` at {room}"
                );
            }
        }
        // THE THREE SHAPES, widest first.
        let least = window_least(true);
        assert_eq!(window_arrangement(true, 8 * least), Arrangement::Beside);
        assert_eq!(
            window_arrangement(true, 8 * least - 1),
            Arrangement::Stacked
        );
        assert_eq!(window_arrangement(true, 4 * least - 1), Arrangement::Narrow);
        // The preference, and more, when there is room for it.
        let roomy = keypad(Arrangement::Basic, 0, 0, 400, 100);
        assert_eq!(roomy.cards[0].cells, tokens::space::CALC_KEYMAXCELLS);
        assert_eq!(roomy.cards[0].lines, tokens::space::CALC_KEYMAXLINES);
    }

    /// FAILS IF: a legend is DRAWN outside its card's ground, or two keys of
    /// a keypad read the same once cut — in every language, at every width
    /// a keypad keeps its legends at.
    #[test]
    fn the_drawn_legends_are_inside_their_grounds_and_told_apart() {
        let f = frame();
        for lang in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(lang).unwrap();
            for scientific in [false, true] {
                for room in (4 * window_least(false)..=140).step_by(2) {
                    let arrangement = window_arrangement(scientific, room);
                    let pad = keypad(arrangement, 1, 0, room, 30);
                    let mut seen = std::collections::BTreeSet::new();
                    for card in &pad.cards {
                        let (legend, column) = legend_of(card, &texts);
                        let (x, _, w, _) = card.rect(f);
                        let from = f.x_of(column);
                        let to = f.x_of(column + ronoctua_render::layout::width(&legend));
                        assert!(
                            from >= x && to <= x + w,
                            "{lang} {room}: `{legend}` of {card:?} is drawn at {from}..{to}, \
                             its ground is {x}..{}",
                            x + w
                        );
                        assert!(
                            seen.insert(legend.clone()),
                            "{lang} {arrangement:?} {room}: `{legend}` twice"
                        );
                    }
                }
            }
        }
    }

    /// FAILS IF: a key is missing from a keypad, or one is there twice.
    #[test]
    fn every_key_is_on_its_keypad_once() {
        let basic = plan(Arrangement::Basic);
        assert_eq!(basic.len(), 19, "twenty places, the zero taking two");
        for d in 0..=9 {
            assert_eq!(
                basic
                    .iter()
                    .filter(|p| p.button == Button::Digit(d))
                    .count(),
                1
            );
        }
        let zero = basic.iter().find(|p| p.button == Button::Digit(0)).unwrap();
        assert_eq!(zero.span, 2);
        assert_eq!(plan(Arrangement::Narrow), basic);
        for arrangement in [Arrangement::Beside, Arrangement::Stacked] {
            let scientific = plan(arrangement);
            assert_eq!(scientific.len(), 39);
            for f in FUNCTIONS {
                assert!(scientific.iter().any(|p| p.button == Button::Function(f)));
            }
            let mut seen = std::collections::BTreeSet::new();
            for p in &scientific {
                assert!(seen.insert((p.row, p.column)), "two keys on {p:?}");
                assert!(p.column + p.span <= arrangement.columns() && p.row < arrangement.rows());
            }
        }
    }

    /// FAILS IF: what a key types is not what the engine reads — every key's
    /// deed, in both languages, parses and answers inside a sum.
    #[test]
    fn every_key_types_what_the_engine_reads() {
        for lang in ["pt-BR", "en-GB"] {
            let texts = ronoctua_i18n::Texts::fresh(lang).unwrap();
            let mark = texts.decimal_mark();
            let env = ronoctua_calc::Env::at(0, ronoctua_files::when::Zone::utc());
            for p in plan(Arrangement::Beside) {
                let Deed::Type(typed) = p.button.deed(&texts) else {
                    continue;
                };
                // Each key in a sum it belongs in: a function of a number, a
                // constant times one, an operation between two.
                let sum = match p.button {
                    Button::Function(_) => format!("{typed}1)"),
                    Button::Open => format!("{typed}1)"),
                    Button::Close => format!("(1{typed}"),
                    Button::Digit(_) | Button::Constant(_) => typed.clone(),
                    Button::Mark => format!("1{typed}5"),
                    Button::Degree => format!("sin(30{typed})"),
                    _ => format!("2{typed}2"),
                };
                let sum = if matches!(
                    p.button,
                    Button::Square | Button::Reciprocal | Button::Factorial | Button::Percent
                ) {
                    format!("3{typed}")
                } else {
                    sum
                };
                let answer =
                    ronoctua_calc::parse(&sum, mark).and_then(|e| ronoctua_calc::eval(&e, &env));
                assert!(answer.is_ok(), "{lang}: `{sum}` ({p:?}) → {answer:?}");
            }
        }
    }

    /// FAILS IF: a legend is a key the language does not have, or two keys
    /// read the same.
    #[test]
    fn every_legend_is_a_word_of_every_language() {
        for lang in ["pt-BR", "en-GB"] {
            let texts = ronoctua_i18n::Texts::fresh(lang).unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for p in plan(Arrangement::Beside) {
                let legend = p.button.legend(&texts);
                assert!(
                    !legend.is_empty() && !legend.starts_with('['),
                    "{lang}: {p:?}"
                );
                assert!(seen.insert(legend.clone()), "{lang}: `{legend}` twice");
            }
        }
    }
}
