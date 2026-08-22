//! The notification manager — one implementation, two doors.
//!
//! # What it holds, and what it does not
//!
//! It holds a cursor, which pane has it, and whether a sheet is open. It holds
//! no notifications: the records live in the [`ronoctua_notify::Queue`] and the
//! catalogue in the [`ronoctua_notify::Book`], and both are handed in when rows
//! are asked for. That is what lets the footer's region and the window draw the
//! same thing without a second copy going stale.
//!
//! It is the shape `files::View` already has, and for the same reason — see the
//! header of `files_window.rs`.
//!
//! # Three lists, and why the third is not a fourth pane
//!
//! The owner asked for two columns: unread (with the read ones under them, in a
//! quieter tone) on the left, and what a filter suppressed on the right. The
//! third list — every notification and filter the distro ships with, so each can
//! be adjusted — is a different KIND of thing: it is not something that
//! happened, it is something that is configured. It takes the whole width, and
//! `TAB` reaches it.
//!
//! # One column or two
//!
//! Two when there is room, one when there is not — the same layout in the
//! footer's region and in the window, deciding by the width it is given. The
//! cut comes from a token; a number written here would be a visual value
//! outside the tokens (invariant 9).

use ronoctua_notify::presentation::Verdict;
use ronoctua_notify::{Book, Life, Presentation, Record, Show, TOP_LEVEL};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit as FilesHit};

/// The commands the MANAGER reaches, with each one's key.
///
/// # Why the keys are a table and not only a `match`
///
/// Because the manual reads it and the orphan check reads it: a command with no
/// path at all is a command nobody uses, and without this list the check would
/// flag these three — rightly, because they have no key in the leader's tree.
///
/// They cannot have one, and that is not an oversight: they act on the FOCUSED
/// ROW, and outside the view there is no focused row to act on. It is exactly
/// the shape `leader::OF_NETWORK_VIEW` already has.
///
/// # And why the letters are bare
///
/// Because with the view open the whole keyboard is its — `d` here forgets a
/// notification and does not fire `LEADER d`, the same way `d` in the clipboard
/// removes an entry. The `C-` family belongs to the file manager, where a bare
/// letter is a QUERY and cannot also be a command.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("SPC", "notif-read"),
    ("d", "notif-forget"),
    ("r", "notif-read-all"),
    ("RET", "notif-act"),
];

/// Which list has the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    /// What happened: unread first, then read, in a quieter tone.
    #[default]
    Happened,
    /// What a filter kept off the screen.
    Suppressed,
    /// Every notification and filter the distro ships with.
    Catalogue,
}

impl Pane {
    /// The heading's text key.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Happened => ronoctua_i18n::key::NOTIF_COLUMN_UNREAD,
            Self::Suppressed => ronoctua_i18n::key::NOTIF_COLUMN_SUPPRESSED,
            Self::Catalogue => ronoctua_i18n::key::NOTIF_COLUMN_CATALOGUE,
        }
    }

    /// The next one, cycling. It is what `TAB` does.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Happened => Self::Suppressed,
            Self::Suppressed => Self::Catalogue,
            Self::Catalogue => Self::Happened,
        }
    }
}

/// A field of the sheet, in the order it is drawn.
///
/// The sheet is how "use the distro's default, or customise" is reached: each
/// row is one field of the presentation, the side arrows change it, and `r`
/// puts it back to what the distro decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// Whether and how it shows.
    Show,
    /// How long it stays.
    Life,
    /// Loud or quiet.
    Loud,
    /// `0..=100`.
    Level,
    /// The text alternates once a second.
    Blink,
    /// Declared, and nothing does it. See [`ronoctua_notify::INERT`].
    Vibrate,
    /// Declared, and nothing does it. See [`ronoctua_notify::INERT`].
    Audio,
}

impl Knob {
    /// Every one, in the order the sheet draws them.
    pub const ALL: [Self; 7] = [
        Self::Show,
        Self::Life,
        Self::Loud,
        Self::Level,
        Self::Blink,
        Self::Vibrate,
        Self::Audio,
    ];

    /// The label's text key.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Show => ronoctua_i18n::key::NOTIF_KNOB_SHOW,
            Self::Life => ronoctua_i18n::key::NOTIF_KNOB_LIFE,
            Self::Loud => ronoctua_i18n::key::NOTIF_LOUD,
            Self::Level => ronoctua_i18n::key::NOTIF_LEVEL,
            Self::Blink => ronoctua_i18n::key::NOTIF_BLINK,
            Self::Vibrate => ronoctua_i18n::key::NOTIF_VIBRATE,
            Self::Audio => ronoctua_i18n::key::NOTIF_AUDIO,
        }
    }

    /// The field's name, as the tables and [`ronoctua_notify::INERT`] spell it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Life => "life",
            Self::Loud => "loud",
            Self::Level => "level",
            Self::Blink => "blink",
            Self::Vibrate => "vibrate",
            Self::Audio => "audio",
        }
    }

    /// Why this switch does nothing, when it does nothing.
    ///
    /// `None` for the ones that work. It is read from
    /// [`ronoctua_notify::INERT`] and not from a second list here — a switch
    /// that switches nothing off and says nothing is the trap this project has
    /// already paid for once.
    #[must_use]
    pub fn inert(self) -> Option<&'static str> {
        ronoctua_notify::INERT
            .iter()
            .find(|i| i.field == self.id())
            .map(|i| i.why)
    }
}

/// One row the manager draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// Is the cursor on it?
    pub focused: bool,
}

/// What the manager wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Mark this notification read.
    Read(u32),
    /// Forget this notification: out of the record for good.
    Forget(u32),
    /// Mark everything read.
    ReadAll,
    /// Run action `n` of the notification with this id.
    Act(u32, usize),
    /// The person adjusted `id`. `None` puts it back to the distro's default.
    ///
    /// The verdict travels WITH the errand rather than being written here: this
    /// module holds no book, and giving it one would be the second copy that
    /// goes stale — see the header.
    Adjusted(String, Option<Verdict>),
    /// Close the manager.
    Close,
}

/// The manager's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    pane: Pane,
    happened: usize,
    suppressed: usize,
    catalogue: usize,
    /// Which knob the sheet has focused. `None` = the sheet is closed.
    knob: Option<usize>,
}

