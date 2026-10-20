//! THE CALCULATOR (`LEADER =`): one view, drawn by two doors.
//!
//! # One view, two doors
//!
//! `LEADER = =` opens it in the footer's region and `LEADER = w` in a window
//! of its own (`crate::calc_window`), and both hold THIS view: the same modes,
//! the same keys ([`press_of_key`]), the same lines ([`View::top_lines`],
//! [`View::history_lines`], [`View::convert_lines`]) and the same answer to a
//! click on any of them ([`View::clicked`]). What differs is the keypad's
//! shape: in a window a key is a card a finger presses
//! (`crate::calc_keypad`), in the footer — which is lines of text — a key is
//! one line of its row ([`View::footer_lines`]). Every key is also a key of
//! the keyboard, so a door with no pointer lacks nothing.
//!
//! # Three modes
//!
//! Basic is the arithmetic; Scientific adds the functions, the constants and
//! the powers; Convert is a ring of categories, a unit to convert from and one
//! to convert to (`crate::calc_convert`). The expression line is the same in
//! the three — in Convert it is the amount.
//!
//! # Exact, and said when it is not
//!
//! The engine (`ronoctua_calc`) is exact through the four operations and
//! integer powers; an answer a root or a sine touched is drawn with `≈`
//! (`calc-equals-approx`). A sum that has no answer says why, in the words of
//! `calc-why-<id>`, on the line where the answer would be — once `RET` asked
//! for it; while a sum is being typed, half of it is not yet a sum.
//!
//! # The history
//!
//! Every answer kept is a line of `calc.json` (`crate::calc::FILE`, through
//! `ronoctua-store`), the newest `calcHistoryEntries` of them
//! (`assets/cache.nix`). Both doors write it, so each MERGES what it keeps
//! into what the file says (by the moment it was kept), and the window
//! watches it: a sum kept in the footer is in the window's history at once.
//! A click on a line picks it; a second click — or `RET` — puts its sum back
//! on the expression line; `C-c` copies its answer.

use crate::calc_convert::{Converter, RING, Side};
use crate::calc_keypad::{Button, Deed};
use crate::fields::{Field, Hit, Ink};
use ronoctua_calc::{Base, Number, Refusal, Value};
use ronoctua_keymap::{Key, Sym};
use ronoctua_tokens as tokens;

/// The file the history is kept in, under the person's configuration.
pub const FILE: &str = "calc.json";

/// The three modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The arithmetic.
    #[default]
    Basic,
    /// With the functions, the constants and the powers.
    Scientific,
    /// Units and currencies.
    Convert,
}

impl Mode {
    /// Every mode, in the order `TAB` walks them.
    pub const ALL: [Self; 3] = [Self::Basic, Self::Scientific, Self::Convert];

    /// Its name's key (`calc-mode-<id>`).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Basic => "calc-mode-basic",
            Self::Scientific => "calc-mode-scientific",
            Self::Convert => "calc-mode-convert",
        }
    }

    /// The next one — or the one before — round the ends.
    #[must_use]
    pub fn beside(self, forward: bool) -> Self {
        let at = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        let n = Self::ALL.len();
        Self::ALL[if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

/// One term of an answer's unit, as the file keeps it: a unit's id (the tail
/// of `unit-<id>`) or a currency's code, and its power.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Term {
    /// The unit's id, or the currency's code.
    pub base: String,
    /// Its power (`m²` is 2).
    pub power: i8,
}

/// An answer kept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    /// When it was kept, in milliseconds since the epoch — its identity, which
    /// two doors merging the file compare.
    pub at_ms: u64,
    /// The sum as it was typed.
    pub expression: String,
    /// The answer's number, with `.` for its mark — written in the language's
    /// own mark when it is drawn.
    pub number: String,
    /// Whether the answer is an approximation (`≈`).
    #[serde(default)]
    pub approx: bool,
    /// Its unit, when it has one.
    #[serde(default)]
    pub unit: Vec<Term>,
}

/// What `calc.json` holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Kept {
    /// The answers kept, oldest first.
    #[serde(default)]
    pub history: Vec<Entry>,
}

/// Why an answer was not written into the file.
#[derive(Debug)]
pub enum Unkept {
    /// The file is there and does not parse: writing it whole would throw
    /// away what the person kept, so nothing is written.
    Unparsed,
    /// The file could not be written.
    NotWritten(std::io::Error),
}

/// The person's history file.
#[must_use]
pub fn store() -> ronoctua_store::Store<Kept> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// `entry` merged into what `store` holds — by its moment, so the same answer
/// kept twice is kept once — the newest [`ronoctua_calc::HISTORY_ENTRIES`]
/// written back, and the history as it now stands.
///
/// # Errors
///
/// [`Unkept::Unparsed`] when the file will not parse (read with `parsed`,
/// never `read`: the default a failed read gives is right for a reader and
/// erases the file for a writer), and [`Unkept::NotWritten`].
pub fn keep_in(store: &ronoctua_store::Store<Kept>, entry: Entry) -> Result<Vec<Entry>, Unkept> {
    let mut kept = store.parsed().ok_or(Unkept::Unparsed)?;
    if !kept.history.iter().any(|e| e.at_ms == entry.at_ms) {
        kept.history.push(entry);
    }
    kept.history.sort_by_key(|e| e.at_ms);
    let over = kept
        .history
        .len()
        .saturating_sub(ronoctua_calc::HISTORY_ENTRIES);
    kept.history.drain(..over);
    store.write(&kept).map_err(Unkept::NotWritten)?;
    Ok(kept.history)
}

/// What a click, a finger or a key lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A mode's name.
    Mode(Mode),
    /// A key of the keypad.
    Button(Button),
    /// An answer of the history, by its place from the newest.
    History(usize),
    /// A category of the ring, by its place.
    Ring(usize),
    /// The ring's arrows: a step back or forward.
    Turn(bool),
    /// The From or the To row: choose that side.
    Side(Side),
    /// A unit of the converter's list, by its place in what the search left.
    Unit(usize),
    /// The swap.
    Swap,
    /// The row that names the host the rates come from — pressing it is the
    /// consent.
    Allow,
}

/// One line either door draws: its fields, what each one is, and where the
/// text caret stands.
///
/// # What the pointer lights is what the click takes
///
/// A line has two kinds of thing a press reaches. A FIELD of a line of
/// fields side by side (a mode, a key of the footer's keypad, a side of the
/// converter) is reached on its own cells; a ROW of a list (an answer kept,
/// a unit, the row that names the rates' host) is reached anywhere on its
/// band — the gap between a sum and its answer included. [`Line::band_at`]
/// is the one answer to both, and both doors paint the pointer's band from
/// it AND take the click from it ([`Line::target_at`]): the band is the
/// pointer's mark, never a colour of the text, which is the cursor's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Line {
    /// The pieces, left to right, in the row's own cells.
    pub fields: Vec<Field>,
    /// What a press on each field reaches — parallel to `fields`.
    pub targets: Vec<Option<Target>>,
    /// The cell the text caret stands before, on the expression line.
    pub caret: Option<u16>,
    /// The ROWS of a list on this line: the cells `[from, to)` a press
    /// anywhere on reaches the target — what the band under the pointer
    /// covers. A footer line holds a row of keys and an answer beside them.
    pub bands: Vec<(u16, u16, Target)>,
}

impl Line {
    fn put(&mut self, at: u16, text: String, color: tokens::Color, target: Option<Target>) {
        if text.is_empty() {
            return;
        }
        self.fields.push(Field {
            at,
            text,
            color,
            hit: Hit::Nothing,
            explain: None,
            ink: Ink::Grid,
        });
        self.targets.push(target);
    }

    /// What a press on `cell` of this line reaches: the row whose band holds
    /// it, or the field under it — through the one ruler both doors light a
    /// field with (`fields::lit_field`).
    #[must_use]
    pub fn target_at(&self, cell: u16) -> Option<Target> {
        if let Some((_, _, t)) = self.bands.iter().find(|(a, b, _)| cell >= *a && cell < *b) {
            return Some(*t);
        }
        crate::fields::lit_field(&self.fields, cell)
            .and_then(|i| self.targets.get(i).copied().flatten())
    }

    /// The cells `[from, to)` the pointer on `cell` bands — the row's band,
    /// or the field's own cells (the ruler's, its gap included) — and
    /// `None` where a press reaches nothing, so nothing lights there.
    #[must_use]
    pub fn band_at(&self, cell: u16) -> Option<(u16, u16)> {
        if let Some((a, b, _)) = self.bands.iter().find(|(a, b, _)| cell >= *a && cell < *b) {
            return Some((*a, *b));
        }
        let i = crate::fields::lit_field(&self.fields, cell)?;
        self.targets.get(i).copied().flatten()?;
        let f = &self.fields[i];
        Some((f.at, f.at + ronoctua_render::layout::width(&f.text) + 1))
    }

    /// Whether this line goes on the control the line `above` it is a line
    /// of: both take ONE press whole, the same over the same cells — a
    /// sentence wrapped over several lines (the consent's row, a failed
    /// fetch's), whose lines light together and take the one press. Lines of
    /// a list that take each its own press (the answers kept, the units) are
    /// each a control of its own.
    #[must_use]
    pub fn continues(&self, above: &Self) -> bool {
        match (self.bands.as_slice(), above.bands.as_slice()) {
            ([band], [over]) => band == over,
            _ => false,
        }
    }

    /// The line where the control line `n` of `lines` is a line of begins —
    /// its first line ([`Line::continues`]), which the footer lights and
    /// clicks for every line of it (`Region::control_of`); `n` itself for
    /// every other line.
    #[must_use]
    pub fn control_of(lines: &[Self], n: usize) -> usize {
        let goes_on = |k: usize| k > 0 && k < lines.len() && lines[k].continues(&lines[k - 1]);
        let mut first = n;
        while goes_on(first) {
            first -= 1;
        }
        first
    }

    /// A line that acts as a whole, `[from, to)`: every field and the gaps
    /// between them reach the same target.
    fn whole(mut self, target: Target, from: u16, to: u16) -> Self {
        for t in &mut self.targets {
            *t = Some(target);
        }
        self.bands.push((from, to.max(from + 1), target));
        self
    }

    /// The line moved `by` cells to the right — fields, bands and caret.
    #[must_use]
    pub fn shifted(mut self, by: u16) -> Self {
        for f in &mut self.fields {
            f.at += by;
        }
        for (a, b, _) in &mut self.bands {
            *a += by;
            *b += by;
        }
        self.caret = self.caret.map(|c| c + by);
        self
    }

