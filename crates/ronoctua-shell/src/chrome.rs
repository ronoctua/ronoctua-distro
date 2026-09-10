//! Where content begins, on every surface this shell draws.
//!
//! # Why this module exists
//!
//! Because the same view was drawn at two different distances from the screen's
//! edge depending on which door it came through, and nothing anywhere said so.
//! The owner reported it from the screen: *"when something new is drawn in the
//! minibuffer region it comes out glued to the edge and has different spacing
//! from the same thing in its own window; there is no standard at all."*
//!
//! He was right, and the reason was that there was no single number to be
//! right about. The footer's expanded region indented its field rows by
//! `space.files.gutter` — one cell, which is the MARK's cell and not a margin
//! at all — while both own-window doors added `space.content.inset` minus that
//! same gutter on top. One cell against two — and at the time the two doors
//! did not even have the same cell: the footer drew at `font.size.base` and a
//! window at `font.size.large`, ten point eight pixels against twelve.
//!
//! # The rule, in one sentence
//!
//! **Content is inset by [`INSET`] cells on every surface. The modeline and the
//! minibuffer are not content and keep `space.modeline.edgeLeft`.**
//!
//! That exception is not a leak in the rule, it is the rule's other half, and
//! `space.nix` argues it where the token lives: those two lines are a status
//! line — peripheral reference one glances at — and two cells there "would
//! start to look like a margin". Everything else on screen is something one
//! reads, and it gets the inset.
//!
//! **The exception has exactly four sites, and they are named here so a fifth
//! has to be argued for**: the minibuffer's line (`surface.rs`, the one
//! `column: MODELINE_EDGELEFT` left in that file), the modeline's segments
//! (`ronoctua_render::layout`), the notification band (`band.rs`) and the
//! brain's line (`brain.rs`). The band is on the list because it is the same
//! kind of thing as the modeline — one line of peripheral reference, above the
//! pair, that one glances at and does not read; the brain's line is the same
//! kind of thing one row below, and it was argued for on exactly those terms.
//! The notification MANAGER is not: it is a list in the expanded region, and
//! it takes the inset like every other list.
//!
//! # What is left over, and why it is nothing
//!
//! Nothing is left over, and a fifth of a cell used to be: two cells of
//! `font.size.base` are 21,6 px and two of `font.size.large` were 24,0 px.
//! That residue belonged to the GRID and not to the layout, because the two
//! doors drew text at different sizes on purpose, and the test kept it inside
//! one cell so it could not grow back into a factor of two.
//!
//! The owner has since asked for ONE size across the system — see
//! `ronoctua_tokens::text::size` —
//! so the residue is zero and
//! [`tests::both_doors_put_content_in_the_same_place`] says so exactly, with
//! no tolerance at all.

use ronoctua_tokens as tokens;

/// How far in content starts, in cells, on every surface.
///
/// One number, and the reason it is re-exported here rather than read from
/// `tokens::space` at each site is the module header: the sites that got this
/// wrong got it wrong by reaching for a DIFFERENT token, and a name that says
/// `chrome::INSET` is a name somebody has to deliberately not use.
pub const INSET: u16 = tokens::space::CONTENT_INSET;

/// The inset owed by a row that already carries its own first cell.
///
/// A file manager's row, a notification's row and a transfer's row all begin
/// at `space.files.gutter` — the mark's cell, and the air an icon needs so its
/// glyph does not touch the border and read as clipped. That cell is part of
/// the TABLE, not part of the margin, so a surface drawing such a row owes the
/// difference and not the whole inset.
///
/// Both windows worked this out for themselves and agreed; the footer never
/// did it at all, which is exactly the defect. One constant, three readers.
pub const FIELDS_LEAD: u16 = INSET.saturating_sub(tokens::space::FILES_GUTTER);

/// How wide a cell is, in pixels.
///
/// Every hit test on every surface comes through here or through
/// [`Frame::cell`]. One answer, so a site that reached for a font size
/// directly would be a second grid nobody named.
#[must_use]
pub fn cell_width() -> f32 {
    tokens::metrics::cell_width(tokens::text::size())
}

/// How many pixels from the surface's left edge a row of fields begins.
///
/// # Why this is behind `cfg(test)`
///
/// Because nothing in the drawing needs it, and that is the point rather than
/// an omission. The layout is integer arithmetic over the CELL — the whole
/// footer's one-frame budget is bought with that — so production counts cells
/// and never pixels.
///
/// The pixel value exists for exactly one question: whether the surfaces put
/// content in the same place. With one size that is now arithmetic rather than
/// a measurement, and the test that asks it says so.
#[cfg(test)]
#[must_use]
pub fn content_left_px() -> f32 {
    f32::from(FIELDS_LEAD + tokens::space::FILES_GUTTER) * cell_width()
}

// ---------------------------------------------------------------------------
// THE FRAME — where a surface ends, in the two units it is measured in
// ---------------------------------------------------------------------------

/// A surface's measurements, carried instead of recomputed.
///
/// # Why this exists
///
/// Because the same numbers were worked out at each site that needed them, and
/// the sites disagreed. Two units are in play and both are legitimate:
///
/// - **the grid** — `u16` cells and lines, which is where TEXT goes
///   (`ronoctua_render::Span` carries a `column` and a `line` and nothing else);
/// - **the surface** — `f32` pixels, which is where EDGES go: rules, dividers,
///   hover bands, a scrollbar's track and thumb, a picture's box.
///
/// They coincide only when the surface is an exact multiple of the cell and the
/// line, and it never is: `columns` is `floor(width / cell)`, so a remainder of
/// less than one cell sits at the right border and less than one line at the
/// bottom. Every geometry defect this file has been asked to fix lived in that
/// remainder — a rule that stopped short, a scrollbar drawn past the border, a
/// strip of blank pixels under a panel that changed size with the window.
///
/// So the conversion happens **here**, once, and the two units stop being a
/// convention that each site has to keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The surface, in pixels. Never a count of anything.
    pub width: f32,
    /// The surface's height, in pixels — what the BUFFER can show, which is
    /// the number the compositor gave except where the GPU's texture ceiling
    /// cut it (see [`crate::hidpi::resized`]). The shell still does not choose
    /// it (invariant 4): it is told twice over, once by the compositor and
    /// once by the hardware, and this is where the two have already been
    /// reconciled.
    pub height: f32,
    /// The region's first line, in the grid.
    pub first: u16,
}

impl Frame {
    /// A frame at this surface.
    #[must_use]
    pub const fn of(width: f32, height: f32, first: u16) -> Self {
        Self {
            width,
            height,
            first,
        }
    }

    /// The arithmetic, which lives in the design system.
    ///
    /// # Why the conversion is not written here any more
    ///
    /// Because it is not this door's, nor this shell's: it is the same
    /// conversion for the footer, for each window, and for anything drawn
    /// later. It was written out here and had begun to be written out again
    /// beside its callers — which is the shape every defect in this file
    /// started as.
    ///
    /// What stays on this side is what a cell and a line MEASURE.
    /// `ronoctua-design` takes those two as values because it has no tokens
    /// and may not have any (invariant 11).
    fn rail(self) -> ronoctua_design::rails::Frame {
        ronoctua_design::rails::Frame::of(
            cell_width(),
            tokens::metrics::line_height(tokens::text::size()),
            self.width,
            self.height,
            self.first,
        )
    }

    /// How wide a cell is, in pixels.
    #[must_use]
    pub fn cell(self) -> f32 {
        self.rail().cell
    }

    /// How tall a line is, in pixels.
    #[must_use]
    pub fn line(self) -> f32 {
        self.rail().line
    }

    /// How many whole cells the surface holds.
    #[must_use]
    pub fn columns(self) -> u16 {
        self.rail().columns()
    }

    /// How many whole lines the surface holds.
    #[must_use]
    pub fn lines(self) -> u16 {
        self.rail().lines()
    }

    /// The pixel a column begins at, counted from the LEFT.
    #[must_use]
    pub fn x_of(self, column: u16) -> f32 {
        self.rail().x_of(column)
    }

    /// The pixel a line begins at.
    #[must_use]
    pub fn y_of(self, line: u16) -> f32 {
        self.rail().y_of(line)
    }

    /// The surface's bottom edge, in pixels. The TRUE one.
    #[must_use]
    pub const fn bottom(self) -> f32 {
        self.height
    }

    /// Which column a pixel is in.
    #[must_use]
    pub fn column_at(self, x: f32) -> u16 {
        self.rail().column_at(x)
    }

    /// Which line a pixel is on, or `None` above the surface.
    #[must_use]
    pub fn line_at(self, y: f32) -> Option<u16> {
        self.rail().line_at(y)
    }
}

// ---------------------------------------------------------------------------
// SEPARATORS — the second defect the owner photographed, and the same cause
// ---------------------------------------------------------------------------

/// A rule across the whole surface.
///
/// # Why this takes the surface's WIDTH and not a rectangle
///
/// Because every site that got this wrong got it wrong the same way, and the
/// shape of the mistake is worth naming: they wrote `x: margin * cell` and
/// `width: self.width`. That is an inset on the left and the FULL width on the
/// right — a rule that starts one cell in, runs past the right edge and is
/// clipped there. On screen it reads as a line glued to the right border with
/// a gap on the left, which is exactly what the owner reported and what
/// `12-update-window.png` shows at `y=377`, starting at `x=12`.
///
/// The owner's rule is *"sangrada, de borda a borda"* — bled, edge to edge —
/// and it is decided once, here. A caller hands in how wide the surface is and
/// which line the rule sits on; it cannot express an inset, because there is
/// no parameter for one.
///
/// The Y is the caller's, in pixels, because the Y was never the defect: every
/// one of these sat on the right row and started in the wrong column.
#[must_use]
pub fn rule(width_px: f32, y_px: f32) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        x: 0.0,
        y: y_px,
        width: width_px,
        height: f32::from(tokens::space::RULE_THICKNESS),
        color: tokens::color::RULE_DEFAULT,
    }
}

