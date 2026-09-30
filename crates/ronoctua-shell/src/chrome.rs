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

/// THE COLUMN HAS THE KEYBOARD: a line across its top, edge to edge
/// (`crate::column`), in the colour of a focused window's outline.
///
/// # Why a line, and why there
///
/// Because nothing else on the screen says it. The modeline takes the accent
/// only while the FOOTER holds the keyboard, and a column is not the footer:
/// a click in it gave it the keys and nothing turned blue anywhere, which is
/// the one hole in "exactly one thing is in the accent". The compositor's own
/// focus border is two pixels (`default_border pixel 2`), so this is two too
/// (`space.column.focusLine`), and a column that has the keys reads the way a
/// window that has them does.
///
/// # Why the borders' colour
///
/// For the same reason, and as loud: with discreet borders (`LEADER s t`) the
/// focused window's outline is `border.discreet`, and an accent line on the
/// column would be the one loud outline left on the screen. So it is the
/// accent or the discreet outline, whichever the person chose
/// (`crate::theme::focus_line`), asked at the moment it is drawn.
///
/// # Why it moves nothing
///
/// It lies in the LEADING above line 0, which nothing in a column inks: a line
/// is 1.45 em (26.1 px at 18 px) and the font's box 1.25 em, so the box starts
/// ~1.8 px under the top and the ink of a glyph lower still. So it is drawn OVER
/// everything — a stage picture included — without taking a pixel of room from
/// anything, and turning it on or off changes no span and no mark.
#[must_use]
pub fn focus_line(width_px: f32) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        x: 0.0,
        y: 0.0,
        width: width_px,
        height: f32::from(tokens::space::COLUMN_FOCUSLINE),
        color: crate::theme::focus_line(),
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
    /// A key notation that exists ONLY because of what is under the cursor.
    ///
    /// A film's rewind, a shape's zoom, a playlist's next track: keys that are
    /// on the line now and will not be on it a row later. They were taught
    /// unconditionally and the owner asked for both halves — that they appear
    /// only in their context, *and* that they read differently from the keys
    /// that are always there. A reader who has learnt the fixed ones should be
    /// able to tell at a glance which of these is a passing guest.
    Contextual,
    /// The words around a key.
    Prose,
    /// The mark between one shortcut and the next.
    Separator,
    /// The SENTENCE about the shortcut under the pointer, a line further down.
    ///
    /// It was drawn in [`Register::Prose`] — the hint's own words — and the
    /// owner read it off the screen: *"a cor do texto está igual à dos textos
    /// dos outros atalhos, assim não criando uma boa distinção visual"*. It is
    /// not one more shortcut on that row; it is the answer to a question
    /// somebody asked by putting the pointer somewhere, and the only line on
    /// the screen that exists because they did.
    ///
    /// Its tone is told apart from BOTH sides, which is what he asked for: the
    /// registers above it, and the content that begins on the row below. See
    /// `text.subtle.explain` for why the answer is no colour at all.
    Explain,
    /// WHAT THE MINIBUFFER IS SAYING — the answer, rather than the words
    /// around it.
    ///
    /// It resolves to the same token [`Register::Key`] does, and that is
    /// deliberate rather than a duplicate: `colors.nix` makes the same
    /// decision about `label.acts` and `state.accent`, and for the same
    /// reason. A key and an answer are drawn at the same weight and they are
    /// NOT the same thing — `laid` splits a key at its modifier so no ligature
    /// can cross it, and doing that to a filename would cut it in half.
    ///
    /// It exists because a sentence on that line has two halves and they were
    /// drawn as one: the owner read a cut label in the panel and asked that
    /// *"a cor da legenda e do valor devem se diferenciar"*. The half that
    /// NAMES a fact takes [`Register::Prose`], the half that IS the fact takes
    /// this one — which is the pair the hint line already puts side by side,
    /// and which every theme therefore already keeps apart.
    Said,
    /// THE GLYPH IN FRONT OF A HEADER'S TITLE — the one accent a header has,
    /// saying what the window is at a glance.
    Glyph,
    /// A LABEL IN FRONT OF A TITLE, the way `MANUAL:` stands before the name
    /// of the manual: what KIND of window this is, in the discreet cool grey
    /// the owner chose for it, so the name after it is what reads.
    Label,
    /// THE TITLE OF A HEADER, in the header's own title ink
    /// (`text.header-title`) — not the content's `text.heading`, which the
    /// first section under it wears. Hierarchy is colour and air, never size.
    Title,
    /// A SHORTCUT UNDER THE POINTER, LIT — its key, in the lit tone of the
    /// register it rests in. The owner asked for every header to light the
    /// shortcut a click would press, and for the lit tones to be small
    /// variations of the resting ones (`text.subtle.lit.*`). See [`lit`].
    KeyLit,
    /// The same, for a key whose meaning follows the focused section.
    AlternateLit,
    /// The same, for a key that exists only in its context.
    ContextualLit,
    /// The same, for the words beside the key.
    ProseLit,
    /// AN ACTION OFFERED AFTER WHAT THE LINE SAYS — an event's, on the
    /// minibuffer (`crate::events`), which runs a command of the registry.
    ///
    /// Not a key and not prose: it is a VERB somebody presses, and the
    /// notification band already draws that kind of thing in a tone of its
    /// own (`notification.acts`) because an action drawn in the message's ink
    /// read as one more line of the message. The owner asked for an action to
    /// look like ONE thing wherever it is, so this is that same token — the
    /// band's button and the line's, told apart from nothing.
    Action,
    /// The same action under the pointer, where a click would land.
    ActionLit,
    /// A KEY THAT A CLICK CANNOT PRESS.
    ///
    /// Every shortcut a header teaches is a CONTROL: a click presses the key
    /// it names ([`Piece::press`]). A few notations are not one key — the
    /// drawer's `RCtrl ↑↓`, a range, a modifier held while something else is
    /// pressed — and the owner asked for those to read *"levemente
    /// diferente"*, so the hand does not reach for a control that does
    /// nothing. It is not chosen by anybody: [`laid`] gives it to every key
    /// piece that arrives without a press, so a key cannot look clickable
    /// and not be.
    Inert,
    /// THE CHORD THAT IS WAITING for its next key.
    ///
    /// Blue is what says where the next key goes, and that is not a matter of
    /// register: it is the one exception `colors.nix` already names. Named all
    /// the same, because a `_` arm would let the next register added arrive at
    /// `laid` without anybody deciding how it is shaped.
    Pending,
}

impl Register {
    /// The one place that says what colour a run of a hint is.
    #[must_use]
    pub const fn color(self) -> ronoctua_tokens::Color {
        match self {
            Self::Key => tokens::color::TEXT_SUBTLE_STRONG,
            Self::Alternate => tokens::color::TEXT_SUBTLE_ALTERNATE,
            Self::Contextual => tokens::color::TEXT_SUBTLE_CONTEXTUAL,
            Self::Prose => tokens::color::TEXT_SUBTLE_DEFAULT,
            Self::Separator => tokens::color::TEXT_SUBTLE_FAINT,
            Self::Explain => tokens::color::TEXT_SUBTLE_EXPLAIN,
            Self::Said => tokens::color::TEXT_SUBTLE_STRONG,
            Self::Pending => tokens::color::STATE_ACCENT,
            Self::Inert => tokens::color::TEXT_SUBTLE_INERT,
            Self::KeyLit => tokens::color::TEXT_SUBTLE_LIT_KEY,
            Self::AlternateLit => tokens::color::TEXT_SUBTLE_LIT_ALTERNATE,
            Self::ContextualLit => tokens::color::TEXT_SUBTLE_LIT_CONTEXTUAL,
            Self::ProseLit => tokens::color::TEXT_SUBTLE_LIT_PROSE,
            Self::Action => tokens::color::NOTIFICATION_ACTS,
            Self::ActionLit => tokens::color::NOTIFICATION_ACTS_LIT,
            Self::Glyph => tokens::color::STATE_ACCENT,
            Self::Label => tokens::color::TEXT_SUBTLE_EXPLAIN,
            Self::Title => tokens::color::TEXT_HEADER_TITLE,
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
    /// Which of the five registers it is drawn in.
    pub register: Register,
    /// WHICH SHORTCUT this run belongs to, when it belongs to one.
    ///
    /// The key's notation and the words that say what it does carry the same
    /// id; the dot between two shortcuts carries none, and neither does a line
    /// that is not a hint at all. It is what lets the pointer name the
    /// shortcut it is resting on — see [`hint_marks`] — and it travels on the
    /// PIECE rather than beside the line because the line gets WRAPPED
    /// (`crate::fold::wrap_pieces`), and a table of ranges measured before the
    /// wrap would name the wrong key on every row but the first.
    pub id: Option<&'static str>,
    /// WHAT A CLICK ON THIS RUN DOES, when it does something.
    ///
    /// Carried by both halves of a shortcut — the notation and its words —
    /// because both are what a hand points at. `None` on a key piece is a
    /// key that cannot be pressed by a click, and [`laid`] draws it in
    /// [`Register::Inert`].
    pub press: Option<Act>,
}

/// What a click on a shortcut of a hint line does.
///
/// Two answers, because a line teaches two kinds of thing: a KEY of the
/// view that has the keyboard, which a click presses through the same path
/// the keyboard takes, and a COMMAND of the registry spelt as a sequence
/// (`SUPER 0`, a compositor mode's key), which a click runs by its id —
/// a sequence is several keys, and pressing only the first would do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Press this key, as if it had been typed.
    Key(ronoctua_keymap::Key),
    /// Run this command of the registry.
    Run(&'static str),
}

impl Piece {
    /// A run of prose.
    #[must_use]
    pub fn prose(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            register: Register::Prose,
            id: None,
            press: None,
        }
    }

    /// The chord that is waiting, in the one register that is a colour.
    #[must_use]
    pub fn pending(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            register: Register::Pending,
            id: None,
            press: None,
        }
    }

    /// A whole line that the minibuffer is SAYING, in one run.
    ///
    /// For the sentences that do not come from an `Explanation` — an echo,
    /// what is being typed, a message a view raised. They have no halves to
    /// tell apart, and saying so here is what keeps the drawing to one shape.
    #[must_use]
    pub fn said(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            register: Register::Said,
            id: None,
            press: None,
        }
    }

    /// An ACTION the line offers — its label, and the command of the registry
    /// a click runs.
    ///
    /// It carries the command as its id too, so the pointer resting on it
    /// names it and the line under it reads `cmd-<id>` (`hint_sentence`), and
    /// it lights under the pointer ([`lit`]) like every shortcut does.
    #[must_use]
    pub fn action(label: impl Into<String>, command: &'static str) -> Self {
        Self {
            text: label.into(),
            register: Register::Action,
            id: Some(command),
            press: Some(Act::Run(command)),
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
            id: None,
            press: None,
        }
    }

    /// A key notation in a register the caller has already decided.
    ///
    /// [`Piece::key`] answers the two-way question — fixed, or following the
    /// focused section — which is all there was when it was written. A key that
    /// exists only in a context is a THIRD answer, and a `bool` cannot carry
    /// three.
    #[must_use]
    pub fn key_in(text: impl Into<String>, register: Register) -> Self {
        Self {
            text: text.into(),
            register,
            id: None,
            press: None,
        }
    }

    /// The same run, saying which shortcut it is part of.
    ///
    /// Only the two halves of an entry take one — the notation and its words.
    /// A separator belongs to no shortcut, and neither does prose that is not
    /// a hint.
    #[must_use]
    pub fn about(mut self, id: &'static str) -> Self {
        self.id = Some(id);
        self
    }

    /// The same run, saying which key a click on it presses.
    #[must_use]
    pub const fn pressing(mut self, key: ronoctua_keymap::Key) -> Self {
        self.press = Some(Act::Key(key));
        self
    }

    /// The same run, saying which command a click on it runs.
    #[must_use]
    pub const fn running(mut self, id: &'static str) -> Self {
        self.press = Some(Act::Run(id));
        self
    }

    /// Is this run a key notation — in any of the registers a key is drawn in?
    #[must_use]
    pub const fn is_a_key(&self) -> bool {
        matches!(
            self.register,
            Register::Key
                | Register::Alternate
                | Register::Contextual
                | Register::Inert
                | Register::KeyLit
                | Register::AlternateLit
                | Register::ContextualLit
        )
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
            id: None,
            press: None,
        }
    }
}

/// WHAT A HEADER SAYS BEFORE ITS SHORTCUTS: a glyph, a label and a title,
/// each optional.
///
/// # Why a header has one shape
///
/// Because it had several. The brain's header carried a glyph and a name on a
/// ground of its own, the manual's window a grey `MANUAL:` and the name, and
/// every other window only its keys — each written by hand, each in its own
/// colours, and the brain's on a ground the hint's tones were never measured
/// against. The owner asked for the header to be ONE thing in layout and in
/// behaviour, with a title, an icon before it or a label before it foreseen
/// for any window that wants one. This is the one: the pieces go in front of
/// the shortcuts, on the same line, through the same painter, in the three
/// registers that say what each is.
#[derive(Debug, Clone, Copy, Default)]
pub struct Title<'a> {
    /// A glyph from the icon font, before everything.
    pub glyph: Option<&'static str>,
    /// What kind of window this is — `MANUAL:`.
    pub label: Option<&'a str>,
    /// The window's own name.
    pub name: Option<&'a str>,
}

impl Title<'_> {
    /// These pieces, then a separator, then the shortcuts.
    #[must_use]
    pub fn before(self, shortcuts: Vec<Piece>) -> Vec<Piece> {
        let mut out: Vec<Piece> = Vec::with_capacity(shortcuts.len() + 5);
        let mut put = |text: String, register: Register| {
            if !out.is_empty() {
                // ONE CELL OF AIR between the parts, carried by the part that
                // comes after — the rule `fold::wrap_pieces` keeps.
                out.push(Piece::prose(" "));
            }
            out.push(Piece::key_in(text, register));
        };
        if let Some(g) = self.glyph {
            put(g.to_owned(), Register::Glyph);
        }
        if let Some(l) = self.label {
            put(l.to_owned(), Register::Label);
        }
        if let Some(n) = self.name {
            put(n.to_owned(), Register::Title);
        }
        if !out.is_empty() && !shortcuts.is_empty() {
            out.push(Piece::separator());
        }
        out.extend(shortcuts);
        out
    }
}