    /// `other`'s fields and bands put on this line, as they are.
    fn join(&mut self, other: Self) {
        self.fields.extend(other.fields);
        self.targets.extend(other.targets);
        self.bands.extend(other.bands);
    }
}

/// What the person did, as the view reads it — from a key, or a click on
/// the header's shortcut for one (a key of the keypad is a [`Target`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Press {
    /// Text typed: a character, or what a key of the keypad types.
    Type(String),
    /// `DEL`.
    Back,
    /// `C-DEL`.
    Clear,
    /// `RET`.
    Enter,
    /// `TAB` and `S-TAB`.
    Mode(bool),
    /// `<up>` and `<down>`.
    Walk(bool),
    /// `C-p` and `C-n`: the history, in every mode — `true` is older.
    History(bool),
    /// `<left>` and `<right>`: the ring.
    Turn(bool),
    /// `C-<up>` and `C-<down>`: which side a unit goes to.
    Aim(Side),
    /// `C-s`.
    Swap,
    /// `C-c`.
    Copy,
    /// `C-v` or `C-y`.
    Paste,
    /// `C-f`: the window floats, or goes back in the layout.
    Float,
    /// `C-a`: the row that names the rates' host, pressed from the keyboard.
    Allow,
    /// `Esc`.
    Escape,
}

/// What the view wants of its door after a press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// A frame, and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Write this answer into `calc.json` ([`keep_in`]) and tell the view
    /// what the file says now ([`View::history_is`]).
    Keep(Entry),
    /// Put this on the clipboard, and say so.
    Copy(String),
    /// Read the clipboard for [`View::pasted`].
    Paste,
    /// Ask for the exchange rates, off the loop (the consent is given).
    Fetch,
    /// The person pressed the row that names the host: write the consent,
    /// then ask for the rates.
    Allow,
    /// The window floats, or goes back in the layout.
    Float,
}

/// The exchange rates, as far as this view knows them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Money {
    /// Not looked for: nothing typed needed them.
    #[default]
    Unlooked,
    /// Needed, not on disk fresh, and the person has not let the host be
    /// asked: the row that names it is drawn.
    Asking,
    /// Being fetched.
    Fetching,
    /// At hand.
    Have(ronoctua_calc::rates::Rates),
    /// Fetching failed, and why.
    Failed(ronoctua_calc::rates::Why),
}

/// What a door found when it looked for the rates: the file on disk, and
/// whether the host may be asked.
#[derive(Debug, Clone)]
pub struct Probe {
    /// The cached rates, when a file parses.
    pub cached: Option<ronoctua_calc::rates::Cached>,
    /// Whether `www.ecb.europa.eu` is let (`ronoctua-consent`).
    pub allowed: bool,
}

/// What a fetch of the rates came back with.
#[derive(Debug)]
pub enum Fetched {
    /// Today's rates.
    Got(ronoctua_calc::rates::Rates),
    /// None, and why.
    Failed(ronoctua_calc::rates::Why),
    /// The consent was taken back between the press and the fetch: nothing
    /// was asked.
    NotLet,
}

/// What the machine holds about the rates: the cache on disk, and the
/// person's consent for the host — read at the moment of asking.
#[must_use]
pub fn probe() -> Probe {
    Probe {
        cached: ronoctua_calc::rates::cached(),
        allowed: ronoctua_consent::allows(ronoctua_calc::rates::HOST),
    }
}

/// Asks for today's rates ON A THREAD — an HTTP request never holds a
/// door's loop — with the pass the person's consent mints, and hands what
/// came back to `done`.
pub fn fetch_then(done: impl FnOnce(Fetched) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("calc-rates".into())
        .spawn(move || {
            let got = match ronoctua_consent::pass(ronoctua_calc::rates::HOST) {
                Ok(pass) => match ronoctua_calc::rates::fetch(&pass) {
                    Ok(r) => Fetched::Got(r),
                    Err(why) => Fetched::Failed(why),
                },
                Err(_) => Fetched::NotLet,
            };
            done(got);
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "the exchange rates' thread could not start");
    }
}

/// The keys every mode but Convert teaches — and a click on each presses it.
///
/// The words are the ids read as text keys, the sentence under the pointer
/// is `about-<id>` (`every-hint-key-explains-itself`).
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    (EQUALS_KEY, "calc-equals"),
    ("TAB", "calc-mode"),
    ("C-DEL", "calc-clear"),
    ("C-c", "calc-copy"),
    ("C-p", "calc-history"),
];

/// The keys of the converter.
pub const OF_THE_CONVERTER: &[(&str, &str)] = &[
    ("RET", "calc-take"),
    ("TAB", "calc-mode"),
    ("<right>", "calc-ring-next"),
    ("<left>", "calc-ring-previous"),
    ("C-<up>", "calc-from"),
    ("C-<down>", "calc-to"),
    ("C-s", "calc-swap"),
    ("C-c", "calc-copy"),
];

/// The key that keeps an answer — taught by the header, and named by the
/// sentence of an empty history (`calc-history-empty`, `{ $keys }`).
pub const EQUALS_KEY: &str = "RET";

/// The key only the window has: its place on the screen.
pub const OF_THE_WINDOW: &[(&str, &str)] = &[("C-f", "calc-float")];

/// The key of the row that names the rates' host — taught while that row is
/// drawn, and only then: a key that does nothing is not taught. The row is
/// every app's (`consent_rows`), and so is its key.
pub const OF_THE_CONSENT: &[(&str, &str)] = crate::consent_rows::OF_THE_CONSENT;

/// What the header teaches depends on: the mode, and whether the row that
/// names the rates' host is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taught {
    /// The mode.
    pub mode: Mode,
    /// Whether the consent's row is drawn ([`View::asks`]).
    pub asks: bool,
}