/// A vertical separator between two columns, from a top edge to the surface's
/// BOTTOM.
///
/// # Why it takes the bottom and not a height
///
/// Same defect, turned ninety degrees. The five sites that drew one computed
/// `height: lines * line_height`, and a line height is fractional — 26,5 px
/// here — so the product lands short of where the surface actually ends. In
/// `10-manager-with-preview.png` all three dividers stop at `y=798` on an
/// 800-pixel surface, and on the owner's own screen the shortfall is a whole
/// row: *"the vertical line that separates the preview from the info is also
/// not touching the bottom of the screen, which is wrong"*.
///
/// A count of lines is a guess at where the bottom is. The bottom is not a
/// guess, so this asks for it.
#[must_use]
pub fn divider(x_px: f32, from_px: f32, bottom_px: f32) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        x: x_px,
        y: from_px,
        width: f32::from(tokens::space::RULE_THICKNESS),
        height: (bottom_px - from_px).max(0.0),
        color: tokens::color::RULE_DEFAULT,
    }
}

/// The same divider, for the section that has the KEYBOARD.
///
/// # Why the focus mark is a rule and not something new
///
/// Because everything else was tried and refused. A one-line TITLE per panel
/// cost a line of the panel and was text where a tone would do; the panel's
/// own SCROLLBAR, lit, was the owner's second refusal. What is left that is
/// already drawn beside a section is the rule that begins it — so the mark is
/// that rule, in the colour this palette reserves for focus.
///
/// It is one logical pixel wide, which is what makes it quiet enough to be
/// worth trying where a whole lit cell was not.
///
/// Here and not at the call site for the reason [`divider`] is here: the two
/// doors had three different answers about a separator between them, and
/// `separators-come-from-chrome` refuses a fourth.
#[must_use]
pub fn divider_lit(x_px: f32, from_px: f32, bottom_px: f32) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        color: tokens::color::STATE_ACCENT,
        ..divider(x_px, from_px, bottom_px)
    }
}

// ---------------------------------------------------------------------------
// THE HINT LINE — one sentence, two registers, one decision
// ---------------------------------------------------------------------------

/// What a run of a hint line IS, which is the same as what colour it is.
///
/// Four and not two, and every one of them was asked for from the screen.
/// `Key` and `Prose` are the original pair — what the eye hunts for and what
/// it means. `Separator` is the dot between one shortcut and the next: the
/// owner asked for it to differ from both, and punctuation drawn in the tone
/// of the words it separates is read as a word. `Alternate` is a key whose
/// MEANING follows the section that has the keyboard — *"faça com que esses
/// atalhos que mudam de acordo com a seção em foco tenham cores diferentes dos
/// demais atalhos listados"*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    /// A key notation that means the same thing wherever one is.
    Key,
    /// A key notation whose meaning is the focused section's, and which
    /// therefore changes under the reader without the reader pressing anything.
    Alternate,
    /// The words around a key.
    Prose,
    /// The mark between one shortcut and the next.
    Separator,
}

impl Register {
    /// The one place that says what colour a run of a hint is.
    #[must_use]
    pub const fn color(self) -> ronoctua_tokens::Color {
        match self {
            Self::Key => tokens::color::TEXT_SUBTLE_STRONG,
            Self::Alternate => tokens::color::TEXT_SUBTLE_ALTERNATE,
            Self::Prose => tokens::color::TEXT_SUBTLE_DEFAULT,
            Self::Separator => tokens::color::TEXT_SUBTLE_FAINT,
        }
    }
}

/// One run of a hint line: the text, and which register it belongs to.
///
/// A hint reads "type to filter · C-m action menu", and the parts of that
/// answer different questions: the key is what the eye hunts for, the prose is
/// what it means, the dot is neither. Where a sentence is one `.ftl` value the
/// split cannot happen in Rust — the ORDER is the language's, and what comes
/// back from the localiser is the only thing that knows where it put the
/// argument (`ronoctua_i18n::Marked`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The run, exactly as it reads.
    pub text: String,
    /// Which of the four registers it is drawn in.
    pub register: Register,
}

impl Piece {
    /// A run of prose.
    #[must_use]
    pub fn prose(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            register: Register::Prose,
        }
    }

    /// A key notation, fixed or following the focused section.
    #[must_use]
    pub fn key(text: impl Into<String>, alternate: bool) -> Self {
        Self {
            text: text.into(),
            register: if alternate {
                Register::Alternate
            } else {
                Register::Key
            },
        }
    }

    /// The mark between one shortcut and the next, with its air on both sides.
    ///
    /// # Why the spaces travel with the dot
    ///
    /// Because they are the dot's air and nobody else's. Written into the
    /// neighbouring runs they would be drawn in the neighbour's colour, which
    /// costs nothing today and would be a bug the first time a register grows
    /// a background. And they are what keeps `·` off the words: the owner's
    /// own photograph of the seam that lost its space says `facts· TAB`.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            text: format!(" {} ", tokens::mark::SEPARATOR),
            register: Register::Separator,
        }
    }
}

/// A hint line GENERATED from a view's key table, in the table's order.
///
/// The file manager's `View::hint` wrote this loop first, and its header says
/// why a hint is generated rather than written: a second list kept by hand
/// beside the table is a list that falls behind — and it had, in two doors
/// at once. Every view that has a table and no section-dependent keys calls
/// this; the file manager keeps its own loop because four of its keys change
/// meaning with the focused section and take the `Alternate` register.
///
/// An id bound twice shows once, under its FIRST notation, which makes the
/// table's order a teaching decision. The words are the id read as a TEXT
/// KEY (`notif-read` → "read"), the file manager's own convention — short,
/// because the line is one line; the long form is `cmd-<id>`, which the
/// manual and which-key read.
#[must_use]
pub fn hint_of(table: &[(&str, &str)], texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
    hint_of_with(table, |_| None, texts)
}

/// The same line, where SOME of the keys mean what the focused thing decides.
///
/// `means` is asked about every id in the table. `None` is the ordinary case:
/// the id is its own phrase and the key is drawn in the plain register. `Some`
/// is a key whose meaning MOVES under the reader without the reader pressing
/// anything — the file manager's `S-<arrows>`, which point wherever the focused
/// section points, and the notification manager's `SPC`, which reads or unreads
/// depending on the row it is on. Those are drawn in [`Register::Alternate`],
/// because a key that changes meaning has to say so.
///
/// # Why this is one function and not two
///
/// It was two: this loop, and a copy of it in `files::View::hint` written to
/// add exactly that. The copy drifted, and in the one way that matters — it
/// spent the id BEFORE the notation parsed, which is the defect the comment
/// below says was fixed here. In the copy a mistyped notation took its id off
/// the line for good, including a later row of the table that spelt the same
/// id correctly.
pub fn hint_of_with(
    table: &[(&str, &str)],
    means: impl Fn(&str) -> Option<&'static str>,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Piece> {
    let mut out = Vec::with_capacity(table.len() * 3);
    let mut said: Vec<&str> = Vec::with_capacity(table.len());
    for (notation, id) in table {
        if said.contains(id) {
            continue;
        }
        // THE ID IS SPENT ONLY ONCE THE NOTATION PARSES.
        //
        // It used to be pushed first: a notation this crate could not read —
        // a typo, a key spelt the compositor's way — silently suppressed that
        // id for the rest of the line, INCLUDING a later row of the same
        // table that spelt it correctly. So one bad row could take a working
        // key off the screen, and the only symptom was a sentence one item
        // shorter than the table.
        let Ok(t) = ronoctua_keymap::Key::parse(notation) else {
            continue;
        };
        said.push(id);
        if !out.is_empty() {
            out.push(Piece::separator());
        }
        let (says, alternate) = match means(id) {
            Some(other) => (other, true),
            None => (*id, false),
        };
        out.push(Piece::key(key_text(&t), alternate));
        // THE SPACE BETWEEN THE KEY AND ITS WORDS BELONGS TO THE WORDS, for
        // the reason `fold::wrap_pieces` gives about the other seam: a key
        // piece has to be exactly the notation, because that is what the
        // colour is about.
        out.push(Piece::prose(format!(" {}", texts.t(says))));
    }
    out
}

/// A moment, as a date a person reads.
///
/// # Why it is here and not written where it is needed
///
/// Because it was written where it was needed, twice, as a bare
/// `format!("{:04}-{:02}-{:02} {:02}:{:02}")` — a UI string in code, which
/// invariant 8 does not allow, and beside two `.ftl` keys (`files-when`,
/// `files-when-utc`) that had been declared for exactly this and were read by
/// nothing. Three views ask the question now, and a fourth reading of the same
/// clock would be a third answer waiting to differ.
///
/// The ORDER is the key's, so a language may write it its own way; the padding
/// is not, because a translation cannot pad and a column of dates that does not
/// line up is a column that cannot be scanned. And a zone that could not be
/// read says UTC rather than drawing a local time it cannot vouch for — the
/// reason [`ronoctua_files::when::Zone::is_utc`] exists.
/// `say_zone` is for a LINE, never for a COLUMN. When `/etc/localtime` could
/// not be read the date is UTC and saying so is the difference between a date
/// one can trust and one that is an hour out for ever — but the caveat is four
/// cells wide, and a measured column has no room it did not ask for. Where the
/// date is a column the caveat is what the pointer explains instead, which is
/// what the listing already does with this very key.
#[must_use]
pub fn when_text(
    secs: i64,
    zone: &ronoctua_files::when::Zone,
    say_zone: bool,
    texts: &ronoctua_i18n::Texts,
) -> String {
    // THE SHAPE IS THE PERSON'S, and it reaches here the way the theme
    // reaches a colour: through the process's current value, not through five
    // signatures. See `locale::prefs::current`.
    let choice = crate::locale::prefs::current();
    let date = date_text(secs, zone, choice.order, texts);
    let time = clock_text(secs, zone, choice.clock, texts);
    texts.t_with(
        if say_zone && zone.is_utc() {
            ronoctua_i18n::key::FILES_WHEN_UTC
        } else {
            ronoctua_i18n::key::FILES_WHEN
        },
        &[("date", date.as_str()), ("time", time.as_str())],
    )
}