/// The line, with the shortcut `id` LIT — both halves of it, the key and its
/// words — when it is one a click can press.
///
/// ONE function for every header of the system (the door's, the file
/// manager's line 0, the footer's minibuffer), so a lit shortcut is lit the
/// same way wherever a line of shortcuts is drawn. A shortcut that is only
/// read (`Register::Inert`, no press) does not light: nothing would happen
/// under the click the light promises.
#[must_use]
pub fn lit(pieces: &[Piece], id: Option<&str>) -> Vec<Piece> {
    let Some(id) = id else {
        return pieces.to_vec();
    };
    pieces
        .iter()
        .map(|p| {
            if p.id != Some(id) || p.press.is_none() {
                return p.clone();
            }
            let register = match p.register {
                Register::Key => Register::KeyLit,
                Register::Alternate => Register::AlternateLit,
                Register::Contextual => Register::ContextualLit,
                Register::Prose => Register::ProseLit,
                Register::Action => Register::ActionLit,
                other => other,
            };
            Piece {
                register,
                ..p.clone()
            }
        })
        .collect()
}

/// A line that SAYS something and offers what can be done about it: the
/// sentence, then each action, a dot before every one.
///
/// The minibuffer's line for an event (`crate::events`) whose catalogue entry
/// has actions. The dot is the hint line's own ([`Piece::separator`]), for
/// the reason the band gives for its: labels side by side in one tone read as
/// one sentence, and an action's label IS a sentence.
#[must_use]
pub fn said_with_actions(
    said: impl Into<String>,
    actions: &[(String, &'static str)],
) -> Vec<Piece> {
    let mut out = vec![Piece::said(said)];
    for (label, command) in actions {
        out.push(Piece::separator());
        out.push(Piece::action(label.clone(), command));
    }
    out
}

/// Where the actions at the end of a line begin — the dot before the first
/// of them included, because it is theirs — or `None` for a line with none.
fn actions_from(pieces: &[Piece]) -> Option<usize> {
    let first = pieces
        .iter()
        .position(|p| matches!(p.register, Register::Action | Register::ActionLit))?;
    Some(
        if first > 0 && pieces[first - 1].register == Register::Separator {
            first - 1
        } else {
            first
        },
    )
}

/// The cells a run of pieces takes.
fn cells_of(pieces: &[Piece]) -> u16 {
    pieces
        .iter()
        .map(|p| ronoctua_render::layout::width(&p.text))
        .sum()
}

/// The same line fitted to `room` cells with its actions FLUSH AT THE END:
/// the sentence is cut when it is too long for what the actions leave, and
/// followed by air when it is short.
///
/// # Why the sentence gives way
///
/// [`laid`] cuts whatever overruns, which is the END of a line — and on a line
/// that ends with its actions, the end is the one part a click needs. So the
/// actions' cells come off the room before the words are measured, the rule
/// the band keeps for the button at the end of a line it had to cut.
///
/// # Why at the end of the room, and not right after the words
///
/// Because what an action DOES is said in the same place the event is: there
/// is no row under the line at rest (`Footer::explain_rows` takes its row out
/// of the region, and at rest there is none), so the sentence about the
/// action under the pointer takes the words' place ([`explained_in_place`]).
/// With the actions right after the words, a sentence of another length would
/// move them out from under the pointer that asked, the hover would end, the
/// words would come back — the loop `Footer::explain_rows` is written about.
/// Anchored to the end of the room, they never move while the line is up.
///
/// A ROOM TOO NARROW FOR BOTH keeps the sentence, and the band's rule again
/// says why: an action after a sentence nobody could read offers to act on
/// nothing. The line's drawer (`crate::fold`) shows the whole of it anyway.
///
/// A line with no action comes back as it went in.
#[must_use]
pub fn room_for_actions(pieces: &[Piece], room: u16) -> Vec<Piece> {
    let Some(from) = actions_from(pieces) else {
        return pieces.to_vec();
    };
    let tail = cells_of(&pieces[from..]);
    let Some(head) = room.checked_sub(tail).filter(|h| *h >= tail) else {
        return pieces.to_vec();
    };
    let mut out = Vec::with_capacity(pieces.len() + 1);
    let mut left = head;
    for p in &pieces[..from] {
        let w = ronoctua_render::layout::width(&p.text);
        if w <= left {
            out.push(p.clone());
            left -= w;
        } else {
            if left > 0 {
                out.push(Piece {
                    text: ronoctua_render::layout::elide_end(&p.text, left),
                    ..p.clone()
                });
            }
            left = 0;
        }
    }
    // THE AIR THAT PUTS THE ACTIONS AT THE END, as a run of its own: carried
    // by the sentence it would be drawn in the sentence's ink, and a register
    // that ever grows a background would paint it.
    if left > 0 {
        out.push(Piece::prose(" ".repeat(usize::from(left))));
    }
    out.extend(pieces[from..].iter().cloned());
    out
}

/// The same line with `sentence` where its words were — the sentence about
/// the action under the pointer — in exactly the cells the words took, so
/// the actions stay where they are.
///
/// Drawn in [`Register::Explain`], the register the answer to a pointer's
/// question has on every other line. A line with no action is left alone:
/// there is nothing on it for the pointer to have asked about.
#[must_use]
pub fn explained_in_place(pieces: &[Piece], sentence: &str) -> Vec<Piece> {
    let Some(from) = actions_from(pieces) else {
        return pieces.to_vec();
    };
    let head = cells_of(&pieces[..from]);
    if head == 0 {
        return pieces.to_vec();
    }
    let text = ronoctua_render::layout::elide_end(sentence, head);
    let air = head.saturating_sub(ronoctua_render::layout::width(&text));
    let mut out = vec![Piece {
        text,
        register: Register::Explain,
        id: None,
        press: None,
    }];
    if air > 0 {
        out.push(Piece::prose(" ".repeat(usize::from(air))));
    }
    out.extend(pieces[from..].iter().cloned());
    out
}

/// The one key every view of the footer's region teaches first: `Esc`, the
/// way back.
///
/// # Why it is on the line at all
///
/// Because a view opened in the region had no way back for the POINTER. `Esc`
/// goes up one level or closes, and nothing on screen was a control that did
/// that — the owner asked for it by name. A key on a hint line is a click
/// (`Piece::pressing`), so the way back is this row of a table and nothing
/// else: the click travels the keyboard's own path, and each view keeps
/// deciding what its `Esc` means.
pub const OF_THE_WAY_BACK: &[(&str, &str)] = &[("ESC", "view-back")];

/// A view's hint with the way back in front of it — unless the view already
/// teaches `Esc` itself, which then stays where the view put it.
///
/// FIRST, because the line is elided at the far end: the way out is the one
/// key that must never be the part cut off.
#[must_use]
pub fn with_the_way_back(hint: Vec<Piece>, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
    let esc = ronoctua_keymap::Key::parse("ESC").ok();
    if hint
        .iter()
        .any(|p| matches!(p.press, Some(Act::Key(k)) if Some(k) == esc))
    {
        return hint;
    }
    let mut out = hint_of(OF_THE_WAY_BACK, texts);
    if !hint.is_empty() {
        out.push(Piece::separator());
        out.extend(hint);
    }
    out
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
pub fn hint_of(table: &[(&'static str, &'static str)], texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
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
    table: &[(&'static str, &'static str)],
    means: impl Fn(&str) -> Option<&'static str>,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Piece> {
    let marked: Vec<(&'static str, &'static str, bool)> =
        table.iter().map(|(n, id)| (*n, *id, false)).collect();
    hint_of_marked(&marked, means, texts)
}

/// The same line, where some of the keys exist only in the CONTEXT showing.
///
/// The third column says so, and those keys are drawn in
/// [`Register::Contextual`]: a film's rewind is on the line while a film is
/// under the cursor and gone a row later, and a reader who has learnt the
/// fixed keys should be able to tell which of these is a passing guest.
///
/// A view that gates its table by a `Where` filters the rows away first and
/// marks what survives; a view with no contexts passes `false` throughout,
/// which is what [`hint_of_with`] does for it.
pub fn hint_of_marked(
    table: &[(&'static str, &'static str, bool)],
    means: impl Fn(&str) -> Option<&'static str>,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Piece> {
    let mut out = Vec::with_capacity(table.len() * 3);
    let mut said: Vec<&str> = Vec::with_capacity(table.len());
    for (notation, id, contextual) in table {
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
        // WHICH ID THE POINTER WILL BE TOLD ABOUT, and it is `says` and not the
        // table's own. A key whose meaning follows the focused section says
        // what it does NOW: resting on `S-<left>` over a film has to explain
        // rewinding, not the parent directory that the same row of the table
        // spells when nothing is playing. What a `means` can answer is part of
        // what `every-hint-key-explains-itself` covers, so the resolved id has
        // a sentence for the same reason the table's own does.
        let about = says;
        // ALTERNATE WINS OVER CONTEXTUAL, and a key is never both in this
        // tree: the four that follow the focused section are always on the
        // line, and what changes is what they mean.
        out.push(
            Piece::key_in(
                key_text(&t),
                if alternate {
                    Register::Alternate
                } else if *contextual {
                    Register::Contextual
                } else {
                    Register::Key
                },
            )
            .about(about)
            // A CLICK ON IT IS THE KEY — the notation parsed, so it is one.
            .pressing(t),
        );
        // THE SPACE BETWEEN THE KEY AND ITS WORDS BELONGS TO THE WORDS, for
        // the reason `fold::wrap_pieces` gives about the other seam: a key
        // piece has to be exactly the notation, because that is what the
        // colour is about.
        out.push(
            Piece::prose(format!(" {}", texts.t(says)))
                .about(about)
                .pressing(t),
        );
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
    clock_of(
        &ronoctua_files::when::civil(secs, zone),
        clock,
        false,
        texts,
    )
}

/// The same clock, with its seconds.
///
/// For the line that EXPLAINS, never for a column: a column is scanned and has
/// to line up, and three more cells on every row of it buy nothing. The
/// pointer's sentence has a whole line and was asked for the exact instant.
#[must_use]
pub fn clock_exact(
    c: &ronoctua_files::when::Civil,
    clock: crate::locale::Clock,
    texts: &ronoctua_i18n::Texts,
) -> String {
    clock_of(c, clock, true, texts)
}

/// Both clocks, from a wall clock that is already local.
///
/// # Why the arithmetic is in one function and not two
///
/// Because noon and midnight are the cases that go wrong — `0` is twelve at
/// night and `12` is twelve at noon, and a bare `hour % 12` gives zero for
/// both — and a second copy of that is a second chance to get it wrong. The
/// signature of `clock_text` does not change either, because it has five
/// callers outside this file and none of them is about this.
fn clock_of(
    c: &ronoctua_files::when::Civil,
    clock: crate::locale::Clock,
    exact: bool,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let seconds = format!("{:02}", c.second);
    match clock {
        crate::locale::Clock::TwentyFour => {
            let mut args: Vec<(&str, &str)> = Vec::with_capacity(3);
            let hh = format!("{:02}", c.hour);
            let mm = format!("{:02}", c.minute);
            args.push(("hh", &hh));
            args.push(("mm", &mm));
            if exact {
                args.push(("ss", &seconds));
            }
            texts.t_with(
                if exact {
                    ronoctua_i18n::key::LOCALE_CLOCK_EXACT_24
                } else {
                    ronoctua_i18n::key::LOCALE_CLOCK_SAMPLE_24
                },
                &args,
            )
        }
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
            let hh = hour.to_string();
            let mm = format!("{:02}", c.minute);
            let mut args: Vec<(&str, &str)> = Vec::with_capacity(4);
            args.push(("hh", &hh));
            args.push(("mm", &mm));
            if exact {
                args.push(("ss", &seconds));
            }
            args.push(("half", &half));
            texts.t_with(
                if exact {
                    ronoctua_i18n::key::LOCALE_CLOCK_EXACT_12
                } else {
                    ronoctua_i18n::key::LOCALE_CLOCK_SAMPLE_12
                },
                &args,
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

/// THE ARGUMENT THAT NAMES A FACT, rather than being it.
///
/// Every other argument of an explanation IS the fact — a filename, an owner,
/// a byte count, a date — so it takes the loud register and the words around
/// it take the quiet one. `$label` is the exception, and there is exactly ONE
/// key in the tree that interpolates it (`files-fact-says`): it is the NAME of
/// the fact, the quiet half in the panel, and it stays the quiet half here.
///
/// The alternative was carrying a register on `mode::Explanation`, which would
/// have put a drawing decision inside `ronoctua-files` and touched thirty call
/// sites to say nothing at twenty-nine of them.
const NAMES_RATHER_THAN_SAYS: &str = "label";

/// An explanation, as the runs the message line draws.
///
/// # Why the halves are told apart here and not in the sentence
///
/// Because the ORDER is the language's. `ronoctua_i18n::Marked` hands back one
/// byte range per interpolated argument **in reading order**, which is the
/// only honest source for where a translation put each one — searching the
/// finished sentence for the value is what `t_marked`'s own doc rejects, and
/// for good reason.
///
/// # How a range is matched to the argument that made it
///
/// By its value, consuming each argument once, in the order they were passed.
/// The ranges are authoritative — Fluent said where each argument landed — so
/// this is not a search through prose; it is asking which of two or three
/// known strings a known slice is.
///
/// The one thing that can go wrong is two arguments whose values are
/// byte-identical, and then the two are swapped. That is invisible by
/// construction: the runs read the same, and the only difference is which of
/// them is drawn quiet — which for `label: value` would mean a fact whose name
/// is its own value.
#[must_use]
pub fn sentence(e: &ronoctua_files::mode::Explanation, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
    let borrowed = e.borrowed();
    let marked = texts.t_marked(e.key, &borrowed);
    let mut taken = vec![false; borrowed.len()];
    let mut out = Vec::with_capacity(marked.at.len() * 2 + 1);
    for (run, is_arg) in marked.pieces() {
        let register = if is_arg {
            let which = borrowed
                .iter()
                .enumerate()
                .find(|(n, (_, v))| !taken[*n] && *v == run)
                .map(|(n, (name, _))| {
                    taken[n] = true;
                    name
                });
            match which {
                Some(name) if *name == NAMES_RATHER_THAN_SAYS => Register::Prose,
                // AN ARGUMENT WITH NO MATCH IS STILL AN ARGUMENT. Fluent may
                // have reshaped it — a number formatted for the language, a
                // selector's branch — and what it is does not change: it is
                // the thing the sentence is about.
                _ => Register::Said,
            }
        } else {
            Register::Prose
        };
        out.push(Piece {
            text: run.to_owned(),
            register,
            id: None,
            press: None,
        });
    }
    out
}

/// A date written out, with the month as a WORD.
///
/// # Why this exists beside `date_text`
///
/// Because a column and a sentence are asked different questions. `date_text`
/// answers the COLUMN — an order, two digits a field, everything the same
/// width so the eye can scan down it. This answers the pointer, which has a
/// whole line and asked *which day is this*, and the owner asked for it in as
/// many words: *"em vez de mostrar no minibuffer algo como 'e 20 segundos',
/// quero que mostre a data completa no idioma do usuário"*.
///
/// # Why the ORDER is still the person's
///
/// The obvious argument is that a written-out date has no ambiguity to undo —
/// `3 de setembro` cannot be read two ways, which is the whole reason the
/// preference exists for `03/09`. It is wrong on the side that matters:
/// somebody who chose *month first* and got `3 de setembro` would read a
/// preference being ignored. So there are three keys here as there are three
/// there, and each language writes its three as that language writes them.
///
/// The day and the year are NOT padded here, unlike `Civil::args`: `03 de
/// setembro` is a column's spelling, and this is not a column.
#[must_use]
pub fn date_long(
    c: &ronoctua_files::when::Civil,
    order: crate::locale::Order,
    texts: &ronoctua_i18n::Texts,
) -> String {
    // THE MONTH ARRIVES AS A NUMBER, and that is load-bearing: the twelve
    // names are one key with a Fluent selector — which numbers get a word of
    // their own is the LANGUAGE's business — and `"01"` does not match `[1]`.
    let month = texts.t_args(
        ronoctua_i18n::key::LOCALE_MONTH,
        &[("m", ronoctua_i18n::Arg::Number(i64::from(c.month)))],
    );
    texts.t_with(
        match order {
            crate::locale::Order::BigEndian => ronoctua_i18n::key::LOCALE_DATE_LONG_YMD,
            crate::locale::Order::DayFirst => ronoctua_i18n::key::LOCALE_DATE_LONG_DMY,
            crate::locale::Order::MonthFirst => ronoctua_i18n::key::LOCALE_DATE_LONG_MDY,
        },
        &[
            ("d", &c.day.to_string()),
            ("month", &month),
            ("y", &c.year.to_string()),
        ],
    )
}

/// A whole instant in words, from a wall clock.
///
/// # Why it takes a `Civil` and never a `(secs, zone)`
///
/// Because the other caller of this is a date a CAMERA wrote, and a camera
/// writes no offset: `DateTimeOriginal` is the wall clock where the shutter
/// opened. Turning it into seconds-since-the-epoch so it could share the
/// signature below is the tempting edit and it is the wrong one — it would
/// show a photograph taken in Tokyo at a London hour, silently, and no test
/// running in one zone would ever catch it. Taking the six numbers as written
/// is what makes that edit impossible rather than merely discouraged.
#[must_use]
pub fn civil_long(c: &ronoctua_files::when::Civil, texts: &ronoctua_i18n::Texts) -> String {
    // THE SHAPE IS THE PERSON'S, from the process-wide value the theme is
    // adopted by — see `locale::prefs::current`.
    let choice = crate::locale::prefs::current();
    let date = date_long(c, choice.order, texts);
    let time = clock_exact(c, choice.clock, texts);
    texts.t_with(
        ronoctua_i18n::key::FILES_WHEN_LONG,
        &[("date", date.as_str()), ("time", time.as_str())],
    )
}

/// The same sentence, for an instant this machine can place in a zone.
#[must_use]
pub fn when_long(
    secs: i64,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    civil_long(&ronoctua_files::when::civil(secs, zone), texts)
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
    laid(pieces, column, room)
        .into_iter()
        .map(|r| ronoctua_render::Span {
            size,
            text: r.text,
            column: r.at,
            line,
            color: r.register.color(),
            edge: false,
            foot: false,
        })
        .collect()
}

/// WHERE A SHORTCUT WAS DRAWN, so the pointer can name the one it is on.
///
/// One per run of the line that belongs to a shortcut — the notation and the
/// words that say what it does, which is the whole of what a person would
/// point at. The dot between two of them belongs to neither and is not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// Which line of the surface it was drawn on.
    pub line: u16,
    /// The first cell it occupies.
    pub at: u16,
    /// How many cells it occupies.
    pub cells: u16,
    /// The shortcut's id.
    pub id: &'static str,
    /// What a click on it does — `None` for a key that is only read.
    pub press: Option<Act>,
}

impl Mark {
    /// Does this run cover that cell?
    #[must_use]
    pub const fn covers(&self, line: u16, cell: u16) -> bool {
        self.line == line && cell >= self.at && cell < self.at + self.cells
    }
}

/// The same line, as the marks the pointer asks about.
///
/// # Why it is not a second arithmetic
///
/// Because it is the SAME one: [`laid`] places the runs, and this and
/// [`hint_spans`] are two readings of its answer. Written twice — once to draw
/// and once to hit-test — the two would drift the first time an elision moved,
/// and the symptom would be a line explaining the key next to the one under
/// the pointer, which is worse than explaining nothing.
#[must_use]
pub fn hint_marks(pieces: &[Piece], column: u16, line: u16, room: u16) -> Vec<Mark> {
    laid(pieces, column, room)
        .into_iter()
        .filter_map(|r| {
            Some(Mark {
                line,
                at: r.at,
                cells: ronoctua_render::layout::width(&r.text),
                id: r.id?,
                press: r.press,
            })
        })
        .filter(|m| m.cells > 0)
        .collect()
}

/// The long sentence for a shortcut, for the line under the pointer.
///
/// # Two namespaces, and why there are two
///
/// `cmd-<id>` is the long form of a COMMAND — what which-key and the manual
/// read, and what a hundred of these ids already have because they are in the
/// registry. The rest are a view's own keys: `files-preview`, `notif-read`,
/// `transfers-pause`. Giving those a `cmd-` key would say they are registered
/// commands, which is exactly the thing `cmd-<id>` means; they take `about-`
/// instead. One id never has both — `every-hint-key-explains-itself` refuses
/// that, because two sentences for one key is two places to keep in step.
///
/// `None` when neither exists. The line then shows nothing rather than the
/// `[about-x]` marker, which is a note for a screenshot and not a sentence.
#[must_use]
pub fn hint_sentence(id: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
    let command = format!("cmd-{id}");
    if texts.has(&command) {
        return Some(texts.t(&command));
    }
    let about = format!("about-{id}");
    texts.has(&about).then(|| texts.t(&about))
}

/// A WINDOW'S HEADER: the line that teaches its keys, the drawer that opens
/// it, and the shortcut the pointer is resting on it.
///
/// # Why the ten doors share a struct and the footer does not
///
/// Because the ten are one shape — a header on line 0 and a list under it —
/// and what is kept for it is the same four things each: where every shortcut
/// landed, what the pointer is on, whether the line is open and how far down
/// it is scrolled. Ten copies of that is ten places for the answer to drift,
/// which is what this whole module exists to stop — and the drawer is the
/// proof of it, because it was written for the footer, written again for the
/// file manager's window, and written in none of the other ten.
///
/// The footer keeps its own (`Footer::hint_placed`, `Footer::hint_said`,
/// `Footer::fold`) because there the row is not free: it comes out of the
/// region and never off the footer's height, for a reason written in full on
/// `Footer::explain_rows`. A window is not anchored to the bottom of the
/// screen and simply shows one row fewer of its list.
#[derive(Debug, Clone)]
pub struct Hinted {
    /// The hint, as the LAST FRAME laid it out.
    placed: Vec<Mark>,
    /// The sentence about the shortcut under the pointer, while there is one.
    said: Option<String>,
    /// Whether the line is OPEN, and how far down it is scrolled.
    ///
    /// # Why the ten windows have one at all
    ///
    /// Because the line is CUT, and what it carries is a table of shortcuts
    /// that is routinely longer than the window. The footer's minibuffer has
    /// opened into a drawer since `crate::fold` was written, and so has the
    /// file manager's window; the other ten doors drew the same line with an
    /// ellipsis at the end of it and no way to see the rest. The owner found
    /// it on the language and region window — *"a linha do header com os
    /// atalhos não tem a feature com o ícone de chevron para expandir/recolher
    /// o header"* — and asked for every door to be checked.
    ///
    /// It is HERE and not in each door for the reason the marks are: ten
    /// copies of a drawer is ten places for the answer to drift.
    fold: crate::fold::Fold,
    /// The cell the chevron was DRAWN at — always one, once a frame is drawn.
    ///
    /// Written down by the painter and read by the pointer — a control is
    /// taken where it was drawn, never where a second sum says it should be.
    control: Option<u16>,
    /// The drawer's bar, from the wrap that was drawn.
    scroll: ronoctua_design::Scroll,
    /// How tall the line came to at this width, for [`Hinted::settled`].
    ///
    /// The height is asked for BEFORE the frame and the wrap is only known
    /// inside it, so the drawing writes this down and the settling pass reads
    /// it back — once. See `fold::Fold::held`.
    wrapped: usize,
    /// Is the drawer's bar being dragged?
    dragging: bool,
    /// A door with a drawer of its OWN — the file manager's window — says
    /// here whether it is open, because the sentence below follows whichever
    /// drawer the line has (see [`Hinted::rows`]).
    open_elsewhere: bool,
    /// The shortcut under the pointer, LIT when it is drawn — see [`lit`].
    lit: Option<&'static str>,
    /// Whether the pointer rests on the chevron, which lights too.
    chevron_lit: bool,
    /// Is the RIGHT `Ctrl` down?
    ///
    /// Tracked by its keysym, as the footer tracks it: xkb merges both Ctrls
    /// into one bit and the modifier event does not say which key went down.
    /// It is the one modifier family free in every view by construction — no
    /// view sees it — and this line belongs to every view, which is why its
    /// keys live on it. See `Hinted::took_a_key`.
    right_ctrl: bool,
    /// WHICH LINE the header is on — `0` in every window, and `1` in a
    /// column, whose line 0 is the column's own control line
    /// (`crate::column`).
    ///
    /// Inside the header and not an offset the column adds around it:
    /// the door asks `light_at`, `press_under` and `drawer_track` in the
    /// surface's own coordinates, so a wrapper translating them would have
    /// to be every one of those calls, and the one forgotten would take a
    /// click a line away from where the header was drawn.
    top: u16,
}

impl Default for Hinted {
    fn default() -> Self {
        Self {
            placed: Vec::new(),
            said: None,
            fold: crate::fold::Fold::default(),
            control: None,
            scroll: ronoctua_design::Scroll {
                total: 0,
                lines: 0,
                offset: 0,
                top: 0,
            },
            wrapped: 0,
            dragging: false,
            open_elsewhere: false,
            lit: None,
            chevron_lit: false,
            right_ctrl: false,
            top: 0,
        }
    }
}

impl Hinted {
    /// A header drawn on line `top` — see [`Hinted::top`].
    #[must_use]
    pub fn at(top: u16) -> Self {
        Self {
            top,
            ..Self::default()
        }
    }

    /// The line the header is on.
    #[must_use]
    pub const fn top(&self) -> u16 {
        self.top
    }

    /// Takes the header OFF the surface: no shortcut, no chevron and no
    /// sentence is left where the pointer could find it, and the drawer
    /// shuts. For a door that stops drawing a header — the column in its bar
    /// — whose last frame's marks would otherwise still answer a click on
    /// what is no longer there.
    pub fn withdraw(&mut self) {
        self.placed.clear();
        self.control = None;
        self.said = None;
        self.lit = None;
        self.chevron_lit = false;
        self.dragging = false;
        let _ = self.fold.close();
    }

    /// Swaps in the per-pointer half of this header: whether a pointer is
    /// dragging the drawer's bar.
    ///
    /// A door calls it twice around every pointer frame, with the state of
    /// the pointer that spoke (`crate::seats`): in, so the header answers
    /// that pointer, and out, so the drag is kept for that pointer and no
    /// other. Without it a second cursor moving over the header would carry
    /// the first one's drag along.
    pub fn swap_hand(&mut self, hand: &mut bool) {
        std::mem::swap(&mut self.dragging, hand);
    }

    /// The same, for a door that draws the line over SEVERAL rows and so
    /// collects the marks itself — the file manager's window, whose hint line
    /// opens into a drawer.
    pub fn took(&mut self, marks: Vec<Mark>) {
        self.placed = marks;
    }

    /// Which shortcut the pointer is resting on, at a window's own pixels.
    #[must_use]
    pub fn under(&self, x: f64, y: f64) -> Option<&'static str> {
        let line_height = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let cell_px = f64::from(cell_width());
        if line_height <= 0.0 || cell_px <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_height) as u16;
        let cell = u16::try_from((x / cell_px) as u64).ok()?;
        self.placed
            .iter()
            .find(|m| m.covers(line, cell))
            .map(|m| m.id)
    }

    /// What a click at that point does — the shortcut under it, when it is a
    /// control. See [`Piece::press`].
    #[must_use]
    pub fn press_under(&self, x: f64, y: f64) -> Option<Act> {
        let line_height = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let cell_px = f64::from(cell_width());
        if line_height <= 0.0 || cell_px <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_height) as u16;
        let cell = u16::try_from((x / cell_px) as u64).ok()?;
        self.placed
            .iter()
            .find(|m| m.covers(line, cell))
            .and_then(|m| m.press)
    }

    /// Lights the shortcut a click at that point would press — or none — and
    /// says whether that changed anything. Only a CONTROL lights: a read-only
    /// key under the pointer is not one.
    pub fn light_at(&mut self, at: Option<(f64, f64)>) -> bool {
        let now = at.and_then(|(x, y)| {
            self.press_under(x, y)?;
            self.under(x, y)
        });
        // AND THE CHEVRON, the header's other control: the cell it was DRAWN
        // in, and the air beside it (`on_the_control`'s own rule).
        let line_height = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let cell_px = f64::from(cell_width());
        // ON THE HEADER'S OWN LINE, which is not line 0 in a column.
        let row = line_height * f64::from(self.top);
        let chevron = at.is_some_and(|(x, y)| {
            line_height > 0.0
                && cell_px > 0.0
                && y >= row
                && y < row + line_height
                && self.control.is_some_and(|c| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let cell = (x / cell_px) as u16;
                    cell + 1 >= c
                })
        });
        let changed = now != self.lit || chevron != self.chevron_lit;
        self.lit = now;
        self.chevron_lit = chevron;
        changed
    }

    /// The shortcut that is lit, for a door that draws its line itself.
    #[must_use]
    pub const fn lit(&self) -> Option<&'static str> {
        self.lit
    }

    /// Puts the sentence on the line, and says whether that changed anything.
    ///
    /// The answer is what decides a repaint: a `Motion` arrives per pixel, and
    /// moving along one key's own words changes nothing at all.
    pub fn say(&mut self, said: Option<String>) -> bool {
        if said == self.said {
            return false;
        }
        self.said = said;
        true
    }

    /// How many lines the header costs UNDER line 0: the drawer, and the
    /// sentence about the shortcut the pointer is on.
    ///
    /// The sentence is one row while a shortcut is under the pointer and none
    /// otherwise — the owner chose that over a row that stands empty at rest,
    /// *"só existe enquanto há hover"*. And ONLY WITH THE LINE OPEN: a
    /// sentence that appeared under a folded line whenever the hand crossed
    /// it moved the list under it by a row, and the owner asked for it to
    /// belong to the opened line — what one opens to read the keys in full.
    #[must_use]
    pub fn rows(&self) -> u16 {
        self.folded_rows() + u16::from(self.said().is_some())
    }

    /// Whether the line is open — its own drawer, or the door's.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.fold.open || self.open_elsewhere
    }

    /// For a door whose line opens into a drawer of its own: whether it is
    /// open, which is what shows the sentence under it.
    pub const fn open_elsewhere(&mut self, open: bool) {
        self.open_elsewhere = open;
    }

    /// How many rows the OPENED line takes under itself. `0` while it is shut.
    #[must_use]
    pub fn folded_rows(&self) -> u16 {
        crate::fold::rows(self.fold)
    }

    /// The sentence, when there is one and the line is open to show it.
    #[must_use]
    pub fn said(&self) -> Option<&str> {
        self.said.as_deref().filter(|_| self.is_open())
    }

    /// LINE 0 OF A WINDOW, and the drawer it opens into.
    ///
    /// # Why the painting is here and not in the ten doors
    ///
    /// Because it was in the ten doors, and they had ten chances to answer
    /// differently. What this draws is: the line — the view's shortcuts, or
    /// what the last order said — the chevron on its right edge when it does
    /// not fit, the wrapped rest of it when the chevron has been pressed, and
    /// the sentence about the shortcut the pointer is resting on. Every one of
    /// those was written out in each window, and the drawer was written in
    /// none of them.
    ///
    /// `hinting` says whether the runs are SHORTCUTS. When they are, each one
    /// is recorded where it landed so the pointer can name it, and the keys
    /// that walk the drawer are taught inside it (see `fold::teaching`); when
    /// the line is carrying a message there is no shortcut on it to point at.
    ///
    /// ONE PAINTER for both, which is a correction this module has already
    /// made for the footer: a message went through `elide_end` with a
    /// hand-written token beside a hint that went through [`hint_spans`], and
    /// the same sentence came out in two colours in two doors.
    pub fn head(
        &mut self,
        pieces: &[Piece],
        hinting: bool,
        margin: u16,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<ronoctua_render::Span> {
        let size = tokens::text::size();
        // THE SHORTCUT UNDER THE POINTER, LIT — see [`lit`].
        let mut line: Vec<Piece> = lit(pieces, self.lit);
        // AND THE KEYS THAT OPEN AND WALK IT, open or shut — see
        // `fold::teaching`.
        if hinting {
            line.extend(crate::fold::teaching(texts));
        }
        // THE CONTROL TAKES ITS CELLS BEFORE THE TEXT IS MEASURED, exactly as
        // the sidebar's eject glyph does: a control drawn over text that was
        // measured without it is a control that covers a word.
        //
        // It is ALWAYS there, the owner's rule: it used to be offered only when
        // the line would be cut, but opening the line is also what shows the
        // sentence about the key under the pointer, and a line that fits still
        // has that to show.
        let room = columns.saturating_sub(crate::fold::WIDTH);
        self.wrapped = crate::fold::wrap_pieces(&line, room).len();
        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let mut marks: Vec<Mark> = Vec::new();
        if self.fold.open {
            // OPENED, the line is its own first row and the rest follow under
            // it.
            let (showing, scroll) = crate::fold::opened(&line, room, self.fold);
            for (n, row) in showing.into_iter().enumerate() {
                let at = self.top + u16::try_from(n).unwrap_or(0);
                spans.extend(hint_spans(
                    &row,
                    ronoctua_render::Size::Px(size),
                    margin,
                    at,
                    room,
                ));
                // EVERY ROW OF THE DRAWER ANSWERS THE POINTER, not just the
                // first: opened, this line is where the whole table is
                // legible, which is where somebody goes looking.
                if hinting {
                    marks.extend(hint_marks(&row, margin, at, room));
                }
            }
            self.scroll = scroll;
        } else {
            spans.extend(hint_spans(
                &line,
                ronoctua_render::Size::Px(size),
                margin,
                self.top,
                room,
            ));
            if hinting {
                marks.extend(hint_marks(&line, margin, self.top, room));
            }
        }
        self.placed = marks;
        // THE CONTROL THAT OPENS THE LINE, flush against the right edge. No
        // box and no border (invariant 15): a glyph on the line that is
        // already there.
        self.control = Some(columns.saturating_sub(1) + margin);
        if let Some(at) = self.control {
            let chevron = if self.fold.open {
                tokens::icon::COLLAPSE
            } else {
                tokens::icon::EXPAND
            };
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::OnTheCell(crate::surface::chrome_size_of(chevron)),
                text: chevron.to_string(),
                column: at,
                line: self.top,
                // AND UNDER THE POINTER, LIT — the same small step.
                color: if self.chevron_lit {
                    tokens::color::TEXT_SUBTLE_LIT_KEY
                } else if self.fold.open {
                    tokens::color::TEXT_SUBTLE_STRONG
                } else {
                    tokens::color::TEXT_SUBTLE_DEFAULT
                },
                // COUNTED FROM THE BORDER, so the air after it is the door's
                // margin at every width — counted from the left, the grid's
                // remainder became air that changed as the window was resized.
                edge: true,
                foot: false,
            });
        }
        // AND THE SHORTCUT UNDER THE POINTER, SPELT OUT, on the line below the
        // header: the line the owner asked for. It costs the list one row for
        // as long as the pointer is on a key, and nothing above it moves.
        if let Some(said) = self.said() {
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: ronoctua_render::layout::elide_end(said, columns),
                column: margin,
                line: self.top + 1 + self.folded_rows(),
                color: Register::Explain.color(),
                edge: false,
                foot: false,
            });
        }
        spans
    }

    /// The opened drawer's bar, when the wrap ran past it.
    ///
    /// One rectangle, from `Scroll` — the same call every other bar in this
    /// system makes.
    #[must_use]
    pub fn bar(&self, f: Frame) -> Option<ronoctua_render::quad::Quad> {
        if !self.fold.open || !self.scroll.cut() {
            return None;
        }
        let top = f.y_of(self.top);
        let bottom =
            top + f.line() * f32::from(u16::try_from(self.scroll.lines).unwrap_or(u16::MAX));
        let (x, y, width, height) = self.scroll.bar(f.width, top, bottom, f.cell(), f.line())?;
        Some(ronoctua_render::quad::Quad {
            x,
            y,
            width,
            height,
            color: tokens::color::RULE_STRONG,
        })
    }

    /// The opened drawer's bar TRACK, when the wrap ran past it — what
    /// `crate::bars` records for the header, in every door.
    #[must_use]
    pub fn drawer_track(&self, f: Frame) -> Option<crate::bars::Track> {
        let (top, tall) = self.track(f)?;
        Some(crate::bars::Track::of((
            f.width - f.cell(),
            top,
            f.cell(),
            tall,
        )))
    }

    /// Measures the opened drawer ONCE, after the frame that drew it.
    ///
    /// `true` when the height changed and the door owes another frame. It
    /// answers once and then never again — see `fold::settle`.
    pub fn settled(&mut self) -> bool {
        crate::fold::settle(&mut self.fold, self.wrapped)
    }

    /// Is the pointer on the control that opens the line?
    ///
    /// It asks the cell that was DRAWN rather than working the position out
    /// again: what the pointer hits has to be what the eye saw.
    #[must_use]
    pub fn on_the_control(&self, f: Frame, x: f64, y: f64) -> bool {
        let Some(at) = self.control else {
            return false;
        };
        #[allow(clippy::cast_possible_truncation)]
        if f.line_at(y as f32) != Some(self.top) {
            return false;
        }
        #[allow(clippy::cast_possible_truncation)]
        let cell = f.column_at(x as f32);
        // THE AIR BESIDE THE GLYPH IS PART OF THE TARGET: a chevron does not
        // fill its cell, and a target the size of the ink is one to aim at.
        cell + 1 >= at
    }

    /// Is the pointer inside the drawer — its own line and the rows under it?
    #[must_use]
    pub fn on_the_drawer(&self, f: Frame, y: f64) -> bool {
        #[allow(clippy::cast_possible_truncation)]
        let Some(line) = f.line_at(y as f32) else {
            return false;
        };
        self.fold.open && line >= self.top && line <= self.top + self.folded_rows()
    }

    /// The drawer's bar as a rectangle to PRESS.
    ///
    /// The whole TRACK and not the drawn handle, which is the rule every bar
    /// in this system follows: `Scroll::offset_at` maps any point of it, so
    /// nobody has to aim at the thumb.
    fn track(&self, f: Frame) -> Option<(f32, f32)> {
        if !self.fold.open || !self.scroll.cut() {
            return None;
        }
        Some((f.y_of(self.top), f.line() * f32::from(self.folded_rows())))
    }

    /// A press on the line's own controls: the chevron, or its bar.
    ///
    /// `true` when it was taken, and then the door does nothing else with it.
    pub fn pressed(&mut self, f: Frame, x: f64, y: f64) -> bool {
        if self.on_the_control(f, x, y) {
            self.fold.toggle();
            return true;
        }
        let Some((top, tall)) = self.track(f) else {
            return false;
        };
        #[allow(clippy::cast_possible_truncation)]
        let (px, py) = (x as f32, y as f32);
        if px < f.width - f.cell() || py < top || py >= top + tall {
            return false;
        }
        self.dragging = true;
        self.drag_to(f, y);
        true
    }

    /// A motion while the drawer's bar is held. `true` when it was taken.
    pub fn dragged(&mut self, f: Frame, y: f64) -> bool {
        if !self.dragging {
            return false;
        }
        self.drag_to(f, y);
        true
    }

    /// The button came up, or the pointer left the surface.
    ///
    /// A drag left standing makes the next ENTER, with no button held, jump
    /// the drawer to wherever the pointer came in.
    pub const fn let_go(&mut self) {
        self.dragging = false;
    }

    /// A wheel over the drawer. `true` when it was taken.
    pub fn turned(&mut self, f: Frame, y: f64, by: i64) -> bool {
        if !self.on_the_drawer(f, y) {
            return false;
        }
        self.walk(i32::try_from(by).unwrap_or(0));
        true
    }

    /// THE KEYS OF THIS LINE, on the right `Ctrl` — and `Esc`, which shuts it.
    ///
    /// `true` when the key belonged to the line and nothing else should see
    /// it. `down` is false on release, which is how the modifier is let go.
    ///
    /// One pair, and it reads as one gesture: down opens it and then walks
    /// down it; up walks back and, at the top, shuts it. It is the footer's
    /// own pair, the same notation (`minibuffer-fold-keys`) taught inside the
    /// drawer, so the gesture is one gesture wherever this line is drawn.
    pub fn took_a_key(
        &mut self,
        sym: smithay_client_toolkit::seat::keyboard::Keysym,
        down: bool,
    ) -> bool {
        use smithay_client_toolkit::seat::keyboard::Keysym;
        if sym == Keysym::Control_R {
            self.right_ctrl = down;
            return true;
        }
        if !down {
            return false;
        }
        // `Esc` SHUTS THE DRAWER BEFORE IT CLOSES THE WINDOW, which is what
        // the file manager's window already does with its own.
        if sym == Keysym::Escape {
            return self.fold.close();
        }
        if !self.right_ctrl {
            return false;
        }
        match sym {
            Keysym::Down => {
                if self.fold.open {
                    self.walk(1);
                } else {
                    self.fold.toggle();
                }
                true
            }
            Keysym::Up if self.fold.open => {
                if self.fold.offset == 0 {
                    self.fold.close();
                } else {
                    self.walk(-1);
                }
                true
            }
            _ => false,
        }
    }

    /// The right `Ctrl` went down or up while the header did not have its
    /// keys — followed all the same, and taken by nobody (`Ui::header_took`).
    pub const fn right_ctrl_is(&mut self, down: bool) {
        self.right_ctrl = down;
    }

    /// The pointer left, or the keyboard did: nothing is held any more.
    pub const fn let_everything_go(&mut self) {
        self.dragging = false;
        self.right_ctrl = false;
    }

    /// Walks the opened line by `by` wrapped lines, stopping at its end.
    ///
    /// The stop comes from the `Scroll` that was DRAWN, which is the only
    /// thing that knows how many lines the text wrapped to at this width.
    fn walk(&mut self, by: i32) {
        let last = self.scroll.total.saturating_sub(self.scroll.lines);
        let next = i64::try_from(self.fold.offset).unwrap_or(0) + i64::from(by);
        self.fold.offset = usize::try_from(next.max(0)).unwrap_or(0).min(last);
    }

    /// Puts the drawer where a point of its track says.
    fn drag_to(&mut self, f: Frame, y: f64) {
        let Some((top, tall)) = self.track(f) else {
            return;
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = y as f32 - top;
        self.fold.offset = self.scroll.offset_at(at, tall, f.line());
    }
}