/// What goes in each pane, resolved once so the drawing and the keys agree.
///
/// # Why it is built and not walked twice
///
/// Because the two halves used to disagree: the drawing filtered the records
/// one way and the cursor counted them another, and the row a key acted on was
/// not the row the eye was on. It is the same defect `ExpansionRow` carries a
/// comment about, one level up.
pub struct Lists<'a> {
    /// Unread first, then read — one list, two tones.
    pub happened: Vec<&'a Record>,
    /// What a filter kept off the screen.
    pub suppressed: Vec<&'a Record>,
    /// `(id, is a rule)` for everything the distro ships with.
    pub catalogue: Vec<(String, bool)>,
}

impl<'a> Lists<'a> {
    /// Splits the records and lays the catalogue out.
    #[must_use]
    pub fn of(records: &'a [Record], book: &Book) -> Self {
        let (suppressed, shown): (Vec<&Record>, Vec<&Record>) = records
            .iter()
            .rev()
            .partition(|r| matches!(r.presentation.show, Show::Hidden));
        // UNREAD FIRST, and the read ones under them in a quieter tone — the
        // owner's arrangement, and one list rather than two because they are
        // the same thing at two ages.
        let mut happened: Vec<&Record> = shown.iter().copied().filter(|r| !r.read).collect();
        happened.extend(shown.iter().copied().filter(|r| r.read));
        let catalogue = book
            .entries
            .iter()
            .map(|e| (e.id.clone(), false))
            .chain(book.rules.iter().map(|r| (r.id.clone(), true)))
            .collect();
        Self {
            happened,
            suppressed,
            catalogue,
        }
    }

    /// How many rows one pane has.
    #[must_use]
    pub fn len(&self, pane: Pane) -> usize {
        match pane {
            Pane::Happened => self.happened.len(),
            Pane::Suppressed => self.suppressed.len(),
            Pane::Catalogue => self.catalogue.len(),
        }
    }
}

/// Where a pane's rows begin and how wide they are, in cells.
///
/// # Why this is one function and not four
///
/// The drawing worked it out inline and the pointer worked it out again, and
/// the two agreed by luck: a bin drawn at one cell and hit-tested at another is
/// a control that does nothing where it appears and something where it does
/// not. It is the divergence `ExpansionRow` carries a comment about, one level
/// up.
///
/// # And why the left pane stops short of the split
///
/// Because its two targets sit at its right end, and with the pane ending
/// exactly at the split they landed against the other column's first cell —
/// reported from the screen as bins that looked like they belonged to the
/// notifications on the right.
///
/// The gap is `space.modeline.segmentGap`, which is the number this system
/// already uses for "two things on one line are two things". The right pane
/// stops one cell short of the screen for the same reason its band does.
#[must_use]
pub fn extent_of(pane: Pane, columns: u16) -> (u16, u16) {
    let left = tokens::space::MODELINE_EDGELEFT;
    if pane == Pane::Catalogue || !two_columns(columns) {
        return (left, columns.saturating_sub(left * 2));
    }
    let split = split_at(columns);
    match pane {
        Pane::Suppressed => (split, columns.saturating_sub(split + left)),
        _ => (
            left,
            split.saturating_sub(left + tokens::space::MODELINE_SEGMENTGAP),
        ),
    }
}

/// Does the width afford two columns?
///
/// The token is the cut and the arithmetic is here: below it the panes stack,
/// above it they sit side by side. It is the same answer in the footer's region
/// and in the window, which is what "one implementation, two doors" has to mean
/// for a layout.
#[must_use]
pub const fn two_columns(columns: u16) -> bool {
    columns >= tokens::space::NOTIFICATIONS_TWOCOLUMNSFROM
}

/// Where the right-hand column starts, in cells.
///
/// The left one is wider: it holds sentences, and the right one holds what was
/// filtered out — which is a reference list, read rarely and never at length.
#[must_use]
pub fn split_at(columns: u16) -> u16 {
    columns * 3 / 5
}

impl View {
    /// A fresh manager, on the first pane.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Which pane has the cursor.
    #[must_use]
    pub const fn pane(&self) -> Pane {
        self.pane
    }

    /// Where the cursor is, in the pane that has it.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        match self.pane {
            Pane::Happened => self.happened,
            Pane::Suppressed => self.suppressed,
            Pane::Catalogue => self.catalogue,
        }
    }

    /// Puts the cursor on row `n`, and acts if the click landed on a target.
    ///
    /// # Why the mouse acts here, when it only aims in the transfer panel
    ///
    /// Because these two targets are DRAWN, one per row, and each says what it
    /// does: an open envelope and a bin. The transfer panel's rule — the keys
    /// act, the mouse aims — is about a click on the ROW, where nothing on
    /// screen says which of five destructive things it would run. A click on a
    /// bin is not a guess.
    ///
    /// Clicking anywhere else on the row still only aims.
    pub fn clicked(
        &mut self,
        n: usize,
        cell: Option<u16>,
        lists: &Lists<'_>,
        columns: u16,
    ) -> Errand {
        self.set_cursor(n);
        let (at, room) = self.row_extent(columns);
        let Some(hit) = cell.and_then(|c| target_at(c, at, room)) else {
            return Errand::Nothing;
        };
        let id = match self.pane {
            Pane::Happened => lists.happened.get(n).map(|r| r.notification.id),
            Pane::Suppressed => lists.suppressed.get(n).map(|r| r.notification.id),
            Pane::Catalogue => None,
        };
        let Some(id) = id else { return Errand::Nothing };
        match hit {
            Hit::MarkRead => Errand::Read(id),
            Hit::Delete => Errand::Forget(id),
        }
    }

    /// What the message line says about the focused row.
    ///
    /// `None` when there is nothing to add. Today it is the reason a switch
    /// does nothing — see the note in `sheet_rows`.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let knob = self.knob?;
        Some(texts.t(Knob::ALL.get(knob)?.inert()?))
    }

    /// Where the focused pane's rows begin and how wide they are.
    ///
    /// One place, read by the drawing and by the pointer — two answers to
    /// "where does this row end" is how a bin comes to be one cell off.
    #[must_use]
    pub fn row_extent(&self, columns: u16) -> (u16, u16) {
        extent_of(self.pane, columns)
    }

    fn set_cursor(&mut self, n: usize) {
        match self.pane {
            Pane::Happened => self.happened = n,
            Pane::Suppressed => self.suppressed = n,
            Pane::Catalogue => self.catalogue = n,
        }
    }
}