/// The keys the header teaches — the consent's while its row is drawn, the
/// window's own last.
#[must_use]
pub fn hint(
    taught: Taught,
    window: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<crate::chrome::Piece> {
    let mut table: Vec<(&'static str, &'static str)> = if taught.mode == Mode::Convert {
        OF_THE_CONVERTER.to_vec()
    } else {
        OF_THE_VIEW.to_vec()
    };
    if taught.asks {
        table.extend_from_slice(OF_THE_CONSENT);
    }
    if window {
        table.extend_from_slice(OF_THE_WINDOW);
    }
    crate::chrome::hint_of(&table, texts)
}

/// A key, by its whole notation, as the view's press — the one reader both
/// doors call. `None` for a key the view does not take (the door swallows it).
#[must_use]
pub fn press_of_key(key: Key) -> Option<Press> {
    let m = key.mods;
    if crate::line::pastes(key) {
        return Some(Press::Paste);
    }
    if m.alt || m.logo {
        return None;
    }
    Some(match (m.ctrl, key.sym) {
        (_, Sym::Esc) => Press::Escape,
        (false, Sym::Enter) => Press::Enter,
        (false, Sym::Tab) => Press::Mode(!m.shift),
        (false, Sym::Backspace) => Press::Back,
        (true, Sym::Backspace) => Press::Clear,
        (false, Sym::Up) => Press::Walk(false),
        (false, Sym::Down) => Press::Walk(true),
        (true, Sym::Up) => Press::Aim(Side::From),
        (true, Sym::Down) => Press::Aim(Side::To),
        (false, Sym::Left) => Press::Turn(false),
        (false, Sym::Right) => Press::Turn(true),
        (true, Sym::Char('p')) => Press::History(true),
        (true, Sym::Char('n')) => Press::History(false),
        (true, Sym::Char('c')) => Press::Copy,
        (true, Sym::Char('s')) => Press::Swap,
        (true, Sym::Char('f')) => Press::Float,
        (true, Sym::Char('a')) => Press::Allow,
        (false, Sym::Space) => Press::Type(" ".to_owned()),
        // A CHARACTER TYPED, whatever it is: a digit, a sign, a letter of a
        // unit or a function — the engine decides what it means.
        (false, Sym::Char(c)) => Press::Type(c.to_string()),
        _ => return None,
    })
}

/// The calculator's state — everything both doors draw.
#[derive(Debug, Clone)]
pub struct View {
    /// The mode.
    pub mode: Mode,
    /// The expression being typed — the amount, in Convert.
    text: String,
    /// What `RET` refused, until the next change.
    refused: Option<Refusal>,
    /// In Convert, whether the live refusal is the AMOUNT's (half a sum, not
    /// yet a number) rather than the conversion's — held until `RET`.
    amount_refused: bool,
    /// The line holds the answer `RET` just kept: a sign typed goes on from
    /// it, anything else starts a new sum — as on every calculator.
    answered: bool,
    /// The answer of what is typed, worked out at every change.
    live: Option<Result<Value, Refusal>>,
    /// The answers kept, oldest first.
    history: Vec<Entry>,
    /// The answer picked, by its place from the newest.
    picked: Option<usize>,
    /// The converter.
    pub convert: Converter,
    /// The exchange rates.
    money: Money,
    /// Whether a click picked the rates' row — the consent's, or a failed
    /// fetch's — so the next click on it is its press (invariant 19).
    ///
    /// # Why the row needs a first click of its own
    ///
    /// Because the row comes up under the pointer that asked for it: a
    /// category of the ring chosen by a click wants the rates, the consent's
    /// lines are drawn above the ring, and the ring moves down under the
    /// hand. The second press of a double click on a currency let the host
    /// and fetched, with nobody choosing to. Every other view's consent row
    /// is selected by the first click and pressed by the second; this was the
    /// one whose first click let the host.
    allow_picked: bool,
    /// Now, in milliseconds since the epoch — what `now` and `today` are, and
    /// the moment an answer is kept at. The door sets it before every press.
    now_ms: u64,
    /// The person's zone.
    zone: ronoctua_files::when::Zone,
    /// Where other zones are read from (`RONOCTUA_ZONEINFO`).
    zoneinfo: Option<std::path::PathBuf>,
}

impl View {
    /// A calculator over these answers kept, in the person's zone.
    #[must_use]
    pub fn fresh(
        history: Vec<Entry>,
        zone: ronoctua_files::when::Zone,
        zoneinfo: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            mode: Mode::Basic,
            text: String::new(),
            refused: None,
            amount_refused: false,
            answered: false,
            live: None,
            history,
            picked: None,
            convert: Converter::default(),
            money: Money::Unlooked,
            allow_picked: false,
            now_ms: 0,
            zone,
            zoneinfo,
        }
    }

    /// The expression being typed.
    #[cfg(test)]
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The answers kept, NEWEST FIRST — the order they are drawn in.
    pub fn newest_first(&self) -> impl Iterator<Item = &Entry> {
        self.history.iter().rev()
    }

    /// The answers kept, oldest first — as the file keeps them.
    #[must_use]
    pub fn history(&self) -> &[Entry] {
        &self.history
    }

    /// How many answers are kept.
    #[must_use]
    pub fn kept(&self) -> usize {
        self.history.len()
    }

    /// The answer picked, by its place from the newest.
    #[must_use]
    pub const fn picked(&self) -> Option<usize> {
        self.picked
    }

    /// The exchange rates, as far as this view knows them.
    #[cfg(test)]
    #[must_use]
    pub const fn money(&self) -> &Money {
        &self.money
    }

    /// Now, for the next press.
    pub const fn at(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    fn env(&self) -> ronoctua_calc::Env {
        let mut env = ronoctua_calc::Env::at(
            i64::try_from(self.now_ms / 1000).unwrap_or(0),
            self.zone.clone(),
        );
        env.zoneinfo.clone_from(&self.zoneinfo);
        if let Money::Have(r) = &self.money {
            env.rates = Some(r.clone());
        }
        env
    }

    /// Works out the answer of what is typed — in Convert, the amount in the
    /// To unit.
    fn recompute(&mut self, mark: char) {
        if self.text.trim().is_empty() {
            self.live = None;
            self.amount_refused = false;
            return;
        }
        let env = self.env();
        let answer =
            ronoctua_calc::parse(&self.text, mark).and_then(|e| ronoctua_calc::eval(&e, &env));
        self.amount_refused = answer.is_err();
        self.live = Some(if self.mode == Mode::Convert {
            answer.and_then(|v| {
                self.convert
                    .converted(&v, &env)
                    .map_err(|why| Refusal { at: 0, why })
            })
        } else {
            answer
        });
    }

    /// The answer of what is typed, when there is one.
    #[cfg(test)]
    #[must_use]
    pub const fn live(&self) -> Option<&Result<Value, Refusal>> {
        self.live.as_ref()
    }

    /// Whether what is on screen needs the exchange rates: two currencies in
    /// the sum, or on the converter's two sides.
    #[must_use]
    pub fn wants_rates(&self, mark: char) -> bool {
        if self.mode == Mode::Convert {
            return self.convert.wants_rates();
        }
        ronoctua_calc::parse(&self.text, mark).is_ok_and(|e| e.wants_rates())
    }

    /// What the door found on disk and in the consent, when the rates are
    /// wanted and not yet looked for: the file's rates when they are fresh,
    /// a fetch when the host may be asked, and the row that names it when it
    /// may not.
    pub fn looked(&mut self, probe: Probe, texts: &ronoctua_i18n::Texts) -> Errand {
        let now = i64::try_from(self.now_ms / 1000).unwrap_or(0);
        let stale = probe.cached.as_ref().is_none_or(|c| c.stale(now));
        let errand = match (probe.cached, stale, probe.allowed) {
            (Some(c), false, _) => {
                self.money = Money::Have(c.rates);
                Errand::Nothing
            }
            // STALE RATES ARE STILL RATES: drawn while the fresh ones come.
            (Some(c), true, true) => {
                self.money = Money::Have(c.rates);
                Errand::Fetch
            }
            (None, _, true) => {
                self.money = Money::Fetching;
                Errand::Fetch
            }
            (Some(c), true, false) => {
                self.money = Money::Have(c.rates);
                Errand::Nothing
            }
            (None, _, false) => {
                self.money = Money::Asking;
                Errand::Nothing
            }
        };
        self.recompute(texts.decimal_mark());
        errand
    }

    /// The consent was written: the rates are being fetched.
    pub fn allowed(&mut self) {
        self.allow_picked = false;
        if !matches!(self.money, Money::Have(_)) {
            self.money = Money::Fetching;
        }
    }

    /// What the fetch came back with.
    pub fn rates_arrived(
        &mut self,
        got: Result<ronoctua_calc::rates::Rates, ronoctua_calc::rates::Why>,
        texts: &ronoctua_i18n::Texts,
    ) {
        match got {
            Ok(r) => self.money = Money::Have(r),
            // A FAILURE DOES NOT TAKE THE STALE RATES AWAY: they convert, and
            // the date on the row says how old they are.
            Err(why) if !matches!(self.money, Money::Have(_)) => self.money = Money::Failed(why),
            Err(why) => tracing::warn!(why = why.id(), "fresh exchange rates did not come"),
        }
        self.recompute(texts.decimal_mark());
    }

    /// What a fetch came back with, from either door.
    pub fn fetched(&mut self, got: Fetched, texts: &ronoctua_i18n::Texts) {
        match got {
            Fetched::Got(r) => self.rates_arrived(Ok(r), texts),
            Fetched::Failed(why) => self.rates_arrived(Err(why), texts),
            // THE CONSENT WAS TAKEN BACK: the row that names the host again.
            Fetched::NotLet => {
                if !matches!(self.money, Money::Have(_)) {
                    self.money = Money::Asking;
                }
            }
        }
    }

    /// Whether the row that names the rates' host is drawn — pressed, or
    /// `C-a`, it is the consent (or, after a failure, the ask again).
    #[must_use]
    pub fn asks(&self, mark: char) -> bool {
        matches!(self.money, Money::Asking | Money::Failed(_)) && self.wants_rates(mark)
    }

    /// What the header teaches now.
    #[must_use]
    pub fn taught(&self, mark: char) -> Taught {
        Taught {
            mode: self.mode,
            asks: self.asks(mark),
        }
    }

    /// Whether the rates are wanted and nobody looked yet — what the door
    /// asks after every press ([`View::looked`]).
    #[must_use]
    pub fn to_look(&self, mark: char) -> bool {
        matches!(self.money, Money::Unlooked) && self.wants_rates(mark)
    }

    /// The file changed — this door's own write, or another door's: the
    /// history is what it says. The pick stays on the same answer.
    pub fn history_is(&mut self, history: Vec<Entry>) {
        let at = self
            .picked
            .and_then(|i| self.history.iter().rev().nth(i))
            .map(|e| e.at_ms);
        self.history = history;
        self.picked = at.and_then(|at| self.history.iter().rev().position(|e| e.at_ms == at));
    }

    /// Text pasted: onto the expression line, on one line.
    pub fn pasted(&mut self, text: &str, texts: &ronoctua_i18n::Texts) {
        let text = crate::line::one_line(text);
        if text.is_empty() {
            return;
        }
        self.typed(&text, texts);
    }

    fn changed(&mut self, texts: &ronoctua_i18n::Texts) {
        self.refused = None;
        self.answered = false;
        self.picked = None;
        self.recompute(texts.decimal_mark());
    }

    /// One press, and what the door is to do after it.
    pub fn press(&mut self, press: Press, texts: &ronoctua_i18n::Texts) -> Errand {
        let converting = self.mode == Mode::Convert;
        match press {
            Press::Escape => {
                if self.picked.take().is_some() || self.refused.take().is_some() {
                    return Errand::Nothing;
                }
                if converting && !self.convert.search.is_empty() {
                    self.convert.search.clear();
                    self.convert.cursor = 0;
                    return Errand::Nothing;
                }
                return Errand::Close;
            }
            Press::Type(t) => {
                // IN THE CONVERTER A LETTER FINDS A UNIT: the amount is a
                // number, and the units are chosen from the list.
                let letter =
                    t.chars().next().is_some_and(char::is_alphabetic) && t.chars().count() == 1;
                if converting && letter {
                    t.chars().for_each(|c| self.convert.letter(c));
                    return Errand::Nothing;
                }
                self.typed(&t, texts);
            }
            Press::Back => {
                if converting && self.convert.back() {
                    return Errand::Nothing;
                }
                crate::line::pop_cluster(&mut self.text);
                self.changed(texts);
            }
            Press::Clear => {
                self.text.clear();
                self.changed(texts);
            }
            Press::Enter => return self.enter(texts),
            Press::Mode(forward) => self.go_to_mode(self.mode.beside(forward), texts),
            Press::Walk(down) if converting => {
                let n = self
                    .convert
                    .listed(&crate::calc_convert::subtag_of(texts), texts)
                    .len();
                self.convert.walk(down, n);
            }
            Press::Walk(down) | Press::History(down) => self.walk_history(down),
            Press::Turn(forward) => {
                if converting {
                    self.convert.turn(if forward { 1 } else { -1 });
                    self.money_again();
                    self.recompute(texts.decimal_mark());
                }
            }
            Press::Aim(side) => {
                if converting {
                    self.convert.aim(side);
                }
            }
            Press::Swap => {
                if converting {
                    self.convert.swap();
                    self.recompute(texts.decimal_mark());
                }
            }
            Press::Copy => {
                return self.copy_text(texts).map_or(Errand::Nothing, Errand::Copy);
            }
            Press::Paste => return Errand::Paste,
            Press::Float => return Errand::Float,
            // THE CONSENT FROM THE KEYBOARD: the row's own press, and only
            // while the row is drawn.
            Press::Allow => {
                if self.asks(texts.decimal_mark()) {
                    return Errand::Allow;
                }
            }
        }
        Errand::Nothing
    }

    /// Rates wanted anew, once more: a category of the ring chosen, or a
    /// currency typed, after the last look found nothing to want.
    fn money_again(&mut self) {
        // A ROW ASKED ANEW IS NOT THE ROW THAT WAS PICKED.
        self.allow_picked = false;
        if matches!(self.money, Money::Asking | Money::Failed(_)) {
            self.money = Money::Unlooked;
        }
    }

    fn go_to_mode(&mut self, mode: Mode, texts: &ronoctua_i18n::Texts) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.refused = None;
        self.money_again();
        self.recompute(texts.decimal_mark());
    }

    fn walk_history(&mut self, older: bool) {
        let n = self.history.len();
        if n == 0 {
            return;
        }
        // FROM THE LINE, the first step is the newest answer, whichever way:
        // the history is drawn newest first, under or beside the line.
        self.picked = match (self.picked, older) {
            (None, _) => Some(0),
            (Some(i), true) => Some((i + 1).min(n - 1)),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        };
    }

    /// `t` typed at the end — or, over an answer just kept, in its place
    /// unless it is a sign that goes on from it.
    fn typed(&mut self, t: &str, texts: &ronoctua_i18n::Texts) {
        let goes_on = t
            .chars()
            .next()
            .is_some_and(|c| "+-−*×/÷^²%!)".contains(c) || c == ' ');
        if self.answered && !goes_on {
            self.text.clear();
        }
        self.text.push_str(t);
        self.changed(texts);
    }

    fn button(&mut self, b: Button, texts: &ronoctua_i18n::Texts) -> Errand {
        match b.deed(texts) {
            Deed::Type(t) => {
                self.typed(&t, texts);
                Errand::Nothing
            }
            Deed::Back => self.press(Press::Back, texts),
            Deed::Clear => self.press(Press::Clear, texts),
            Deed::Equals => self.enter(texts),
        }
    }

    /// `RET`: puts the picked answer's sum back, takes the unit under the
    /// converter's cursor, or keeps the answer of what is typed.
    fn enter(&mut self, texts: &ronoctua_i18n::Texts) -> Errand {
        if let Some(i) = self.picked.take() {
            if let Some(e) = self.history.iter().rev().nth(i) {
                self.text.clone_from(&e.expression);
            }
            self.changed(texts);
            return Errand::Nothing;
        }
        if self.mode == Mode::Convert {
            let listed = self
                .convert
                .listed(&crate::calc_convert::subtag_of(texts), texts);
            if let Some(b) = listed.get(self.convert.cursor.min(listed.len().saturating_sub(1))) {
                self.convert.take(*b);
                self.money_again();
                self.recompute(texts.decimal_mark());
            }
            // AN AMOUNT THAT IS NOT A NUMBER is said now that `RET` asked —
            // while it was typed, half a sum was not yet a sum.
            if let Some(Err(r)) = &self.live
                && self.amount_refused
            {
                self.refused = Some(*r);
            }
            return Errand::Nothing;
        }
        if self.text.trim().is_empty() {
            return Errand::Nothing;
        }
        let mark = texts.decimal_mark();
        self.recompute(mark);
        match self.live.clone() {
            Some(Ok(v)) => {
                // NOT PUT IN THE LIST HERE: the door writes it into the file
                // and the list is what the file then says (`history_is`) — an
                // answer the file refused is not drawn as kept.
                let entry = entry_of(&self.text, &v, self.now_ms);
                // THE ANSWER IS THE NEXT SUM'S START: `+ 3` after `RET`
                // goes on from it, as on every calculator.
                self.text = typed_again(&entry, mark);
                self.refused = None;
                self.answered = true;
                self.recompute(mark);
                Errand::Keep(entry)
            }
            Some(Err(r)) => {
                self.refused = Some(r);
                Errand::Nothing
            }
            None => Errand::Nothing,
        }
    }

    /// A click, or a finger, on `target`. The first click on an answer of
    /// the history or a unit of the list picks it; the second does what
    /// `RET` does (invariant 19).
    pub fn clicked(&mut self, target: Target, texts: &ronoctua_i18n::Texts) -> Errand {
        // A CLICK ANYWHERE ELSE lets go of the rates' row picked.
        if target != Target::Allow {
            self.allow_picked = false;
        }
        match target {
            Target::Mode(m) => {
                self.go_to_mode(m, texts);
                Errand::Nothing
            }
            Target::Button(b) => self.button(b, texts),
            Target::History(i) => {
                if self.picked == Some(i) {
                    self.enter(texts)
                } else {
                    self.picked = (i < self.history.len()).then_some(i);
                    Errand::Nothing
                }
            }
            Target::Ring(i) => {
                self.go_to_mode(Mode::Convert, texts);
                self.convert.choose(i);
                self.money_again();
                self.recompute(texts.decimal_mark());
                Errand::Nothing
            }
            Target::Turn(forward) => self.press(Press::Turn(forward), texts),
            Target::Side(side) => {
                self.convert.aim(side);
                Errand::Nothing
            }
            Target::Unit(i) => {
                self.picked = None;
                if self.convert.cursor == i {
                    self.enter(texts)
                } else {
                    self.convert.cursor = i;
                    Errand::Nothing
                }
            }
            Target::Swap => self.press(Press::Swap, texts),
            // THE FIRST CLICK PICKS THE ROW, the second lets the host — see
            // `View::allow_picked`.
            Target::Allow if !self.allow_picked => {
                self.allow_picked = true;
                Errand::Nothing
            }
            Target::Allow => {
                self.allow_picked = false;
                Errand::Allow
            }
        }
    }

    /// What `C-c` copies: the picked answer, or the live one — its number in
    /// the language's mark and its unit's symbol.
    #[must_use]
    pub fn copy_text(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let mark = texts.decimal_mark();
        if let Some(e) = self.picked.and_then(|i| self.history.iter().rev().nth(i)) {
            return Some(typed_again(e, mark));
        }
        match &self.live {
            Some(Ok(v)) => Some(typed_again(&entry_of(&self.text, v, 0), mark)),
            _ => None,
        }
    }

    /// THE HEAD: the modes, the expression line and the answer — or why
    /// there is none, or the row that asks for the rates.
    #[must_use]
    pub fn top_lines(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        let room = columns.saturating_sub(left);
        let mut out = self.modes_lines(left, room, texts);
        // THE EXPRESSION, keeping its END: the caret is where the next
        // character goes, and a line cut there hides what the hand is doing.
        let mut typed = Line::default();
        let label = texts.t(if self.mode == Mode::Convert {
            "calc-amount"
        } else {
            "calc-sum"
        });
        let at = sum_column(left, texts);
        typed.put(left, label, acts().0, None);
        let shown =
            ronoctua_render::layout::elide_start(&self.text, (left + room).saturating_sub(at + 1));
        let caret = at + ronoctua_render::layout::width(&shown);
        typed.put(at, shown, acts().1, None);
        typed.caret = Some(caret);
        if self.text.is_empty() {
            typed.put(
                caret + 1,
                ronoctua_render::layout::elide_end(
                    &texts.t(crate::key::CALC_EMPTY),
                    (left + room).saturating_sub(caret + 1),
                ),
                tokens::color::TEXT_DIM,
                None,
            );
        }
        out.push(typed);
        if self.mode != Mode::Convert {
            out.extend(self.answer_lines(left, room, texts));
        }
        out
    }

    /// THE MODES, every one of them: a name that does not fit on the line
    /// goes on the next — a mode dropped would be a mode the pointer cannot
    /// reach.
    fn modes_lines(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let mut out = vec![Line::default()];
        let mut at = left;
        for m in Mode::ALL {
            let name = texts.t(m.key());
            let w = ronoctua_render::layout::width(&name);
            if at > left && at + w > left + room {
                out.push(Line::default());
                at = left;
            }
            // THE MODE IN USE is the accent: it is where the keyboard is —
            // `TAB` walks from it. The pointer's mark is a band, never this.
            let colour = if m == self.mode {
                tokens::color::STATE_ACCENT
            } else {
                acts().0
            };
            let name = ronoctua_render::layout::elide_end(&name, room);
            if let Some(line) = out.last_mut() {
                line.put(at, name, colour, Some(Target::Mode(m)));
            }
            at += w + tokens::space::MODELINE_SEGMENTGAP;
        }
        out
    }

    /// The answer's line (a note of several when a refusal wraps), and the
    /// rates' row when a sum needs them.
    fn answer_lines(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let at = sum_column(left, texts);
        let mut out = Vec::new();
        let mark = texts.decimal_mark();
        let wants = self.wants_rates(mark);
        match (&self.refused, &self.live) {
            (Some(r), _) => out.extend(refusal_lines(r.why, left, room, texts)),
            (None, Some(Ok(v))) => {
                let mut line = Line::default();
                line.put(
                    at,
                    ronoctua_render::layout::elide_end(
                        &answer_text(v, mark, texts),
                        (left + room).saturating_sub(at),
                    ),
                    fact().1,
                    None,
                );
                out.push(line);
            }
            // HALF A SUM IS NOT A SUM YET: nothing is said while it is
            // typed — but a sum that needs the rates says what it waits on.
            _ => out.push(Line::default()),
        }
        if wants {
            out.extend(self.money_lines(left, room, texts));
        }
        out
    }

    /// The rates' row: the host to press, the fetch under way, why it failed,
    /// or the day the rates are of.
    fn money_lines(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let host = ronoctua_calc::rates::HOST;
        let (text, colour, target) = match &self.money {
            Money::Unlooked => return Vec::new(),
            // THE ROW EVERY APP DRAWS for a host nobody let yet
            // (`consent_rows`): the host to press, then what it is for — each
            // line the same press.
            Money::Asking => {
                return crate::consent_rows::lines(
                    crate::consent_rows::Ask {
                        hosts: &[host],
                        what: "calc-rates-for",
                    },
                    room,
                    // PICKED BY A FIRST CLICK, it is drawn as a row the
                    // cursor is on: the second click is its press.
                    self.allow_picked,
                    texts,
                )
                .into_iter()
                .map(|(l, colour)| {
                    let mut line = Line::default();
                    line.put(left, l, colour, Some(Target::Allow));
                    line.whole(Target::Allow, 0, left + room)
                })
                .collect();
            }
            Money::Fetching => (
                texts.t_with(crate::key::CALC_RATES_ASKING, &[("host", host)]),
                tokens::color::TEXT_SECONDARY,
                None,
            ),
            Money::Failed(why) => (
                texts.t(&format!("calc-rates-why-{}", why.id())),
                tokens::color::STATE_ERROR,
                // A FAILURE IS PRESSED AGAIN to ask again.
                Some(Target::Allow),
            ),
            Money::Have(r) => (
                texts.t_with(crate::key::CALC_RATES_OF, &[("date", &rates_day(r, texts))]),
                crate::chrome::Kind::ListedFact.tones(false).1,
                None,
            ),
        };
        crate::chrome::note_lines(&text, room)
            .into_iter()
            .map(|l| {
                let mut line = Line::default();
                line.put(left, l, colour, target);
                match target {
                    Some(t) => line.whole(t, 0, left + room),
                    None => line,
                }
            })
            .collect()
    }

    /// THE HISTORY, `room` cells wide: its title, then one line per answer,
    /// the newest first — or the sentence that says none is kept.
    #[must_use]
    pub fn history_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        // THE LAST CELL IS THE BAR'S, while the list is longer than its box —
        // and kept free when it is not, so the answers do not move; and the
        // text keeps the air before it that it keeps after the left edge.
        let band = room.saturating_sub(crate::chrome::BAR_CELLS);
        let room = band.saturating_sub(2 * left);
        let mut title = Line::default();
        title.put(
            left,
            texts.t(crate::key::CALC_HISTORY_TITLE),
            tokens::color::TEXT_HEADING,
            None,
        );
        let mut out = vec![title];
        if self.history.is_empty() {
            out.extend(note(
                &texts.t_with(crate::key::CALC_HISTORY_EMPTY, &[("keys", EQUALS_KEY)]),
                left,
                room,
                tokens::color::TEXT_DIM,
            ));
            return out;
        }
        let mark = texts.decimal_mark();
        for (i, e) in self.newest_first().enumerate() {
            let picked = self.picked == Some(i);
            let answer = entry_answer(e, mark, texts);
            let aw = ronoctua_render::layout::width(&answer).min(room);
            // THE PICKED ANSWER IS THE CURSOR, and the cursor is a colour.
            let (sum_colour, answer_colour) = crate::chrome::Kind::ListedSetting.tones(picked);
            let mut line = Line::default();
            let sum_room = room.saturating_sub(aw + 1);
            line.put(
                left,
                ronoctua_render::layout::elide_end(&e.expression, sum_room),
                sum_colour,
                None,
            );
            // THE ANSWER AGAINST THE COLUMN'S RIGHT, where the eye runs down
            // a column of results.
            line.put(
                left + room - aw,
                ronoctua_render::layout::elide_end(&answer, aw),
                answer_colour,
                None,
            );
            out.push(line.whole(Target::History(i), 0, band));
        }
        out
    }

    /// THE CONVERTER'S HEAD: the ring, From, To and the search — and, under
    /// them, why there is no value, as a note.
    #[must_use]
    pub fn convert_lines(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        let room = columns.saturating_sub(left);
        let mark = texts.decimal_mark();
        let c = &self.convert;
        let mut out = vec![ring_line(c, left, room, texts)];
        // THE VALUES BEGIN AFTER THE WIDER OF THE TWO LABELS, so the unit
        // converted into sits under the unit converted from.
        let at = left
            + ["calc-from", "calc-to"]
                .iter()
                .map(|k| ronoctua_render::layout::width(&texts.t(k)))
                .max()
                .unwrap_or(0)
            + tokens::space::CALC_HISTORYGAP;
        let end = (left + room).saturating_sub(2);
        // FROM: the unit the amount is in, counted by the amount.
        let amount = ronoctua_calc::parse(&self.text, mark)
            .and_then(|e| ronoctua_calc::eval(&e, &self.env()))
            .map_or(2, |v| count_of(&v.number));
        let answer = match &self.live {
            Some(Ok(v)) => Some(v),
            _ => None,
        };
        for side in [Side::From, Side::To] {
            let (base, count) = match side {
                Side::From => (c.from, amount),
                Side::To => (c.to, answer.map_or(2, |v| count_of(&v.number))),
            };
            let (label_colour, value_colour) = crate::chrome::Kind::Setting.tones(c.side == side);
            let mut line = Line::default();
            let key = if side == Side::From {
                "calc-from"
            } else {
                "calc-to"
            };
            line.put(left, texts.t(key), label_colour, Some(Target::Side(side)));
            let number = match (side, answer) {
                (Side::To, Some(v)) => format!("{} ", number_text(v, mark, texts)),
                _ => String::new(),
            };
            // THE UNIT BY ITS NAME AND ITS SYMBOL, or by its symbol alone
            // where the name does not fit — a symbol is the whole of it.
            let symbol = crate::calc_convert::symbol_of(base);
            let name = crate::calc_convert::name_of(base, count, texts);
            let room_here = end.saturating_sub(at);
            let long = if name == symbol {
                format!("{number}{symbol}")
            } else {
                format!("{number}{name} ({symbol})")
            };
            let value = if ronoctua_render::layout::width(&long) <= room_here {
                long
            } else {
                ronoctua_render::layout::elide_end(&format!("{number}{symbol}"), room_here)
            };
            line.put(at, value, value_colour, Some(Target::Side(side)));
            if side == Side::From {
                line.put(
                    end + 1,
                    tokens::icon::CALC_SWAP.to_string(),
                    acts().0,
                    Some(Target::Swap),
                );
            }
            out.push(line);
        }
        // THE SEARCH, for the side being chosen, its caret where the next
        // letter goes.
        let mut search = Line::default();
        let side = texts.t(if c.side == Side::From {
            "calc-from"
        } else {
            "calc-to"
        });
        // CUT ONLY WHERE NOTHING ELSE FITS: the words say which side the
        // letters go to, and the letters keep a cell or two after them.
        let asks = ronoctua_render::layout::elide_end(
            &texts.t_with(crate::key::CALC_FIND, &[("side", &side)]),
            room.saturating_sub(2),
        );
        let typed_at = left + ronoctua_render::layout::width(&asks) + 1;
        search.put(left, asks, acts().0, None);
        let typed = ronoctua_render::layout::elide_start(
            &c.search,
            (left + room).saturating_sub(typed_at + 1),
        );
        if !c.search.is_empty() {
            search.caret = Some(typed_at + ronoctua_render::layout::width(&typed));
        }
        search.put(typed_at, typed, acts().1, None);
        out.push(search);
        // WHY THERE IS NO VALUE, wrapped: a note is never cut. The AMOUNT's
        // refusal waits for `RET`, as a sum's does — half a number typed is
        // not yet a number; the CONVERSION's (two units that do not convert)
        // is said at once, since nothing more typed would change it. A
        // conversion that waits on the rates says what it waits on instead.
        let wants = self.wants_rates(mark);
        if let Some(r) = &self.refused {
            out.extend(refusal_lines(r.why, left, room, texts));
        } else if let Some(Err(r)) = &self.live
            && !self.amount_refused
            && !(wants && r.why == ronoctua_calc::Why::NoRates)
        {
            out.extend(refusal_lines(r.why, left, room, texts));
        }
        if wants {
            out.extend(self.money_lines(left, room, texts));
        }
        out
    }

    /// THE CONVERTER'S LIST: the units the search leaves, the one under its
    /// cursor in the cursor's colour, the one already on the side marked.
    #[must_use]
    pub fn unit_lines(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        // THE LAST CELL IS THE LIST'S BAR'S, and the symbols keep the air
        // before it that the names keep after the left edge.
        let band = columns.saturating_sub(crate::chrome::BAR_CELLS);
        let room = band.saturating_sub(2 * left);
        let subtag = crate::calc_convert::subtag_of(texts);
        let listed = self.convert.listed(&subtag, texts);
        if listed.is_empty() {
            return note(
                &texts.t(crate::key::CALC_FIND_NONE),
                left,
                room,
                tokens::color::TEXT_DIM,
            );
        }
        let on_side = match self.convert.side {
            Side::From => self.convert.from,
            Side::To => self.convert.to,
        };
        listed
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let here = i == self.convert.cursor;
                let (name_colour, symbol_colour) = crate::chrome::Kind::ListedSetting.tones(here);
                let mut line = Line::default();
                let symbol = crate::calc_convert::symbol_of(*b);
                let name = crate::calc_convert::name_of(*b, 2, texts);
                // THE UNIT ALREADY ON THE SIDE says so in a heading's colour:
                // chosen, and not under the cursor.
                let name_colour = if *b == on_side && !here {
                    tokens::color::TEXT_HEADING
                } else {
                    name_colour
                };
                // A CURRENCY IS ITS CODE, once: it has no other name here.
                if name == symbol {
                    line.put(
                        left,
                        ronoctua_render::layout::elide_end(&name, room),
                        name_colour,
                        None,
                    );
                } else {
                    let sw = ronoctua_render::layout::width(&symbol).min(room);
                    line.put(
                        left,
                        ronoctua_render::layout::elide_end(&name, room.saturating_sub(sw + 2)),
                        name_colour,
                        None,
                    );
                    line.put(left + room - sw, symbol, symbol_colour, None);
                }
                line.whole(Target::Unit(i), 0, band)
            })
            .collect()
    }

    /// Where the converter's cursor is among [`View::unit_lines`].
    #[must_use]
    pub const fn unit_cursor(&self) -> usize {
        self.convert.cursor
    }

    /// The sentence a keypad too narrow for the functions' keys says in
    /// their place, `columns` wide — they are typed by name.
    #[must_use]
    pub fn narrow_lines(columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        note(
            &texts.t(crate::key::CALC_KEYS_NARROW),
            left,
            columns.saturating_sub(2 * left),
            tokens::color::TEXT_SECONDARY,
        )
    }

    /// THE FOOTER'S ROWS: the head, then the keypad as lines of keys with the
    /// history beside it — every answer, the lines past the keypad's last
    /// holding answers alone, so the region's own scroll, wheel and bar
    /// reach the oldest — or under it when the region is narrow; in
    /// Convert, the converter and its list.
    #[must_use]
    pub fn footer_lines(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let mut out = self.top_lines(columns, texts);
        if self.mode == Mode::Convert {
            out.extend(self.convert_lines(columns, texts));
            out.push(Line::default());
            out.extend(self.unit_lines(columns, texts));
            return out;
        }
        out.push(Line::default());
        let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        let room = columns.saturating_sub(left);
        let arrangement = crate::calc_keypad::arrange(
            self.mode == Mode::Scientific,
            room,
            crate::calc_keypad::footer_least(true),
        );
        if arrangement == crate::calc_keypad::Arrangement::Narrow {
            out.extend(Self::narrow_lines(columns, texts));
        }
        let across = arrangement.columns();
        // A KEY IS WHAT IT PREFERS — or as wide as its legends, when they are
        // wider — and narrower only where the region has no more.
        let least = crate::calc_keypad::footer_least(arrangement.has_functions());
        let w = (room / across).clamp(1, tokens::space::CALC_FOOTERKEYCELLS.max(least));
        let pad_cells = across * w;
        let gap = tokens::space::CALC_HISTORYGAP;
        let beside = columns >= left + pad_cells + gap + tokens::space::CALC_HISTORYMINCELLS;
        let mut keys: Vec<Line> = (0..arrangement.rows()).map(|_| Line::default()).collect();
        for p in crate::calc_keypad::plan(arrangement) {
            let cells = p.span * w;
            let room = cells.saturating_sub(1);
            let legend = ronoctua_render::layout::elide_end(&p.button.legend(texts), room);
            // THE LEGEND IN THE MIDDLE OF ITS KEY, padded to the key's width
            // so the field — what lights and what is pressed — IS the key.
            let text = format!("{legend:^width$}", width = usize::from(room));
            let colour = match p.button.tone() {
                crate::calc_keypad::Tone::Number => fact().1,
                crate::calc_keypad::Tone::Action => acts().0,
                // `=` IS THE ANSWER'S KEY, in the tone of what a row acts
                // on — not the accent, which is where the keyboard is.
                crate::calc_keypad::Tone::Answer => acts().1,
            };
            if let Some(line) = keys.get_mut(usize::from(p.row)) {
                line.put(
                    left + p.column * w,
                    text,
                    colour,
                    Some(Target::Button(p.button)),
                );
            }
        }
        if beside {
            let from = left + pad_cells + gap;
            let history = self.history_lines(columns.saturating_sub(from), texts);
            let rows = keys.len().max(history.len());
            let mut history = history.into_iter();
            for k in 0..rows {
                let mut line = keys.get(k).cloned().unwrap_or_default();
                if let Some(h) = history.next() {
                    line.join(h.shifted(from));
                }
                out.push(line);
            }
        } else {
            out.extend(keys);
            out.push(Line::default());
            out.extend(self.history_lines(columns, texts));
        }
        out
    }

    /// Where the footer's list is to follow: the converter's cursor, or the
    /// picked answer.
    #[must_use]
    pub fn footer_cursor(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        let lines = self.footer_lines(columns, texts);
        let wanted = if self.mode == Mode::Convert {
            Target::Unit(self.convert.cursor)
        } else {
            Target::History(self.picked?)
        };
        lines
            .iter()
            .position(|l| l.bands.iter().any(|(_, _, t)| *t == wanted))
    }
}