/// A clock, in the form the person chose.
///
/// # Why the twelve-hour half is not a `.ftl` format string
///
/// Because `am`/`pm` is a WORD and the number in front of it is arithmetic:
/// `14` becomes `2`, and `0` becomes `12` rather than `0`. A key with
/// `{ $hh }:{ $mm } { $half }` would put that arithmetic in whoever calls it,
/// twice, and get the midnight case wrong once. Here the two halves are named
/// keys and the hour is computed in one place.
#[must_use]
pub fn clock_text(
    secs: i64,
    zone: &ronoctua_files::when::Zone,
    clock: crate::locale::Clock,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let c = ronoctua_files::when::civil(secs, zone);
    match clock {
        crate::locale::Clock::TwentyFour => texts.t_with(
            ronoctua_i18n::key::LOCALE_CLOCK_SAMPLE_24,
            &[
                ("hh", &format!("{:02}", c.hour)),
                ("mm", &format!("{:02}", c.minute)),
            ],
        ),
        crate::locale::Clock::Twelve => {
            // NOON AND MIDNIGHT ARE THE CASES THAT GO WRONG. `0` is twelve at
            // night and `12` is twelve at noon; a bare `hour % 12` gives zero
            // for both.
            let hour = match c.hour % 12 {
                0 => 12,
                h => h,
            };
            let half = texts.t(if c.hour < 12 {
                ronoctua_i18n::key::LOCALE_CLOCK_AM
            } else {
                ronoctua_i18n::key::LOCALE_CLOCK_PM
            });
            texts.t_with(
                ronoctua_i18n::key::LOCALE_CLOCK_SAMPLE_12,
                &[
                    ("hh", &hour.to_string()),
                    ("mm", &format!("{:02}", c.minute)),
                    ("half", &half),
                ],
            )
        }
    }
}

/// A date, in the order the person chose.
///
/// The ORDER is the key's, as it always was — a language may write its own —
/// and which of the three keys is asked for is the preference. The padding
/// stays in Rust for the reason `Civil::args` gives: a translation cannot pad,
/// and a column that does not line up cannot be scanned.
#[must_use]
pub fn date_text(
    secs: i64,
    zone: &ronoctua_files::when::Zone,
    order: crate::locale::Order,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let c = ronoctua_files::when::civil(secs, zone);
    let args = c.args();
    let args: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
    texts.t_with(
        match order {
            crate::locale::Order::BigEndian => ronoctua_i18n::key::LOCALE_DATE_YMD,
            crate::locale::Order::DayFirst => ronoctua_i18n::key::LOCALE_DATE_DMY,
            crate::locale::Order::MonthFirst => ronoctua_i18n::key::LOCALE_DATE_MDY,
        },
        &args,
    )
}

/// The name of the zone `/etc/localtime` points at — `Europe/London`.
///
/// # Why the symlink and not the file
///
/// Because a TZif file does not carry its own name: it is a table of
/// transitions, and the name is the PATH it was installed at. On NixOS
/// `/etc/localtime` is a symlink into the tzdata package, so the name is the
/// tail of the target after `zoneinfo/`. Where it is a copy rather than a
/// link — some containers do that — there is no name to be had, and saying so
/// is better than inventing one from the offset, which would call every
/// British winter `UTC`.
#[must_use]
pub fn zone_name() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let text = target.to_str()?;
    text.split_once("zoneinfo/")
        .map(|(_, name)| name.to_owned())
}

/// How a key is DRAWN, as against how it is written down.
///
/// # Why this is not `Key::notation`
///
/// Because they answer different questions and only one of them is read by a
/// person. `notation` is the wire form: `<down>`, `S-<left>` — round-trippable
/// through `Key::parse`, which is what a table and a `.ftl` need. On screen it
/// is six cells of markup, and a line that teaches forty-one keys would spend
/// twenty-four of them drawing angle brackets.
///
/// So the arrows are drawn as arrows, out of `mark::ARROW_*`, and everything
/// else keeps the notation it already had. One place, for the reason
/// `syntax_color` gives one module down: two places would answer differently
/// and the difference would be a shortcut printed one way in the hint and
/// another in the manual.
///
/// See [`split_at_the_modifier`] for the other half of "how a key is drawn":
/// the shaper's ligatures, which turned one of these into a key that does not
/// exist.
#[must_use]
pub fn key_text(t: &ronoctua_keymap::Key) -> String {
    use ronoctua_keymap::Sym;
    let arrow = match t.sym {
        Sym::Left => Some(tokens::mark::ARROW_LEFT),
        Sym::Right => Some(tokens::mark::ARROW_RIGHT),
        Sym::Up => Some(tokens::mark::ARROW_UP),
        Sym::Down => Some(tokens::mark::ARROW_DOWN),
        _ => None,
    };
    let Some(a) = arrow else {
        return t.notation();
    };
    // THE MODIFIERS STAY AS THEY ARE WRITTEN. `S-` is two cells and is the
    // same two cells everywhere else in the system; only the SYMBOL was
    // markup, and only the symbol changes. The four are the only syms
    // `notation` writes inside angle brackets, so the last `<` is the seam —
    // and cutting at a MEASURED position rather than a remembered length is
    // what keeps `<up>` (four bytes) and `<right>` (seven) from needing to be
    // listed here a second time.
    let mut out = t.notation();
    let Some(at) = out.rfind('<') else {
        return out;
    };
    out.truncate(at);
    out.push(a);
    out
}

impl Piece {
    /// Appends one localised sentence to a hint line that is being built.
    ///
    /// # Where this is still used, now that the manager's line is a table
    ///
    /// In every line that is a SENTENCE with a key inside it — the drawer's
    /// own teaching line, and whatever else grows one. The file manager's hint
    /// stopped being a sentence: it is one shortcut per chunk, in the order of
    /// the key table, and there the separator and the order are the code's.
    ///
    /// # What makes a run a key
    ///
    /// Being an interpolated argument is not enough: `files-panel-no-room`
    /// interpolates a NUMBER, and a number painted in the key's colour would
    /// be a lie about what to press. So a run is a key when the localiser says
    /// it is an argument AND its text is one of the notations this caller
    /// passed in. A value that grows a numeric argument corrects itself.
    pub fn extend(out: &mut Vec<Self>, m: &ronoctua_i18n::Marked, notations: &[&str]) {
        if !out.is_empty() {
            out.push(Self::prose(" "));
        }
        for (text, arg) in m.pieces() {
            if text.is_empty() {
                continue;
            }
            let key = arg && notations.contains(&text);
            out.push(if key {
                Self::key(text, false)
            } else {
                Self::prose(text)
            });
        }
    }
}

/// Where a key notation is CUT so a ligature cannot cross the modifier.
///
/// # The defect, photographed
///
/// `C->` came out on screen as `C-→`. The face ligates `->` — that is what the
/// feature is for, and in a line of code it is right — but here it turns a
/// notation into a DIFFERENT key, and worse into one this system forbids: an
/// arrow held with `Ctrl` is the footer's paging chord, taken before the view
/// ever sees it. The line exists to teach forty keys, and it was teaching one
/// that does not exist.
///
/// It was invisible until the line began to show the whole table — `C->` had
/// been bound for as long as the table has and drawn nowhere.
///
/// # Why the cut, and not a different key
///
/// Because the key is not wrong, the DRAWING is. `cosmic-text` shapes a run at
/// a time and exposes no way to switch a feature off, so the fix is to stop
/// the two characters being in one run: a hint's every glyph is one cell wide
/// and placed by this module's own arithmetic, so two runs land exactly where
/// one did.
///
/// And it is done for EVERY key rather than for `>`: which pairs a face
/// ligates is the face's business and changes when a face does. `C-` `>` and
/// `C-` `[` shape identically once they are apart.
fn split_at_the_modifier(text: &str) -> Option<(&str, &str)> {
    // THE MODIFIERS ARE EATEN FROM THE FRONT, and the rest is the symbol.
    //
    // Not `rfind('-')`, which is the same answer for every notation in the
    // table except one: `C--` is the film's rewind, and the LAST hyphen there
    // is the symbol itself — cutting after it leaves nothing on the right and
    // the two hyphens stay in one run. That is harmless against this face,
    // which does not ligate `--`, and "harmless against this face" is not a
    // rule. `Key::notation` builds the prefix out of exactly these four, so
    // reading them back is the same arithmetic in reverse.
    //
    // By `strip_prefix` and never by slicing: `key_text` has already replaced
    // an arrow's symbol with a multi-byte character, and `&text[at..at + 2]`
    // on `S-←` would cut a codepoint in half.
    let mut rest = text;
    let mut at = 0;
    while let Some(next) = rest
        .strip_prefix("C-")
        .or_else(|| rest.strip_prefix("M-"))
        .or_else(|| rest.strip_prefix("S-"))
        .or_else(|| rest.strip_prefix("s-"))
    {
        at += 2;
        rest = next;
    }
    // A CUT THAT LEAVES NOTHING IS NOT A CUT: `RET` has no modifier, and a
    // notation that is nothing BUT modifiers has no symbol to separate.
    (at > 0 && !rest.is_empty()).then(|| text.split_at(at))
}

/// Draws a hint line, and is the ONLY place that says what colour one is.
///
/// # Why it is here and not in each door
///
/// Because it was in each door, and that is precisely what the owner
/// photographed: the same line came out `text.primary` in the footer and
/// `text.secondary` in the window. Neither was a decision — the window wrote a
/// token at its push site, and the footer inherited one from the arm that
/// exists for "the user is typing in `M-x`", which the hint had been hung off
/// years later. Two doors, two answers, and nothing anywhere that could be
/// called wrong, which is the shape of defect this whole module exists to end.
///
/// # The elision happens once, across the pieces
///
/// The window elided and the footer did not, so the footer's line ran off the
/// screen with no `…` to say it had been cut. Cutting per piece would put an
/// ellipsis in the middle of the sentence, so the walk carries the room that
/// is left and only the piece that overruns is cut.
///
/// `room` is how many cells the line may occupy starting at `column`.
#[must_use]
pub fn hint_spans(
    pieces: &[Piece],
    size: ronoctua_render::Size,
    column: u16,
    line: u16,
    room: u16,
) -> Vec<ronoctua_render::Span> {
    let mut out = Vec::with_capacity(pieces.len());
    let mut at = column;
    let mut left = room;
    for p in pieces {
        if left == 0 {
            break;
        }
        // A KEY IS DRAWN IN TWO RUNS, so that no ligature can cross its
        // modifier — see `split_at_the_modifier`. Prose is one run: a sentence
        // is what the shaper is FOR.
        let runs: Vec<&str> = match p.register {
            Register::Key | Register::Alternate => split_at_the_modifier(&p.text)
                .map_or_else(|| vec![p.text.as_str()], |(a, b)| vec![a, b]),
            Register::Prose | Register::Separator => vec![p.text.as_str()],
        };
        for run in runs {
            if left == 0 {
                break;
            }
            // `layout::width`, which is the ruler the rest of both doors
            // position by: it counts CLUSTERS. An emoji advances more than one
            // cell and this would place the next run early — but every other
            // span on the line is placed the same way, and a hint that used a
            // second ruler would be the only thing on screen that did. No hint
            // value contains an emoji; if one ever does, the fix belongs to the
            // grid.
            let width = ronoctua_render::layout::width(run);
            let text = if width <= left {
                run.to_owned()
            } else {
                ronoctua_render::layout::elide_end(run, left)
            };
            let drawn = ronoctua_render::layout::width(&text);
            out.push(ronoctua_render::Span {
                size,
                text,
                column: at,
                line,
                color: p.register.color(),
                edge: false,
                foot: false,
            });
            at = at.saturating_add(drawn);
            left = left.saturating_sub(drawn);
        }
    }
    out
}