/// The presentation an id has in force, and the distro's for the same id.
///
/// Two, and not one: the manager draws BOTH, because "use the default or
/// customise" is a comparison, and a screen that only shows the result cannot
/// say which of the two you are looking at.
#[must_use]
pub fn in_force(book: &Book, id: &str) -> (Presentation, bool) {
    let mut p = Presentation::from_urgency(ronoctua_notify::Urgency::Normal);
    if let Some(e) = book.entry(id) {
        e.apply.over(&mut p);
    } else if let Some(r) = book.rules.iter().find(|r| r.id == id) {
        r.apply.over(&mut p);
    }
    let adjusted = book.adjustments.get(id).is_some_and(|a| !a.says_nothing());
    if let Some(a) = book.adjustments.get(id) {
        a.over(&mut p);
    }
    (p, adjusted)
}

/// Turns a knob one step, writing the result into the verdict to save.
///
/// `step` is `+1` or `-1` — the side arrows, the same gesture the Lucide sheet
/// already teaches. Cycling and not a free field: every one of these has a
/// handful of values, and a text box would be a second way of typing.
pub fn turn(now: &Presentation, knob: Knob, step: i32, into: &mut Verdict) {
    let forward = step >= 0;
    match knob {
        Knob::Show => {
            let i = Show::ALL.iter().position(|s| *s == now.show).unwrap_or(0);
            into.show = Some(Show::ALL[wrap(i, Show::ALL.len(), forward)]);
        }
        Knob::Life => {
            let i = Life::ALL.iter().position(|l| *l == now.life).unwrap_or(0);
            into.life = Some(Life::ALL[wrap(i, Life::ALL.len(), forward)]);
        }
        Knob::Loud => into.loud = Some(!now.loud),
        Knob::Blink => into.blink = Some(!now.blink),
        Knob::Vibrate => into.vibrate = Some(!now.vibrate),
        Knob::Audio => into.audio = Some(!now.audio),
        Knob::Level => {
            // TEN AT A TIME. One at a time would be eleven presses to cross the
            // range, and nobody distinguishes level 63 from level 64.
            let step = if forward { 10_i32 } else { -10 };
            let v = i32::from(now.level) + step;
            into.level = Some(v.clamp(0, i32::from(TOP_LEVEL)) as u8);
        }
    }
}

const fn wrap(i: usize, len: usize, forward: bool) -> usize {
    if forward {
        (i + 1) % len
    } else {
        (i + len - 1) % len
    }
}

/// How a knob's value reads on screen.
#[must_use]
pub fn value_of(p: &Presentation, knob: Knob, texts: &ronoctua_i18n::Texts) -> String {
    let yes_no = |v: bool| {
        texts.t(if v {
            ronoctua_i18n::key::NOTIF_YES
        } else {
            ronoctua_i18n::key::NOTIF_NO
        })
    };
    match knob {
        Knob::Show => texts.t(match p.show {
            Show::Hidden => ronoctua_i18n::key::NOTIF_SHOW_HIDDEN,
            Show::Brief => ronoctua_i18n::key::NOTIF_SHOW_BRIEF,
            Show::Full => ronoctua_i18n::key::NOTIF_SHOW_FULL,
        }),
        Knob::Life => texts.t(match p.life {
            Life::Quick => ronoctua_i18n::key::NOTIF_LIFE_QUICK,
            Life::Normal => ronoctua_i18n::key::NOTIF_LIFE_NORMAL,
            Life::Long => ronoctua_i18n::key::NOTIF_LIFE_LONG,
        }),
        Knob::Loud => yes_no(p.loud),
        Knob::Blink => yes_no(p.blink),
        Knob::Vibrate => yes_no(p.vibrate),
        Knob::Audio => yes_no(p.audio),
        Knob::Level => p.level.to_string(),
    }
}

/// A piece of a row, with nothing clickable of the file manager's kind.
///
/// The manager answers pointer questions from its OWN row model — see
/// [`View::at`] — so the field's `hit` is always `Nothing`. Reusing
/// [`crate::files::Field`] is what lets the expansion draw this without a
/// second kind of row.
fn piece(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: FilesHit::Nothing,
        explain: None,
    }
}

/// How many cells a row keeps at its right end for the two mouse targets.
///
/// Two glyphs and the air around them. They are the only clickable things in
/// the list, and they sit at the RIGHT for the reason the band's own control
/// does: the left margin is where the sentence starts.
pub const TARGETS: u16 = 4;

/// Which of a row's two targets is at cell `column`.
///
/// The row's own arithmetic, and the only place it is written — the drawing
/// reads the same function, so what the pointer hits is what was drawn.
#[must_use]
pub fn target_at(column: u16, at: u16, room: u16) -> Option<Hit> {
    let end = at + room;
    match end.checked_sub(column) {
        Some(1) => Some(Hit::Delete),
        Some(3) => Some(Hit::MarkRead),
        _ => None,
    }
}

/// What a click on a row of the manager does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Mark it read, without taking it out of the record.
    MarkRead,
    /// Forget it. The history is the record of what happened, and this is the
    /// person deciding one line of it is not worth keeping.
    Delete,
}