/// The day the rates are of, in the person's order of a date
/// (`chrome::date_text`) — the date a door's watch of the date's form
/// follows, never an order written here.
pub fn rates_day(r: &ronoctua_calc::rates::Rates, texts: &ronoctua_i18n::Texts) -> String {
    // `epoch_of` counts DAYS; the day's noon, in UTC, is that day anywhere
    // the date is read.
    let (day, noon) = (86_400, 43_200);
    ronoctua_files::when::epoch_of(
        i64::from(r.day.year),
        u32::from(r.day.month),
        u32::from(r.day.day),
    )
    .map_or_else(String::new, |days| {
        crate::chrome::date_text(
            days * day + noon,
            &ronoctua_files::when::Zone::utc(),
            crate::when::prefs::current().order,
            texts,
        )
    })
}

/// The tones of a label that ACTS and of what it changes — a mode, a key, a
/// side, the ring — from the one place a row's tones are decided
/// (`chrome::Kind`).
fn acts() -> (tokens::Color, tokens::Color) {
    crate::chrome::Kind::Setting.tones(false)
}

/// The tones of a FACT: an answer, a rate's day.
fn fact() -> (tokens::Color, tokens::Color) {
    crate::chrome::Kind::Fact.tones(false)
}

/// Where the sum — and its answer under it — begins: after the widest of
/// the line's two labels, so the answer sits under the sum it answers.
fn sum_column(left: u16, texts: &ronoctua_i18n::Texts) -> u16 {
    let widest = ["calc-sum", "calc-amount"]
        .iter()
        .map(|k| ronoctua_render::layout::width(&texts.t(k)))
        .max()
        .unwrap_or(0);
    left + widest + tokens::space::CALC_HISTORYGAP
}