/// One run of a hint line, placed.
struct Laid {
    at: u16,
    text: String,
    register: Register,
    id: Option<&'static str>,
    press: Option<Act>,
}

/// Places every run of the line, cutting the one that overruns.
///
/// The one walk both the painter and the pointer read. `room` is how many
/// cells the line may occupy starting at `column`.
fn laid(pieces: &[Piece], column: u16, room: u16) -> Vec<Laid> {
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
        // A KEY NO CLICK CAN PRESS reads as one — see `Register::Inert`. The
        // register is decided HERE, where every hint line of every door is
        // laid, so no door can draw a key that looks like a control and is
        // not.
        let register = if p.is_a_key() && p.press.is_none() {
            Register::Inert
        } else {
            p.register
        };
        let runs: Vec<&str> = match register {
            Register::Key
            | Register::Alternate
            | Register::Contextual
            | Register::Inert
            | Register::KeyLit
            | Register::AlternateLit
            | Register::ContextualLit => split_at_the_modifier(&p.text)
                .map_or_else(|| vec![p.text.as_str()], |(a, b)| vec![a, b]),
            // AND `Explain` IS A SENTENCE, drawn whole. It reaches here in one
            // place only — an event's line at rest, where the answer about an
            // action takes the words' cells (`explained_in_place`); everywhere
            // else it has a row of its own, drawn by each door from
            // `Hinted::said`. It is named all the same — a `_` arm would let
            // the next register added arrive here without anybody deciding
            // how it is shaped.
            // AND `Said` IS NOT SPLIT EITHER, which is the whole reason it is
            // not `Key`: what it carries is a filename, a date or a path, and
            // a `+` inside one of those is a character rather than a modifier.
            Register::Prose
            | Register::Separator
            | Register::Explain
            | Register::Said
            | Register::Pending
            | Register::Glyph
            | Register::Label
            | Register::Title
            | Register::ProseLit
            // AN ACTION IS A SENTENCE TOO — "Open it" — and nothing in it is
            // a modifier to keep a ligature off.
            | Register::Action
            | Register::ActionLit => vec![p.text.as_str()],
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
            out.push(Laid {
                at,
                text,
                register,
                id: p.id,
                press: p.press,
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

/// The hover bands of a list: one per row a pointer rests on, in the colour
/// of that pointer's cursor (`crate::seats`).
///
/// `lit` is every pointer's row with its colour, in the order the pointers
/// arrived; a row two pointers rest on takes the FIRST one's colour, so the
/// picture is the same on every frame. A row past `total` — the empty room
/// under a short list — gets no band: there is nothing there to click.
///
/// Full width, flat, no border and no radius (invariant 15), and drawn
/// before the text like every band.
#[must_use]
pub fn hover_bands(
    lit: impl Iterator<Item = (tokens::Color, Option<usize>)>,
    offset: usize,
    room: usize,
    total: usize,
    first_line: u16,
    width: f32,
) -> Vec<ronoctua_render::quad::Quad> {
    let line_px = tokens::metrics::line_height(tokens::text::size());
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for (color, row) in lit {
        let Some(row) = row else { continue };
        if row < offset || row >= offset + room || row >= total || seen.contains(&row) {
            continue;
        }
        seen.push(row);
        let line = usize::from(first_line) + (row - offset);
        #[allow(clippy::cast_precision_loss)]
        out.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: line as f32 * line_px,
            width,
            height: line_px,
            color,
        });
    }
    out
}

/// That list's bar's TRACK — the whole column it is grabbed in — and `None`
/// when the list fits. What `crate::bars` records, so the column is the bar's.
#[must_use]
pub fn list_track(
    scroll: ronoctua_design::Scroll,
    first_line: u16,
    width: f32,
    height: f32,
) -> Option<crate::bars::Track> {
    if !scroll.cut() {
        return None;
    }
    let f = Frame::of(width, height, first_line);
    let top = f.y_of(first_line);
    Some(crate::bars::Track::of((
        f.width - f.cell(),
        top,
        f.cell(),
        f.bottom() - top,
    )))
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

/// A rectangle in a surface's logical pixels: `(x, y, width, height)`.
pub type Rect = (f32, f32, f32, f32);

/// A bar over a BAND of a surface that is not the whole list — its track and
/// its thumb, from one `Scroll`: the track is where a press grabs it, the
/// thumb is what is painted. `right` is the pixel the bar's cell ends at,
/// and the band runs from `top` to `bottom` (the leftover under the last
/// line goes to the track, as `Scroll::track` decides for every bar).
#[must_use]
pub fn band_bar(
    scroll: ronoctua_design::Scroll,
    right: f32,
    top: f32,
    bottom: f32,
    cell: f32,
    line: f32,
) -> (Rect, Option<Rect>) {
    let tall = scroll.track(top, bottom, line);
    (
        (right - cell, top, cell, tall),
        scroll.bar(right, top, top + tall, cell, line),
    )
}

/// WHERE A LIST'S BAR WAS DRAWN — and so which cells are the BAR'S.
///
/// # Why the bar has a column of its own, and only while it is drawn
///
/// Because the owner asked for both halves. A row lights and takes a click
/// across the whole width of the window — the band is what says which row a
/// click would land on, and a band that stopped short of the edge for no
/// reason would be a band with a hole in it. But once a bar is DRAWN, its
/// cells are the bar's: the pointer resting on the bar was lighting the row
/// beside it, and a click there was both a grab of the bar and, to the eye, a
/// click on the row. So the band stops where the bar begins, and the row does
/// not answer the bar's column — exactly while there is a bar, and not a
/// frame before.
///
/// It is written by the PAINT and read by the POINTER, which is the rule
/// every control of this shell keeps: taken where it was drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Room {
    /// The surface's width, in pixels.
    pub width: f32,
    /// Where the bar begins, in pixels — `None` while the list fits.
    pub bar_from: Option<f32>,
}

impl Room {
    /// The room of a list whose bar is `scroll`, drawn at the surface's
    /// right edge — where [`list_bar`] draws it and [`on_the_list_bar`]
    /// grabs it.
    #[must_use]
    pub fn of(scroll: ronoctua_design::Scroll, width: f32, height: f32) -> Self {
        let f = Frame::of(width, height, 0);
        Self {
            width,
            bar_from: scroll.cut().then(|| f.width - f.cell()),
        }
    }

    /// Is `x` on the bar's column?
    #[must_use]
    pub fn on_the_bar(self, x: f32) -> bool {
        self.bar_from.is_some_and(|from| x >= from)
    }

    /// How wide a band of the list is: to the bar while there is one, and to
    /// the edge while there is not.
    #[must_use]
    pub fn band_width(self) -> f32 {
        self.bar_from.unwrap_or(self.width)
    }
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

/// Where a run anchored to the surface's RIGHT EDGE begins, in cells.
///
/// # Why anything is anchored there at all
///
/// Because the grid does not reach the border. `columns` is
/// `floor(width / cell)`, so between the last cell and the surface's edge
/// there is a remainder of nought to one cell — and it changes with the
/// window. [`ronoctua_render::Span::edge`] is what places such a run in pixels
/// from the border instead, and the remainder then falls where nobody is
/// looking, on the inside.
///
/// # And why the column is a function rather than a subtraction
///
/// Because a run placed this way is placed TWICE: once in ink, by whoever
/// pushes the `Span`, and once in the pointer, by [`at_the_edge`]. The two
/// have to be one answer, and the way this repository makes that true is a
/// function both of them read — `hint_spans`/`hint_marks` are the precedent,
/// and the scrollbar's `paint::bar_box` is the last thing that had to be
/// rebuilt for want of it.
#[must_use]
pub const fn from_the_edge(columns: u16, cells: u16) -> u16 {
    columns
        .saturating_sub(tokens::space::MODELINE_EDGERIGHT)
        .saturating_sub(cells)
}

/// Where cell `column` of a run anchored to the border BEGINS, in logical
/// pixels from the left — THE RENDERER'S OWN FUNCTION for `Span::edge`
/// ([`ronoctua_render::from_the_border`], which `Text::prepare` places every
/// edge run by), for whoever has to put something BETWEEN two such runs: a
/// change there moves the runs and this together.
#[must_use]
pub fn edge_x(width_px: f32, columns: u16, column: u16) -> f32 {
    ronoctua_render::from_the_border(width_px, columns, column.min(columns), cell_width())
}

/// Which cell the pointer is over, COUNTED FROM THE BORDER — the reading that
/// answers for a run placed by [`from_the_edge`].
///
/// The column it returns is still counted from the LEFT, so it compares
/// against a piece's `at` directly; what differs is where the remainder falls.
/// Asking the ordinary `floor(x / cell)` about an edge-anchored run is off by
/// one cell for every width whose remainder is more than half a cell — which
/// is the defect `[[um-controle-e-pego-onde-e-desenhado]]` records, arriving
/// by the other axis.
///
/// `None` when the pointer is outside the surface or before any cell exists.
#[must_use]
pub fn at_the_edge(x: f64, width_px: f64, columns: u16) -> Option<u16> {
    let cell = f64::from(cell_width());
    if cell <= 0.0 || width_px <= 0.0 || x < 0.0 || x > width_px || columns == 0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let from_border = ((width_px - x) / cell) as u16;
    // ZERO IS THE RIGHTMOST CELL, so the column is the last one less that.
    columns.checked_sub(1)?.checked_sub(from_border)
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

/// WHAT IS HELD, for a POINTER.
///
/// # Why the keyboard's two converters are not it
///
/// Because both of them subtract something that is true of a KEY and false of
/// a click. `Footer::Inheritance::discount` takes away the modifiers that were
/// already down when the leader's sequence began — a fact about a sequence,
/// and a pointer is in none. `door::keys::key_of` refuses to count `Shift`
/// for a character, because xkb has already applied it in producing that
/// character — and a click produces no character.
///
/// So a press reads the modifiers as they are. It is the one conversion of
/// SCTK's `Modifiers` into the type the rest of this shell speaks, and it is
/// here rather than in either door for the reason everything else in this
/// module is: two doors with one answer.
#[must_use]
pub const fn held(
    mods: smithay_client_toolkit::seat::keyboard::Modifiers,
) -> ronoctua_keymap::Mods {
    ronoctua_keymap::Mods {
        ctrl: mods.ctrl,
        alt: mods.alt,
        shift: mods.shift,
        logo: mods.logo,
    }
}

/// A NOTE, BROKEN INTO THE LINES IT TAKES.
///
/// # Why it is one function and not eight
///
/// Because it was eight, and seven of them cut. Every label-and-value view in
/// this system writes an explanation under a heading — what a setting does,
/// why a section is empty, where the value really comes from — and each had
/// its own `sentence` builder ending in `layout::elide_end`, which is one row
/// with an ellipsis where the room ran out.
///
/// Half of one of these is worse than none: the half an ellipsis takes is
/// always the END, which is where the consequence is. `LEADER s l` was
/// changed first, alone, and six more were written down as owed — the eighth,
/// the screens' view, was on nobody's list, and the sentence it cut is the one
/// that screen exists to draw on the worst day: *"The compositor could not
/// read its configuration…"*. This is the function that pays them, so that no
/// view can wrap by one rule and cut by another.
///
/// # And it never loses a line
///
/// It used to stop at eight lines, "a bound on work": the ninth line of a
/// note — the end of a sentence in the small column, where the consequence
/// is — vanished with no ellipsis, which is a cut with the mark taken off.
/// A NOTE WRAPS WHOLE, and a page that runs past its door scrolls; the work
/// is bounded by the words the note has, which `wrap` walks either way.
///
/// # And it never answers with nothing
///
/// `wrap` walks `text.lines()`, and an empty string has none — so it would
/// give back an empty list and the caller would push no row at all. These
/// views index their cursor by a row's PLACE in the list, and a row that is
/// sometimes not there is a cursor that sometimes lands on its neighbour.
#[must_use]
pub fn note_lines(text: &str, room: u16) -> Vec<String> {
    let mut out = ronoctua_render::layout::wrap(text, room, usize::MAX);
    if out.is_empty() {
        out.push(String::new());
    }
    out
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

/// WHAT A ROW OF LABEL AND VALUE IS — and so the two colours it is drawn in.
///
/// # Why the colours are not the caller's any more
///
/// Because they were, in twenty places, and they said different things. The
/// four tones existed — a label that only informs, a label whose value can be
/// changed, and the two values — but every view picked its own pair: three
/// drew a fact's label in the content's grey and its value dim, two drew a
/// fact's value at full strength, one drew a setting with the fact's label.
/// The owner asked for one standard for each of the four, applied everywhere;
/// a type is how "everywhere" stays true, since a colour argument can be given
/// any colour and a `Kind` can only be one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A row that INFORMS: nothing about it can be changed here.
    Fact,
    /// A row whose value can be CHANGED — a ring, a switch, a door.
    Setting,
    /// A row drawn REFUSING: reachable, so it can say why, and inert.
    Disabled,
    /// A fact that is one ITEM of a list: the thing's name is the strong half.
    ListedFact,
    /// A setting that is one item of a list.
    ListedSetting,
}

impl Kind {
    /// A fact, or a setting when the row `acts`.
    #[must_use]
    pub const fn of(acts: bool) -> Self {
        if acts { Self::Setting } else { Self::Fact }
    }

    /// Does a row of this kind act?
    #[must_use]
    pub const fn acts(self) -> bool {
        matches!(self, Self::Setting | Self::ListedSetting)
    }

    /// The label's colour and the value's, under the cursor or not.
    ///
    /// UNDER THE CURSOR the label takes the accent — it is what says "the
    /// keyboard is here", since no window draws a band behind the cursor — and
    /// the value keeps saying whether it can be changed (`text.here.*`).
    #[must_use]
    pub const fn tones(self, focused: bool) -> (tokens::Color, tokens::Color) {
        if focused {
            return (
                tokens::color::STATE_ACCENT,
                if self.acts() {
                    tokens::color::TEXT_HERE_ACTS
                } else {
                    tokens::color::TEXT_HERE_READ
                },
            );
        }
        match self {
            Self::Fact => (
                tokens::color::TEXT_LABEL_FACT,
                tokens::color::TEXT_READ_STRONG,
            ),
            Self::Setting => (tokens::color::TEXT_LABEL_ACTS, tokens::color::TEXT_ANSWER),
            Self::Disabled => (tokens::color::TEXT_DIM, tokens::color::TEXT_DIM),
            Self::ListedFact => (
                tokens::color::TEXT_LABEL_FACT,
                tokens::color::TEXT_READ_WEAK,
            ),
            Self::ListedSetting => (
                tokens::color::TEXT_LABEL_ACTS,
                tokens::color::TEXT_SECONDARY,
            ),
        }
    }
}

/// The same row with its VALUE in a colour that says a STATE — a battery at
/// its alarm, a link that is down — except under the cursor, where the value
/// says what the cursor says.
#[must_use]
pub fn tinted(
    mut fields: Vec<crate::files::Field>,
    status: Option<tokens::Color>,
    focused: bool,
) -> Vec<crate::files::Field> {
    if let Some(c) = status.filter(|_| !focused) {
        for f in fields.iter_mut().skip(1) {
            f.color = c;
        }
    }
    fields
}

/// A field whose letters at `positions` (counted in CLUSTERS of its text, as
/// `minibuffer::match_up` gives them) MATCHED a filter, split so those letters
/// are lit — the finder's rule, for a list whose rows are fields.
///
/// On the CURSOR's row the text is already the accent, so the matched letters
/// take `text.match-here`, the step beside it; elsewhere they take the accent.
/// Letters the field elided away light nothing.
#[must_use]
pub fn lit_letters(
    field: &crate::files::Field,
    positions: &[usize],
    here: bool,
) -> Vec<crate::files::Field> {
    let lit_colour = if here {
        tokens::color::TEXT_MATCH_HERE
    } else {
        tokens::color::STATE_ACCENT
    };
    let mut out: Vec<crate::files::Field> = Vec::new();
    let mut run = String::new();
    let mut run_lit = false;
    let mut at = field.at;
    let flush = |run: &mut String, lit: bool, at: &mut u16, out: &mut Vec<crate::files::Field>| {
        if run.is_empty() {
            return;
        }
        let wide = ronoctua_render::layout::width(run);
        out.push(crate::files::Field {
            at: *at,
            text: std::mem::take(run),
            color: if lit { lit_colour } else { field.color },
            ..field.clone()
        });
        *at += wide;
    };
    for (i, cluster) in ronoctua_render::layout::cells_of(&field.text).enumerate() {
        let lit = positions.contains(&i);
        if lit != run_lit {
            flush(&mut run, run_lit, &mut at, &mut out);
            run_lit = lit;
        }
        run.push_str(cluster);
    }
    flush(&mut run, run_lit, &mut at, &mut out);
    if out.is_empty() {
        out.push(field.clone());
    }
    out
}

/// The label and the value of one row, placed and cut, in the colours of its
/// [`Kind`].
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
    kind: Kind,
    focused: bool,
) -> Vec<crate::files::Field> {
    let (label_colour, value_colour) = kind.tones(focused);
    let piece = |at: u16, text: String, color: tokens::Color| crate::files::Field {
        at,
        text,
        color,
        hit: crate::files::Hit::Nothing,
        explain: None,
        ink: crate::files::Ink::Grid,
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

/// Where the values of ONE SECTION of `label: value` rows begin, when the
/// section is laid out whole ([`aligned_pair`]) — the player's pages.
///
/// # One column a section, and no label cut
///
/// [`field_pair`] places each row on its own: a label longer than the column
/// pushes its value, and where the column is clamped to half a narrow row a
/// section of long labels becomes a zigzag of values, each cut at the row's
/// end. A section laid out whole knows every row, and can do better: its
/// values begin after its widest label and the two cells of air, so each
/// label is read whole and every value of the section is in one column —
/// never before [`value_column`], the column every other view has at this
/// width, and never so far along that the section's widest value is cut.
/// Where the row cannot hold both, the labels give way: they WRAP onto the
/// lines under their row ([`aligned_pair`]), and the values stay whole.
///
/// `pairs` is each row's `(label's first cell, label cells, value cells)`.
/// EVERY label counts, the ones with nothing beside them today included: on a
/// page of choices the value is "in use" on one row, and a column measured
/// from that row alone moved every time another was chosen. A row with no
/// value still has the whole row for its label.
#[must_use]
pub fn section_column(left: u16, room: u16, pairs: &[(u16, u16, u16)]) -> u16 {
    let floor = value_column(left, room);
    let fit = pairs
        .iter()
        .map(|(at, label, _)| at + label + 2)
        .max()
        .unwrap_or(floor);
    let widest = pairs.iter().map(|(_, _, value)| *value).max().unwrap_or(0);
    let most = (left + room).saturating_sub(widest);
    fit.min(most).max(floor)
}

/// The lines of one row of a section laid out whole ([`section_column`]), in
/// the colours of its [`Kind`]: the label from `left`, WRAPPED in the room
/// before `column` and never cut, and the value at `column`, cut only where
/// it runs past `end`. The first line carries the value; each line after it
/// is the rest of the label, in the label's tone. An empty value gives the
/// label the whole row, wrapped the same way.
#[must_use]
pub fn aligned_pair(
    left: u16,
    column: u16,
    end: u16,
    label: &str,
    value: &str,
    kind: Kind,
    focused: bool,
) -> Vec<Vec<crate::files::Field>> {
    let (label_colour, value_colour) = kind.tones(focused);
    let piece = |at: u16, text: String, color: tokens::Color| crate::files::Field {
        at,
        text,
        color,
        hit: crate::files::Hit::Nothing,
        explain: None,
        ink: crate::files::Ink::Grid,
    };
    // TWO CELLS OF AIR before the value, as `field_pair` keeps.
    let room = if value.is_empty() {
        end.saturating_sub(left)
    } else {
        column.saturating_sub(left + 2)
    };
    // AN EMPTY PIECE IS NOT A PIECE, as in `field_pair`: a line with no label
    // is a line with nothing on it but, perhaps, the value.
    let mut lines: Vec<Vec<crate::files::Field>> = note_lines(label, room.max(1))
        .into_iter()
        .map(|line| {
            if line.is_empty() {
                Vec::new()
            } else {
                vec![piece(left, line, label_colour)]
            }
        })
        .collect();
    let value = ronoctua_render::layout::elide_end(value, end.saturating_sub(column));
    if !value.is_empty()
        && let Some(first) = lines.first_mut()
    {
        first.push(piece(column, value, value_colour));
    }
    lines
}

/// A SUBTITLE LANE'S CHIP (`player_rows::Chip`): the ground of
/// `subtitle.edge` its glyph is drawn on, for a chip whose first cell's left
/// edge is `left` and whose line's top is `top`, `line` tall — the ONE
/// drawing every door paints under a lane's colour.
///
/// The glyph sits in the chip's middle cell and fills it (a Material drawing
/// is one cell of ink at the body size, `icon::ink_across`), so the ground
/// is that cell and half a cell more on each side — the same air left and
/// right of the drawing — and `space.player.chipAir` short of the line at the
/// top and the foot, so two chips on two lines never touch. All of it lies
/// inside the chip's `space.player.chipCells`, which is what a click on it
/// takes: nothing drawn here is outside the control.
#[must_use]
pub fn chip(left: f32, top: f32, cell: f32, line: f32) -> ronoctua_render::quad::Quad {
    let air = f32::from(tokens::space::PLAYER_CHIPAIR);
    let cells = f32::from(tokens::space::PLAYER_CHIPCELLS);
    ronoctua_render::quad::Quad {
        x: left + cell / 2.0,
        y: top + air,
        width: (cells - 1.0) * cell,
        height: (line - 2.0 * air).max(0.0),
        color: tokens::color::SUBTITLE_EDGE,
    }
}

/// A TRACK across a line — the player's time bar, its volume: a rule three
/// times a rule's weight in the middle of the line whose top is `top` and
/// which is `line` tall, `left` to `right`, and the share `filled` of it in
/// the colour it comes with. One drawing for every door that has one, so the
/// window's time bar, its volume and the footer's panel are one weight.
#[must_use]
pub fn track_bar(
    left: f32,
    right: f32,
    top: f32,
    line: f32,
    filled: Option<(f32, tokens::Color)>,
) -> Vec<ronoctua_render::quad::Quad> {
    let thickness = f32::from(tokens::space::RULE_THICKNESS).max(1.0) * 3.0;
    let y = top + (line - thickness) / 2.0;
    let width = (right - left).max(0.0);
    let mut out = vec![ronoctua_render::quad::Quad {
        x: left,
        y,
        width,
        height: thickness,
        color: tokens::color::RULE_DEFAULT,
    }];
    if let Some((share, color)) = filled {
        out.push(ronoctua_render::quad::Quad {
            x: left,
            y,
            width: width * share.clamp(0.0, 1.0),
            height: thickness,
            color,
        });
    }
    out
}

/// THE TIME BAR: a [`track_bar`], and the share already `played` in the
/// accent. The window's, the footer's panel's and a deck of one lane's
/// (`crate::player_timeline`) — one drawing.
#[must_use]
pub fn time_bar(
    left: f32,
    right: f32,
    top: f32,
    line: f32,
    played: Option<f32>,
) -> Vec<ronoctua_render::quad::Quad> {
    track_bar(
        left,
        right,
        top,
        line,
        played.map(|p| (p, tokens::color::STATE_ACCENT)),
    )
}

/// A LANE'S BAND on its track (`crate::player_timeline`): from `from` to
/// `to`, `space.player.laneBand` thick and set in the middle of the line
/// whose top is `top` and which is `line` tall — the track's own middle, so
/// the band lies ON the track. In the colour of its kind, or of a lane that
/// is off, or of the ghost of one being dragged; the deck says which.
#[must_use]
pub fn lane_band(
    from: f32,
    to: f32,
    top: f32,
    line: f32,
    color: tokens::Color,
) -> ronoctua_render::quad::Quad {
    let thickness = f32::from(tokens::space::PLAYER_LANEBAND).min(line);
    ronoctua_render::quad::Quad {
        x: from,
        y: top + (line - thickness) / 2.0,
        width: (to - from).max(0.0),
        height: thickness,
        color,
    }
}

/// THE GHOST of a lane's band being dragged: where it would land if let go,
/// `space.player.ghostBand` thick and set in the middle of the line — ON the
/// band, and thinner than it, so the band's own ends still show round it and
/// the ghost's show past it. In `player.lane.held`.
#[must_use]
pub fn lane_ghost(from: f32, to: f32, top: f32, line: f32) -> ronoctua_render::quad::Quad {
    let thickness = f32::from(tokens::space::PLAYER_GHOSTBAND)
        .max(1.0)
        .min(line);
    ronoctua_render::quad::Quad {
        x: from,
        y: top + (line - thickness) / 2.0,
        width: (to - from).max(0.0),
        height: thickness,
        color: tokens::color::PLAYER_LANE_HELD,
    }
}

/// THE PLAYHEAD: the one vertical line of "now" across every lane of a deck,
/// `space.player.playhead` wide and CENTRED on `x` — so the line a press
/// within half a cell of `x` grabs is the line drawn there — from `top` to
/// `bottom`.
///
/// # Why its own colour and not the lit divider
///
/// [`divider_lit`] already means "this section has the keyboard", and one
/// logical pixel of it is the quietest mark this system draws. The playhead
/// is the loudest line of the deck and means something else, so it is a
/// role of its own (`player.playhead`).
///
/// `bottom` is a pixel the caller took from [`Frame::y_of`], for the reason
/// [`divider`] asks for the bottom: a count of lines times a line height
/// lands short of where the last line ends.
#[must_use]
pub fn playhead(x_px: f32, top_px: f32, bottom_px: f32) -> ronoctua_render::quad::Quad {
    let width = f32::from(tokens::space::PLAYER_PLAYHEAD).max(1.0);
    ronoctua_render::quad::Quad {
        x: x_px - width / 2.0,
        y: top_px,
        width,
        height: (bottom_px - top_px).max(0.0),
        color: tokens::color::PLAYER_PLAYHEAD,
    }
}

/// How high a tick of a deck's ruler rises from the line's foot:
/// `space.player.tickQuarters` quarters of the line — and how high the
/// playhead rises on the ruler, so it never crosses the times written above
/// the ticks.
#[must_use]
pub fn tick_rise(line: f32) -> f32 {
    (line * f32::from(tokens::space::PLAYER_TICKQUARTERS) / 4.0)
        .max(1.0)
        .min(line)
}

/// A TICK of a deck's time ruler: a short mark at `x`, rising
/// [`tick_rise`] from the line's foot — under the time it names, which is
/// written from the same cell. A rule's weight, so it reads as a graduation
/// and never as the playhead.
#[must_use]
pub fn tick(x_px: f32, top: f32, line: f32) -> ronoctua_render::quad::Quad {
    let tall = tick_rise(line);
    ronoctua_render::quad::Quad {
        x: x_px,
        y: top + line - tall,
        width: f32::from(tokens::space::RULE_THICKNESS),
        height: tall,
        color: tokens::color::RULE_DEFAULT,
    }
}

/// A PICTURE'S PLACE on the Layout page's schematic (`ronoctua_player::
/// arrange::place` over a small rectangle): a flat tile, and the lit one —
/// the picture whose row the cursor is on — in the accent. No border, no
/// radius and no text on it (invariant 15): the air between two tiles
/// (`space.player.tileGap`) is what tells them apart.
///
/// NOT REACHED YET: the Layout page draws it once the daemon sends the
/// arrangement (the second protocol).
#[must_use]
#[allow(dead_code)]
pub fn tile(rect: Rect, lit: bool) -> ronoctua_render::quad::Quad {
    let (x, y, width, height) = rect;
    ronoctua_render::quad::Quad {
        x,
        y,
        width: width.max(0.0),
        height: height.max(0.0),
        color: if lit {
            tokens::color::PLAYER_TILE_LIT
        } else {
            tokens::color::PLAYER_TILE
        },
    }
}

/// A BAR'S THUMB as a rectangle painted — [`band_bar`]'s second answer, in
/// the tone every thumb of this shell wears. The pure layouts that place a
/// bar over a band (`crate::player_timeline`) hand the rectangle here rather
/// than assembling the quad themselves.
#[must_use]
pub fn thumb(rect: Rect) -> ronoctua_render::quad::Quad {
    let (x, y, width, height) = rect;
    ronoctua_render::quad::Quad {
        x,
        y,
        width,
        height,
        color: tokens::color::RULE_STRONG,
    }
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
        BAR_CELLS, FIELDS_LEAD, Frame, INSET, Kind, Piece, Register, SHELF_PAD, SHELF_ROOM,
        aligned_pair, at_the_edge, cell_width, clock_exact, clock_text, content_left_px, date_long,
        field_pair, from_the_edge, hint_marks, hint_of, hint_sentence, hint_spans, key_text,
        list_bar, list_offset_at, list_scroll, lit, on_the_band, on_the_list_bar, section_column,
        sentence, shelf_bar, shelf_box, shelf_ground, split_at_the_modifier, track_bar,
        value_column,
    };
    use ronoctua_tokens as tokens;

    /// A key of a hint line — a CONTROL, as every key a table generates is
    /// (`Piece::press`) — or its words.
    fn piece(text: &str, key: bool) -> Piece {
        if key {
            let press = ronoctua_keymap::Key::parse(text)
                .unwrap_or_else(|_| ronoctua_keymap::Key::parse("RET").expect("RET parses"));
            Piece::key(text, false).pressing(press)
        } else {
            Piece::prose(text)
        }
    }

    /// FAILS IF: a note loses a line — the ninth, the fortieth — instead of
    /// wrapping whole: the end of a sentence is where its consequence is,
    /// and a line dropped with no ellipsis is a cut with the mark taken off.
    #[test]
    fn a_note_never_loses_a_line() {
        let words: Vec<String> = (1..=120).map(|n| format!("w{n}")).collect();
        let text = words.join(" ");
        let lines = super::note_lines(&text, 10);
        assert!(lines.len() > 8, "{} lines", lines.len());
        let back: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
        assert_eq!(back, words, "a word of the note is gone");
    }

    /// A KEY NO CLICK CAN PRESS reads as one, and nobody chose that: `laid`
    /// gives the inert register to every key that arrives without a press.
    #[test]
    fn a_key_a_click_cannot_press_is_drawn_inert() {
        let line = [Piece::key("RCtrl ↑↓", false)];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 80);
        assert!(
            spans
                .iter()
                .all(|s| s.color == tokens::color::TEXT_SUBTLE_INERT)
        );
        let marks = hint_marks(&[piece("C-m", true).about("x")], 0, 0, 80);
        assert!(
            marks.iter().all(|m| m.press.is_some()),
            "a table key is a control"
        );
    }

    /// THE SHORTCUT UNDER THE POINTER IS LIT — both halves, and only when a
    /// click would press it.
    #[test]
    fn the_shortcut_under_the_pointer_is_lit() {
        let line = [
            piece("C-m", true).about("m"),
            Piece::prose(" menu")
                .about("m")
                .pressing(ronoctua_keymap::Key::parse("C-m").expect("C-m")),
            Piece::key("RCtrl ↑↓", false).about("walk"),
        ];
        let lit_m = lit(&line, Some("m"));
        assert_eq!(lit_m[0].register, Register::KeyLit);
        assert_eq!(lit_m[1].register, Register::ProseLit);
        let lit_walk = lit(&line, Some("walk"));
        assert_eq!(
            lit_walk[2].register,
            Register::Key,
            "a read-only key does not light"
        );
        assert_eq!(lit(&line, None), line.to_vec());
    }

    /// EVERY CELL A SHORTCUT WAS DRAWN IN NAMES THAT SHORTCUT — the key and
    /// the words beside it, which is the whole of what a person points at.
    ///
    /// And the marks are read off the SAME walk the spans are, so a mark can
    /// only be wrong where the drawing is wrong too.
    /// THE TWO READINGS OF ONE PLACEMENT AGREE, at every remainder.
    ///
    /// THE DEFECT THIS CATCHES: the pointer asked the left-counted grid about
    /// a run anchored to the border. `columns` is `floor(width / cell)`, so
    /// between the last cell and the border there is a remainder of nought to
    /// one cell — and where that remainder is more than half a cell, the two
    /// readings name different cells. A control found that way answers one
    /// cell from where it was drawn, which is the whole subject of
    /// `paint::bar_box` and of the memory beside it.
    #[test]
    fn a_cell_placed_at_the_edge_is_read_back_at_the_edge() {
        let cell = f64::from(cell_width());
        assert!(cell > 1.0, "no cell to measure with");
        let mut remainders = 0;
        // WIDTHS WITH DIFFERENT REMAINDERS, which is the only thing that makes
        // this a test rather than an identity: at a width that is an exact
        // number of cells the two readings cannot disagree.
        for width_px in [800.0_f64, 1000.0, 1366.0, 1920.0, 2560.0, 3440.0] {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let columns = (width_px / cell) as u16;
            if (width_px - f64::from(columns) * cell) > cell / 2.0 {
                remainders += 1;
            }
            for cells in 1..=3u16 {
                let at = from_the_edge(columns, cells);
                // WHERE THE INK ACTUALLY LANDS, by `Span::edge`'s own sum.
                let left = width_px - f64::from(columns - at) * cell;
                for step in 0..cells {
                    let middle = left + f64::from(step).mul_add(cell, cell / 2.0);
                    assert_eq!(
                        at_the_edge(middle, width_px, columns),
                        Some(at + step),
                        "at {width_px} px, {cells} cell(s) from the edge"
                    );
                }
                // AND THE CELL BEFORE IT IS NOT IT.
                assert_eq!(
                    at_the_edge(left - cell / 2.0, width_px, columns),
                    at.checked_sub(1)
                );
            }
        }
        assert!(
            remainders > 0,
            "no width left a remainder over half a cell, so this proves nothing"
        );
        // Outside the surface is nowhere.
        assert_eq!(at_the_edge(-1.0, 1920.0, 100), None);
        assert_eq!(at_the_edge(1921.0, 1920.0, 100), None);
    }

    #[test]
    fn a_mark_covers_the_key_and_the_words_that_say_what_it_does() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let table: &[(&'static str, &'static str)] =
            &[("C-v", "files-preview"), ("C-i", "files-facts")];
        let pieces = hint_of(table, &texts);
        let spans = hint_spans(&pieces, ronoctua_render::Size::Grid, 2, 0, 200);
        let marks = hint_marks(&pieces, 2, 0, 200);
        assert!(!marks.is_empty(), "a hint of two keys marks nothing");
        // THE INK AND THE MARKS COVER THE SAME CELLS. Every span that came out
        // of a piece with an id has a mark over it — the separator's does not,
        // and that is the only run of the line that belongs to no shortcut.
        for m in &marks {
            assert!(m.cells > 0);
        }
        // THE CELL DECIDES, and it decides the right one of the two.
        let first = marks
            .iter()
            .find(|m| m.id == "files-preview")
            .expect("the first key is marked");
        let second = marks
            .iter()
            .find(|m| m.id == "files-facts")
            .expect("the second key is marked");
        assert!(first.at < second.at, "the marks follow the drawing order");
        assert!(marks.iter().any(|m| m.covers(0, first.at)));
        assert!(!marks.iter().any(|m| m.covers(1, first.at)), "another line");
        // AND NOTHING IS MARKED PAST THE INK.
        let ends = spans
            .iter()
            .map(|s| s.column + ronoctua_render::layout::width(&s.text))
            .max()
            .unwrap_or(0);
        for m in &marks {
            assert!(m.at + m.cells <= ends, "a mark past the last drawn cell");
        }
    }

    /// A KEY THAT IS A COMMAND READS ITS `cmd-`; ONE THAT IS A VIEW'S READS
    /// ITS `about-`. Neither is ever the missing-key marker.
    #[test]
    fn every_shortcut_of_a_table_has_a_sentence_of_its_own() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        // One of each namespace, so a rename on either side is caught here and
        // not only in the check that walks all sixteen tables.
        for id in ["notif-read", "files-preview", "transfers-pause"] {
            let said = hint_sentence(id, &texts).expect("a shortcut with no sentence");
            assert!(!said.is_empty());
            assert!(
                !said.starts_with('['),
                "{id} resolved to the missing-key marker: {said}"
            );
        }
        // AND A KEY NOBODY DECLARED SAYS NOTHING, rather than showing the
        // marker on the line under the hint.
        assert!(hint_sentence("no-such-shortcut-at-all", &texts).is_none());
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
                    Kind::Fact,
                    false,
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
        let fields = field_pair(2, 60, long, "", Kind::Fact, false);
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
            Kind::Fact,
            false,
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
        let acts = field_pair(2, 60, "radio", "on", Kind::Setting, false);
        let fact = field_pair(2, 60, "band", "5 GHz", Kind::Fact, false);
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
        let fields = field_pair(2, 80, long, "on", Kind::Fact, false);
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
        let fields = field_pair(2, 40, &"x".repeat(200), "on", Kind::Fact, false);
        for f in &fields {
            let ends = f.at + ronoctua_render::layout::width(&f.text);
            assert!(ends <= 2 + 40, "a field ran to {ends} in a row of 40");
        }
        assert_eq!(fields.len(), 2, "the value disappeared: {fields:?}");
    }

    /// FAILS IF: a section laid out whole cuts one of its labels while its
    /// values could move, moves its values before the column every other view
    /// has, or so far along that its widest value is cut — or leaves out a
    /// label whose value is empty now, and moves when another row is chosen.
    #[test]
    fn a_section_s_values_begin_after_its_widest_label() {
        let floor = value_column(1, 80);
        // SHORT LABELS: the column every view has.
        assert_eq!(section_column(1, 80, &[(1, 6, 4), (1, 9, 20)]), floor);
        // A LONG ONE moves the whole section, a nested one measured from its
        // own first cell.
        assert_eq!(section_column(1, 80, &[(1, 6, 4), (3, 40, 3)]), 3 + 40 + 2);
        // NEVER SO FAR THAT THE WIDEST VALUE IS CUT.
        assert_eq!(section_column(1, 80, &[(1, 70, 12)]), 1 + 80 - 12);
        // AND NEVER BEFORE THE FLOOR, whatever the values ask.
        assert_eq!(section_column(1, 30, &[(1, 20, 28)]), value_column(1, 30));
        // A LABEL WITH NOTHING BESIDE IT TODAY counts too: on a page of
        // choices, the value is on the one in use.
        assert_eq!(section_column(1, 80, &[(1, 6, 4), (1, 40, 0)]), 1 + 40 + 2);
    }

    /// FAILS IF: a label of a section laid out whole is cut instead of
    /// wrapped, a line of it runs into the value's column, the value leaves
    /// the first line, or is cut while it fits.
    #[test]
    fn an_aligned_label_wraps_and_its_value_stays_whole() {
        let label = "Talk inside a music video clip, off topic";
        let lines = aligned_pair(3, 21, 31, label, "off", Kind::Setting, false);
        assert!(lines.len() > 1, "{lines:?}");
        let words: Vec<String> = lines
            .iter()
            .flat_map(|l| l.iter().filter(|f| f.at == 3).map(|f| f.text.clone()))
            .collect();
        assert_eq!(words.join(" "), label, "the label was cut");
        for f in lines.iter().flatten().filter(|f| f.at == 3) {
            assert!(
                f.at + ronoctua_render::layout::width(&f.text) + 2 <= 21,
                "{f:?} runs into the value"
            );
        }
        let value: Vec<_> = lines
            .iter()
            .enumerate()
            .flat_map(|(i, l)| l.iter().filter(|f| f.at == 21).map(move |f| (i, f)))
            .collect();
        assert_eq!(value.len(), 1);
        assert_eq!((value[0].0, value[0].1.text.as_str()), (0, "off"));
        // NO VALUE: the label has the whole row.
        let alone = aligned_pair(
            1,
            15,
            31,
            "Close the player everywhere",
            "",
            Kind::Setting,
            false,
        );
        assert_eq!(alone.len(), 1);
    }

    /// FAILS IF: a track stops being one weight in every door — the rule's,
    /// three times — or its filled share runs past it.
    #[test]
    fn a_track_is_a_rule_three_times_and_filled_inside_it() {
        let q = track_bar(
            10.0,
            110.0,
            40.0,
            26.0,
            Some((1.5, tokens::color::STATE_ACCENT)),
        );
        let weight = f32::from(tokens::space::RULE_THICKNESS).max(1.0) * 3.0;
        assert_eq!(q.len(), 2);
        assert!((q[0].height - weight).abs() < f32::EPSILON);
        assert!((q[1].width - 100.0).abs() < f32::EPSILON, "{q:?}");
        assert_eq!(track_bar(10.0, 110.0, 40.0, 26.0, None).len(), 1);
    }

    /// FAILS IF: a lane's band stops lying ON its track — the same middle of
    /// the line, thicker than it — or the playhead stops being centred on the
    /// x it is given, which is the x a press within half a cell takes.
    #[test]
    fn a_band_lies_on_its_track_and_the_playhead_is_centred_on_now() {
        let track = &track_bar(10.0, 110.0, 40.0, 26.0, None)[0];
        let band = super::lane_band(30.0, 70.0, 40.0, 26.0, tokens::color::PLAYER_LANE_VIDEO);
        let middle = |q: &ronoctua_render::quad::Quad| q.y + q.height / 2.0;
        assert!(
            (middle(track) - middle(&band)).abs() < 0.01,
            "{track:?} {band:?}"
        );
        assert!(
            band.height > track.height,
            "a band as thin as its track is the track"
        );
        assert!((band.x - 30.0).abs() < f32::EPSILON && (band.width - 40.0).abs() < f32::EPSILON);
        // A BAND GIVEN BACKWARDS is nothing, never a negative rectangle.
        assert!(
            super::lane_band(70.0, 30.0, 40.0, 26.0, tokens::color::PLAYER_LANE_VIDEO).width == 0.0
        );

        let head = super::playhead(100.0, 5.0, 205.0);
        assert!((head.x + head.width / 2.0 - 100.0).abs() < 0.01, "{head:?}");
        assert!((head.y - 5.0).abs() < f32::EPSILON && (head.height - 200.0).abs() < f32::EPSILON);
        assert_eq!(head.color, tokens::color::PLAYER_PLAYHEAD);
    }

    /// FAILS IF: the lit tile is drawn like the others, or the time bar's
    /// played share stops being the accent a track is filled with.
    #[test]
    fn a_lit_tile_and_the_played_time_wear_their_own_tones() {
        let rect = (1.0, 2.0, 30.0, 20.0);
        assert_eq!(super::tile(rect, false).color, tokens::color::PLAYER_TILE);
        assert_eq!(
            super::tile(rect, true).color,
            tokens::color::PLAYER_TILE_LIT
        );
        assert_ne!(tokens::color::PLAYER_TILE, tokens::color::PLAYER_TILE_LIT);
        let played = super::time_bar(10.0, 110.0, 40.0, 26.0, Some(0.5));
        assert_eq!(
            played,
            track_bar(
                10.0,
                110.0,
                40.0,
                26.0,
                Some((0.5, tokens::color::STATE_ACCENT))
            )
        );
        // THE TICK stands on the line's foot, under the time it names.
        let t = super::tick(50.0, 40.0, 26.0);
        assert!((t.y + t.height - 66.0).abs() < 0.01 && (t.x - 50.0).abs() < f32::EPSILON);
    }

    /// FAILS IF: a written-out date loses its month, its second, or the
    /// person's order.
    ///
    /// The owner asked for the whole date in his own language — *"3 de
    /// setembro de 2026 às 12:45:20"* — where the line used to say `and 20
    /// seconds`, which is the seconds-of-the-minute the column has no room
    /// for, pinned to a cell the sentence never names.
    #[test]
    fn a_long_date_names_its_month_and_its_second() {
        let en = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let pt = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        // 2026-09-03 12:45:20 UTC, which is the owner's own example.
        let c = ronoctua_files::when::civil(1_788_439_520, &ronoctua_files::when::Zone::utc());
        assert_eq!((c.year, c.month, c.day), (2026, 9, 3));
        assert_eq!((c.hour, c.minute, c.second), (12, 45, 20));

        // THE MONTH IS A WORD, in both languages, and it comes from a key.
        let day_first = date_long(&c, crate::locale::Order::DayFirst, &en);
        assert!(
            day_first.contains("September")
                && day_first.contains('3')
                && day_first.contains("2026"),
            "en-GB day-first reads {day_first}"
        );
        let pt_day_first = date_long(&c, crate::locale::Order::DayFirst, &pt);
        assert!(
            pt_day_first.contains("setembro"),
            "pt-BR day-first reads {pt_day_first}"
        );

        // AND THE ORDER IS STILL THE PERSON'S: the three do not read alike.
        let month_first = date_long(&c, crate::locale::Order::MonthFirst, &en);
        let big_endian = date_long(&c, crate::locale::Order::BigEndian, &en);
        assert_ne!(day_first, month_first, "the order preference does nothing");
        assert_ne!(day_first, big_endian, "the order preference does nothing");

        // THE SECOND SURVIVES THE JOURNEY, on both clocks.
        let exact = clock_exact(&c, crate::locale::Clock::TwentyFour, &en);
        assert_eq!(exact, "12:45:20");
        let twelve = clock_exact(&c, crate::locale::Clock::Twelve, &en);
        assert!(
            twelve.contains(":20"),
            "the twelve-hour clock reads {twelve}"
        );
        // AND THE COLUMN'S FORM DOES NOT GROW ONE: three more cells on every
        // row buy nothing, and `Civil::args` is that contract.
        assert_eq!(
            clock_text(
                1_788_439_520,
                &ronoctua_files::when::Zone::utc(),
                crate::locale::Clock::TwentyFour,
                &en
            ),
            "12:45"
        );
    }

    /// FAILS IF: a pre-epoch instant counts its seconds backwards.
    ///
    /// `rem_euclid` is what makes this right, and a truncating remainder would
    /// put every date before 1970 one second out in the other direction.
    #[test]
    fn the_second_before_the_epoch_counts_forwards() {
        let c = ronoctua_files::when::civil(-1, &ronoctua_files::when::Zone::utc());
        assert_eq!((c.hour, c.minute, c.second), (23, 59, 59));
    }

    /// FAILS IF: a fact's sentence comes out in one register.
    ///
    /// The owner read a cut label in the info panel and asked for both halves
    /// in full and told apart — *"a cor da legenda e do valor devem se
    /// diferenciar"*. The panel already drew them in two tones; the line that
    /// explains them drew the whole sentence in one.
    #[test]
    fn a_fact_says_its_name_and_its_value_in_two_registers() {
        let en = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let e = ronoctua_files::mode::Explanation {
            key: ronoctua_i18n::key::FILES_FACT_SAYS,
            args: vec![
                ("label", "modified".to_owned()),
                ("value", "3 September 2026 at 12:45:20".to_owned()),
            ],
        };
        let pieces = sentence(&e, &en);
        let name = pieces
            .iter()
            .find(|p| p.text == "modified")
            .expect("the label is not a run of its own");
        let said = pieces
            .iter()
            .find(|p| p.text == "3 September 2026 at 12:45:20")
            .expect("the value is not a run of its own");
        assert_eq!(
            name.register,
            Register::Prose,
            "the name of the fact is the QUIET half"
        );
        assert_eq!(
            said.register,
            Register::Said,
            "the fact itself is the LOUD half"
        );
        assert_ne!(
            name.register.color(),
            said.register.color(),
            "the two halves resolve to one ink"
        );
        // AND THE RUNS ARE THE SENTENCE, exactly: a caller that draws them one
        // after another draws what `t_with` would have produced.
        let joined: String = pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(joined, en.t_with(e.key, &e.borrowed()));
    }

    /// FAILS IF: two arguments of one sentence collapse into one register.
    ///
    /// Everything that is not the NAME of a fact is the fact, whatever it is
    /// called — there is exactly one key in the tree that interpolates
    /// `$label`, and that is what makes the rule provable rather than magic.
    #[test]
    fn an_argument_that_is_not_a_name_is_the_answer() {
        let en = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let e = ronoctua_files::mode::Explanation {
            key: ronoctua_i18n::key::FILES_EXPLAIN_OWNER,
            args: vec![("who", "noctua".to_owned())],
        };
        let pieces = sentence(&e, &en);
        assert!(
            pieces
                .iter()
                .any(|p| p.text == "noctua" && p.register == Register::Said),
            "the owner's name is not drawn as the answer"
        );
        assert!(
            pieces.iter().any(|p| p.register == Register::Prose),
            "the words around it lost their own register"
        );
    }

    /// THE WAY BACK IS THE FIRST KEY, and a click on it is `Esc` — unless the
    /// view already teaches `Esc`, which then is not taught twice.
    #[test]
    fn the_way_back_leads_every_view_and_is_never_doubled() {
        use super::{Act, with_the_way_back};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let esc = ronoctua_keymap::Key::parse("ESC").expect("parses");
        let plain = hint_of(&[("C-w", "network-window")], &t);
        let led = with_the_way_back(plain.clone(), &t);
        assert_eq!(led[0].press, Some(Act::Key(esc)), "Esc is not first");
        assert_eq!(
            led.len(),
            2 + 1 + plain.len(),
            "the key, its words, a dot, the rest"
        );
        let again = with_the_way_back(led.clone(), &t);
        assert_eq!(again, led, "Esc was taught twice");
        assert_eq!(with_the_way_back(Vec::new(), &t).len(), 2);
    }

    /// FAILS IF: a header told to sit on line 1 — a column's, under the
    /// column's own control line — is drawn there and still answers on line
    /// 0: its shortcuts, its chevron and its sentence all have to be taken
    /// where they were drawn.
    #[test]
    fn a_header_on_line_one_is_drawn_and_taken_on_line_one() {
        use super::{Act, Hinted, OF_THE_WAY_BACK};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let esc = ronoctua_keymap::Key::parse("ESC").expect("parses");
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let cell = f64::from(cell_width());
        let columns = 40;
        let mut h = Hinted::at(1);
        let spans = h.head(&hint_of(OF_THE_WAY_BACK, &t), true, 1, columns, &t);
        assert!(!spans.is_empty());
        assert!(
            spans.iter().all(|s| s.line == 1),
            "a run of the header left line 1: {:?}",
            spans.iter().map(|s| s.line).collect::<Vec<_>>()
        );
        // THE SHORTCUT, on line 1 and not on line 0.
        let x = cell * 1.5;
        assert_eq!(h.press_under(x, line * 1.5), Some(Act::Key(esc)));
        assert_eq!(h.press_under(x, line * 0.5), None);
        // THE CHEVRON, drawn on the last content cell of line 1.
        let f = Frame::of((cell * f64::from(columns + 2)) as f32, 600.0, 0);
        let chevron = cell * f64::from(columns) + cell / 2.0;
        assert!(h.on_the_control(f, chevron, line * 1.5));
        assert!(!h.on_the_control(f, chevron, line * 0.5));
        assert!(
            h.light_at(Some((chevron, line * 1.5))),
            "the chevron did not light"
        );
        assert!(
            h.light_at(Some((chevron, line * 0.5))),
            "it stayed lit off its line"
        );
        // AND THE LIST STARTS UNDER IT: the header is still one line.
        assert_eq!(h.top(), 1);
        assert_eq!(Hinted::default().top(), 0, "every window moved");
    }

    /// FAILS IF: a withdrawn header — the column in its bar, where no header
    /// is drawn — still answers a click on the marks of its last frame.
    #[test]
    fn a_withdrawn_header_answers_nothing() {
        use super::{Hinted, OF_THE_WAY_BACK};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let cell = f64::from(cell_width());
        let mut h = Hinted::at(1);
        let _ = h.head(&hint_of(OF_THE_WAY_BACK, &t), true, 1, 40, &t);
        assert!(h.press_under(cell * 1.5, line * 1.5).is_some());
        h.withdraw();
        assert_eq!(h.press_under(cell * 1.5, line * 1.5), None);
        let f = Frame::of((cell * 42.0) as f32, 600.0, 0);
        assert!(
            !h.on_the_control(f, cell * 40.5, line * 1.5),
            "the chevron is still there"
        );
        assert_eq!(h.rows(), 0);
    }

    /// FAILS IF: the focus line stops wearing the colour the person's
    /// borders give a focused outline (`crate::theme::focus_line` — the
    /// accent on the defaults every test process wears), stops running edge
    /// to edge, or grows past the leading above line 0 — where it would
    /// cover the ink of the line under it.
    #[test]
    fn the_focus_line_is_the_outlines_colour_across_the_top() {
        let q = super::focus_line(333.0);
        assert_eq!(q.color, crate::theme::focus_line());
        assert_eq!(q.color, tokens::color::STATE_ACCENT, "the defaults");
        assert_eq!((q.x, q.y, q.width), (0.0, 0.0, 333.0));
        assert_eq!(q.height, f32::from(tokens::space::COLUMN_FOCUSLINE));
        // THE INK OF LINE 0 starts under the font's box, which starts half
        // the difference between the line and the box under the top.
        let size = tokens::text::size();
        let leading = (tokens::metrics::line_height(size) - size * 1.25) / 2.0;
        assert!(
            q.height < leading * 2.0,
            "{} over a leading of {leading}",
            q.height
        );
    }

    /// FAILS IF: an action on a line stops wearing the band's own button
    /// colour, stops lighting under the pointer, or stops running its command.
    #[test]
    fn an_action_is_the_bands_button_and_lights_like_a_shortcut() {
        let a = Piece::action("Open it", "software-launch-latest");
        assert_eq!(a.register.color(), tokens::color::NOTIFICATION_ACTS);
        assert_eq!(a.id, Some("software-launch-latest"));
        assert_eq!(
            a.press,
            Some(super::Act::Run("software-launch-latest")),
            "a click runs the command"
        );
        assert!(!a.is_a_key(), "an action is not a key, and never inert");
        let line = super::said_with_actions(
            "Firefox is installed",
            &[("Open it".to_owned(), "software-launch-latest")],
        );
        let lit_line = lit(&line, Some("software-launch-latest"));
        let action = lit_line.last().expect("the action closes the line");
        assert_eq!(action.register, Register::ActionLit);
        assert_eq!(
            action.register.color(),
            tokens::color::NOTIFICATION_ACTS_LIT
        );
        assert_ne!(
            tokens::color::NOTIFICATION_ACTS_LIT,
            tokens::color::NOTIFICATION_ACTS,
            "lit and at rest are one role"
        );
        assert_eq!(
            lit_line[0].register,
            Register::Said,
            "the sentence does not light: it is not a control"
        );
    }

    /// FAILS IF: the actions at the end of an event's line stop being marks
    /// the pointer finds — one per action, carrying its command — or the
    /// sentence and the dots start answering a click.
    #[test]
    fn the_actions_of_a_line_are_placed_and_clickable() {
        let line = super::said_with_actions(
            "A camera is open",
            &[
                ("First".to_owned(), "camera-settings-open"),
                ("Second".to_owned(), "software-launch-latest"),
            ],
        );
        let marks = hint_marks(&line, 2, 7, 200);
        assert_eq!(marks.len(), 2, "{marks:?}");
        assert_eq!(marks[0].id, "camera-settings-open");
        assert_eq!(marks[1].id, "software-launch-latest");
        assert!(
            marks
                .iter()
                .all(|m| matches!(m.press, Some(super::Act::Run(id)) if id == m.id))
        );
        // WHERE THEY WERE DRAWN is where they are found: after the sentence
        // and its dot, on the line they were laid on.
        let sentence = ronoctua_render::layout::width("A camera is open");
        let dot = ronoctua_render::layout::width(&Piece::separator().text);
        assert_eq!(marks[0].at, 2 + sentence + dot);
        assert!(marks[0].covers(7, marks[0].at));
        assert!(!marks[0].covers(7, 2), "the sentence is not the action");
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 2, 7, 200);
        assert!(
            spans
                .iter()
                .any(|s| s.text == "First" && s.color == tokens::color::NOTIFICATION_ACTS)
        );
    }

    /// FAILS IF: a sentence too long for the line pushes its actions off the
    /// end — where `laid` cuts — instead of giving way to them, or a room too
    /// narrow for both loses the sentence.
    #[test]
    fn a_long_sentence_gives_way_to_its_actions() {
        let long = "A very long program name that will never fit on a narrow line is installed";
        let line =
            super::said_with_actions(long, &[("Open it".to_owned(), "software-launch-latest")]);
        let fitted = super::room_for_actions(&line, 40);
        let marks = hint_marks(&fitted, 0, 0, 40);
        assert_eq!(marks.len(), 1, "the action was cut off the line");
        assert_eq!(marks[0].id, "software-launch-latest");
        assert_eq!(
            super::cells_of(&fitted),
            40,
            "the line is the room, exactly"
        );
        assert!(fitted[0].text.ends_with('…'), "{:?}", fitted[0].text);
        assert_eq!(fitted.last().map(|p| p.text.as_str()), Some("Open it"));
        // A LINE WITH NO ACTION is left alone.
        let plain = vec![Piece::said(long)];
        assert_eq!(super::room_for_actions(&plain, 10), plain);
        // AND A ROOM TOO NARROW FOR BOTH keeps the sentence whole: `laid`
        // then cuts the end, and the drawer shows the rest.
        assert_eq!(super::room_for_actions(&line, 12), line);
    }

    /// FAILS IF: the actions stop sitting at the END of the room — a short
    /// sentence pulls them left, so an answer of another length would move
    /// them — or the answer about the action under the pointer moves them,
    /// loses the sentence's cells, or takes the action's register.
    #[test]
    fn the_answer_about_an_action_takes_the_words_place_and_moves_nothing() {
        let line = super::said_with_actions(
            "Mail is ready",
            &[("Open it".to_owned(), "software-launch-latest")],
        );
        let fitted = super::room_for_actions(&line, 60);
        assert_eq!(
            super::cells_of(&fitted),
            60,
            "short words are followed by air"
        );
        let at_rest = hint_marks(&fitted, 2, 0, 60);
        let action = ronoctua_render::layout::width("Open it");
        assert_eq!(at_rest.len(), 1);
        assert_eq!(at_rest[0].at + at_rest[0].cells, 2 + 60, "flush at the end");
        assert_eq!(at_rest[0].cells, action);
        for sentence in [
            "Open",
            "Open what was installed last, and a great deal more than that",
        ] {
            let asked = super::explained_in_place(&fitted, sentence);
            assert_eq!(
                hint_marks(&asked, 2, 0, 60),
                at_rest,
                "the action moved under the pointer that asked"
            );
            assert_eq!(asked[0].register, Register::Explain);
            assert!(sentence.starts_with(asked[0].text.trim_end_matches('…')));
            assert_eq!(super::cells_of(&asked), 60);
        }
        // A LINE WITH NO ACTION has nothing to explain.
        let plain = vec![Piece::said("x")];
        assert_eq!(super::explained_in_place(&plain, "y"), plain);
    }
}