/// One record, as one line.
///
/// `focused` is drawn as a COLOUR and not as a box: it is the file manager's
/// own answer to the same question, and invariant 15 leaves no other.
fn record_line(
    r: &Record,
    at: u16,
    room: u16,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Field> {
    let tone = if focused {
        tokens::color::STATE_ACCENT
    } else if matches!(r.presentation.show, Show::Hidden) {
        tokens::color::NOTIFICATION_SUPPRESSED
    } else if r.read {
        tokens::color::NOTIFICATION_READ
    } else if r.presentation.loud {
        tokens::color::NOTIFICATION_LOUD
    } else {
        tokens::color::TEXT_SECONDARY
    };
    // WHAT IT IS, in the margin — its own glyph or the generic one for its
    // severity. It used to be a read/unread circle; what says read here is the
    // TONE, and the margin is worth more as "which of these is the one I am
    // looking for".
    let mut out = vec![piece(at, crate::band::glyph_of_record(r).to_string(), tone)];
    let text_at = at + 2;
    let mut line = r.notification.line();
    // WHICH FILTER CAUGHT IT, next to what it caught. A filter that does not
    // say why it took something off the screen is a filter nobody can correct.
    if let Some(by) = &r.presentation.by {
        line.push(' ');
        line.push_str(&texts.t_with(
            if matches!(r.presentation.show, Show::Hidden) {
                ronoctua_i18n::key::NOTIF_SUPPRESSED_BY
            } else {
                ronoctua_i18n::key::NOTIF_DECIDED_BY
            },
            &[("rule", by)],
        ));
    }
    let room_for_text = room.saturating_sub(2 + TARGETS);
    out.push(piece(
        text_at,
        layout::elide_end(&line, room_for_text),
        tone,
    ));

    // AND THE TWO TARGETS, at the row's right end.
    //
    // Each has a key — `Espaço` marks read, `d` forgets — which is the rule
    // this footer holds the mouse to: a second way to the same door, never a
    // door of its own.
    //
    // An UNREAD one gets the envelope; a read one does not, because marking
    // read what is already read is a gesture with no effect, and a control that
    // does nothing is worse than one that is not offered.
    let end = at + room;
    if !r.read {
        out.push(piece(
            end.saturating_sub(3),
            tokens::icon::NOTIFICATION_MARK_READ.to_string(),
            tone,
        ));
    }
    out.push(piece(
        end.saturating_sub(1),
        tokens::icon::NOTIFICATION_DELETE.to_string(),
        tone,
    ));
    out
}

/// One catalogue row: what it is, and whether it is yours or the distro's.
fn catalogue_line(
    id: &str,
    is_rule: bool,
    book: &Book,
    at: u16,
    room: u16,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Field> {
    let (p, adjusted) = in_force(book, id);
    let name = texts.t(&if is_rule {
        format!("notif-rule-{id}")
    } else {
        format!("notif-{id}-summary")
    });
    let source = texts.t(if adjusted {
        ronoctua_i18n::key::NOTIF_YOURS
    } else {
        ronoctua_i18n::key::NOTIF_DEFAULT
    });
    let summary = format!(
        "{} · {} · {}",
        value_of(&p, Knob::Show, texts),
        value_of(&p, Knob::Life, texts),
        source
    );
    let half = room / 2;
    vec![
        piece(
            at,
            layout::elide_end(&name, half.saturating_sub(1)),
            if focused {
                tokens::color::STATE_ACCENT
            } else if adjusted {
                tokens::color::TEXT_PRIMARY
            } else {
                tokens::color::TEXT_SECONDARY
            },
        ),
        piece(
            at + half,
            layout::elide_end(&summary, room.saturating_sub(half)),
            tokens::color::TEXT_DIM,
        ),
    ]
}

/// A key, already reduced to what the manager cares about.
///
/// # Why not the keysym
///
/// So the whole manager can be driven by a test with no compositor. Mapping
/// `Keysym` to this is four lines in `surface.rs` and it is the only part that
/// needs a seat; everything below is arithmetic over a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Left — inside the sheet it turns the knob back.
    Left,
    /// Right — inside the sheet it turns the knob forward.
    Right,
    /// Next pane.
    Tab,
    /// Act on the focused row.
    Enter,
    /// Mark the focused notification read.
    Space,
    /// One level back: the sheet closes, then the manager.
    Escape,
    /// A letter.
    Letter(char),
}

impl View {
    /// The rows to draw, for the width given.
    ///
    /// # Why the headings are rows
    ///
    /// Because the region draws a list, and a heading that lived outside it
    /// would need its own arithmetic for where the list starts — which is the
    /// divergence this file's header warns about, one level down.
    #[must_use]
    pub fn rows(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let left = tokens::space::MODELINE_EDGELEFT;
        let room = columns.saturating_sub(left * 2);

        // THE SHEET takes the whole region: it is one thing being adjusted, and
        // a list beside it would be a second place for the eye to be.
        if let Some(knob) = self.knob {
            return self.sheet_rows(lists, book, left, room, knob, texts);
        }

        // THE CATALOGUE takes the whole width too — it is not something that
        // happened, it is something that is configured, and it has two columns
        // of its own.
        if self.pane == Pane::Catalogue || !two_columns(columns) {
            return self.one_column(lists, book, left, room, texts);
        }

        let split = split_at(columns);
        let mut out = vec![Row {
            fields: vec![
                piece(
                    left,
                    texts.t(Pane::Happened.heading()),
                    tokens::color::TEXT_GROUP,
                ),
                piece(
                    split,
                    texts.t(Pane::Suppressed.heading()),
                    tokens::color::TEXT_GROUP,
                ),
            ],
            focused: false,
        }];
        let tall = lists.happened.len().max(lists.suppressed.len());
        for i in 0..tall {
            let mut fields = Vec::new();
            // THE CURSOR IS IN ONE PANE, and only that pane's half lights up.
            // Lighting the whole row would say the two items were one thing —
            // and they are two different notifications, in two lists.
            let here = self.cursor() == i;
            if let Some(r) = lists.happened.get(i) {
                let focused = here && self.pane == Pane::Happened;
                let (at, room) = extent_of(Pane::Happened, columns);
                fields.extend(record_line(r, at, room, focused, texts));
            }
            if let Some(r) = lists.suppressed.get(i) {
                let focused = here && self.pane == Pane::Suppressed;
                let (at, room) = extent_of(Pane::Suppressed, columns);
                fields.extend(record_line(r, at, room, focused, texts));
            }
            out.push(Row {
                fields,
                focused: here,
            });
        }
        if tall == 0 {
            out.push(Row {
                fields: vec![piece(
                    left,
                    texts.t(ronoctua_i18n::key::NOTIF_NOTHING_HERE),
                    tokens::color::TEXT_DIM,
                )],
                focused: false,
            });
        }
        out
    }

    /// The panes stacked, one under the other. The narrow case, and the
    /// catalogue's.
    fn one_column(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let mut out = vec![Row {
            fields: vec![piece(
                left,
                texts.t(self.pane.heading()),
                tokens::color::TEXT_GROUP,
            )],
            focused: false,
        }];
        let n = lists.len(self.pane);
        for i in 0..n {
            let focused = self.cursor() == i;
            let fields = match self.pane {
                Pane::Happened => record_line(lists.happened[i], left, room, focused, texts),
                Pane::Suppressed => record_line(lists.suppressed[i], left, room, focused, texts),
                Pane::Catalogue => {
                    let (id, rule) = &lists.catalogue[i];
                    catalogue_line(id, *rule, book, left, room, focused, texts)
                }
            };
            out.push(Row { fields, focused });
        }
        if n == 0 {
            out.push(Row {
                fields: vec![piece(
                    left,
                    texts.t(ronoctua_i18n::key::NOTIF_NOTHING_HERE),
                    tokens::color::TEXT_DIM,
                )],
                focused: false,
            });
        }
        out
    }

    /// One catalogue entry, field by field, with the distro's answer beside it.
    fn sheet_rows(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        left: u16,
        room: u16,
        knob: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let Some((id, is_rule)) = lists.catalogue.get(self.catalogue) else {
            return Vec::new();
        };
        let (p, adjusted) = in_force(book, id);
        let name = texts.t(&if *is_rule {
            format!("notif-rule-{id}")
        } else {
            format!("notif-{id}-summary")
        });
        let mut out = vec![Row {
            fields: vec![
                piece(left, name, tokens::color::TEXT_PRIMARY),
                piece(
                    left + room / 2,
                    texts.t(if adjusted {
                        ronoctua_i18n::key::NOTIF_YOURS
                    } else {
                        ronoctua_i18n::key::NOTIF_DEFAULT
                    }),
                    tokens::color::TEXT_DIM,
                ),
            ],
            focused: false,
        }];
        for (i, k) in Knob::ALL.iter().enumerate() {
            // THE FOCUSED KNOB IS THE ONE THE ARROWS TURN, and it says so in
            // colour — the same mark the lists use, and the only one invariant
            // 15 leaves. Without it the sheet is seven rows and no cursor, and
            // pressing an arrow changes a line nobody was looking at.
            let focused = i == knob;
            let label = if focused {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_SECONDARY
            };
            let value = if focused {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            // A SWITCH THAT SWITCHES NOTHING OFF STILL SAYS SO — but on the
            // MESSAGE LINE, when the row is focused, and not as a third column
            // beside it. See [`View::hint`].
            //
            // It was drawn inline and the owner asked for it gone: two rows out
            // of seven carrying a sentence made the sheet read as a list of
            // problems. The honesty is not negotiable — the trap has a name in
            // `CLAUDE.md` and this project has paid for it twice — so it moved
            // to where this footer already puts explanations, which is the same
            // line the file manager's fields explain themselves on.
            //
            // What stays on the row is the value being `no`, which is true.
            let fields = vec![
                piece(left, texts.t(k.label()), label),
                piece(left + room / 3, value_of(&p, *k, texts), value),
            ];
            out.push(Row { fields, focused });
        }
        out
    }

    /// Acts on a key.
    #[must_use]
    pub fn key(&mut self, press: Press, lists: &Lists<'_>, book: &Book) -> Errand {
        if let Some(knob) = self.knob {
            return self.sheet_key(press, lists, book, knob);
        }
        let n = lists.len(self.pane);
        match press {
            Press::Up => {
                self.set_cursor(self.cursor().saturating_sub(1));
                Errand::Nothing
            }
            Press::Down => {
                self.set_cursor((self.cursor() + 1).min(n.saturating_sub(1)));
                Errand::Nothing
            }
            Press::Tab => {
                self.pane = self.pane.next();
                // THE NEW PANE MAY BE SHORTER. Each pane keeps its own cursor —
                // coming back to a list should come back to where you were —
                // but a cursor past the end would light a row that is not there
                // and act on nothing.
                let there = lists.len(self.pane);
                self.set_cursor(self.cursor().min(there.saturating_sub(1)));
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            Press::Space => match self.pane {
                Pane::Happened => lists
                    .happened
                    .get(self.cursor())
                    .map_or(Errand::Nothing, |r| Errand::Read(r.notification.id)),
                // A SUPPRESSED ONE IS ALREADY OFF THE COUNT, so marking it read
                // would be a gesture with no effect — and a key that does
                // nothing is worse than a key that is not bound.
                _ => Errand::Nothing,
            },
            Press::Enter => match self.pane {
                // On the catalogue, `Enter` opens the sheet: that is where the
                // distro's default is either kept or changed.
                Pane::Catalogue if self.cursor() < n => {
                    self.knob = Some(0);
                    Errand::Nothing
                }
                Pane::Happened => lists
                    .happened
                    .get(self.cursor())
                    .filter(|r| !r.notification.actions.is_empty())
                    .map_or(Errand::Nothing, |r| Errand::Act(r.notification.id, 0)),
                _ => Errand::Nothing,
            },
            Press::Letter('r') => Errand::ReadAll,
            // `d` FORGETS the focused one. Reserved, and the generator refuses
            // an action that asks for the same letter — an action shadowed by a
            // view key is an action nobody can reach, and the way to find out
            // would be pressing it and watching something else happen.
            Press::Letter('d') => match self.pane {
                Pane::Happened => lists
                    .happened
                    .get(self.cursor())
                    .map_or(Errand::Nothing, |r| Errand::Forget(r.notification.id)),
                Pane::Suppressed => lists
                    .suppressed
                    .get(self.cursor())
                    .map_or(Errand::Nothing, |r| Errand::Forget(r.notification.id)),
                Pane::Catalogue => Errand::Nothing,
            },
            // THE ACTION'S OWN LETTER. It is what makes an action on the band
            // reachable without the mouse: the band cannot take a key (the
            // footer at rest does not hold the keyboard, or no buffer would
            // ever receive one), so it is pressed here.
            Press::Letter(c) if self.pane == Pane::Happened => lists
                .happened
                .get(self.cursor())
                .and_then(|r| {
                    r.notification
                        .actions
                        .iter()
                        .position(|a| a.key == Some(c))
                        .map(|i| Errand::Act(r.notification.id, i))
                })
                .unwrap_or(Errand::Nothing),
            Press::Letter(_) | Press::Left | Press::Right => Errand::Nothing,
        }
    }

    /// The sheet's keys: the arrows turn, `r` restores, `Esc` closes it.
    fn sheet_key(&mut self, press: Press, lists: &Lists<'_>, book: &Book, knob: usize) -> Errand {
        let Some((id, _)) = lists.catalogue.get(self.catalogue) else {
            self.knob = None;
            return Errand::Nothing;
        };
        match press {
            Press::Escape => {
                self.knob = None;
                Errand::Nothing
            }
            Press::Up => {
                self.knob = Some(knob.saturating_sub(1));
                Errand::Nothing
            }
            Press::Down => {
                self.knob = Some((knob + 1).min(Knob::ALL.len() - 1));
                Errand::Nothing
            }
            Press::Left | Press::Right => {
                let (p, _) = in_force(book, id);
                let mut v = book.adjustments.get(id).cloned().unwrap_or_default();
                turn(
                    &p,
                    Knob::ALL[knob],
                    if press == Press::Right { 1 } else { -1 },
                    &mut v,
                );
                Errand::Adjusted(id.clone(), Some(v))
            }
            // BACK TO THE DISTRO'S. The adjustment is REMOVED, not set to the
            // default value: "yours" and "the distro's, which happens to be the
            // same" are different answers to "who decided this?", and the
            // manager draws the difference.
            Press::Letter('r') => Errand::Adjusted(id.clone(), None),
            Press::Tab | Press::Enter | Press::Space | Press::Letter(_) => Errand::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_notify::{Notification, Urgency};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn record(id: u32, show: Show, read: bool) -> Record {
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.show = show;
        Record {
            notification: Notification::fresh_one(
                id,
                "app".into(),
                None,
                format!("r{id}"),
                String::new(),
                Urgency::Normal,
            ),
            presentation: p,
            read,
        }
    }

    #[test]
    fn the_unread_come_first_and_the_read_follow() {
        // The owner's arrangement: one list, two tones — they are the same
        // thing at two ages, and two lists would make the eye look twice.
        let records = vec![
            record(1, Show::Brief, true),
            record(2, Show::Brief, false),
            record(3, Show::Hidden, false),
            record(4, Show::Brief, false),
        ];
        let l = Lists::of(&records, &Book::distro());
        let ids: Vec<u32> = l.happened.iter().map(|r| r.notification.id).collect();
        // Newest first inside each half: 4 then 2 (unread), then 1 (read).
        assert_eq!(ids, vec![4, 2, 1]);
        // And the suppressed one is not among them — it has a column of its own.
        assert_eq!(l.suppressed.len(), 1);
        assert_eq!(l.suppressed[0].notification.id, 3);
    }

    #[test]
    fn the_catalogue_lists_everything_the_distro_ships() {
        // The owner asked for it by name: one list with all of them, so each
        // can be managed on its own.
        let b = Book::distro();
        let l = Lists::of(&[], &b);
        assert_eq!(l.catalogue.len(), b.entries.len() + b.rules.len());
        assert!(
            l.catalogue
                .iter()
                .any(|(id, rule)| id == "screenshot" && !rule)
        );
        assert!(
            l.catalogue
                .iter()
                .any(|(id, rule)| id == "important-gets-room" && *rule)
        );
    }

    #[test]
    fn one_column_when_narrow_and_two_when_wide() {
        // The same answer in the footer's region and in the window: what
        // decides is the width it is given, not which door it came through.
        assert!(!two_columns(60));
        assert!(two_columns(tokens::space::NOTIFICATIONS_TWOCOLUMNSFROM));
        assert!(two_columns(200));
        // The left column is the wider one: it holds sentences.
        assert!(split_at(100) > 100 - split_at(100));
    }

    #[test]
    fn a_knob_cycles_and_the_level_moves_by_ten() {
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.show = Show::Brief;
        let mut v = Verdict::default();
        turn(&p, Knob::Show, 1, &mut v);
        assert_eq!(v.show, Some(Show::Full));
        turn(&p, Knob::Show, -1, &mut v);
        assert_eq!(v.show, Some(Show::Hidden));
        // Ten at a time: one would be eleven presses to cross the range, and
        // nobody tells level 63 from 64.
        p.level = 50;
        turn(&p, Knob::Level, 1, &mut v);
        assert_eq!(v.level, Some(60));
        p.level = 95;
        turn(&p, Knob::Level, 1, &mut v);
        assert_eq!(v.level, Some(TOP_LEVEL), "passou do teto");
        p.level = 5;
        turn(&p, Knob::Level, -1, &mut v);
        assert_eq!(v.level, Some(0), "passou do chao");
    }

    #[test]
    fn the_two_dead_switches_say_why() {
        // `vibrate` and `audio` are declared and inert. The list comes from
        // `ronoctua_notify::INERT` and not from a second one here — two lists
        // would be one more thing to keep in step, and the day they diverge is
        // the day a switch goes quiet again.
        assert!(Knob::Vibrate.inert().is_some());
        assert!(Knob::Audio.inert().is_some());
        assert!(Knob::Show.inert().is_none());
        assert!(Knob::Loud.inert().is_none());
    }

    #[test]
    fn the_manager_shows_whose_decision_it_is() {
        // "use the distro's default, or customise" is a COMPARISON: a screen
        // that only shows the result cannot say which of the two you are on.
        let mut b = Book::distro();
        let (_, adjusted) = in_force(&b, "screenshot");
        assert!(!adjusted, "sem ajuste, e o padrao da distro");
        b.adjustments.insert(
            "screenshot".into(),
            Verdict {
                loud: Some(true),
                ..Verdict::default()
            },
        );
        let (p, adjusted) = in_force(&b, "screenshot");
        assert!(adjusted);
        assert!(p.loud);
    }

    #[test]
    fn an_empty_adjustment_is_not_an_adjustment() {
        let mut b = Book::distro();
        b.adjustments
            .insert("screenshot".into(), Verdict::default());
        assert!(!in_force(&b, "screenshot").1, "ajuste vazio nao e ajuste");
    }

    #[test]
    fn a_row_says_which_filter_caught_it() {
        let t = texts();
        let mut r = record(1, Show::Hidden, false);
        r.presentation.by = Some("minha-regra".into());
        let fields = record_line(&r, 0, 120, false, &t);
        let text: String = fields.iter().map(|f| f.text.as_str()).collect();
        assert!(text.contains("minha-regra"), "{text}");
    }

    #[test]
    fn two_columns_draw_both_lists_on_one_row() {
        // The owner's arrangement: what happened on the left, what a filter
        // suppressed on the right, side by side while there is room.
        let t = texts();
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let rows = View::fresh().rows(&lists, &b, 200, &t);
        // A heading row, then one row carrying BOTH items.
        assert_eq!(rows.len(), 2);
        let text: String = rows[1].fields.iter().map(|f| f.text.as_str()).collect();
        assert!(text.contains("r1") && text.contains("r2"), "{text}");
        // And the right-hand one starts past the split.
        let split = split_at(200);
        assert!(rows[1].fields.iter().any(|f| f.at >= split));
    }

    #[test]
    fn one_column_stacks_them_instead() {
        // A small display, or a narrow window: the same code, deciding by the
        // width it was given.
        let t = texts();
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let rows = View::fresh().rows(&lists, &b, 50, &t);
        // Only the focused pane, and its heading.
        assert_eq!(rows.len(), 2);
        let text: String = rows[1].fields.iter().map(|f| f.text.as_str()).collect();
        assert!(text.contains("r1") && !text.contains("r2"), "{text}");
    }

    #[test]
    fn an_empty_pane_says_so() {
        // An empty region is indistinguishable from a key that did not work.
        let t = texts();
        let b = Book::distro();
        let rows = View::fresh().rows(&Lists::of(&[], &b), &b, 50, &t);
        assert_eq!(rows.len(), 2);
        assert!(!rows[1].fields.is_empty());
    }

    #[test]
    fn space_marks_the_focused_one_read() {
        let b = Book::distro();
        let records = vec![record(9, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(v.key(Press::Space, &lists, &b), Errand::Read(9));
        // A SUPPRESSED one is already off the count, so the same key on the
        // other column does nothing — a key with no effect is worse than a key
        // that is not bound.
        v.pane = Pane::Suppressed;
        assert_eq!(v.key(Press::Space, &lists, &b), Errand::Nothing);
    }

    #[test]
    fn the_sheet_opens_on_the_catalogue_and_esc_backs_out_one_level() {
        // `Esc` walks back ONE level, which is the same contract every buffer's
        // state has: the sheet first, then the view.
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        assert_eq!(v.key(Press::Enter, &lists, &b), Errand::Nothing);
        assert!(v.knob.is_some(), "o Enter nao abriu a folha");
        assert_eq!(v.key(Press::Escape, &lists, &b), Errand::Nothing);
        assert!(v.knob.is_none(), "o Esc nao fechou a folha");
        assert_eq!(v.key(Press::Escape, &lists, &b), Errand::Close);
    }

    #[test]
    fn an_arrow_in_the_sheet_writes_an_adjustment() {
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b);
        let Errand::Adjusted(id, verdict) = v.key(Press::Right, &lists, &b) else {
            panic!("a seta nao ajustou nada");
        };
        assert_eq!(id, lists.catalogue[0].0);
        assert!(verdict.is_some_and(|x| !x.says_nothing()));
        // And `r` puts it back to the distro's — REMOVING the adjustment, not
        // setting it to the same value: "yours" and "the distro's, which
        // happens to agree" are different answers to "who decided this?".
        assert_eq!(
            v.key(Press::Letter('r'), &lists, &b),
            Errand::Adjusted(lists.catalogue[0].0.clone(), None)
        );
    }

    #[test]
    fn a_notifications_own_letter_runs_its_action() {
        // The band cannot take a key — the footer at rest does not hold the
        // keyboard, or no buffer would ever receive one. This is where the
        // letter drawn on the band is actually pressed.
        let b = Book::distro();
        let mut r = record(5, Show::Brief, false);
        r.notification.actions = vec![
            ronoctua_notify::Action {
                id: "open".into(),
                key: Some('a'),
                label: ronoctua_notify::Label::Text("abrir".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
            ronoctua_notify::Action {
                id: "edit".into(),
                key: Some('e'),
                label: ronoctua_notify::Label::Text("editar".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
        ];
        let records = vec![r];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(v.key(Press::Letter('e'), &lists, &b), Errand::Act(5, 1));
        // A letter nothing answers to does nothing, rather than something else.
        assert_eq!(v.key(Press::Letter('z'), &lists, &b), Errand::Nothing);
    }

    #[test]
    fn every_key_in_the_table_does_something() {
        // THE TABLE IS THE DECLARATION and the `match` is the behaviour, so
        // they can drift. A key that is listed and answers nothing is a line in
        // the manual promising something that does not happen — and it is the
        // check `files-everything-has-a-key` exists for, one view over.
        let b = Book::distro();
        let mut r = record(1, Show::Brief, false);
        r.notification.actions = vec![ronoctua_notify::Action {
            id: "open".into(),
            key: Some('a'),
            label: ronoctua_notify::Label::Text("abrir".into()),
            doing: ronoctua_notify::Doing::Invoke,
        }];
        let records = vec![r];
        let lists = Lists::of(&records, &b);
        for (key, id) in OF_THE_VIEW {
            let press = match *key {
                "SPC" => Press::Space,
                "RET" => Press::Enter,
                k => Press::Letter(k.chars().next().unwrap()),
            };
            let mut v = View::fresh();
            let did = v.key(press, &lists, &b);
            assert_ne!(did, Errand::Nothing, "`{key}` ({id}) nao faz nada");
        }
    }

    #[test]
    fn the_left_column_stops_short_of_the_right_one() {
        // Reported from the screen: the bins at the end of the left column sat
        // against the right column's first cell, and read as if they belonged
        // to the notifications over there.
        let split = split_at(200);
        let (at, room) = extent_of(Pane::Happened, 200);
        let gap = split - (at + room);
        assert_eq!(gap, tokens::space::MODELINE_SEGMENTGAP);
        // And the right column does not end against the screen's edge either.
        let (at, room) = extent_of(Pane::Suppressed, 200);
        assert_eq!(at, split);
        assert!(at + room < 200);
    }

    #[test]
    fn the_drawing_and_the_pointer_measure_the_same_row() {
        // They worked it out in two places and agreed by luck. A bin drawn at
        // one cell and hit-tested at another is a control that does nothing
        // where it appears and something where it does not.
        let t = texts();
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        for pane in [Pane::Happened, Pane::Suppressed] {
            let mut v = View::fresh();
            v.pane = pane;
            let (at, room) = v.row_extent(200);
            let rows = v.rows(&lists, &b, 200, &t);
            // The row carrying THIS pane's item has a bin at the pane's end.
            assert!(
                rows[1].fields.iter().any(|f| f.at == at + room - 1),
                "{pane:?}: lixeira desenhada fora de {:?}",
                (at, room)
            );
            assert_eq!(target_at(at + room - 1, at, room), Some(Hit::Delete));
        }
    }

    #[test]
    fn the_two_targets_sit_at_the_row_s_right_end() {
        // Drawn, one per row, each saying what it does — an open envelope and a
        // bin. The pointer's answer and the drawing's come from ONE function,
        // so a bin cannot end up one cell from where it was drawn.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let v = View::fresh();
        let (at, room) = v.row_extent(200);
        let rows = v.rows(&lists, &b, 200, &t);
        let drawn: Vec<u16> = rows[1].fields.iter().map(|f| f.at).collect();
        let end = at + room;
        assert!(drawn.contains(&(end - 1)), "sem a lixeira em {}", end - 1);
        assert!(drawn.contains(&(end - 3)), "sem o envelope em {}", end - 3);
        assert_eq!(target_at(end - 1, at, room), Some(Hit::Delete));
        assert_eq!(target_at(end - 3, at, room), Some(Hit::MarkRead));
        // And the sentence's cells are not targets: a click there only aims.
        assert_eq!(target_at(at + 5, at, room), None);
    }

    #[test]
    fn a_read_one_offers_no_envelope() {
        // Marking read what is already read is a gesture with no effect, and a
        // control that does nothing is worse than one that is not offered.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, true)];
        let lists = Lists::of(&records, &b);
        let v = View::fresh();
        let (at, room) = v.row_extent(200);
        let rows = v.rows(&lists, &b, 200, &t);
        let drawn: Vec<u16> = rows[1].fields.iter().map(|f| f.at).collect();
        assert!(drawn.contains(&(at + room - 1)), "a lixeira sumiu");
        assert!(!drawn.contains(&(at + room - 3)), "envelope numa ja lida");
    }

    #[test]
    fn clicking_a_target_acts_and_clicking_the_row_only_aims() {
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false), record(2, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let (at, room) = v.row_extent(200);
        // Newest first: row 0 is id 2.
        assert_eq!(v.clicked(0, Some(at + 5), &lists, 200), Errand::Nothing);
        assert_eq!(v.cursor(), 0, "o clique na linha ao menos mira");
        assert_eq!(
            v.clicked(1, Some(at + room - 3), &lists, 200),
            Errand::Read(1)
        );
        assert_eq!(
            v.clicked(1, Some(at + room - 1), &lists, 200),
            Errand::Forget(1)
        );
    }

    #[test]
    fn d_forgets_the_focused_one() {
        // The keyboard's half of the bin. The generator refuses an action that
        // asks for `d`, so nothing can shadow it.
        let b = Book::distro();
        let records = vec![record(7, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(v.key(Press::Letter('d'), &lists, &b), Errand::Forget(7));
        // On the catalogue there is nothing to forget: those are rows of a
        // table, not things that happened.
        v.pane = Pane::Catalogue;
        assert_eq!(v.key(Press::Letter('d'), &lists, &b), Errand::Nothing);
    }

    #[test]
    fn the_sheet_says_why_a_switch_is_dead_on_the_message_line() {
        // It was a third column on the row and the owner asked for it gone. The
        // honesty had to stay — the trap has a name in `CLAUDE.md` — so it
        // moved to the line this footer already explains things on.
        let t = texts();
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b);
        // The first knob works, so it has nothing to add.
        assert_eq!(v.hint(&t), None);
        // Walk down to `vibrate`.
        let dead = Knob::ALL.iter().position(|k| k.inert().is_some()).unwrap();
        for _ in 0..dead {
            let _ = v.key(Press::Down, &lists, &b);
        }
        assert!(v.hint(&t).is_some(), "o interruptor morto ficou mudo");
        // And the row itself carries no third column any more.
        let rows = v.rows(&lists, &b, 200, &t);
        for (i, r) in rows.iter().enumerate().skip(1) {
            let texts: Vec<&str> = r.fields.iter().map(|f| f.text.as_str()).collect();
            assert!(r.fields.len() <= 2, "linha {i} com {texts:?}");
        }
    }

    #[test]
    fn changing_pane_brings_the_cursor_inside_it() {
        // Each pane keeps its own cursor — coming back to a list should come
        // back to where you were — but a cursor past the end would light a row
        // that is not there and act on nothing.
        let b = Book::distro();
        let records: Vec<Record> = (0..5).map(|i| record(i, Show::Brief, false)).collect();
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        for _ in 0..4 {
            let _ = v.key(Press::Down, &lists, &b);
        }
        assert_eq!(v.cursor(), 4);
        // The suppressed list is empty here.
        let _ = v.key(Press::Tab, &lists, &b);
        assert_eq!(v.pane(), Pane::Suppressed);
        assert_eq!(v.cursor(), 0);
        // And the one it left is where it was.
        let _ = v.key(Press::Tab, &lists, &b);
        let _ = v.key(Press::Tab, &lists, &b);
        assert_eq!(v.pane(), Pane::Happened);
        assert_eq!(v.cursor(), 4);
    }

    #[test]
    fn tab_goes_round_the_three_panes() {
        let mut p = Pane::default();
        let mut seen = vec![p];
        for _ in 0..3 {
            p = p.next();
            seen.push(p);
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[0], seen[3], "TAB tem de voltar ao comeco");
    }
}