/// The ring's line: the arrows, and the categories from the one in use.
fn ring_line(c: &Converter, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Line {
    let mut line = Line::default();
    let earlier = tokens::icon::CALC_EARLIER.to_string();
    let later = tokens::icon::CALC_LATER.to_string();
    line.put(left, earlier, acts().0, Some(Target::Turn(false)));
    let end = (left + room).saturating_sub(2);
    let mut at = left + 2;
    for k in 0..RING.len() {
        let i = (c.ring() + k) % RING.len();
        let name = texts.t(&RING[i].key());
        let w = ronoctua_render::layout::width(&name);
        if at + w > end {
            break;
        }
        let colour = if k == 0 {
            tokens::color::STATE_ACCENT
        } else {
            acts().0
        };
        line.put(at, name, colour, Some(Target::Ring(i)));
        at += w + tokens::space::MODELINE_SEGMENTGAP;
    }
    line.put(end + 1, later, acts().0, Some(Target::Turn(true)));
    line
}

/// A sentence as lines of a note, `room` wide — a note is never cut.
fn note(text: &str, left: u16, room: u16, colour: tokens::Color) -> Vec<Line> {
    crate::chrome::note_lines(text, room)
        .into_iter()
        .map(|l| {
            let mut line = Line::default();
            line.put(left, l, colour, None);
            line
        })
        .collect()
}

/// A refusal's sentence, wrapped — a note is never cut.
fn refusal_lines(
    why: ronoctua_calc::Why,
    left: u16,
    room: u16,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Line> {
    note(
        &texts.t(&format!("calc-why-{}", why.id())),
        left,
        room,
        tokens::color::STATE_ERROR,
    )
}

/// How many of a unit a number is, for its name's plural: one only when it
/// is exactly one.
fn count_of(n: &Number) -> i64 {
    let one = ronoctua_calc::BigInt::from(1);
    match n {
        Number::Exact(r) if r.is_integer() && (r.to_integer() == one || r.to_integer() == -one) => {
            1
        }
        _ => 2,
    }
}

/// An answer's number in the language's mark, as the engine writes it.
fn number_text(v: &Value, mark: char, texts: &ronoctua_i18n::Texts) -> String {
    let _ = texts;
    ronoctua_calc::format(v, mark, usize::from(tokens::space::CALC_DIGITS))
}

/// An answer as the line says it: `= 4`, `≈ 1,4142 metres`.
#[must_use]
pub fn answer_text(v: &Value, mark: char, texts: &ronoctua_i18n::Texts) -> String {
    let value = match &v.unit {
        Some(q) if !q.terms.is_empty() => {
            let count = count_of(&v.number);
            format!(
                "{} {}",
                number_text(v, mark, texts),
                unit_text(&q.terms, count, texts)
            )
        }
        _ => number_text(v, mark, texts),
    };
    texts.t_with(
        if v.number.is_exact() {
            "calc-equals-exact"
        } else {
            "calc-equals-approx"
        },
        &[("value", &value)],
    )
}

/// A unit as the line says it: one unit by its NAME, for the count; a
/// product by its symbols and powers (`m²`, `km·h⁻¹`), which are notation.
fn unit_text(terms: &[(Base, i8)], count: i64, texts: &ronoctua_i18n::Texts) -> String {
    if let [(base, 1)] = terms {
        return crate::calc_convert::name_of(*base, count, texts);
    }
    terms
        .iter()
        .map(|(b, p)| format!("{}{}", crate::calc_convert::symbol_of(*b), power_text(*p)))
        .collect::<Vec<_>>()
        .join("·")
}

/// A power in superscript digits — nothing for 1.
fn power_text(p: i8) -> String {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    if p == 1 {
        return String::new();
    }
    let sign = if p < 0 { "⁻" } else { "" };
    let digits: String = p
        .unsigned_abs()
        .to_string()
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| DIGITS[d as usize]))
        .collect();
    format!("{sign}{digits}")
}