/// A window's list, as a [`ronoctua_design::Scroll`].
///
/// # The four windows that had no bar at all
///
/// The notification, power, Bluetooth and network windows are one shape: a
/// hint line, and under it a list that scrolls. Not one of them drew a
/// scrollbar — only the file manager's window did — so the wheel worked, there
/// was nothing to grab, and nothing on screen said how far down a nine-section
/// list one was. The owner reported it as *"na janela de redes a scrollbar não
/// está funcionando"*, and the answer is that there was none to work.
///
/// The arithmetic is the one every other bar in this system already uses.
/// `first_line` is where the list starts — the line under the hint.
#[must_use]
pub fn list_scroll(total: usize, lines: usize, offset: usize) -> ronoctua_design::Scroll {
    ronoctua_design::Scroll {
        total,
        lines,
        offset,
        top: 0,
    }
}

/// That list's bar, as a rectangle — `None` when the list fits.
#[must_use]
pub fn list_bar(
    scroll: ronoctua_design::Scroll,
    first_line: u16,
    width: f32,
    height: f32,
) -> Option<ronoctua_render::quad::Quad> {
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    // THE LAST BAND TAKES THE LEFTOVER, which is what `Scroll::track` decides:
    // a surface is not a whole number of lines, and a track that stops at
    // `lines × line` leaves a strip of background under the thumb at the last
    // page. The list IS the last band in these four windows.
    let (x, y, w, h) = scroll.bar(f.width, top, f.bottom(), f.cell(), f.line())?;
    Some(ronoctua_render::quad::Quad {
        x,
        y,
        width: w,
        height: h,
        color: tokens::color::RULE_STRONG,
    })
}

/// The air inside a container, in cells — `space.shelf.pad`, and the reason
/// is written there.
pub const SHELF_PAD: u16 = tokens::space::SHELF_PAD;

/// How many cells a scrollbar takes.
///
/// # Why this is not a token
///
/// Because it is not a choice: it is a MEASUREMENT of what
/// [`ronoctua_design::Scroll::bar`] returns. That function is handed the cell
/// and answers with a rectangle exactly one cell wide, on every surface this
/// shell draws — so a token here would be a second opinion about somebody
/// else's arithmetic, and the day the two disagreed the box would reserve one
/// width and the thumb would draw another.
///
/// It is a constant rather than `f.cell()` written out at each site because
/// the box counts CELLS and the drawing counts pixels, and the conversion
/// belongs to `Frame`.
pub const BAR_CELLS: u16 = 1;

/// How many cells of TEXT a row inside a container gets, at full width.
///
/// It is the two columns the row holds and the mark in front of them, and
/// nothing else: a container is as wide as one of its rows needs, which is why
/// one number serves all five of them.
pub const SHELF_ROOM: u16 = tokens::space::VIEW_LABELCELLS + tokens::space::VIEW_VALUECELLS;

/// Where a container is drawn, in the SURFACE's own cells, and how much text
/// fits in it.
///
/// # Why the surface's cells and not the row's
///
/// Because the ground, the bar and the pointer are the three that read this,
/// and all three work in the surface's — a door draws a row's cell `c` at
/// `FIELDS_LEAD + c`. The row's cells go IN; the box comes out in the
/// coordinate it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShelfBox {
    /// Its left edge, in the surface's cells.
    pub from: u16,
    /// How many cells wide it is.
    pub cells: u16,
    /// How many cells of text a row inside it gets — what a door hands to
    /// [`field_pair`] as the room.
    pub room: u16,
}

/// The box a container draws in, from the row's own `left` and `room`.
///
/// # Why a container is not the width of the surface
///
/// Because its bar goes at its own right edge, and a container as wide as the
/// screen puts that bar a hundred columns from the list it belongs to. The
/// owner asked for it in those words: *"o container não precisa ter toda a
/// largura do espaço disponível... assim trazendo a scrollbar dos containers
/// para mais perto de seus conteúdos"*.
///
/// # And it is the same width for every one of them
///
/// A box that followed the widest row it happened to hold would move its right
/// edge every time a network appeared — which is horizontally the very thing a
/// shelf exists to stop vertically. So it is [`SHELF_ROOM`] of text, clamped by
/// what the row was given: one measure, and it stops being a measure the moment
/// the surface is too narrow to hold it.
///
/// # What it can never reach
///
/// The page's own bar, which is the surface's LAST cell. A container is laid
/// out as air, text, air, its own bar — and then one more cell of air before
/// the surface's edge, so that on a narrow screen, where the box is clamped
/// rather than measured, the two bars are still two bars with a gap between
/// them.
///
/// That gap is the whole defect, twice over. The two thumbs were drawn in the
/// SAME column and the owner saw them collide; then the container's ground,
/// being the width of the surface and drawn afterwards, PAINTED OVER the
/// page's thumb — which is how a scrollbar comes to disappear and change size
/// while nobody is touching it.
#[must_use]
pub fn shelf_box(left: u16, room: u16) -> ShelfBox {
    let at = FIELDS_LEAD + left;
    let from = at.saturating_sub(SHELF_PAD);
    // THE AIR ON THE LEFT IS WHAT THERE IS ROOM FOR. On every surface this
    // draws on it is `SHELF_PAD`; the subtraction is what keeps a door that
    // starts its rows at cell zero from being given a negative edge.
    let lead = at - from;
    // AIR, BAR AND THE GAP: what the box spends to the right of its text.
    let tail = SHELF_PAD + BAR_CELLS + SHELF_PAD;
    let inner = SHELF_ROOM.min(room.saturating_sub(tail)).max(1);
    ShelfBox {
        from,
        cells: lead + inner + SHELF_PAD + BAR_CELLS,
        room: inner,
    }
}

/// A container's own ground, as a rectangle.
///
/// # Why a shelf has a ground at all
///
/// Because nothing else on screen says where one ends. The air before a
/// heading says where a SECTION starts, and a shelf is not a section — it is a
/// list inside one, with a ceiling of its own, and the rows above and below it
/// belong to the same section. The owner asked for it in those terms: a
/// different and DISCREET background.
///
/// It is a change of ground and nothing else — no border, no radius, no shadow
/// (invariant 15) — the same device the modeline and the notification band
/// already use, one rung lower on the same ladder.
#[must_use]
pub fn shelf_ground(
    shelf: ShelfBox,
    first_line: u16,
    lines: usize,
    width: f32,
    height: f32,
) -> ronoctua_render::quad::Quad {
    let f = Frame::of(width, height, first_line);
    #[allow(clippy::cast_precision_loss)]
    ronoctua_render::quad::Quad {
        x: f32::from(shelf.from) * f.cell(),
        y: f.y_of(first_line),
        width: f32::from(shelf.cells) * f.cell(),
        height: f.line() * lines as f32,
        color: tokens::color::SURFACE_SHELF,
    }
}

/// A container's bar: the same arithmetic, at its own right edge, and bounded
/// by its band.
///
/// `scroll.lines` is the band's own height, so the track stops where the shelf
/// does — a shelf is never the last band, and handing it the surface's bottom
/// would run its thumb down into the section underneath.
#[must_use]
pub fn shelf_bar(
    scroll: ronoctua_design::Scroll,
    shelf: ShelfBox,
    first_line: u16,
    width: f32,
    height: f32,
) -> Option<ronoctua_render::quad::Quad> {
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    #[allow(clippy::cast_precision_loss)]
    let bottom = top + f.line() * scroll.lines as f32;
    let right = f32::from(shelf.from + shelf.cells) * f.cell();
    let (x, y, w, h) = scroll.bar(right, top, bottom, f.cell(), f.line())?;
    Some(ronoctua_render::quad::Quad {
        x,
        y,
        width: w,
        height: h,
        color: tokens::color::RULE_STRONG,
    })
}

/// Where a press at height `y` puts that list.
///
/// The whole TRACK answers, not the thumb — [`ronoctua_design::Scroll::offset_at`]
/// is `thumb`'s own inverse, so nobody has to aim at a handle two rows tall.
#[must_use]
pub fn list_offset_at(
    scroll: ronoctua_design::Scroll,
    first_line: u16,
    width: f32,
    height: f32,
    y: f32,
) -> usize {
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    let track = scroll.track(top, f.bottom(), f.line());
    scroll.offset_at(y - top, track, f.line())
}

/// Where a press at height `y` puts a SHELF, whose track is its own band.
#[must_use]
pub fn shelf_offset_at(
    scroll: ronoctua_design::Scroll,
    first_line: u16,
    width: f32,
    height: f32,
    y: f32,
) -> usize {
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    #[allow(clippy::cast_precision_loss)]
    let track = (f.line() * scroll.lines as f32).max(f.line());
    scroll.offset_at(y - top, track, f.line())
}

/// Is the pointer on that bar's track?
///
/// The bar is the surface's LAST CELL, which is the same edge the footer's
/// region and the manager's four sections use.
#[must_use]
pub fn on_the_list_bar(
    scroll: ronoctua_design::Scroll,
    first_line: u16,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
) -> bool {
    if !scroll.cut() {
        return false;
    }
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    x >= f.width - f.cell() && y >= top && y <= f.bottom()
}

/// The same question for a band that is NOT the last one on the surface.
///
/// # Why it is a second function
///
/// Because a container's bar is drawn at ITS right edge and not at the
/// surface's, and a press there has to reach the bar that was DRAWN at that
/// height. Asking [`on_the_list_bar`] would answer for the wrong column and,
/// with its track running to the surface's bottom, for every line below the
/// band as well: the shelf's thumb would be visible and dragging it would move
/// the whole list instead. Where a bar is drawn is where it is grabbed, and
/// that promise needs both the band's own bottom and the box's own edge.
#[must_use]
pub fn on_the_band(
    scroll: ronoctua_design::Scroll,
    shelf: ShelfBox,
    first_line: u16,
    width: f32,
    height: f32,
    x: f32,
    y: f32,
) -> bool {
    if !scroll.cut() {
        return false;
    }
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    #[allow(clippy::cast_precision_loss)]
    let bottom = top + f.line() * scroll.lines as f32;
    // ITS OWN COLUMN: the last cell of the box, and never the surface's — see
    // `shelf_box`.
    let right = f32::from(shelf.from + shelf.cells) * f.cell();
    x >= right - f.cell() && x < right && y >= top && y < bottom
}