/// An answer, kept: the sum, the number with `.` for its mark, and its unit.
fn entry_of(expression: &str, v: &Value, at_ms: u64) -> Entry {
    let unit = v
        .unit
        .as_ref()
        .map(|q| {
            q.terms
                .iter()
                .map(|(b, p)| Term {
                    base: match b {
                        Base::Unit(u) => ronoctua_calc::units::table()
                            .unit(*u)
                            .map_or_else(String::new, |u| u.id.to_owned()),
                        Base::Currency(c) => c.code().to_owned(),
                    },
                    power: *p,
                })
                .collect()
        })
        .unwrap_or_default();
    Entry {
        at_ms,
        expression: expression.trim().to_owned(),
        number: ronoctua_calc::format(v, '.', usize::from(tokens::space::CALC_DIGITS)),
        approx: !v.number.is_exact(),
        unit,
    }
}

/// A term kept, as a base again — `None` for one the table no longer has.
fn base_of(t: &Term) -> Option<Base> {
    ronoctua_calc::units::table()
        .by_id(&t.base)
        .map(Base::Unit)
        .or_else(|| ronoctua_calc::rates::Currency::new(&t.base).map(Base::Currency))
}

/// A kept answer's number in the language's mark — `.` swapped for it only
/// where the number is a decimal, never in a date or a base's digits.
fn entry_number(e: &Entry, mark: char) -> String {
    let decimal = e
        .number
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | 'e'));
    if decimal {
        e.number.replace('.', &mark.to_string())
    } else {
        e.number.clone()
    }
}

/// What a kept answer reads on its line of the history.
fn entry_answer(e: &Entry, mark: char, texts: &ronoctua_i18n::Texts) -> String {
    let number = entry_number(e, mark);
    let terms: Vec<(Base, i8)> = e
        .unit
        .iter()
        .filter_map(|t| base_of(t).map(|b| (b, t.power)))
        .collect();
    let exactly_one = e.number == "1" || e.number == "-1";
    let value = if terms.is_empty() {
        number
    } else {
        format!(
            "{number} {}",
            unit_text(&terms, if exactly_one { 1 } else { 2 }, texts)
        )
    };
    texts.t_with(
        if e.approx {
            "calc-equals-approx"
        } else {
            "calc-equals-exact"
        },
        &[("value", &value)],
    )
}

/// A kept answer as a sum the engine reads again: its number in the
/// language's mark, then its unit's symbols.
fn typed_again(e: &Entry, mark: char) -> String {
    let number = entry_number(e, mark);
    let symbols: Vec<String> = e
        .unit
        .iter()
        .filter_map(|t| {
            base_of(t).map(|b| {
                let s = crate::calc_convert::symbol_of(b);
                if t.power == 1 {
                    s
                } else {
                    format!("{s}^{}", t.power)
                }
            })
        })
        .collect();
    if symbols.is_empty() {
        number
    } else {
        format!("{number} {}", symbols.join(" "))
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "calc",
    commands: &[
        // THE CALCULATOR, `LEADER =` — the footer's door and the window's
        // (`calc`, `calc_window`).
        ronoctua_cmd::Command {
            id: "calc-open",
            label: crate::key::CMD_CALC_OPEN,
        },
        ronoctua_cmd::Command {
            id: "calc-window",
            label: crate::key::CMD_CALC_WINDOW,
        },
    ],
    bindings: &[
        // `=` — THE CALCULATOR, the key a sum ends with: the doubled key is
        // the footer's door, `w` the window's.
        ("= =", "calc-open", None),
        ("= w", "calc-window", None),
    ],
    groups: &[("=", crate::key::GROUP_CALC)],
    ..crate::bindings::Contribution::NONE
};

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_calc::Quantity;

    fn quantity(ids: &[&str]) -> Quantity {
        Quantity {
            terms: ids
                .iter()
                .map(|id| {
                    (
                        Base::Unit(ronoctua_calc::units::table().by_id(id).unwrap()),
                        1,
                    )
                })
                .collect(),
        }
    }

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn view() -> View {
        let mut v = View::fresh(Vec::new(), ronoctua_files::when::Zone::utc(), None);
        v.at(1_790_769_600_000);
        v
    }

    /// Keys typed, and what a door does with an answer kept: the file takes
    /// it, and the list is what the file says.
    fn typed(v: &mut View, keys: &str, t: &ronoctua_i18n::Texts) -> Vec<Errand> {
        keys.split_whitespace()
            .map(|k| {
                let key = Key::parse(k).unwrap();
                let errand = v.press(press_of_key(key).expect(k), t);
                if let Errand::Keep(e) = &errand {
                    let mut kept = v.history().to_vec();
                    kept.push(e.clone());
                    v.history_is(kept);
                }
                errand
            })
            .collect()
    }

    /// Every word of a view's lines, joined.
    fn words(lines: &[Line]) -> String {
        lines
            .iter()
            .flat_map(|l| l.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// FAILS IF: an answer the file refused is drawn as kept — the list is
    /// what the file says, and `RET` alone puts nothing in it.
    #[test]
    fn an_answer_the_file_refused_is_not_listed() {
        let t = texts("en-GB");
        let mut v = view();
        let errand = v.press(Press::Type("2".into()), &t);
        assert_eq!(errand, Errand::Nothing);
        let errand = v.press(Press::Enter, &t);
        assert!(matches!(errand, Errand::Keep(_)));
        // THE DOOR'S WRITE FAILED: it never called `history_is`.
        assert_eq!(v.kept(), 0);
        assert!(!words(&v.history_lines(40, &t)).contains("= 2"));
    }

    /// FAILS IF: the row that names the rates' host can only be reached by a
    /// pointer — `C-a` is its key, taught while it is drawn and only then.
    #[test]
    fn the_consent_is_reached_by_the_keyboard() {
        let t = texts("pt-BR");
        let mut v = view();
        // NOTHING ASKS yet: the key does nothing, and is not taught.
        assert_eq!(typed(&mut v, "C-a", &t), vec![Errand::Nothing]);
        assert!(!v.taught(',').asks);
        typed(&mut v, "1 0 SPC U S D SPC e m SPC E U R", &t);
        v.looked(
            Probe {
                cached: None,
                allowed: false,
            },
            &t,
        );
        assert!(
            v.taught(',').asks,
            "the consent's row is drawn and not taught"
        );
        let taught = hint(v.taught(','), false, &t);
        assert!(
            taught.iter().any(|p| p.id == Some("consent-allow")),
            "{taught:?}"
        );
        assert_eq!(typed(&mut v, "C-a", &t), vec![Errand::Allow]);
        // AND AFTER A FAILURE, the same key asks again.
        v.allowed();
        v.rates_arrived(Err(ronoctua_calc::rates::Why::Unreachable), &t);
        assert_eq!(typed(&mut v, "C-a", &t), vec![Errand::Allow]);
    }

    /// FAILS IF: a row of a list takes a click only on its words — the
    /// band the pointer lights is the whole row, and every cell of it takes
    /// the row, in both doors (they read the same `Line`).
    #[test]
    fn a_row_is_taken_anywhere_on_its_band() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "1 2 + 3 0 RET C-DEL 7 RET", &t);
        let lines = v.history_lines(60, &t);
        let row = lines
            .iter()
            .find(|l| l.bands.iter().any(|b| b.2 == Target::History(1)))
            .expect("the older answer's row");
        let (from, to, _) = row.bands[0];
        assert_eq!(from, 0);
        for cell in from..to {
            assert_eq!(row.target_at(cell), Some(Target::History(1)), "cell {cell}");
            assert_eq!(row.band_at(cell), Some((from, to)), "cell {cell}");
        }
        // THE BAR'S CELL IS NOT THE ROW'S.
        assert_eq!(row.target_at(60 - crate::chrome::BAR_CELLS), None);
        // A FIELD OF A LINE OF FIELDS lights its own cells — what it takes.
        let modes = &v.top_lines(80, &t)[0];
        let at = modes.fields[1].at;
        let band = modes.band_at(at).expect("a mode lights");
        for cell in band.0..band.1 {
            assert_eq!(modes.target_at(cell), Some(Target::Mode(Mode::Scientific)));
        }
    }

    /// FAILS IF: a mode's name is dropped when the line is narrow — a mode
    /// not drawn is a mode the pointer cannot reach.
    #[test]
    fn every_mode_is_drawn_at_every_width() {
        for lang in ["pt-BR", "en-GB"] {
            let t = texts(lang);
            let v = view();
            for columns in [10u16, 16, 24, 40, 80] {
                let lines = v.top_lines(columns, &t);
                for m in Mode::ALL {
                    assert!(
                        lines
                            .iter()
                            .any(|l| l.targets.contains(&Some(Target::Mode(m)))),
                        "{lang} {columns}: {m:?} is not drawn"
                    );
                }
            }
        }
    }

    /// FAILS IF: the converter says an amount is wrong while it is typed —
    /// `12+` is half a sum — or does not say it once `RET` took a unit.
    #[test]
    fn the_amount_is_refused_only_when_asked_for() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "TAB TAB 1 2 +", &t);
        let refusal = t.t(&format!(
            "calc-why-{}",
            v.live().unwrap().as_ref().unwrap_err().why.id()
        ));
        assert!(
            !words(&v.convert_lines(80, &t)).contains(&refusal),
            "said while typing"
        );
        typed(&mut v, "RET", &t);
        assert!(words(&v.convert_lines(80, &t)).contains(&refusal));
        typed(&mut v, "3", &t);
        assert!(!words(&v.convert_lines(80, &t)).contains(&refusal));
    }

    /// FAILS IF: an answer kept is out of the pointer's reach in the footer —
    /// beside the keys, every answer is a row of the region, which its own
    /// scroll, wheel and bar reach.
    #[test]
    fn the_footer_reaches_every_answer() {
        let t = texts("en-GB");
        let mut v = view();
        for n in 1..=12 {
            v.at(1_790_769_600_000 + n);
            let digits: Vec<String> = n.to_string().chars().map(String::from).collect();
            typed(&mut v, &format!("C-DEL {} RET", digits.join(" ")), &t);
        }
        let lines = v.footer_lines(160, &t);
        for i in 0..12 {
            assert!(
                lines
                    .iter()
                    .any(|l| l.bands.iter().any(|b| b.2 == Target::History(i))),
                "answer {i} is not in the footer"
            );
        }
        // AND BESIDE THE KEYS: the newest shares a line with a key.
        let newest = lines
            .iter()
            .find(|l| l.bands.iter().any(|b| b.2 == Target::History(0)))
            .unwrap();
        assert!(
            newest
                .targets
                .iter()
                .any(|x| matches!(x, Some(Target::Button(_))))
        );
        // THE PICK IS FOLLOWED to its line.
        typed(&mut v, "C-p C-p C-p C-p C-p C-p C-p C-p C-p C-p", &t);
        let at = v.footer_cursor(160, &t).unwrap();
        assert!(lines[at].bands.iter().any(|b| b.2 == Target::History(9)));
    }

    /// FAILS IF: a sum typed is not answered, `RET` does not keep it, or the
    /// answer is not the next sum's start.
    #[test]
    fn a_sum_is_answered_kept_and_goes_on() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "2 + 2", &t);
        assert!(matches!(v.live(), Some(Ok(_))));
        let head = v.top_lines(80, &t);
        let said: Vec<String> = head
            .iter()
            .flat_map(|l| l.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(said.iter().any(|s| s.contains("= 4")), "{said:?}");
        let errands = typed(&mut v, "RET", &t);
        assert!(matches!(&errands[0], Errand::Keep(e) if e.expression == "2+2" && e.number == "4"));
        assert_eq!(v.text(), "4");
        typed(&mut v, "* 3 RET", &t);
        assert_eq!(v.kept(), 2);
        assert_eq!(v.newest_first().next().unwrap().number, "12");
        // A DIGIT OVER AN ANSWER starts a new sum; a sign went on from it.
        typed(&mut v, "5", &t);
        assert_eq!(v.text(), "5");
    }

    /// FAILS IF: a sum with no answer says nothing after `RET`, or says it
    /// while it is still being typed.
    #[test]
    fn a_refusal_is_said_only_when_asked_for() {
        let t = texts("pt-BR");
        let mut v = view();
        typed(&mut v, "7 / 0", &t);
        let words = |v: &View| -> String {
            v.top_lines(80, &t)
                .iter()
                .flat_map(|l| l.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let refusal = t.t("calc-why-divide-by-zero");
        assert!(!words(&v).contains(&refusal), "said while typing");
        typed(&mut v, "RET", &t);
        assert!(words(&v).contains(&refusal), "{}", words(&v));
        assert_eq!(v.kept(), 0);
        // AND A CHANGE TAKES IT AWAY.
        typed(&mut v, "DEL", &t);
        assert!(!words(&v).contains(&refusal));
    }

    /// FAILS IF: the history does not recall on the second click or `RET`,
    /// or the first click acts.
    #[test]
    fn the_history_is_picked_then_recalled() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "1 + 1 RET", &t);
        v.at(1_790_769_601_000);
        typed(&mut v, "C-DEL 3 * 3 RET", &t);
        assert_eq!(v.clicked(Target::History(1), &t), Errand::Nothing);
        assert_eq!(v.picked(), Some(1));
        assert_eq!(v.text(), "9", "the first click picks and does nothing else");
        v.clicked(Target::History(1), &t);
        assert_eq!(v.text(), "1+1");
        // THE KEYBOARD REACHES IT TOO.
        typed(&mut v, "C-p RET", &t);
        assert_eq!(v.text(), "3*3");
        // AND COPIES IT.
        typed(&mut v, "C-p", &t);
        assert_eq!(v.copy_text(&t).as_deref(), Some("9"));
    }

    /// FAILS IF: two doors keeping answers into one file lose one, the same
    /// answer is kept twice, the bound is not kept, or an unparseable file is
    /// overwritten.
    #[test]
    fn the_file_is_merged_bounded_and_never_overwritten_unparsed() {
        let dir = std::env::temp_dir().join(format!("ronoctua-calc-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = ronoctua_store::Store::<Kept>::at(Some(dir.clone()), FILE);
        let e = |at_ms: u64| Entry {
            at_ms,
            expression: format!("{at_ms}"),
            number: format!("{at_ms}"),
            approx: false,
            unit: Vec::new(),
        };
        keep_in(&store, e(2)).unwrap();
        keep_in(&store, e(1)).unwrap();
        let all = keep_in(&store, e(2)).unwrap();
        assert_eq!(all.iter().map(|x| x.at_ms).collect::<Vec<_>>(), vec![1, 2]);
        for n in 10..(10 + ronoctua_calc::HISTORY_ENTRIES as u64 + 5) {
            keep_in(&store, e(n)).unwrap();
        }
        let all = store.parsed().unwrap().history;
        assert_eq!(all.len(), ronoctua_calc::HISTORY_ENTRIES);
        assert!(all.iter().all(|x| x.at_ms >= 10), "the oldest go first");
        std::fs::write(dir.join(FILE), "{ not json").unwrap();
        assert!(matches!(keep_in(&store, e(99)), Err(Unkept::Unparsed)));
        assert_eq!(
            std::fs::read_to_string(dir.join(FILE)).unwrap(),
            "{ not json"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the keypad's keys and the keyboard's disagree — a key's
    /// press, a click on the footer's field of the same key, and typing it
    /// give three different sums.
    #[test]
    fn a_key_clicked_is_the_key_typed() {
        let t = texts("pt-BR");
        let mut by_click = view();
        by_click.mode = Mode::Scientific;
        let lines = by_click.footer_lines(120, &t);
        let target = |b: Button| {
            lines
                .iter()
                .find_map(|l| {
                    l.fields.iter().zip(&l.targets).find_map(|(f, tg)| {
                        (*tg == Some(Target::Button(b))).then(|| l.target_at(f.at + 1))
                    })
                })
                .flatten()
                .unwrap_or_else(|| panic!("{b:?} is not on the footer's keypad"))
        };
        for b in [
            Button::Function(ronoctua_calc::units::Function::Sqrt),
            Button::Digit(9),
            Button::Close,
        ] {
            let tg = target(b);
            by_click.clicked(tg, &t);
        }
        assert_eq!(by_click.text(), "√(9)");
        let mut by_key = view();
        typed(&mut by_key, "√ ( 9 )", &t);
        assert_eq!(by_key.text(), by_click.text());
        assert!(matches!(by_click.live(), Some(Ok(_))));
    }

    /// FAILS IF: the converter's letters go to the sum, `RET` does not take
    /// the unit under the cursor, or the live value is not the amount in the
    /// To unit.
    #[test]
    fn the_converter_finds_takes_and_converts() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "TAB TAB", &t);
        assert_eq!(v.mode, Mode::Convert);
        typed(&mut v, "1 0", &t);
        typed(&mut v, "C-<up> m i l e RET", &t);
        assert_eq!(
            v.text(),
            "10",
            "the letters found a unit, and left the amount"
        );
        assert_eq!(
            v.convert.from,
            Base::Unit(ronoctua_calc::units::table().by_id("mile").unwrap())
        );
        typed(&mut v, "k i l o m e t r e RET", &t);
        let Some(Ok(value)) = v.live() else {
            panic!("{:?}", v.live());
        };
        assert_eq!(ronoctua_calc::format(value, '.', 6), "16.09344");
        assert_eq!(value.unit, Some(quantity(&["kilometre"])));
        // THE SWAP, from its glyph.
        let lines = v.convert_lines(80, &t);
        let swap = lines
            .iter()
            .find_map(|l| {
                l.targets
                    .iter()
                    .position(|x| *x == Some(Target::Swap))
                    .map(|i| l.fields[i].at)
            })
            .expect("a swap on the From line");
        let _ = swap;
        v.clicked(Target::Swap, &t);
        let Some(Ok(back)) = v.live() else { panic!() };
        assert_eq!(ronoctua_calc::format(back, '.', 4), "6.2137");
    }

    /// FAILS IF: the first click on the rates' row lets the host — the row
    /// comes up under the pointer that chose a currency, and a double click
    /// let it — or the first click does not show the row picked, or a click
    /// elsewhere between the two leaves it picked (invariant 19).
    #[test]
    fn the_rates_row_is_picked_by_one_click_and_let_by_the_second() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "1 0 SPC U S D SPC i n SPC E U R", &t);
        assert!(v.to_look('.'));
        let _ = v.looked(
            Probe {
                cached: None,
                allowed: false,
            },
            &t,
        );
        assert_eq!(v.money(), &Money::Asking);
        let tone = |v: &View| {
            v.top_lines(80, &t)
                .into_iter()
                .find(|l| l.targets.contains(&Some(Target::Allow)))
                .and_then(|l| l.fields.first().map(|f| f.color))
        };
        let before = tone(&v);
        assert_eq!(
            v.clicked(Target::Allow, &t),
            Errand::Nothing,
            "the first click let the host"
        );
        assert_ne!(
            tone(&v),
            before,
            "the first click did not show the row picked"
        );
        // A CLICK ELSEWHERE LETS GO of the row picked.
        let _ = v.clicked(Target::Swap, &t);
        assert_eq!(
            v.clicked(Target::Allow, &t),
            Errand::Nothing,
            "a click elsewhere left the row picked"
        );
        assert_eq!(
            v.clicked(Target::Allow, &t),
            Errand::Allow,
            "the second click let nothing"
        );
    }

    /// FAILS IF: a currency conversion asks the host before the person let
    /// it, or does not ask once they did — and stale rates are not drawn
    /// while fresh ones come.
    #[test]
    fn currencies_ask_first_and_fetch_once_let() {
        let t = texts("en-GB");
        let mut v = view();
        typed(&mut v, "1 0 SPC U S D SPC i n SPC E U R", &t);
        assert!(v.to_look('.'));
        let e = v.looked(
            Probe {
                cached: None,
                allowed: false,
            },
            &t,
        );
        assert_eq!(e, Errand::Nothing);
        assert_eq!(v.money(), &Money::Asking);
        let rows = v.top_lines(80, &t);
        assert!(
            rows.iter()
                .any(|l| l.targets.contains(&Some(Target::Allow))),
            "no row names the host"
        );
        // THE FIRST CLICK PICKS THE ROW, and lets nothing (invariant 19).
        assert_eq!(v.clicked(Target::Allow, &t), Errand::Nothing);
        assert_eq!(
            v.money(),
            &Money::Asking,
            "the first click asked for the rates"
        );
        assert_eq!(v.clicked(Target::Allow, &t), Errand::Allow);
        v.allowed();
        assert_eq!(v.money(), &Money::Fetching);
        let rates = ronoctua_calc::rates::parse(RATES).unwrap();
        v.rates_arrived(Ok(rates), &t);
        let Some(Ok(value)) = v.live() else {
            panic!("{:?}", v.live());
        };
        assert_eq!(ronoctua_calc::format(value, '.', 2), "5");
        // A LOOK THAT FINDS THE HOST LET FETCHES AT ONCE.
        let mut w = view();
        typed(&mut w, "1 SPC U S D SPC i n SPC E U R", &t);
        let e = w.looked(
            Probe {
                cached: None,
                allowed: true,
            },
            &t,
        );
        assert_eq!(e, Errand::Fetch);
    }

    /// The ECB's daily file, two currencies: 1 € = 2 USD.
    const RATES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
	<Cube>
		<Cube time="2026-09-30">
			<Cube currency="USD" rate="2"/>
			<Cube currency="BRL" rate="6"/>
		</Cube>
	</Cube>