/// The blank line that goes BEFORE a section's title, as a row.
///
/// # Why a row and not a door's decision
///
/// Because the cursor, the click and the scrollbar all count rows, and a blank
/// line drawn by a door would put the three of them one line out of step with
/// what is on screen.
///
/// # What it looks like, and how to recognise one
///
/// A heading with NO fields. It is skipped by the cursor for the reason every
/// heading is, it draws nothing, and `fields.is_empty()` is what tells it from
/// a title — which is what the `a_heading_is_never_alone` tests ask.
///
/// The owner read the absence off the screen: *"os títulos estão colados com
/// os conteúdos acima deles"*. A section that begins immediately under the
/// last row of the one before it does not begin anywhere.
pub const AIR: Vec<crate::files::Field> = Vec::new();

/// What an INFORMATIVE row hands to the clipboard, when `RET` is pressed on it.
///
/// # Why a row that does nothing should do this
///
/// Because the owner asked for it, and the reason is in the rows themselves:
/// half of what these three views draw is a thing one copies — an address, a
/// port, a hardware address, a network's name, a peer's number. Reading it off
/// the screen and typing it back in is the only thing the screen was leaving
/// for the person to do by hand.
///
/// # And it is the VALUE, not the line
///
/// `192.168.1.5/24` is what goes into a terminal; `this machine's address
/// 192.168.1.5/24` is not. A row with no value is a SENTENCE — a section
/// saying it has nothing to list, a note explaining a cost — and there the
/// whole of it is the information.
///
/// The rule falls straight out of [`field_pair`]: it pushes the mark, the
/// label and the value, never an empty piece, and never the mark on a row that
/// does not answer `RET`. So an informative row is one or two pieces, and the
/// LAST of them is the answer either way.
#[must_use]
pub fn information_of(fields: &[crate::files::Field]) -> Option<String> {
    debug_assert!(
        fields.len() <= 2,
        "an informative row grew a third piece; `information_of` copies the last"
    );
    fields
        .last()
        .map(|f| f.text.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Puts an informative row's value on the clipboard, and answers with the
/// sentence to say about it.
///
/// # Why the six doors do not each write this
///
/// Because they would each write it differently, which is the defect this file
/// exists to end — and because there is nothing to decide: a failure here is
/// not something a door can do anything else about, so the whole of the
/// behaviour is "copy, and say which of the two happened".
///
/// The mechanism is `wl-copy`, for the reason written over
/// [`crate::buffer::copy_to_clipboard`]: whoever owns the selection has to
/// stay alive to serve it, and a footer or a window that is closed a moment
/// later would hand over an empty clipboard — which looks exactly like a copy
/// that worked.
#[must_use]
pub fn copied(text: &str, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    match crate::buffer::copy_to_clipboard(text) {
        Ok(()) => texts.t_with(key::ECHO_COPIED_VALUE, &[("value", text)]),
        Err(e) => {
            // THE REASON GOES TO THE LOG AND NOT TO THE LINE. It is a store
            // path or a compositor's refusal, neither of which the person
            // reading a network's address can act on — and the message line is
            // one line.
            tracing::warn!(error = %e, "could not put the row's value on the clipboard");
            texts.t(key::ECHO_COPY_FAILED)
        }
    }
}

/// Where a value begins, in the ROW's own cells.
///
/// # One arithmetic, three views
///
/// The network, power and Bluetooth views are the same shape — a list of a
/// label and a value — and each of them had written the placement itself.
/// Between them they managed three different answers, two of them in the same
/// file: a short value pushed to the last cell of the surface, a long one
/// dropped back to a third of the row, and a fact placed at the third
/// unconditionally. On a wide window the first of those puts `off` a hundred
/// and fifty columns away from `aeroplane mode`, which is what the owner
/// photographed.
///
/// A COLUMN, and not an edge: every value in a section starts in the same
/// place, so reading down it is one movement. The column is
/// `space.view.labelCells` (invariant 9 — a spacing is a token), clamped to
/// half the row, so a narrow footer does not become two columns of nothing.
///
/// This is the rule `rule` and `divider` already established for separators:
/// no view writes its own.
#[must_use]
pub fn value_column(left: u16, room: u16) -> u16 {
    left + tokens::space::VIEW_LABELCELLS.min(room / 2).max(1)
}

/// A bar, `cells` wide, `percent` of it filled.
///
/// # Why there is one of these
///
/// A volume, a microphone's level and anything else that is "this much of
/// that" are the same drawing, and three of them written in three places
/// would be three answers to how a half-filled cell rounds.
///
/// # Why it is glyphs and not a rectangle
///
/// Invariant 10: the icons of this interface are glyphs of the font. A bar of
/// quads would need its own colour, its own rounding and its own answer to how
/// wide a cell is — three questions the text pipeline has already answered —
/// and it could not be cut by the same budget arithmetic as the words beside
/// it. The two marks are measured at exactly one cell each in the real face,
/// and `ronoctua_protocol::cells` calls them narrow, so a bar of `n` glyphs is
/// `n` cells and nothing downstream has to know it is a bar.
///
/// # The rounding
///
/// It ROUNDS rather than truncating, but a percentage above zero never draws
/// as empty and one below full never draws as full. A meter that showed
/// nothing at 2% would say the microphone is dead, and one that showed full at
/// 99% would say a volume is at the top when a key can still move it.
#[must_use]
pub fn meter(percent: u8, cells: u16) -> String {
    if cells == 0 {
        return String::new();
    }
    let room = u32::from(cells);
    let percent = u32::from(percent).min(100);
    // ROUND HALF UP, written as the arithmetic and not as a pair of nested
    // divisions: `(p*c + 50) / 100` is `round(p*c/100)`, and the version this
    // replaced truncated a half — 75% of ten cells drew seven.
    let mut filled = (percent * room + 50) / 100;
    if percent > 0 {
        filled = filled.max(1);
    }
    if percent < 100 {
        filled = filled.min(room.saturating_sub(1));
    }
    let filled = usize::try_from(filled.min(room)).unwrap_or(0);
    let empty = usize::try_from(room).unwrap_or(0).saturating_sub(filled);
    let mut out = String::with_capacity(filled + empty);
    for _ in 0..filled {
        out.push(tokens::mark::METER_FULL);
    }
    for _ in 0..empty {
        out.push(tokens::mark::METER_EMPTY);
    }
    out
}

/// The label and the value of one row, placed and cut.
///
/// An EMPTY value gives the label the whole row, because a row with nothing on
/// the right is not a definition — it is a sentence, and cutting it at the
/// value column would cut it for no reason. `hit` and `explain` are the
/// caller's to fill in afterwards: this decides WHERE, never what a click does.
#[must_use]
pub fn field_pair(
    left: u16,
    room: u16,
    label: &str,
    value: &str,
    label_colour: tokens::Color,
    value_colour: tokens::Color,
) -> Vec<crate::files::Field> {
    let piece = |at: u16, text: String, color: tokens::Color| crate::files::Field {
        at,
        text,
        color,
        hit: crate::files::Hit::Nothing,
        explain: None,
    };
    let from = left;
    let mut out: Vec<crate::files::Field> = Vec::with_capacity(2);
    let for_text = room;
    // THE COLUMN IS A MINIMUM, NOT A CEILING.
    //
    // A label longer than it pushes the value right instead of being cut. The
    // column is what makes a section readable down one line, and cutting a
    // label to keep it is the worse trade: `keep awake: no sleep, no idle
    // action` is 36 cells beside a two-letter value, and cutting it to `keep
    // awake: no s…` is exactly the defect the power view's own comment was
    // written about. One row's value sitting further along costs a glance;
    // a name nobody can read costs the row.
    //
    // What it may never do is push the value off the end: below three cells
    // for the value the label yields again, and the elision comes back.
    let at = value_column(left, room)
        .max(from + ronoctua_render::layout::width(label) + 2)
        .min((from + for_text).saturating_sub(3))
        .max(value_column(left, room).min(from + 1));
    let ends = from + for_text;
    // AN EMPTY PIECE IS NOT A PIECE. Below the mark's own width there is no
    // room for a label or a value, and elision hands back `""` — which draws
    // nothing but still occupies a slot, at a column the row does not reach.
    let mut put = |at: u16, text: String, colour| {
        if !text.is_empty() {
            out.push(piece(at, text, colour));
        }
    };
    if value.is_empty() {
        put(
            from,
            ronoctua_render::layout::elide_end(label, for_text),
            label_colour,
        );
        return out;
    }
    put(
        from,
        // TWO CELLS OF AIR before the value, always.
        //
        // It was one, and one is not enough: `the tailnet connection` filled
        // the column exactly and the screen read `this machine on the tailnet
        // up` as a single phrase. A label longer than what is left is CUT
        // here, which is the trade a fixed column makes — and that trade
        // needs no check, because an ellipsis on a label is the one kind of
        // defect that looking at the screen always finds.
        ronoctua_render::layout::elide_end(label, at.min(ends).saturating_sub(from + 2)),
        label_colour,
    );
    put(
        at,
        ronoctua_render::layout::elide_end(value, ends.saturating_sub(at)),
        value_colour,
    );
    out
}

/// What colour a syntax role is drawn in.
///
/// # Why the map lives HERE, and why there is one of it
///
/// Because both of its callers are in this crate — the clipboard's preview in
/// `surface.rs` and the file manager's preview in `files.rs` — and two copies
/// would be the two panels colouring the same Rust differently. That is the
/// defect `Section` and `Field::covers` were each written to end, and this
/// file is already the one allowed to make a visual decision
/// (`separators-come-from-chrome`).
///
/// And it does not live in `ronoctua-syntax`: that crate is `no_std` and
/// receives no token VALUES, which is the same wall invariant 11 draws around
/// a wasm buffer. It gives back a role; turning a role into a colour is the
/// business of whoever draws.
///
/// # Fourteen roles, twelve tokens
///
/// `Constant` shares `number` and `Heading` shares `attribute`, and they stay
/// two roles each for the reason `ronoctua_design::Palette` gives about
/// `Quiet` and `Dim`: the day the theme parts them, the change is one line
/// here rather than a hunt through every call site.
///
/// The `match` is exhaustive, so a role added to the crate is a compile error
/// in exactly one file — which is the property that makes the wall worth
/// having.
#[must_use]
pub(crate) const fn syntax_color(role: ronoctua_syntax::Role) -> tokens::Color {
    use ronoctua_syntax::Role;

    match role {
        Role::Comment => tokens::color::SYNTAX_COMMENT,
        Role::String => tokens::color::SYNTAX_STRING,
        Role::Escape => tokens::color::SYNTAX_ESCAPE,
        Role::Number | Role::Constant => tokens::color::SYNTAX_NUMBER,
        Role::Keyword => tokens::color::SYNTAX_KEYWORD,
        Role::Type => tokens::color::SYNTAX_TYPE,
        Role::Function => tokens::color::SYNTAX_FUNCTION,
        Role::Attribute | Role::Heading => tokens::color::SYNTAX_ATTRIBUTE,
        Role::Property => tokens::color::SYNTAX_PROPERTY,
        Role::Tag => tokens::color::SYNTAX_TAG,
        Role::Added => tokens::color::SYNTAX_ADDED,
        Role::Removed => tokens::color::SYNTAX_REMOVED,
    }
}

#[cfg(test)]
mod meter_tests {
    use super::meter;
    use ronoctua_render::layout;
    use ronoctua_tokens as tokens;

    fn filled(bar: &str) -> usize {
        bar.chars()
            .filter(|c| *c == tokens::mark::METER_FULL)
            .count()
    }

    #[test]
    fn a_bar_is_exactly_as_many_cells_as_it_was_asked_for() {
        // WHAT THE BUDGET ARITHMETIC DEPENDS ON. Both marks are measured at
        // one cell in the real face and `ronoctua_protocol::cells` calls them
        // narrow; a bar that came out wider would push the number beside it
        // off the end without anything saying so.
        for cells in [1_u16, 2, 10, 14, 40] {
            for percent in [0_u8, 1, 37, 50, 99, 100] {
                let bar = meter(percent, cells);
                assert_eq!(
                    layout::width(&bar),
                    cells,
                    "{percent}% in {cells} cells came out {bar:?}"
                );
            }
        }
        assert_eq!(meter(50, 0), "");
    }

    #[test]
    fn nothing_and_everything_are_never_confused_with_the_rest() {
        // A meter that showed nothing at 2% would say the microphone is dead;
        // one that showed full at 99% would say a volume is at the top when a
        // key can still move it. Both are lies a rounding rule tells.
        assert_eq!(filled(&meter(0, 10)), 0);
        assert_eq!(filled(&meter(1, 10)), 1, "2% is not silence");
        assert_eq!(filled(&meter(99, 10)), 9, "99% is not full");
        assert_eq!(filled(&meter(100, 10)), 10);
    }

    #[test]
    fn the_bar_rounds_to_the_nearest_cell() {
        // 40% of fourteen is 5.6, and the eye reads six.
        assert_eq!(filled(&meter(40, 14)), 6);
        assert_eq!(filled(&meter(50, 10)), 5);
        assert_eq!(filled(&meter(38, 10)), 4);
        assert_eq!(filled(&meter(75, 10)), 8);
    }

    #[test]
    fn a_bar_never_goes_backwards() {
        // A louder volume can never draw a shorter bar, at any width.
        for cells in [3_u16, 10, 14] {
            let mut last = 0;
            for percent in 0..=100_u8 {
                let now = filled(&meter(percent, cells));
                assert!(now >= last, "{percent}% in {cells} cells went backwards");
                last = now;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BAR_CELLS, FIELDS_LEAD, Frame, INSET, Piece, Register, SHELF_PAD, SHELF_ROOM, cell_width,
        content_left_px, field_pair, hint_spans, key_text, list_bar, list_offset_at, list_scroll,
        on_the_band, on_the_list_bar, shelf_bar, shelf_box, shelf_ground, split_at_the_modifier,
        value_column,
    };
    use ronoctua_tokens as tokens;

    fn piece(text: &str, key: bool) -> Piece {
        if key {
            Piece::key(text, false)
        } else {
            Piece::prose(text)
        }
    }

    #[test]
    fn the_key_and_the_prose_are_two_colours_and_neither_is_a_content_colour() {
        // THE OWNER'S REPORT, in one assertion: the two halves of the line
        // used to be the same tone, and the whole line used to be one of the
        // tones the listing under it is drawn in.
        let line = [piece("type to filter · ", false), piece("C-m", true)];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 2, 0, 80);
        // THREE SPANS FOR TWO PIECES: the key is drawn in two runs, both in
        // the key's colour — see `split_at_the_modifier`.
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].color, tokens::color::TEXT_SUBTLE_DEFAULT);
        assert_eq!(spans[1].color, tokens::color::TEXT_SUBTLE_STRONG);
        assert_eq!(spans[2].color, tokens::color::TEXT_SUBTLE_STRONG);
        assert_ne!(spans[0].color, spans[1].color);
        for s in &spans {
            for content in [
                tokens::color::TEXT_PRIMARY,
                tokens::color::TEXT_SECONDARY,
                tokens::color::TEXT_DIM,
            ] {
                assert_ne!(s.color, content, "the hint is wearing a content colour");
            }
        }
    }

    #[test]
    fn no_ligature_crosses_a_keys_modifier() {
        // THE DEFECT, PHOTOGRAPHED: `C->` came out on screen as `C-→`. The
        // face ligates `->` — right in a line of code — and here it turned a
        // notation into a key this system FORBIDS, since an arrow held with
        // `Ctrl` is the footer's paging chord and never reaches the view. It
        // was invisible until the hint began to teach the whole key table.
        //
        // The shaper works a RUN at a time, so what this asserts is that the
        // two characters are never in one run.
        for notation in [
            "C->", "C-<", "C--", "C-=", "S-TAB", "C-SPC", "RET", "S-<left>",
        ] {
            let t = ronoctua_keymap::Key::parse(notation).expect("a notation");
            let drawn = key_text(&t);
            let line = [Piece::key(drawn.clone(), false)];
            let spans = hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 40);
            // AGAINST THE DRAWN FORM and not against the notation: `S-<left>`
            // is drawn `S-←`, which is `key_text`'s other half.
            let rebuilt: String = spans.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(rebuilt, drawn, "the key did not survive the cut");
            for s in &spans {
                assert!(
                    !s.text.contains("->"),
                    "`{notation}` still has `->` inside one run: {:?}",
                    s.text
                );
            }
        }
        // AND A KEY WITH NO MODIFIER IS ONE RUN, because there is nothing for a
        // ligature to cross.
        assert!(split_at_the_modifier("RET").is_none());
        assert!(split_at_the_modifier("-").is_none());
        assert!(split_at_the_modifier("C-").is_none());
        // THE SYMBOL CAN BE A HYPHEN, and it is: `C--` is the film's rewind.
        // Read from the RIGHT this one comes back whole, which is the cut that
        // does nothing — harmless against a face that does not ligate `--`,
        // and harmlessness is not a rule.
        assert_eq!(split_at_the_modifier("C--"), Some(("C-", "-")));
        assert_eq!(split_at_the_modifier("S-TAB"), Some(("S-", "TAB")));
        // AND AN ARROW IS MULTI-BYTE, which is why nothing here slices.
        let arrow = format!("S-{}", ronoctua_tokens::mark::ARROW_LEFT);
        assert_eq!(
            split_at_the_modifier(&arrow).map(|(a, _)| a.to_owned()),
            Some("S-".to_owned())
        );
    }

    #[test]
    fn each_piece_starts_where_the_one_before_it_ended() {
        // The pieces are separate spans on ONE line, so the column each starts
        // at is the sum of the widths before it. Get that wrong and the key
        // draws on top of the prose — which is what a single span never had to
        // worry about.
        let line = [
            piece("type to filter · ", false),
            piece("C-m", true),
            piece(" opens the menu", false),
        ];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 2, 0, 80);
        // FOUR SPANS FOR THREE PIECES: a key is drawn in two runs so that no
        // ligature can cross its modifier — see `split_at_the_modifier`.
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].column, 2);
        assert_eq!(spans[1].column, 2 + 17);
        assert_eq!(spans[2].column, 2 + 17 + 2);
        assert_eq!(spans[3].column, 2 + 17 + 3);
        // And the line they draw is the sentence, in order.
        let rebuilt: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, "type to filter · C-m opens the menu");
    }

    #[test]
    fn a_hint_too_long_for_the_room_is_cut_once_at_the_end() {
        // The footer never cut this line at all, so it ran off the screen with
        // nothing saying it had been. Cutting per piece would put an ellipsis
        // in the middle of the sentence.
        let line = [
            piece("type to filter · ", false),
            piece("C-m", true),
            piece(" opens the action menu", false),
        ];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 22);
        let drawn: u16 = spans
            .iter()
            .map(|s| ronoctua_render::layout::width(&s.text))
            .sum();
        assert!(drawn <= 22, "{drawn} cells drawn in room for 22");
        let cut = spans.iter().filter(|s| s.text.ends_with('…')).count();
        assert_eq!(cut, 1, "the line has to be cut exactly once");
        assert!(
            spans.last().is_some_and(|s| s.text.ends_with('…')),
            "the cut has to be at the END of the line"
        );
    }

    #[test]
    fn no_room_draws_nothing_rather_than_one_cell_of_ellipsis() {
        let line = [piece("type to filter", false)];
        assert!(hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 0).is_empty());
    }

    #[test]
    fn the_sentences_are_joined_by_exactly_one_space() {
        // The join used to be written at each call site and one of the three
        // was missing its space: the owner's photograph reads `scrolls the
        // facts· TAB goes to`.
        // The manager's own hint no longer goes through here — it is one
        // shortcut per chunk now, and the seam is `Piece::separator`. What is
        // still assembled this way is every line that is a SENTENCE with a key
        // inside it, and those are the two used here.
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let mut out = Vec::new();
        let a = t.t_marked(ronoctua_i18n::key::FILES_SIDEBAR_ONLY, &[("key", "C-l")]);
        let b = t.t_marked(
            ronoctua_i18n::key::MINIBUFFER_FOLD_HINT,
            &[("keys", "RCtrl")],
        );
        Piece::extend(&mut out, &a, &["C-l"]);
        let seam = out.len();
        Piece::extend(&mut out, &b, &["RCtrl"]);
        assert_eq!(out[seam].text, " ");
        let line: String = out.iter().map(|p| p.text.as_str()).collect();
        assert!(!line.contains("·  "), "two spaces at a seam: {line}");
        assert!(
            !line.contains("e·"),
            "a seam with no space before the separator: {line}"
        );
    }

    #[test]
    fn an_argument_that_is_not_a_key_is_not_painted_as_one() {
        // `files-panel-no-room` interpolates a NUMBER. Painted in the key's
        // colour it would be a lie about what to press, so being an
        // interpolated argument is not enough — the run has to be one of the
        // notations the caller passed in.
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let m = t.t_marked(ronoctua_i18n::key::FILES_PANEL_NO_ROOM, &[("cells", "24")]);
        let mut out = Vec::new();
        Piece::extend(&mut out, &m, &["C-m", "C-v"]);
        assert!(
            out.iter()
                .all(|p| !matches!(p.register, Register::Key | Register::Alternate)),
            "a number came out as a key"
        );
        assert!(out.iter().any(|p| p.text.contains("24")));
    }

    #[test]
    fn the_residue_is_named_and_it_is_smaller_than_one_step() {
        // THE NUMBER THAT HAD NO NAME.
        //
        // `columns` is `floor(width / cell)` and `lines` is `floor(height /
        // line)`, so between the last whole cell and the border there is a
        // remainder, and the same at the bottom. It is never zero for long: it
        // is a function of the surface, and the compositor picks the surface.
        //
        // Nameless, it became a strip of blank pixels under the manager's facts
        // panel that grew and shrank as the window was resized — the owner
        // photographed it three times before it had a name.
        for width in [640.0_f32, 800.0, 1280.0, 1366.0, 1920.0] {
            for height in [200.0_f32, 747.0, 800.0, 1080.0, 1200.5] {
                let f = Frame::of(width, height, 1);
                let over_x = f.width - f.x_of(f.columns());
                let over_y = f.height - f.y_of(f.lines());
                assert!(
                    over_x >= 0.0 && over_x < f.cell(),
                    "{width}x{height}: horizontal residue of {over_x} in a cell of {}",
                    f.cell()
                );
                assert!(
                    over_y >= 0.0 && over_y < f.line(),
                    "{width}x{height}: vertical residue of {over_y} in a line of {}",
                    f.line()
                );
            }
        }
    }

    #[test]
    fn the_border_is_reached_from_the_border_and_never_by_counting() {
        // WHAT COUNTING GETS YOU. A run placed at the last cell counted from
        // the origin stops `residue_x` short of the border; the same run placed
        // FROM the border lands on it exactly. Both are legitimate — one is for
        // text read left to right, the other for anything that must sit against
        // the edge — and the defect was never having said which is which.
        let f = Frame::of(1366.0, 768.0, 1);
        // THE LAST VALID CELL is `columns() - 1`; `columns()` is one past it,
        // which is the off-by-one this very assertion caught while being
        // written.
        let counted = f.x_of(f.columns().saturating_sub(1)) + f.cell();
        let anchored = f.width - f.cell() + f.cell();
        assert!(
            (anchored - f.width).abs() < f32::EPSILON,
            "anchored to the edge ended at {anchored} and the edge is {}",
            f.width
        );
        assert!(
            counted <= f.width,
            "counted from the origin it went past the edge: {counted} > {}",
            f.width
        );
        // And the difference between them IS the residue — said once, so
        // nobody has to rediscover it from a screenshot.
        let over = f.width - f.x_of(f.columns());
        assert!(
            ((f.width - counted) - over).abs() < 0.01,
            "the difference between the two rails is not the residue"
        );
    }

    #[test]
    fn a_point_lands_on_the_cell_it_was_drawn_in() {
        // THE INVERSE PAIR. `x_of`/`column_at` and `y_of`/`line_at` are the
        // drawing and the pointer, and they have to be each other's inverse or
        // a click lands on one thing while the screen shows another. The design
        // crate's `Grid::at`/`index_at` carry this lesson for the buffers; this
        // is the same law for the host.
        let f = Frame::of(1280.0, 800.0, 2);
        for column in 0..f.columns() {
            assert_eq!(f.column_at(f.x_of(column)), column, "column {column}");
            // AND ANYWHERE INSIDE THE CELL, not only at its first pixel.
            assert_eq!(f.column_at(f.x_of(column) + f.cell() * 0.5), column);
        }
        for line in 0..f.lines() {
            assert_eq!(f.line_at(f.y_of(line)), Some(line), "line {line}");
            assert_eq!(f.line_at(f.y_of(line) + f.line() * 0.5), Some(line));
        }
        assert_eq!(f.line_at(-1.0), None, "above the surface is not a line");
    }

    #[test]
    fn both_doors_put_content_in_the_same_place() {
        // THE DEFECT THE OWNER PHOTOGRAPHED. The footer drew a file row one
        // cell in and the window drew the same row two cells in — 10,8 px
        // against 24,0 px, a factor of 2,2, for one table.
        //
        // It used to allow a difference of up to one footer cell, because the
        // doors drew at two font sizes on purpose. They draw at one now, so
        // the tolerance is gone and the two are the SAME NUMBER — which is a
        // stronger statement than the one this test was written to make.
        let cells = FIELDS_LEAD + tokens::space::FILES_GUTTER;
        assert_eq!(cells, INSET, "the sum has to be the inset, in both places");

        // AND IN PIXELS THEY ARE NOW THE SAME NUMBER, which the old form of
        // this test could not say: a cell was a different distance in each
        // door, so it allowed a difference of up to one footer cell. Two
        // surfaces of very different shapes, one column, one pixel.
        let footer = Frame::of(1920.0, 53.0, 0);
        let window = Frame::of(900.0, 700.0, 0);
        assert!(
            (footer.x_of(INSET) - window.x_of(INSET)).abs() < f32::EPSILON,
            "content begins at {} px in one surface and {} px in the other",
            footer.x_of(INSET),
            window.x_of(INSET)
        );
        assert!((footer.cell() - window.cell()).abs() < f32::EPSILON);
        // And the two ways of arriving at that pixel agree, which is what
        // stops a third one being written beside them.
        assert!((footer.x_of(INSET) - content_left_px()).abs() < f32::EPSILON);
        assert!((cell_width() - footer.cell()).abs() < f32::EPSILON);
    }

    #[test]
    fn a_row_that_carries_its_gutter_never_pays_it_twice() {
        // The window's old constant was `CONTENT_INSET - FILES_GUTTER` written
        // out at two call sites, with a comment at each explaining the
        // subtraction. The comment was right and the duplication was the
        // problem: the footer was the third site and it simply did not have
        // one.
        assert_eq!(
            FIELDS_LEAD + tokens::space::FILES_GUTTER,
            INSET,
            "whatever already brings the gutter pays only the difference"
        );
        // And it never goes negative, whatever the two tokens become. A
        // `const` block, because both sides are constants and clippy is right
        // that a runtime assertion over two of them proves nothing at runtime
        // — this way it is the COMPILER that refuses.
        const { assert!(FIELDS_LEAD <= INSET) };
    }

    #[test]
    fn the_value_column_does_not_follow_the_width() {
        // THE DEFECT THIS HOLDS SHUT. The three views placed a short value at
        // the surface's right edge, so widening the window moved the value and
        // left the label where it was: on a 200-column window `off` sat a
        // hundred and fifty columns from `aeroplane mode`. A column that grows
        // with the screen is not a column.
        let wide: Vec<u16> = [80u16, 120, 200, 400]
            .into_iter()
            .map(|columns| value_column(2, columns - 4))
            .collect();
        assert!(
            wide.windows(2).all(|w| w[0] == w[1]),
            "the value moved with the window: {wide:?}"
        );
        assert_eq!(wide[0], 2 + tokens::space::VIEW_LABELCELLS);
    }

    #[test]
    fn a_narrow_row_gives_the_value_half_and_draws_nothing_past_the_end() {
        // A footer can be narrower than the label column, and there the column
        // has to yield: at 24 cells of room, a 28-cell label column would put
        // the value past the end of the row.
        //
        // AND WHAT IS CHECKED IS WHAT IS DRAWN: below three cells for a
        // value the answer is to draw no value, not to place one at a column
        // the row does not reach.
        for room in 1u16..=tokens::space::VIEW_LABELCELLS * 2 {
            {
                let fields = field_pair(
                    2,
                    room,
                    "a label of some length",
                    "a value",
                    tokens::color::TEXT_SECONDARY,
                    tokens::color::TEXT_PRIMARY,
                );
                for f in &fields {
                    let ends = f.at + ronoctua_render::layout::width(&f.text);
                    assert!(
                        ends <= 2 + room,
                        "at {room} cells a field ran to {ends}: {:?}",
                        f.text
                    );
                }
            }
        }
    }

    #[test]
    fn a_row_with_no_value_keeps_the_whole_width_for_its_label() {
        // "share this connection" is a row with nothing on the right. Cutting
        // it at the value column would cut a sentence for no reason.
        let long = "share this connection with the machines around this one";
        let fields = field_pair(
            2,
            60,
            long,
            "",
            tokens::color::TEXT_SECONDARY,
            tokens::color::TEXT_PRIMARY,
        );
        assert_eq!(fields.len(), 1);
        assert_eq!(
            fields[0].text, long,
            "a sentence was cut at the value column"
        );
    }

    #[test]
    fn a_list_that_fits_has_no_bar_and_one_that_does_not_can_be_dragged_to_its_end() {
        // FOUR WINDOWS HAD NO BAR AT ALL, which is what the owner reported as
        // one not working. The two halves that make a bar a control rather
        // than an ornament are here: it does not exist when there is nothing
        // to scroll, and pressing at the bottom of the TRACK lands on the last
        // page — the defect written down in `scroll.rs` is a thumb that stops
        // half way down its own column.
        let (w, h) = (600.0_f32, 400.0_f32);
        let fits = list_scroll(4, 12, 0);
        assert!(
            list_bar(fits, 1, w, h).is_none(),
            "a bar over a list that fits is a control that does nothing"
        );

        let long = list_scroll(200, 12, 0);
        let bar = list_bar(long, 1, w, h).expect("a list of 200 in 12 lines has no bar");
        let f = Frame::of(w, h, 1);
        assert!(
            bar.x >= f.width - f.cell(),
            "the bar is not on the surface's last cell"
        );
        assert!(bar.height > 0.0 && bar.height <= f.bottom() - f.y_of(1));

        // The bottom of the track is the last page, and the top is the first.
        let last = list_offset_at(long, 1, w, h, f.bottom());
        assert_eq!(last, 200 - 12, "the bottom of the track is not the end");
        assert_eq!(list_offset_at(long, 1, w, h, f.y_of(1)), 0);

        // AND THE TRACK IS WHERE IT IS DRAWN. A bar one cannot press is a bar
        // that is not there.
        assert!(on_the_list_bar(long, 1, w, h, f.width - 1.0, f.y_of(2)));
        assert!(!on_the_list_bar(long, 1, w, h, 10.0, f.y_of(2)));
        assert!(
            !on_the_list_bar(fits, 1, w, h, f.width - 1.0, f.y_of(2)),
            "a list that fits offered a track to grab"
        );
    }

    #[test]
    fn the_lists_bar_and_a_shelfs_bar_never_share_a_pixel() {
        // THE OWNER SAW THEM COLLIDE: *"a scrollbar dos containers e a
        // scrollbar do buffer inteiro estão se colidindo"*. Both were drawn on
        // the surface's last cell, in the same colour — two thumbs in one line
        // of pixels, one of them for a list the pointer was not on.
        let (w, h) = (600.0_f32, 400.0_f32);
        let f = Frame::of(w, h, 4);
        let (left, room) = crate::network::extent(f.columns());
        let shelf_of = shelf_box(left, room);
        let outer = list_scroll(200, 12, 40);
        let inner = list_scroll(30, 5, 3);
        let list = list_bar(outer, 1, w, h).expect("the list has no bar");
        let shelf = shelf_bar(inner, shelf_of, 4, w, h).expect("the shelf has no bar");

        assert!(
            list.x >= shelf.x + shelf.width || shelf.x >= list.x + list.width,
            "the two bars overlap: the list at {}..{} and the shelf at {}..{}",
            list.x,
            list.x + list.width,
            shelf.x,
            shelf.x + shelf.width
        );
        // AND THERE IS A CELL BETWEEN THEM. Two thumbs of the same colour in
        // neighbouring columns read as one bar of double width.
        assert!(
            list.x - (shelf.x + shelf.width) >= f.cell() - 0.5,
            "the two bars are neighbours: {} to {}",
            shelf.x + shelf.width,
            list.x
        );

        // AND THE CONTAINER'S GROUND NEVER REACHES THE PAGE'S BAR.
        //
        // This is the second half of what the owner read off the screen: the
        // ground was the width of the surface and was pushed AFTER the list's
        // bar, so it painted the page's thumb out of existence wherever a
        // shelf was on screen — *"ela acaba desaparecendo e mudando de
        // tamanho"*. Order in the quad list cannot be asserted from here;
        // geometry can, and geometry is what makes the order not matter.
        let ground = shelf_ground(shelf_of, 4, 5, w, h);
        assert!(
            ground.x + ground.width <= list.x,
            "the container's ground ({}..{}) covers the page's bar at {}",
            ground.x,
            ground.x + ground.width,
            list.x
        );
        assert!(
            ground.x > 0.0,
            "the container is glued to the screen's edge"
        );

        // AND WHAT IS DRAWN IS WHAT IS GRABBED.
        let on_shelf = shelf.x + shelf.width / 2.0;
        assert!(
            on_shelf < f.width - f.cell(),
            "the shelf's bar is on the list's cell"
        );
        assert!(on_the_band(
            inner,
            shelf_of,
            4,
            w,
            h,
            on_shelf,
            shelf.y + 1.0
        ));
        assert!(
            !on_the_band(inner, shelf_of, 4, w, h, f.width - 1.0, shelf.y + 1.0),
            "the list's own column answered for the shelf"
        );

        // AND THE SHELF'S TRACK STOPS AT ITS BAND: its thumb may not run down
        // into the section underneath.
        #[allow(clippy::cast_precision_loss)]
        let band_bottom = f.y_of(4) + f.line() * inner.lines as f32;
        assert!(
            shelf.y + shelf.height <= band_bottom + 0.5,
            "the shelf's thumb ran past its own band"
        );
    }

    #[test]
    fn a_container_is_narrower_than_the_screen_and_the_same_width_everywhere() {
        // The owner asked for both halves: *"o container não precisa ter toda
        // a largura do espaço disponível, seria melhor uma largura menor,
        // assim trazendo a scrollbar dos containers para mais perto de seus
        // conteúdos"* — and, of the shelves, *"deixando um tamanho padrão para
        // todos"*.
        //
        // A WIDE SURFACE IS WHERE IT MATTERS: at 1920 px the row's own room is
        // over a hundred and fifty cells, and a bar at the end of that is the
        // defect. Below the measure the box is CLAMPED, which is the only
        // honest answer when there is not enough screen for one.
        let wide = shelf_box(1, 160);
        assert_eq!(wide.room, SHELF_ROOM, "a wide screen shrank the container");
        assert!(
            wide.from + wide.cells < 160,
            "the container took the whole row"
        );

        let narrow = shelf_box(1, 30);
        assert!(narrow.room < SHELF_ROOM, "a narrow screen was not clamped");

        // AND "EVERYWHERE" MEANS THE TWO DOORS.
        //
        // They feed this different numbers and both are right: the footer
        // hands the SURFACE's columns, and the window hands columns already
        // net of both margins (`network_window::recount`). The promise is not
        // that the two boxes are equal — they cannot be, the surfaces differ —
        // but that in either the container's bar, the page's bar and the text
        // are three separate columns.
        for columns in 8_u16..=240 {
            for (door, cells) in [
                ("the footer", columns),
                ("the window", columns.saturating_sub(2 * FIELDS_LEAD)),
            ] {
                let (left, room) = crate::network::extent(cells);
                let shelf = shelf_box(left, room);
                let text_ends = shelf.from + shelf.cells - SHELF_PAD - BAR_CELLS;
                assert!(
                    text_ends > shelf.from,
                    "{door} at {columns} columns: the box has no room for text"
                );
                assert_eq!(
                    text_ends,
                    FIELDS_LEAD + left + shelf.room,
                    "{door} at {columns} columns: the ground does not end where the text does"
                );
                // THE PAGE'S BAR IS THE LAST CELL of the door's own surface,
                // and the box may not reach it — nor sit against it.
                assert!(
                    shelf.from + shelf.cells + SHELF_PAD <= cells,
                    "{door} at {columns} columns: the box ends at {}, and the page's bar is at {}",
                    shelf.from + shelf.cells,
                    cells - 1
                );
            }
        }

        // AND THE ROOM IS WHAT A ROW IS CUT TO, which is what keeps the text
        // inside the box: label column, value column and the mark, and not one
        // cell more.
        assert_eq!(
            SHELF_ROOM,
            tokens::space::VIEW_LABELCELLS + tokens::space::VIEW_VALUECELLS
        );
        let row = field_pair(
            1,
            wide.room,
            "a network with a very long name indeed, quite a long one",
            "online · Linux · 100.64.12.34 · exit node",
            tokens::color::TEXT_PRIMARY,
            tokens::color::TEXT_SECONDARY,
        );
        for piece in &row {
            assert!(
                piece.at + ronoctua_render::layout::width(&piece.text) <= 1 + wide.room,
                "a row ran out of its container: {} at {}",
                piece.text,
                piece.at
            );
        }
    }

    #[test]
    fn the_label_says_whether_the_row_is_a_door_and_the_text_never_moves() {
        // THE OWNER REFUSED THE MARK: *"não quero mais o chevron, pois isso é
        // utilizado em elementos de descolapsar/colapsar"* — and then refused
        // a mark of any kind, asking for the difference to be a colour, in
        // colours no VALUE wears.
        //
        // So the two rows are the same shape, to the cell, and differ only in
        // the hue of their label. What this guards is both halves: that the
        // text did not move when the mark left, and that the label's colour is
        // never one of the value's.
        let acts = field_pair(
            2,
            60,
            "radio",
            "on",
            tokens::color::TEXT_LABEL_ACTS,
            tokens::color::TEXT_PRIMARY,
        );
        let fact = field_pair(
            2,
            60,
            "band",
            "5 GHz",
            tokens::color::TEXT_LABEL_FACT,
            tokens::color::TEXT_PRIMARY,
        );
        assert_eq!(acts.len(), 2, "a row grew a piece it does not draw");
        assert_eq!(fact.len(), 2);
        assert_eq!(
            acts[0].at, 2,
            "the label no longer starts at the row's own left"
        );
        assert_eq!(
            acts[0].at, fact[0].at,
            "the two labels start in different places"
        );
        assert_eq!(
            acts[1].at, fact[1].at,
            "the two values start in different places"
        );
        assert_ne!(
            acts[0].color, fact[0].color,
            "a door and a fact wear the same label"
        );
        for row in [&acts, &fact] {
            assert_ne!(
                row[0].color, row[1].color,
                "the label wears the value's own colour"
            );
        }
    }

    #[test]
    fn a_label_longer_than_the_column_pushes_the_value_rather_than_being_cut() {
        // THE COLUMN IS A MINIMUM. `keep awake: no sleep, no idle action` is
        // 36 cells beside a two-letter value; cutting it to `keep awake: no
        // s…` is the defect the power view's own comment was written about.
        let long = "keep awake: no sleep, no idle action";
        let fields = field_pair(
            2,
            80,
            long,
            "on",
            tokens::color::TEXT_SECONDARY,
            tokens::color::TEXT_PRIMARY,
        );
        assert_eq!(fields[0].text, long, "the label was cut");
        assert!(
            fields[1].at > value_column(2, 80),
            "the value did not move out of the label's way"
        );
        assert!(
            fields[1].at >= 2 + ronoctua_render::layout::width(long) + 2,
            "the label ran into the value"
        );

        // AND A LABEL SO LONG THAT THE VALUE WOULD FALL OFF still yields.
        let fields = field_pair(
            2,
            40,
            &"x".repeat(200),
            "on",
            tokens::color::TEXT_SECONDARY,
            tokens::color::TEXT_PRIMARY,
        );
        for f in &fields {
            let ends = f.at + ronoctua_render::layout::width(&f.text);
            assert!(ends <= 2 + 40, "a field ran to {ends} in a row of 40");
        }
        assert_eq!(fields.len(), 2, "the value disappeared: {fields:?}");
    }
}