</gesmes:Envelope>"#;

    /// FAILS IF: an answer kept loses its unit, or reads back in another
    /// language's mark.
    #[test]
    fn an_answer_kept_reads_in_the_language_of_the_day() {
        let pt = texts("pt-BR");
        let mut v = view();
        typed(&mut v, "1 , 5 SPC k m RET", &pt);
        let e = v.newest_first().next().unwrap().clone();
        assert_eq!(e.number, "1.5");
        assert_eq!(e.unit[0].base, "kilometre");
        assert_eq!(v.text(), "1,5 km");
        let lines = v.history_lines(40, &pt);
        let said: String = lines[1].fields.iter().map(|f| f.text.clone()).collect();
        assert!(said.contains("1,5 quilômetros"), "{said}");
        let en = texts("en-GB");
        let lines = v.history_lines(40, &en);
        let said: String = lines[1].fields.iter().map(|f| f.text.clone()).collect();
        assert!(said.contains("1.5 kilometres"), "{said}");
    }

    /// FAILS IF: a line's width runs past the columns it was given — a note
    /// is wrapped, a value cut, at every width.
    #[test]
    fn nothing_runs_past_the_room() {
        for lang in ["pt-BR", "en-GB"] {
            let t = texts(lang);
            for mode in Mode::ALL {
                let mut v = view();
                v.mode = mode;
                typed(&mut v, "1 2 3 4 5 6 7 8 9 * 9 8 7 6 5 4 3 2 1 RET", &t);
                typed(&mut v, "7 / 0 RET", &t);
                for columns in [20u16, 30, 44, 80, 140] {
                    for l in v.footer_lines(columns, &t) {
                        for f in &l.fields {
                            let end = f.at + ronoctua_render::layout::width(&f.text);
                            assert!(
                                end <= columns,
                                "{lang} {mode:?} {columns}: `{}` ends at {end}",
                                f.text
                            );
                        }
                        for (_, to, _) in &l.bands {
                            assert!(*to <= columns, "{lang} {mode:?} {columns}: a band to {to}");
                        }
                    }
                }
            }
        }
    }

    /// FAILS IF: a word the view draws is missing from a language.
    #[test]
    fn every_word_is_in_every_language() {
        for lang in ["pt-BR", "en-GB"] {
            let t = texts(lang);
            for m in Mode::ALL {
                assert!(t.has(m.key()), "{lang}: {}", m.key());
            }
            for key in [
                "calc-sum",
                "calc-amount",
                "calc-empty",
                "calc-history-title",
                "calc-history-empty",
                "calc-from",
                "calc-to",
                "calc-find",
                "calc-find-none",
                "calc-rates-for",
                "calc-rates-asking",
                "calc-rates-of",
                "calc-equals-exact",
                "calc-equals-approx",
                "calc-category-currency",
                "calc-keys-narrow",
            ] {
                assert!(t.has(key), "{lang}: {key}");
            }
            for (_, id) in OF_THE_VIEW
                .iter()
                .chain(OF_THE_CONVERTER)
                .chain(OF_THE_WINDOW)
                .chain(OF_THE_CONSENT)
            {
                assert!(t.has(id), "{lang}: the hint's word `{id}`");
                assert!(t.has(&format!("about-{id}")), "{lang}: `about-{id}`");
            }
            for why in ronoctua_calc::Why::ALL {
                assert!(
                    t.has(&format!("calc-why-{}", why.id())),
                    "{lang}: {}",
                    why.id()
                );
            }
        }
    }
}
