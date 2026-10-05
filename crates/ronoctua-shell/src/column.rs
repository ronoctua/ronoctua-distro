//! EVERY RIGHT COLUMN'S BASE, written once — as the `Door` is every
//! window's.
//!
//! # Why a base, and not each column doing it
//!
//! Because a column is more than a window anchored to the right: it has a
//! SIZE (four of them), a control line that changes it, a line that says it
//! has the keyboard, a WORKSPACE it belongs to and is shown with, and a slot
//! that keeps it one per workspace (`crate::right_column`). Written by each
//! tenant, the next tenant would forget one of them — the focus line, or the
//! workspace, or the zone the windows are told about. So [`Column`] is the
//! `door::Content` of every column, and does all of that; the tenant
//! ([`Tenant`]) only draws inside the [`Area`] it is handed, which is what a
//! window does inside its door.
//!
//! # Four sizes, in cells
//!
//! `space.column` — large, medium, small and the bar — in CONTENT cells, with
//! the margin on each side, so a column grows with the size of the text. None
//! of them takes more than `maxPct` of the screen's logical width — of the
//! room, while the screen resizer cuts that screen ([`room_of`]); one the
//! clamp makes as narrow as the size below it is drawn inert and says why
//! (`column-no-room`). The size is not a preference: every column opens at
//! its tenant's default.
//!
//! # The control line
//!
//! Line 0 is the column's, in every size: the four sizes as glyphs, counted
//! from the right edge from the widest to the narrowest — the way the edge
//! moves — and, left of them and apart by a thin rule, the glyph that CLOSES
//! the column: `[close] │ [L][M][S][B]`. The rule stands where the air on its
//! two sides is equal, measured on the INK of the two glyphs beside it
//! (`tokens::icon::ink_across`) and not on their cells: the cross fills its
//! advance and the widest size starts a fifth of the way into its own, so the
//! cells' middle is a pixel or two off the eye's.
//!
//! Under the pointer, what the glyph does — a short sentence set against the
//! close glyph from the right, a cell of air before it — and at rest nothing.
//! In the bar there is one glyph on line 0, which puts the column back to the
//! size it had, the close on the bar's LAST line, and no sentence: the bar is
//! a remote control of glyphs everybody knows (the owner's decision,
//! 2026-09-27).
//!
//! `C-<left>`, `C-<right>` and `C-q` are the keys, read BEFORE the tenant: the
//! player reads a named key without its modifiers, and would have taken
//! `C-<left>` for its own `<left>`. The tenant's header goes down to line 1
//! (`Content::header_line`, `chrome::Hinted::at`).
//!
//! # Closing
//!
//! The glyph and `C-q` close the COLUMN and nothing else: the tenant is told
//! (`Tenant::closing` — the player lets its pictures go), the door quits, and
//! `Drop` gives the slot up. What the player plays goes on playing.
//!
//! # The focus line
//!
//! While any keyboard is on the column, a line across its top in the colour
//! a focused window's outline has — the accent, or the discreet outline when
//! the person asked for discreet borders (`chrome::focus_line`): nothing else
//! said so when the column took the keys. It lies in the leading above line 0
//! and moves nothing.
//!
//! # The column's ground
//!
//! The FOOTER's page (`LEADER s t`): the chrome's palette, or the desk's — a
//! column is chrome in a strip, the footer's neighbour, and never a window
//! (`Content::page`).
//!
//! # One workspace per column
//!
//! A column belongs to the workspace it was opened on, BY NAME — a workspace
//! that empties is gone, and comes back under the same name — and is shown on
//! whichever screen shows that workspace, so each screen shows its own
//! workspace's column. Off screen it is PARKED (`Shell::Parked`): the role of
//! its surface is destroyed and the process, with everything it keeps, stays.
//! Every change of surface is decided in ONE place, [`Column::place`], from
//! two pure answers: which screen the workspace wants ([`wanted`]) and what
//! that means from where the column is ([`step`]).
//!
//! The cost, said: moving between a workspace with a column and one without
//! lays the windows out again for a frame, because the zone the column
//! carves goes and comes with it.
//!
//! # The column never narrows the footer
//!
//! A layer surface's exclusive zone is carved out of what the surfaces laid
//! out BEFORE it left, in the order they were created. The footer is created
//! first, so the column ends where the footer's area begins — the modeline,
//! the minibuffer and the notification band keep the whole width. A footer
//! that starts again after a column would be laid out after it; that is why
//! the footer, on its start, raises every column again
//! (`right_column::raise_again`).

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
};

use anyhow::{Context as _, Result, bail};
use ronoctua_compositor::{Event, Workspace};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::FrameCallbackData,
    reexports::calloop::{channel, ping},
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface},
    },
};
use wayland_client::{
    QueueHandle,
    protocol::{wl_output, wl_surface},
};

use crate::door::{Canvas, Closed, Content, Door, Hand, LEFT_BUTTON, Shell, Ui, Wiring};

/// The columns' layer-shell namespace, so a compositor rule can tell one
/// from the footer.
pub const NAMESPACE: &str = "ronoctua-column";

/// The margin inside a column, in cells — see `chrome::FIELDS_LEAD`.
pub const MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// The line the tenant's header is on: line 0 is the control line.
pub const HEADER_LINE: u16 = 1;

/// The keys every column teaches after its tenant's: its size, one step at a
/// time, and closing it. `C-` and not `M-`: `M-` is the compositor's family
/// and never reaches a window, and a bare arrow or letter is the tenant's own.
pub const OF_THE_COLUMN: &[(&str, &str)] = &[
    ("C-<left>", "column-wider"),
    ("C-<right>", "column-narrower"),
    ("C-q", "column-close"),
];

/// What a key of [`OF_THE_COLUMN`] does — EVERY ID NAMED, so a key added to
/// the table cannot fall into another's arm: with a catch-all, a third key
/// made the column narrower.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Own {
    /// One size wider.
    Wider,
    /// One size narrower.
    Narrower,
    /// The column closed.
    Close,
}

impl Own {
    /// The act an id of the table names — `None` for one that is not the
    /// column's, which a test refuses for every id of the table.
    fn of(id: &str) -> Option<Self> {
        match id {
            "column-wider" => Some(Self::Wider),
            "column-narrower" => Some(Self::Narrower),
            "column-close" => Some(Self::Close),
            _ => None,
        }
    }
}

/// A column's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// The widest — a second column of files, a larger picture.
    Large,
    /// The usual width; tenants open here.
    Medium,
    /// The same content, narrower.
    Small,
    /// A strip with a few controls of the tenant's own.
    Bar,
}

impl Size {
    /// Every size, from the widest — the order the control line draws them,
    /// the way the edge moves.
    pub const ALL: [Self; 4] = [Self::Large, Self::Medium, Self::Small, Self::Bar];

    /// Its name on the command line and in the slot's record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Large => "large",
            Self::Medium => "medium",
            Self::Small => "small",
            Self::Bar => "bar",
        }
    }

    /// The size by its name; `None` for a name that is not one — refused,
    /// never read as the default.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// How many CONTENT cells wide it is (`space.column`).
    #[must_use]
    pub const fn cells(self) -> u16 {
        match self {
            Self::Large => tokens::space::COLUMN_LARGE,
            Self::Medium => tokens::space::COLUMN_MEDIUM,
            Self::Small => tokens::space::COLUMN_SMALL,
            Self::Bar => tokens::space::COLUMN_BAR,
        }
    }

    /// What its glyph on the control line says under the pointer — the
    /// generated key, so a sentence missing from the `.ftl` does not build.
    #[must_use]
    pub const fn sentence(self) -> &'static str {
        match self {
            Self::Large => ronoctua_i18n::key::ABOUT_COLUMN_LARGE,
            Self::Medium => ronoctua_i18n::key::ABOUT_COLUMN_MEDIUM,
            Self::Small => ronoctua_i18n::key::ABOUT_COLUMN_SMALL,
            Self::Bar => ronoctua_i18n::key::ABOUT_COLUMN_BAR,
        }
    }

    /// Its glyph on the control line.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Large => tokens::icon::COLUMN_LARGE,
            Self::Medium => tokens::icon::COLUMN_MEDIUM,
            Self::Small => tokens::icon::COLUMN_SMALL,
            Self::Bar => tokens::icon::COLUMN_BAR,
        }
    }

    /// How many lines a picture takes at this size before the rows start:
    /// `space.column.pictures` at medium, and the same share of the width at
    /// the others — a picture keeps its shape, so a wider column is a taller
    /// picture. None in the bar.
    #[must_use]
    pub const fn picture_lines(self) -> u16 {
        tokens::space::COLUMN_PICTURES * self.cells() / tokens::space::COLUMN_MEDIUM
    }

    /// The next size up, if there is one. The bar has none: from the bar the
    /// column goes BACK, to the size it had ([`Sizing::wider`]).
    #[must_use]
    pub const fn wider(self) -> Option<Self> {
        match self {
            Self::Large | Self::Bar => None,
            Self::Medium => Some(Self::Large),
            Self::Small => Some(Self::Medium),
        }
    }

    /// The next size down, if there is one.
    #[must_use]
    pub const fn narrower(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => Some(Self::Bar),
            Self::Bar => None,
        }
    }
}

/// How a tenant fills a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Its usual content, laid out again at the width — the rows reflow.
    Adapted,
    /// Content of its own, made for this size — the player's bar.
    Own,
}

/// Whether a size can be chosen, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// It can.
    Yes,
    /// The tenant has nothing made for it.
    NotMade,
    /// The screen clamps it to the width of the size below it.
    NoRoom,
}

impl Offer {
    /// Why a size is not offered, as the key that says it on the tenant's
    /// line — a whole sentence, where there is the room for one.
    const fn refusal(self) -> Option<&'static str> {
        match self {
            Self::Yes => None,
            Self::NotMade => Some(ronoctua_i18n::key::COLUMN_BAR_NOT_OFFERED),
            Self::NoRoom => Some(ronoctua_i18n::key::COLUMN_NO_ROOM),
        }
    }

    /// The same, under the pointer on the control line: as short as the
    /// sizes' own sentences, because the room there is what a small column
    /// leaves beside its glyphs, and a refusal cut short loses its reason.
    const fn refusal_on_the_line(self) -> Option<&'static str> {
        match self {
            Self::Yes => None,
            Self::NotMade => Some(ronoctua_i18n::key::ABOUT_COLUMN_BAR_NOT_MADE),
            Self::NoRoom => Some(ronoctua_i18n::key::ABOUT_COLUMN_NO_ROOM),
        }
    }
}

/// A size's width in LOGICAL pixels at a cell `cell` px wide, on a screen
/// `output` logical pixels wide when that is known — see [`width_px`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn width_at(size: Size, cell: f32, output: Option<u32>) -> u32 {
    let natural = |s: Size| (cell * f32::from(s.cells() + 2 * MARGIN)).ceil() as u32;
    let most = output.map_or(u32::MAX, |w| {
        w.saturating_mul(u32::from(tokens::space::COLUMN_MAXPCT)) / 100
    });
    natural(size).min(most).max(natural(Size::Bar))
}

/// A size's width in LOGICAL pixels: its cells and the margin on both sides,
/// never more than `space.column.maxPct` of the screen — nothing leaks — and
/// never less than the bar, which is the least a column is.
#[must_use]
pub fn width_px(size: Size, output: Option<u32>) -> u32 {
    width_at(
        size,
        tokens::metrics::cell_width(tokens::text::size()),
        output,
    )
}

/// Whether `size` can be chosen for a tenant that `shaped` it, at a cell
/// `cell` px wide on a screen `output` wide.
fn offer_at(size: Size, shaped: bool, cell: f32, output: Option<u32>) -> Offer {
    if !shaped {
        return Offer::NotMade;
    }
    match size.narrower() {
        Some(below) if width_at(size, cell, output) <= width_at(below, cell, output) => {
            Offer::NoRoom
        }
        _ => Offer::Yes,
    }
}

/// The column's size, as a small machine: what was ASKED, what is DRAWN —
/// the size whose width the compositor last configured, so a tenant never
/// draws the bar's content at a medium width or the other way round — and the
/// size before the bar, which is where the bar goes back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizing {
    /// The size asked for.
    pub asked: Size,
    /// The size drawn.
    pub drawn: Size,
    /// The size the column had before the bar.
    pub before_bar: Size,
}

impl Sizing {
    /// A column opened at `size`.
    #[must_use]
    pub const fn at(size: Size) -> Self {
        Self {
            asked: size,
            drawn: size,
            // FROM A BAR IT WAS OPENED AT, back to the usual width.
            before_bar: match size {
                Size::Bar => Size::Medium,
                other => other,
            },
        }
    }

    /// Asks for `to`; whether that changed anything. The size it leaves is
    /// remembered when it is not the bar.
    pub fn ask(&mut self, to: Size) -> bool {
        if self.asked == to {
            return false;
        }
        if self.asked != Size::Bar {
            self.before_bar = self.asked;
        }
        self.asked = to;
        true
    }

    /// The first size up that is offered — from the bar, the size it had.
    /// `Err(Some(why))` names the first size that could not be taken, and
    /// `Err(None)` is the end of the ring, where nothing is said.
    ///
    /// # Errors
    ///
    /// When no wider size is offered.
    pub fn wider(&self, offer: impl Fn(Size) -> Offer) -> Result<Size, Option<Offer>> {
        if self.asked == Size::Bar {
            return Ok(self.before_bar);
        }
        walk(self.asked, offer, Size::wider)
    }

    /// The first size down that is offered.
    ///
    /// # Errors
    ///
    /// When no narrower size is offered.
    pub fn narrower(&self, offer: impl Fn(Size) -> Offer) -> Result<Size, Option<Offer>> {
        walk(self.asked, offer, Size::narrower)
    }
}

/// Walks from `from` by `next` to the first size offered, past those that are
/// not — a size the clamp made as narrow as its neighbour is not a step.
fn walk(
    from: Size,
    offer: impl Fn(Size) -> Offer,
    next: fn(Size) -> Option<Size>,
) -> Result<Size, Option<Offer>> {
    let mut at = from;
    let mut refused = None;
    while let Some(n) = next(at) {
        match offer(n) {
            Offer::Yes => return Ok(n),
            other => {
                refused.get_or_insert(other);
                at = n;
            }
        }
    }
    Err(refused)
}

/// What the tenant is handed to draw in: the size it is drawn at and where
/// its part of the surface begins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// The size it is drawn at.
    pub size: Size,
    /// The first line that is the tenant's — where its header goes.
    pub top: u16,
    /// The same, in logical pixels.
    pub top_px: f32,
    /// Whether this is a column, and not a window.
    column: bool,
}

impl Area {
    /// A WINDOW's area: the whole surface, the header on line 0.
    #[must_use]
    pub const fn whole() -> Self {
        Self {
            size: Size::Medium,
            top: 0,
            top_px: 0.0,
            column: false,
        }
    }

    /// A COLUMN's area at `size`, the tenant's part starting on line `top`,
    /// `top_px` down.
    #[must_use]
    pub const fn of_a_column(size: Size, top: u16, top_px: f32) -> Self {
        Self {
            size,
            top,
            top_px,
            column: true,
        }
    }

    /// The line past the tenant's last on a surface `lines` tall: all of them
    /// — but in a column's BAR the last line is the column's, where its close
    /// glyph is drawn from the bottom edge, and the tenant stops above it.
    #[must_use]
    pub const fn end(&self, lines: u16) -> u16 {
        if self.column && matches!(self.size, Size::Bar) {
            lines.saturating_sub(1)
        } else {
            lines
        }
    }

    /// How many lines a picture may take on a surface `lines` tall: half of
    /// it in a window, and in a column the size's own share, never past half.
    #[must_use]
    pub const fn picture_ceiling(&self, lines: u16) -> u16 {
        if !self.column {
            return lines / 2;
        }
        let own = self.size.picture_lines();
        if own < lines / 2 { own } else { lines / 2 }
    }

    /// The column's own keys, for the tenant's header to teach after its own
    /// — none in a window.
    #[must_use]
    pub fn teach(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        if self.column {
            crate::chrome::hint_of(OF_THE_COLUMN, texts)
        } else {
            Vec::new()
        }
    }
}

/// How a column is opened: `[<tenant>] [--on <workspace>] [--size <size>]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// Who it is — the player when none is named.
    pub tenant: crate::right_column::Tenant,
    /// The workspace it belongs to, by name — the focused one when none is.
    pub on: Option<String>,
    /// The size it opens at — the tenant's own when none is.
    pub size: Option<Size>,
}

impl Opening {
    /// The words after `column`. An unknown tenant, an unknown size, a flag
    /// with nothing after it or a word nobody asked for is REFUSED, never
    /// read as the default: a typo would open a column other than the one
    /// meant.
    ///
    /// # Errors
    ///
    /// With a sentence saying which word was not understood.
    pub fn parse<S: AsRef<str>>(words: &[S]) -> Result<Self> {
        let mut tenant = None;
        let mut on = None;
        let mut size = None;
        let mut words = words.iter().map(AsRef::<str>::as_ref);
        while let Some(word) = words.next() {
            match word {
                "--on" => {
                    let name = words
                        .next()
                        .filter(|w| !w.is_empty())
                        .context("`--on` needs the name of a workspace")?;
                    on = Some(name.to_owned());
                }
                "--size" => {
                    let said = words.next().context("`--size` needs a size")?;
                    size =
                        Some(Size::named(said).with_context(|| format!("`{said}` is not a size"))?);
                }
                other if tenant.is_none() && !other.starts_with('-') => {
                    tenant = Some(
                        crate::right_column::Tenant::named(other)
                            .with_context(|| format!("`{other}` is not a column"))?,
                    );
                }
                other => bail!("`{other}` is not understood here"),
            }
        }
        Ok(Self {
            tenant: tenant.unwrap_or(crate::right_column::Tenant::Player),
            on,
            size,
        })
    }

    /// The words that open it — `column` first, [`Opening::parse`]'s other
    /// half.
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        let mut out = vec!["column".to_owned(), self.tenant.name().to_owned()];
        if let Some(on) = &self.on {
            out.extend(["--on".to_owned(), on.clone()]);
        }
        if let Some(size) = self.size {
            out.extend(["--size".to_owned(), size.name().to_owned()]);
        }
        out
    }
}

/// The screen workspace `name` wants its column on: the one that shows it —
/// or none, when no screen does, or it is gone.
#[must_use]
pub fn wanted(name: &str, all: &[Workspace]) -> Option<String> {
    all.iter()
        .find(|w| w.name == name)
        .filter(|w| w.visible)
        .and_then(|w| w.output.clone())
}

/// What a column does about where it is and where it is wanted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing: it is where it is wanted, or off screen and not wanted on one.
    Stay,
    /// It is off screen, and wanted on this one.
    Show(String),
    /// It is on a screen, and wanted on this other one.
    Move(String),
    /// It is on a screen, and wanted on none.
    Park,
}

/// The step from being `shown_on` a screen to being `wanted` on one.
#[must_use]
pub fn step(shown_on: Option<&str>, wanted: Option<&str>) -> Step {
    match (shown_on, wanted) {
        (Some(here), Some(there)) if here == there => Step::Stay,
        (Some(_), Some(there)) => Step::Move(there.to_owned()),
        (None, Some(there)) => Step::Show(there.to_owned()),
        (Some(_), None) => Step::Park,
        (None, None) => Step::Stay,
    }
}

/// What a column is, to the tenant in it — a window's `door::Content`, less
/// what the column does for it, and every drawing method handed the [`Area`].
///
/// Everything past what a tenant must write has a default.
pub trait Tenant: Sized + 'static {
    /// What the tenant keeps of its own for ONE pointer.
    type Hand: Default;

    /// Who it is in the slot.
    const WHO: crate::right_column::Tenant;

    /// The `app_id`.
    fn app_id(&self) -> &'static str;

    /// Its title.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String;

    /// The size it opens at.
    fn default_size(&self) -> Size {
        Size::Medium
    }

    /// How it fills `size`, or `None` when it offers nothing there — the
    /// glyph is drawn inert and says so. The three widths adapt by default,
    /// and the bar, which needs content of its own, is not offered.
    fn shape(&self, size: Size) -> Option<Shape> {
        (size != Size::Bar).then_some(Shape::Adapted)
    }

    /// Draws one frame in `area`.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>, area: &Area);

    /// One pointer's frame, of the events that are the tenant's.
    fn pointer(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<Self::Hand>,
        events: &[PointerEvent],
        area: &Area,
    );

    /// One key the column did not take.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers, area: &Area);

    /// See `Content::answered`.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let _ = (ui, id, text);
    }

    /// See `Content::unanswered`.
    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        let _ = (ui, id);
    }

    /// See `Content::pasted`.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let _ = (ui, text);
    }

    /// See `Content::types_capitals` — whether a text is being typed where
    /// Caps Lock means capitals. Not, by default.
    fn types_capitals(&self) -> bool {
        false
    }

    /// See `Content::released`.
    fn released(&mut self, ui: &mut Ui, event: &KeyEvent) {
        let _ = (ui, event);
    }

    /// See `Content::text_area` — the whole width under the header, by
    /// default.
    fn text_area(
        &self,
        ui: &Ui,
        line: u16,
        column: u16,
        area: &Area,
    ) -> Option<crate::text_select::Area> {
        let _ = (column, area);
        let top = ui.first_of_list();
        (line >= top).then(|| crate::text_select::Area {
            left: 0,
            right: ui.columns + 2 * ui.margin,
            top,
            bottom: ui.lines,
        })
    }

    /// See `Content::keyboard_entered`.
    fn keyboard_entered(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// See `Content::keyboard_left`.
    fn keyboard_left(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// See `Content::each_turn`.
    fn each_turn(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// See `Content::frame_done`.
    fn frame_done(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// See `Content::resized`.
    fn resized(&mut self, ui: &mut Ui, area: &Area) {
        let _ = (ui, area);
    }

    /// See `Content::language_changed`.
    fn language_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// See `Content::locale_changed`.
    fn locale_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The column is closing — its close glyph, or `C-q` — and the door
    /// quits after this: what the tenant holds for the screen (a picture's
    /// thread) is let go now. Only the COLUMN closes: what the tenant plays
    /// or keeps outside it goes on.
    fn closing(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The column went off the screen: the heavy work (a picture's thread) is
    /// the tenant's to stop until it is shown again.
    fn parked(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The column is on a screen again, on a surface that is new — what was
    /// on the old one's GPU is not on this one's.
    fn shown(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// The column is on the screen `on` (a connector), or on none while it
    /// is parked — said each time that moves, BEFORE [`Tenant::shown`], and
    /// once for the screen a column is BORN on, at its first turn and before
    /// its first frame. The column knows its screen from the compositor (its
    /// workspace's), which is ahead of the surface's own `enter`: what is
    /// routed to one screen is chosen by this.
    fn placed_on(&mut self, ui: &mut Ui, on: Option<&str>) {
        let _ = (ui, on);
    }

    /// See `Content::screens_changed` — a screen described or gone, or the
    /// surface on another.
    fn screens_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
    }

    /// Whether `key` does something the bar can show — a key that acts on a
    /// row nobody can see puts the column back to its size instead, and does
    /// nothing else. Every key acts, by default.
    fn acts_without_rows(&self, key: ronoctua_keymap::Key) -> bool {
        let _ = key;
        true
    }

    /// Whether the tenant holds a QUESTION OF ITS OWN on its rows, waiting
    /// for an answer — "no" first, drawn where the bar draws nothing: the
    /// column goes back to its size while one waits, as it does for the
    /// door's own line. None, by default.
    fn waits_for_an_answer(&self) -> bool {
        false
    }
}

/// A glyph of the control line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// A size.
    Size(Size),
    /// The bar's one glyph on line 0: back to the size before it.
    Widen,
    /// The column closed — left of the sizes, and on the bar's last line.
    Close,
}

/// The glyphs under the pointers, ONE PER POINTER (by its seat's colour
/// number), in the order they came to them.
///
/// Every cursor has its own hover (`door::handlers`, `crate::seats`): kept as
/// one glyph for all of them, a second cursor crossing the tenant's rows put
/// out the glyph the first one rested on, and its sentence with it — every
/// frame it moved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Under(Vec<(u16, Glyph)>);

impl Under {
    /// Pointer `who` is on `glyph` now, or on none. One that says where it
    /// is goes LAST, and the last one's sentence is the one said: the
    /// pointer that moved is the one being looked at.
    fn at(&mut self, who: u16, glyph: Option<Glyph>) {
        self.0.retain(|(w, _)| *w != who);
        if let Some(g) = glyph {
            self.0.push((who, g));
        }
    }

    /// Whether some pointer is on `glyph` — which lights it.
    fn lit(&self, glyph: Glyph) -> bool {
        self.0.iter().any(|(_, g)| *g == glyph)
    }

    /// The glyph whose sentence is said: the last pointer's.
    fn said(&self) -> Option<Glyph> {
        self.0.last().map(|(_, g)| *g)
    }
}

/// Where a glyph of the column was drawn: its first cell, counted from the
/// left as every `Span` is, and whether it is on line 0 or on the LAST line —
/// the bar's close, drawn from the bottom edge (`Span::foot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    /// Its first cell.
    at: u16,
    /// On the last line, from the bottom edge, and not on line 0.
    foot: bool,
    /// Which.
    glyph: Glyph,
}

/// The control line, the sentence under the pointer, the rule and the focus
/// line, as one frame lays them out — the painter draws `spans` and `over`,
/// and the pointer reads `placed`.
#[derive(Debug, Clone)]
struct Decoration {
    spans: Vec<ronoctua_render::Span>,
    over: Vec<ronoctua_render::quad::Quad>,
    placed: Vec<Placed>,
}

/// The two cells a glyph takes on the control line: the glyph, and the air
/// after it, which is part of what the pointer takes.
const GLYPH_CELLS: u16 = 2;

/// Where the close glyph goes, from the first size's cell: its own two cells
/// and the ones the rule stands in (`space.column.ruleCells`) — so the rule is
/// a cell and a half from each glyph's middle before the ink moves it.
const fn close_at(first: u16) -> Option<u16> {
    first.checked_sub(GLYPH_CELLS + tokens::space::COLUMN_RULECELLS)
}

/// Where a chrome glyph drawn on the cell whose left edge is `left_px` has
/// INK, in logical pixels: `(first, last)`. Its run is centred on the cell
/// (`Size::OnTheCell`, the renderer's nudge), and the ink is where the
/// generator measured it inside the drawing (`tokens::icon::ink_across`) —
/// the whole advance for a glyph it has no measure of.
fn ink_of(glyph: char, left_px: f32) -> (f32, f32) {
    // THE RUN AS THE PAINTER MAKES IT — centred on its cell at the measured
    // size — and the size read back from it, so the two cannot differ.
    let run = ronoctua_render::Size::OnTheCell(crate::surface::chrome_size_of(glyph));
    let px = run.px().unwrap_or_else(tokens::text::size);
    let origin = left_px + run.nudge(tokens::text::size());
    let (from, to) = tokens::icon::ink_across(glyph)
        .unwrap_or((0.0, tokens::metrics::cell_width(px) / px.max(1.0)));
    (from.mul_add(px, origin), to.mul_add(px, origin))
}

/// The rule between the close glyph at cell `close` and the widest size at
/// `first`, both counted from the border of a surface `f` describes: WHERE
/// THE AIR ON BOTH SIDES IS EQUAL, from the two glyphs' measured ink, and as
/// tall as the control line less `space.column.ruleInset` at each end.
fn close_rule(f: crate::chrome::Frame, close: u16, first: u16) -> ronoctua_render::quad::Quad {
    let columns = f.columns();
    let (_, close_ends) = ink_of(
        tokens::icon::COLUMN_CLOSE,
        crate::chrome::edge_x(f.width, columns, close),
    );
    let (large_begins, _) = ink_of(
        Size::Large.glyph(),
        crate::chrome::edge_x(f.width, columns, first),
    );
    let thick = f32::from(tokens::space::RULE_THICKNESS);
    let inset = f32::from(tokens::space::COLUMN_RULEINSET);
    crate::chrome::divider(
        f32::midpoint(close_ends, large_begins) - thick / 2.0,
        inset,
        f.line() - inset,
    )
}

/// What glyph `g` says under the pointer on the control line, as a key —
/// its size, or why that size is not offered (`offer_of`), or the close; the
/// bar's widen says nothing (the owner's decision).
fn sentence_on_the_line(g: Glyph, offer_of: impl Fn(Size) -> Offer) -> Option<&'static str> {
    match g {
        Glyph::Size(size) => Some(
            offer_of(size)
                .refusal_on_the_line()
                .unwrap_or_else(|| size.sentence()),
        ),
        Glyph::Close => Some(ronoctua_i18n::key::ABOUT_COLUMN_CLOSE_GLYPH),
        Glyph::Widen => None,
    }
}

/// Lays out the column's own glyphs on a surface `f` describes, drawn at
/// `drawn`: every one under a pointer lit, what the last of them says, the
/// rule, and — with the keyboard on it — the focus line.
///
/// PURE, and the one answer: the painter and [`glyph_at`] both read what it
/// placed, so a glyph is taken where it was drawn.
#[allow(clippy::too_many_lines)]
fn decoration(
    drawn: Size,
    offers: &[(Size, Offer)],
    f: crate::chrome::Frame,
    under: &Under,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Decoration {
    let glyph_span = |g: char, column: u16, line: u16, color: tokens::Color, edge: bool| {
        ronoctua_render::Span {
            size: ronoctua_render::Size::OnTheCell(crate::surface::chrome_size_of(g)),
            text: g.to_string(),
            column,
            line,
            color,
            edge,
            // THE LAST LINE IS COUNTED FROM THE BOTTOM EDGE, so nothing of
            // the grid's remainder sits under the bar's close.
            foot: line > 0,
        }
    };
    let quiet = |g: Glyph| {
        if under.lit(g) {
            tokens::color::TEXT_SUBTLE_LIT_KEY
        } else {
            tokens::color::TEXT_SUBTLE_DEFAULT
        }
    };
    let columns = f.columns();
    let mut spans = Vec::new();
    let mut placed = Vec::new();
    let mut over = Vec::new();
    if drawn == Size::Bar {
        // THE BAR'S TWO GLYPHS, each on its middle cell: widen on line 0 and
        // close on the last line — the bar is five cells, odd so each sits in
        // the middle. No sentence (the owner's decision).
        let at = MARGIN + 1;
        spans.push(glyph_span(
            tokens::icon::COLUMN_WIDEN,
            at,
            0,
            quiet(Glyph::Widen),
            false,
        ));
        placed.push(Placed {
            at,
            foot: false,
            glyph: Glyph::Widen,
        });
        let last = f.lines().saturating_sub(1);
        if last > 0 {
            spans.push(glyph_span(
                tokens::icon::COLUMN_CLOSE,
                at,
                last,
                quiet(Glyph::Close),
                false,
            ));
            placed.push(Placed {
                at,
                foot: true,
                glyph: Glyph::Close,
            });
        }
    } else {
        // FOUR GLYPHS COUNTED FROM THE BORDER, from the widest: two cells
        // each, the modeline's air after the last.
        let first = crate::chrome::from_the_edge(columns, GLYPH_CELLS * 4);
        let offer_of = |s: Size| {
            offers
                .iter()
                .find(|(o, _)| *o == s)
                .map_or(Offer::NotMade, |(_, o)| *o)
        };
        for (k, size) in (0u16..).zip(Size::ALL) {
            let at = first + GLYPH_CELLS * k;
            // THE CURRENT ONE IS BRIGHTER, never the accent: on a column the
            // accent is the focus line's, and a size in it would read as that.
            let color = if size == drawn {
                tokens::color::TEXT_PRIMARY
            } else if offer_of(size) != Offer::Yes {
                tokens::color::TEXT_SUBTLE_INERT
            } else {
                quiet(Glyph::Size(size))
            };
            spans.push(glyph_span(size.glyph(), at, 0, color, true));
            placed.push(Placed {
                at,
                foot: false,
                glyph: Glyph::Size(size),
            });
        }
        // THE CLOSE, left of the sizes and apart from them by a rule — only
        // where the line has room for it past the margin.
        let close = close_at(first).filter(|c| *c >= MARGIN);
        if let Some(close) = close {
            spans.push(glyph_span(
                tokens::icon::COLUMN_CLOSE,
                close,
                0,
                quiet(Glyph::Close),
                true,
            ));
            placed.push(Placed {
                at: close,
                foot: false,
                glyph: Glyph::Close,
            });
            over.push(close_rule(f, close, first));
        }
        // WHAT THE GLYPH UNDER THE POINTER DOES, set against the close from
        // the right with a cell of air — and at rest, nothing: a line that is
        // quiet until it is asked.
        let said = under
            .said()
            .and_then(|g| sentence_on_the_line(g, offer_of))
            .map(|k| texts.t(k));
        let end = close
            .unwrap_or(first)
            .saturating_sub(tokens::space::COLUMN_SAIDAIR);
        if let Some(said) = said
            && end > MARGIN
        {
            let text = ronoctua_render::layout::elide_end(&said, end - MARGIN);
            let cells = ronoctua_protocol::cells(&text);
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(tokens::text::size()),
                column: end.saturating_sub(cells),
                text,
                line: 0,
                color: crate::chrome::Register::Explain.color(),
                edge: true,
                foot: false,
            });
        }
    }
    // THE FOCUS LINE, over everything and in no line's room.
    if focused {
        over.push(crate::chrome::focus_line(f.width));
    }
    Decoration {
        spans,
        over,
        placed,
    }
}

/// The glyph a pointer at `(x, y)` is on, from what was PLACED — on line 0
/// counted from the border as it was drawn (`chrome::at_the_edge`), and in
/// the bar each of its two lines whole, which are their glyphs': line 0 and
/// the last, counted from the bottom edge as it was drawn.
fn glyph_at(
    placed: &[Placed],
    drawn: Size,
    f: crate::chrome::Frame,
    x: f64,
    y: f64,
) -> Option<Glyph> {
    let line = f64::from(f.line());
    let height = f64::from(f.height);
    let on_top = (0.0..line).contains(&y);
    let on_foot = f.lines() > 1 && (height - line..height).contains(&y);
    if drawn == Size::Bar {
        return placed
            .iter()
            .find(|p| if p.foot { on_foot } else { on_top })
            .map(|p| p.glyph);
    }
    if !on_top {
        return None;
    }
    let c = crate::chrome::at_the_edge(x, f64::from(f.width), f.columns())?;
    placed
        .iter()
        .find(|p| !p.foot && c >= p.at && c < p.at + GLYPH_CELLS)
        .map(|p| p.glyph)
}

/// Every screen's logical width, by connector — what the clamp is taken of.
fn widths_of(
    outputs: std::collections::BTreeMap<String, ronoctua_compositor::Output>,
) -> BTreeMap<String, u32> {
    outputs
        .into_iter()
        .filter_map(|(name, o)| o.logical.map(|l| (name, l.width)))
        .collect()
}

/// The width the clamp is taken of on the screen `on`: its logical width —
/// or, where the screen resizer cuts that screen, the room its curtains
/// leave (`resizer::room_width`).
///
/// THE ROOM, because the curtains change no output: nothing tells the column
/// the screen got narrower, and a share of the monitor took most of a narrow
/// room — past the room's own width, the column hung off its left edge and
/// left the windows no room at all. On the room, a column runs the
/// arithmetic it runs on a real screen that size.
fn room_of(
    widths: &BTreeMap<String, u32>,
    on: &str,
    cut: Option<&crate::resizer::State>,
) -> Option<u32> {
    let whole = *widths.get(on)?;
    Some(crate::resizer::room_width(whole, Some(on), cut))
}

/// A column's surface, `width` logical pixels wide, on `output` — or, with
/// none named, on the screen the compositor has focused, which is where a
/// column opened from a key belongs.
fn column_on<T: Tenant>(
    layer_shell: &LayerShell,
    surface: wl_surface::WlSurface,
    qh: &QueueHandle<Door<Column<T>>>,
    output: Option<&wl_output::WlOutput>,
    width: u32,
) -> LayerSurface {
    let layer = layer_shell.create_layer_surface(
        qh,
        surface,
        // `Top`, the footer's layer: above the windows, below a lock.
        Layer::Top,
        Some(NAMESPACE),
        output,
    );
    layer.set_anchor(Anchor::TOP | Anchor::RIGHT | Anchor::BOTTOM);
    layer.set_size(width, 0);
    layer.set_exclusive_zone(i32::try_from(width).unwrap_or(i32::MAX));
    // ON DEMAND, the lifeline's reasoning: a click gives it the keys, and
    // opening it takes them from nobody.
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.commit();
    layer
}

/// A right column: the base, and the tenant in it.
pub struct Column<T: Tenant> {
    /// The tenant.
    pub tenant: T,
    /// Its size.
    sizing: Sizing,
    /// The workspace it belongs to, by name — `None` when no compositor
    /// answered: then it is shown where it was born and never parks.
    workspace: Option<String>,
    /// The workspaces, as the compositor last said them.
    workspaces: Vec<Workspace>,
    /// Whether it is born on a screen, and not parked.
    born_shown: bool,
    /// The screen it is shown on, by connector — `None` while parked, and
    /// while no compositor named one.
    shown_on: Option<String>,
    /// Whether the tenant is still owed the screen the column was BORN on.
    /// The constructor has no `Ui` to tell it with, and the first `place`
    /// finds the column already where it is wanted (`Step::Stay`), so it is
    /// told at the first turn ([`Column::tell_where_it_was_born`]).
    placement_owed: bool,
    /// Every screen's logical width, for the clamp.
    widths: BTreeMap<String, u32>,
    /// The screen resizer's cut, while it is on: the clamp is taken of the
    /// room it leaves ([`room_of`]).
    resizer: Option<crate::resizer::State>,
    /// The watch on the resizer's file, which is its broadcast.
    _resizer_watch: Option<ronoctua_files::notice::Notice>,
    /// The layer surface, kept to tell it its width: a CLONE of what the
    /// door holds, dropped before the door lets go of its own.
    surf: RefCell<Option<LayerSurface>>,
    /// The width the column last told the compositor, so a size of text that
    /// moved tells it again and a configure does not.
    told_width: Cell<u32>,
    /// The keyboards on the column (their seats' colour numbers): the focus
    /// line is drawn while there is any.
    holders: BTreeSet<u16>,
    /// The pointers whose press went to the tenant — a drag of its own, which
    /// goes on over line 0 until the button comes up.
    tenant_holds: BTreeSet<u16>,
    /// The column's glyphs as last drawn.
    placed: Vec<Placed>,
    /// The glyphs under the pointers, each pointer's apart.
    under: Under,
    /// The first frame on a new surface is owed a frame callback, whose
    /// answer means the commit that mapped it was shown — see
    /// [`Column::settle_the_windows`].
    settle_owed: bool,
    /// That callback was asked, and not yet answered.
    settle_asked: bool,
    /// The compositor, through the contract.
    wm: Option<Box<dyn ronoctua_compositor::Compositor>>,
    /// The queue, for the frame callback and a new surface.
    qh: QueueHandle<Door<Self>>,
    /// Rung to place the column on the next turn — once the door exists, or
    /// once a screen is described.
    ping: ping::Ping,
    /// Whether it holds a slot among the columns (`crate::right_column`) —
    /// every column on a screen does; a photograph's holds none, and never
    /// reads the one this machine's own column holds.
    slot: bool,
}

impl<T: Tenant> Drop for Column<T> {
    fn drop(&mut self) {
        if self.slot {
            crate::right_column::leave(T::WHO);
        }
    }
}

/// Opens `opening`'s column with the tenant `make` builds, and runs it until
/// it closes.
///
/// # Errors
///
/// No Wayland compositor, no layer shell, or a tenant that could not be
/// built.
pub fn run<T: Tenant>(
    opening: &Opening,
    make: impl FnOnce(&Wiring<'_, Column<T>>) -> Result<T>,
) -> Result<()> {
    crate::door::run(|w: &Wiring<'_, Column<T>>| {
        let tenant = make(w)?;
        // WHERE IT BELONGS, asked now: the first event may be minutes away.
        let mut wm = crate::daemon::open_the_compositor().ok();
        let workspaces = match wm.as_mut().map(|c| c.workspaces()) {
            Some(Ok(all)) => Some(all),
            Some(Err(e)) => {
                tracing::warn!(error = %e, "the compositor named no workspace");
                None
            }
            None => None,
        };
        let widths = wm
            .as_mut()
            .and_then(|c| c.outputs().ok())
            .map(widths_of)
            .unwrap_or_default();
        let all = workspaces.clone().unwrap_or_default();
        let focused = all.iter().find(|w| w.focused).map(|w| w.name.clone());
        let workspace = workspaces
            .as_ref()
            .and_then(|_| opening.on.clone().or_else(|| focused.clone()));
        if workspace.is_none() {
            tracing::warn!(
                "no workspace to belong to: the column stays on the screen it is born on"
            );
        }
        // BORN ON A SCREEN when its workspace is the one in front — made with
        // no screen named, which is the focused one — and parked otherwise,
        // placed on the first turn.
        let here = workspace.as_deref().and_then(|n| wanted(n, &all));
        let born_shown = workspace.is_none() || (workspace == focused && here.is_some());

        // THE COMPOSITOR'S EVENTS, through the contract: the workspaces
        // moving, and the screens.
        let (sender, events) = channel::channel::<Event>();
        crate::daemon::listen_to_the_compositor(sender);
        w.handle
            .insert_source(events, |event, (), d: &mut Door<Column<T>>| {
                if let channel::Event::Msg(e) = event {
                    Column::heard(d, e);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the compositor channel: {e}"))?;
        // THE SCREEN RESIZER'S CUT, read now and watched: it changes no
        // output, so no event of the compositor says the room moved.
        let resizer = crate::resizer::is_on();
        let (resizer_told, resizer_heard) = channel::channel::<()>();
        let resizer_watch = crate::resizer::watch_target().and_then(|(dir, name)| {
            ronoctua_files::notice::Notice::on_file(
                &dir,
                &name,
                ronoctua_files::notice::Pace::Coalesced,
                move || resizer_told.send(()).is_ok(),
            )
            .ok()
        });
        w.handle
            .insert_source(resizer_heard, |event, (), d: &mut Door<Column<T>>| {
                if let channel::Event::Msg(()) = event {
                    Column::resizer_changed(d);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the resizer's watch: {e}"))?;
        let (ping, pinged) = ping::make_ping().context("a ping for the column")?;
        w.handle
            .insert_source(pinged, |(), (), d: &mut Door<Column<T>>| Column::place(d))
            .map_err(|e| anyhow::anyhow!("could not hang the column's ping: {e}"))?;

        let size = opening
            .size
            .filter(|s| tenant.shape(*s).is_some())
            .unwrap_or_else(|| tenant.default_size());
        Ok(Column {
            tenant,
            sizing: Sizing::at(size),
            workspace,
            workspaces: all,
            born_shown,
            placement_owed: born_shown && here.is_some(),
            shown_on: if born_shown { here } else { None },
            widths,
            resizer,
            _resizer_watch: resizer_watch,
            surf: RefCell::new(None),
            told_width: Cell::new(0),
            holders: BTreeSet::new(),
            tenant_holds: BTreeSet::new(),
            placed: Vec::new(),
            under: Under::default(),
            settle_owed: born_shown,
            settle_asked: false,
            wm,
            qh: w.qh.clone(),
            ping,
            slot: true,
        })
    })
}

impl<T: Tenant> Column<T> {
    /// A column with NO COMPOSITOR BEHIND IT, drawn at `size` on the
    /// connection of `ui` — what `crate::photograph` draws a tenant in.
    ///
    /// It belongs to no workspace and takes no slot, and what it would tell
    /// the compositor goes nowhere. It is on a screen `screen` logical
    /// pixels wide, which clamps it and refuses a size that leaves the
    /// windows no room, as a real one does — or on none, and then nothing
    /// clamps it. Everything it DRAWS is the column's own — the control
    /// line, the tenant's area, the bar.
    ///
    /// # Errors
    ///
    /// When no ping could be made for it.
    pub fn detached(tenant: T, size: Size, ui: &Ui, screen: Option<u32>) -> Result<Self> {
        /// The one screen's name: a photograph's has no connector.
        const SCREEN: &str = "photograph";
        let queue = ui.conn.new_event_queue::<Door<Self>>();
        let (ping, _) = ping::make_ping().context("a ping for the column")?;
        Ok(Self {
            tenant,
            sizing: Sizing::at(size),
            workspace: None,
            workspaces: Vec::new(),
            born_shown: true,
            // A PHOTOGRAPH'S SCREEN HAS NO CONNECTOR: the tenant is told
            // nothing, which counts as every screen — what a photograph of
            // it should show.
            placement_owed: false,
            shown_on: screen.map(|_| SCREEN.to_owned()),
            widths: screen.map(|w| (SCREEN.to_owned(), w)).into_iter().collect(),
            resizer: None,
            _resizer_watch: None,
            surf: RefCell::new(None),
            told_width: Cell::new(0),
            holders: BTreeSet::new(),
            tenant_holds: BTreeSet::new(),
            placed: Vec::new(),
            under: Under::default(),
            settle_owed: false,
            settle_asked: false,
            wm: None,
            qh: queue.handle(),
            ping,
            slot: false,
        })
    }

    /// The size it is drawn at — for a test of a photograph's walk.
    #[cfg(test)]
    #[must_use]
    pub const fn drawn(&self) -> Size {
        self.sizing.drawn
    }

    /// The width of the size it is drawn at, on the screen it is on — what
    /// the compositor configures it at.
    #[must_use]
    pub fn drawn_width(&self) -> u32 {
        width_px(self.sizing.drawn, self.output_width())
    }

    /// What the tenant is handed.
    fn area(&self, ui: &Ui) -> Area {
        Area::of_a_column(
            self.sizing.drawn,
            HEADER_LINE,
            ui.frame(0).y_of(HEADER_LINE),
        )
    }

    /// The logical width of the screen it is on, when known — the room,
    /// where the resizer cuts that screen ([`room_of`]).
    fn output_width(&self) -> Option<u32> {
        room_of(
            &self.widths,
            self.shown_on.as_deref()?,
            self.resizer.as_ref(),
        )
    }

    /// Whether `size` can be chosen now.
    fn offer(&self, size: Size) -> Offer {
        offer_at(
            size,
            self.tenant.shape(size).is_some(),
            tokens::metrics::cell_width(tokens::text::size()),
            self.output_width(),
        )
    }

    /// The compositor, opened again when the last connection failed.
    fn compositor(&mut self) -> Option<&mut Box<dyn ronoctua_compositor::Compositor>> {
        if self.wm.is_none() {
            self.wm = crate::daemon::open_the_compositor().ok();
        }
        self.wm.as_mut()
    }

    /// Tells the compositor the width of the size asked, when it moved — a
    /// size chosen, the text grown, the screen turned. Only while a surface
    /// is up: a parked column is told on the screen it is shown on.
    fn tell_width(&self) {
        let w = width_px(self.sizing.asked, self.output_width());
        if w == self.told_width.get() {
            return;
        }
        if let Some(l) = &*self.surf.borrow() {
            l.set_size(w, 0);
            l.set_exclusive_zone(i32::try_from(w).unwrap_or(i32::MAX));
            l.commit();
            self.told_width.set(w);
        }
    }

    /// The size DRAWN is now `to`. The bar draws no header, so the one the
    /// last frame drew is taken off the surface — its marks and its chevron
    /// would still answer a click on the bar's glyphs.
    fn drawn_at(&mut self, ui: &mut Ui, to: Size) {
        if self.sizing.drawn == to {
            return;
        }
        self.sizing.drawn = to;
        if to == Size::Bar {
            ui.hinted.withdraw();
        }
        ui.ask_paint();
    }

    /// Asks for size `to`: the width is told, the record says it, and the
    /// size is DRAWN once a configure at its width arrives — or at once when
    /// the width did not move, as when the clamp makes two sizes one width.
    fn set_size(&mut self, ui: &mut Ui, to: Size) {
        if !self.sizing.ask(to) {
            return;
        }
        let before = self.told_width.get();
        self.tell_width();
        if self.told_width.get() == before || self.surf.borrow().is_none() {
            self.drawn_at(ui, to);
        }
        if self.slot {
            crate::right_column::resize(T::WHO, to);
        }
        tracing::info!(size = to.name(), "the column's size");
        ui.ask_paint();
    }

    /// One size wider — from the bar, the size it had.
    fn wider(&mut self, ui: &mut Ui) {
        match self.sizing.wider(|s| self.offer(s)) {
            Ok(to) => self.set_size(ui, to),
            Err(why) => self.refused(ui, why),
        }
    }

    /// One size narrower, down to the bar.
    fn narrower(&mut self, ui: &mut Ui) {
        match self.sizing.narrower(|s| self.offer(s)) {
            Ok(to) => self.set_size(ui, to),
            Err(why) => self.refused(ui, why),
        }
    }

    /// Back from the bar to the size it had.
    fn restore(&mut self, ui: &mut Ui) {
        if self.sizing.asked == Size::Bar {
            self.set_size(ui, self.sizing.before_bar);
        }
    }

    /// A size that could not be taken says why, on the tenant's line — and
    /// the end of the ring says nothing.
    fn refused(&self, ui: &mut Ui, why: Option<Offer>) {
        if let Some(key) = why.and_then(Offer::refusal) {
            ui.say(ui.texts.t(key));
            ui.ask_paint();
        }
    }

    /// Closes the column: the tenant is told first — the player lets its
    /// pictures go — and the door quits; `Drop` gives the slot up. Only the
    /// column goes: what plays in it goes on playing.
    fn close(&mut self, ui: &mut Ui) {
        self.tenant.closing(ui);
        ui.quit = true;
        tracing::info!("the column is closed");
    }

    /// A click on a glyph of the column.
    fn pressed_glyph(&mut self, ui: &mut Ui, g: Glyph) {
        match g {
            Glyph::Widen => self.restore(ui),
            Glyph::Close => self.close(ui),
            Glyph::Size(s) => match self.offer(s) {
                Offer::Yes => self.set_size(ui, s),
                // AN INERT GLYPH IS NOT A CONTROL: its sentence is already on
                // the line, under the pointer that pressed it.
                Offer::NotMade | Offer::NoRoom => {}
            },
        }
    }

    /// Everything that belonged to the surface that went: the keyboards on
    /// it, the pointers, the glyphs under them, and a settle it was owed.
    /// The screen the column is shown on is now `on` — `None` parked — and
    /// the tenant is told when that moved (`Tenant::placed_on`): after birth
    /// (the constructor, or `detached` for a photograph) the ONE place
    /// `shown_on` changes, so a tenant cannot miss a move — and the one place
    /// it is told, the screen it was born on included.
    fn now_on(&mut self, ui: &mut Ui, on: Option<&str>) {
        if self.shown_on.as_deref() == on && !self.placement_owed {
            return;
        }
        self.placement_owed = false;
        self.shown_on = on.map(ToOwned::to_owned);
        self.tenant.placed_on(ui, on);
    }

    /// A column born on a screen tells its tenant which, once: at the first
    /// turn, and at the first frame should that come first.
    fn tell_where_it_was_born(&mut self, ui: &mut Ui) {
        if self.placement_owed {
            let on = self.shown_on.clone();
            self.now_on(ui, on.as_deref());
        }
    }

    fn forget_the_surface(&mut self) {
        self.surf.replace(None);
        self.holders.clear();
        self.tenant_holds.clear();
        self.under = Under::default();
        self.placed.clear();
        self.settle_owed = false;
        self.settle_asked = false;
    }

    /// What the compositor said.
    fn heard(d: &mut Door<Self>, e: Event) {
        match e {
            Event::Workspaces(all) => {
                d.content.workspaces = all;
                Self::place(d);
            }
            // A SCREEN PLUGGED, TAKEN AWAY OR TURNED: every width again, the
            // clamp with them, and where the column goes.
            Event::ScreensChanged => {
                let outputs = d.content.compositor().map(|c| c.outputs());
                match outputs {
                    Some(Ok(o)) => d.content.widths = widths_of(o),
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "the screens could not be read again");
                        d.content.wm = None;
                    }
                    None => {}
                }
                d.content.tell_width();
                d.ui.ask_paint();
                Self::place(d);
            }
            _ => {}
        }
    }

    /// The screen resizer's file changed — the cut moved, began or ended.
    /// Only the room's WIDTH on the column's own screen moves the column:
    /// its width is told again and the control line drawn with what is
    /// offered now, as a screen that turned does (`Event::ScreensChanged`).
    fn resizer_changed(d: &mut Door<Self>) {
        let before = d.content.output_width();
        d.content.resizer = crate::resizer::is_on();
        if d.content.output_width() != before {
            d.content.tell_width();
            d.ui.ask_paint();
        }
    }

    /// THE ONE PLACE A COLUMN'S SURFACE CHANGES: shown on the screen its
    /// workspace is on, moved with it, or parked while no screen shows it.
    fn place(d: &mut Door<Self>) {
        let Some(name) = d.content.workspace.as_deref() else {
            return;
        };
        let want = wanted(name, &d.content.workspaces);
        match step(d.content.shown_on.as_deref(), want.as_deref()) {
            Step::Stay => {}
            Step::Park => Self::park_it(d),
            Step::Show(on) | Step::Move(on) => Self::show_on(d, &on),
        }
    }

    /// Shows the column on the screen `on` — the parked surface given a role
    /// again, or a new one on another screen. A screen this connection has
    /// not been told about yet waits for `outputs_changed`.
    fn show_on(d: &mut Door<Self>, on: &str) {
        let Some(output) = d.ui.screen_named(on) else {
            tracing::info!(
                output = on,
                "the column waits for its screen to be described"
            );
            return;
        };
        let Some(layer_shell) = d.layer_shell() else {
            return;
        };
        // THE CLONE GOES FIRST: while it lives, the old surface does too.
        d.content.forget_the_surface();
        let surface = match &d.shell {
            Shell::Parked(bare, _) => bare.clone(),
            _ => d.compositor().create_surface(&d.qh),
        };
        let width = width_px(
            d.content.sizing.asked,
            room_of(&d.content.widths, on, d.content.resizer.as_ref()),
        );
        let layer = column_on(&layer_shell, surface, &d.qh, Some(&output), width);
        let c = &mut d.content;
        c.surf.replace(Some(layer.clone()));
        c.told_width.set(width);
        c.now_on(&mut d.ui, Some(on));
        c.settle_owed = true;
        c.tenant.shown(&mut d.ui);
        tracing::info!(
            output = on,
            workspace = c.workspace.as_deref(),
            "the column is shown"
        );
        d.reopen(Shell::Layer(layer, layer_shell));
    }

    /// Takes the column off the screen and keeps it.
    fn park_it(d: &mut Door<Self>) {
        d.content.tenant.parked(&mut d.ui);
        d.content.forget_the_surface();
        d.content.now_on(&mut d.ui, None);
        tracing::info!(
            workspace = d.content.workspace.as_deref(),
            "the column is parked"
        );
        d.park();
    }

    /// Tells the compositor to send the windows the layout it already
    /// worked out — asked once the commit that MAPPED the column is known to
    /// have been shown (the first frame's callback answers only then).
    ///
    /// # Why the windows were not told on their own
    ///
    /// Two lines of the pinned source. A layer's exclusive zone is counted
    /// only once the surface is MAPPED (`wlr_scene_layer_surface_v1_get_box`,
    /// `surface->mapped && exclusive_zone > 0`), so the windows are re-tiled
    /// by the commit that carries the first frame. And a layer's commit sends
    /// the new layout out (`transaction_commit_dirty`) only when it also
    /// changed some layer state (`handle_surface_commit`), which a frame does
    /// not. The column opened over the windows, and they moved at the next
    /// click anywhere — the owner's screenshot, twice.
    ///
    /// The first attempt said the zone again in that commit, and did nothing:
    /// wlroots IGNORES a value set to what it already is
    /// (`layer_surface_handle_set_exclusive_zone`). This one asks over IPC,
    /// where every command ends in `transaction_commit_dirty`
    /// (`Compositor::settle`).
    fn settle_the_windows(&mut self) {
        match self.compositor().map(|w| w.settle()) {
            Some(Ok(())) => tracing::info!("the windows were told the column's room"),
            Some(Err(e)) => {
                tracing::warn!(error = %e, "the windows were not told the column's room");
                self.wm = None;
            }
            None => tracing::warn!("no compositor to tell the column's room to"),
        }
    }

    /// Which of the column's keys `key` is.
    fn own_key(key: ronoctua_keymap::Key) -> Option<Own> {
        OF_THE_COLUMN
            .iter()
            .find(|(notation, _)| ronoctua_keymap::Key::parse(notation).ok() == Some(key))
            .and_then(|(_, id)| Own::of(id))
    }

    /// Whether `y` is on a line of the column's own — line 0, and in the bar
    /// the last line too, where its close is.
    fn ours(&self, f: crate::chrome::Frame, y: f64) -> bool {
        #[allow(clippy::cast_possible_truncation)]
        let top = f.line_at(y as f32) == Some(0);
        let foot = self.sizing.drawn == Size::Bar
            && f.lines() > 1
            && y >= f64::from(f.height - f.line())
            && y < f64::from(f.height);
        top || foot
    }
}

impl<T: Tenant> Content for Column<T> {
    type Hand = T::Hand;

    fn app_id(&self) -> &'static str {
        self.tenant.app_id()
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        self.tenant.title(texts)
    }

    /// The column's keys first — the tenant may read an arrow without its
    /// `Ctrl` — then, in the bar, a key that acts on rows nobody sees puts
    /// the column back instead; and the rest is the tenant's.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(key) = crate::door::keys::key_of(event, mods) {
            match Self::own_key(key) {
                Some(Own::Wider) => {
                    self.wider(ui);
                    return;
                }
                Some(Own::Narrower) => {
                    self.narrower(ui);
                    return;
                }
                Some(Own::Close) => {
                    self.close(ui);
                    return;
                }
                None => {}
            }
            if self.sizing.asked == Size::Bar && !self.tenant.acts_without_rows(key) {
                self.restore(ui);
                return;
            }
        }
        let area = self.area(ui);
        self.tenant.typed(ui, event, mods, &area);
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        self.tenant.answered(ui, id, text);
    }

    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        self.tenant.unanswered(ui, id);
    }

    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.tenant.pasted(ui, text);
    }

    /// THE TENANT'S ANSWER: a text it is editing types the capitals the
    /// window's would (`door::keys::lock_undone`).
    fn types_capitals(&self) -> bool {
        self.tenant.types_capitals()
    }

    fn released(&mut self, ui: &mut Ui, event: &KeyEvent) {
        self.tenant.released(ui, event);
    }

    /// The slot taken, and the surface: on the focused screen when its
    /// workspace is the one in front, parked otherwise — placed on the first
    /// turn, once the door knows the screens by name.
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let layer_shell = crate::door::layer_shell(wiring)?;
        crate::right_column::take(T::WHO, self.workspace.as_deref(), self.sizing.asked);
        if !self.born_shown {
            self.ping.ping();
            return Ok(Shell::Parked(surface, layer_shell));
        }
        let width = width_px(self.sizing.asked, self.output_width());
        let layer = column_on(&layer_shell, surface, wiring.qh, None, width);
        self.told_width.set(width);
        self.surf.replace(Some(layer.clone()));
        Ok(Shell::Layer(layer, layer_shell))
    }

    fn default_size(&self) -> (u32, u32) {
        (
            width_px(self.sizing.asked, self.output_width()),
            u32::from(ronoctua_tokens::space::WINDOW_COLUMNHEIGHTPX),
        )
    }

    fn margin(&self) -> u16 {
        MARGIN
    }

    fn header_line(&self) -> u16 {
        HEADER_LINE
    }

    /// THE FOOTER'S PAGE, not the windows': a column is chrome drawn in a
    /// strip beside the footer (invariant 14), and the person's answer for
    /// the footer (`LEADER s t`) is the answer for it — its tenant wears it
    /// too.
    fn page(&self) -> crate::theme::Page {
        crate::theme::footer_page()
    }

    /// A screen that went away took the surface: the column parks, and the
    /// next list of workspaces says where it is shown again. With no
    /// compositor to say that, it closes, as it always did.
    fn layer_closed(&mut self, ui: &mut Ui) -> Closed {
        if self.workspace.is_none() {
            return Closed::Quit;
        }
        self.tenant.parked(ui);
        self.forget_the_surface();
        self.now_on(ui, None);
        tracing::info!("the column's screen went away; it is parked");
        Closed::Park
    }

    /// The drawer's keys are the header's — and in the bar there is no
    /// header.
    fn header_keys(&self) -> bool {
        self.sizing.drawn != Size::Bar
    }

    fn keyboard_entered(&mut self, ui: &mut Ui) {
        self.holders.insert(ui.keyboard_seat);
        ui.ask_paint();
        self.tenant.keyboard_entered(ui);
    }

    fn keyboard_left(&mut self, ui: &mut Ui) {
        self.holders.remove(&ui.keyboard_seat);
        ui.ask_paint();
        self.tenant.keyboard_left(ui);
    }

    /// The width told again when the size of the text moved it, and the size
    /// DRAWN once the compositor configured the width asked.
    fn resized(&mut self, ui: &mut Ui) {
        self.tell_width();
        // A CONFIGURE AT THE WIDTH ASKED — to a pixel: the buffer is cut to
        // a multiple of the scale (`crate::hidpi::resized`).
        let asked = width_px(self.sizing.asked, self.output_width());
        if ui.width.abs_diff(asked) <= 1 {
            self.drawn_at(ui, self.sizing.asked);
        }
        let area = self.area(ui);
        self.tenant.resized(ui, &area);
    }

    /// A screen plugged or taken away — a show that waited for its screen
    /// is tried again.
    fn outputs_changed(&mut self, _: &mut Ui) {
        self.ping.ping();
    }

    /// What `ui.screens` says moved — and a show that waited for its screen's
    /// NAME is tried again too: a connector can arrive after the screen,
    /// through `update_output`, which is no `outputs_changed`
    /// (`Ui::screen_named` says this is the moment to ask).
    fn screens_changed(&mut self, ui: &mut Ui) {
        self.ping.ping();
        self.tenant.screens_changed(ui);
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        self.tenant.language_changed(ui);
    }

    fn locale_changed(&mut self, ui: &mut Ui) {
        self.tenant.locale_changed(ui);
    }

    /// A QUESTION IS TYPED WHERE IT IS DRAWN, and the bar draws none: one
    /// asked there — on the door's line, or among the tenant's rows — puts
    /// the column back to its size.
    fn each_turn(&mut self, ui: &mut Ui) {
        self.tell_where_it_was_born(ui);
        if (ui.asking.is_some() || self.tenant.waits_for_an_answer())
            && self.sizing.asked == Size::Bar
        {
            self.restore(ui);
        }
        self.tenant.each_turn(ui);
    }

    fn frame_done(&mut self, ui: &mut Ui) {
        if std::mem::take(&mut self.settle_asked) {
            self.settle_the_windows();
        }
        self.tenant.frame_done(ui);
    }

    /// No sweep of text on the control line.
    fn text_area(&self, ui: &Ui, line: u16, column: u16) -> Option<crate::text_select::Area> {
        if line < HEADER_LINE {
            return None;
        }
        self.tenant.text_area(ui, line, column, &self.area(ui))
    }

    /// Line 0 is the column's — and in the bar its last line too: the pointer
    /// over it lights a glyph and a press takes it, and the tenant is told
    /// the pointer LEFT its part. A drag that began in the tenant's part stays
    /// the tenant's until the button comes up, over the column's lines too; a
    /// release, a leave and the wheel always reach it. What each pointer is
    /// on is its own ([`Under`]).
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<T::Hand>, events: &[PointerEvent]) {
        let f = ui.frame(0);
        let who = ui.pointer_seat;
        let before = self.under.clone();
        let mut theirs: Vec<PointerEvent> = Vec::with_capacity(events.len());
        for e in events {
            let on_the_line = self.ours(f, e.position.1);
            let dragging = self.tenant_holds.contains(&who);
            let glyph = || {
                glyph_at(
                    &self.placed,
                    self.sizing.drawn,
                    f,
                    e.position.0,
                    e.position.1,
                )
            };
            match e.kind {
                PointerEventKind::Leave { .. } => {
                    self.tenant_holds.remove(&who);
                    self.under.at(who, None);
                    theirs.push(e.clone());
                }
                PointerEventKind::Release { .. } => {
                    self.tenant_holds.remove(&who);
                    theirs.push(e.clone());
                }
                PointerEventKind::Axis { .. } => theirs.push(e.clone()),
                _ if dragging => theirs.push(e.clone()),
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } if on_the_line => {
                    let g = glyph();
                    self.under.at(who, g);
                    theirs.push(PointerEvent {
                        surface: e.surface.clone(),
                        position: e.position,
                        kind: PointerEventKind::Leave { serial: 0 },
                    });
                }
                PointerEventKind::Press { button, .. } if on_the_line => {
                    // THE POINTER THAT PRESSED is the one looked at: an inert
                    // glyph's sentence is said for it.
                    let g = glyph();
                    self.under.at(who, g);
                    if button == LEFT_BUTTON
                        && let Some(g) = g
                    {
                        self.pressed_glyph(ui, g);
                    }
                }
                PointerEventKind::Press { .. } => {
                    self.tenant_holds.insert(who);
                    self.under.at(who, None);
                    theirs.push(e.clone());
                }
                _ => {
                    self.under.at(who, None);
                    theirs.push(e.clone());
                }
            }
        }
        if self.under != before {
            ui.ask_paint();
        }
        if !theirs.is_empty() {
            let area = self.area(ui);
            self.tenant.pointer(ui, hand, &theirs, &area);
        }
    }

    /// The tenant, then the control line over its ground and — with a
    /// keyboard on the column — the focus line over everything.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.tell_where_it_was_born(ui);
        let area = self.area(ui);
        self.tenant.paint(ui, canvas, &area);
        let f = ui.frame(0);
        let offers: Vec<(Size, Offer)> = Size::ALL.iter().map(|s| (*s, self.offer(*s))).collect();
        let d = decoration(
            self.sizing.drawn,
            &offers,
            f,
            &self.under,
            !self.holders.is_empty(),
            &ui.texts,
        );
        canvas.spans.extend(d.spans);
        canvas.over.extend(d.over);
        self.placed = d.placed;
        // THE FRAME THAT MAPS THE COLUMN asks to be told when it is SHOWN:
        // the answer can only come after the compositor read that commit and
        // counted the zone, and only then is there a layout to send. A `sync`
        // after presenting was the second attempt, and it raced: the driver
        // may commit the frame on a thread of its own, so the `sync` could
        // arrive first — measured in the VM, the window told 512 again. The
        // callback is asked here, before the commit that carries this frame.
        if std::mem::take(&mut self.settle_owed) {
            let _ = ui
                .surface
                .frame(&self.qh, FrameCallbackData(ui.surface.clone()));
            self.settle_asked = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell() -> f32 {
        tokens::metrics::cell_width(tokens::text::size())
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// What a span IS on screen: its text, where, in what colour, and from
    /// which border.
    fn seen(spans: &[ronoctua_render::Span]) -> Vec<(String, u16, u16, tokens::Color, bool)> {
        spans
            .iter()
            .map(|s| (s.text.clone(), s.column, s.line, s.color, s.edge))
            .collect()
    }

    /// One pointer on `glyph`, or on none.
    fn one(glyph: Option<Glyph>) -> Under {
        let mut under = Under::default();
        under.at(0, glyph);
        under
    }

    fn space(name: &str, visible: bool, output: Option<&str>) -> Workspace {
        Workspace {
            name: name.to_owned(),
            number: name.parse().ok(),
            focused: false,
            occupied: true,
            visible,
            output: output.map(ToOwned::to_owned),
        }
    }

    /// FAILS IF: the sizes stop being ordered from the widest, the medium
    /// stops being the width the player's column always had, or the bar gets
    /// as thin as a line of the modeline — a strip that reads as a rule.
    #[test]
    fn the_sizes_are_ordered_and_medium_is_the_old_width() {
        let widths: Vec<u32> = Size::ALL.iter().map(|s| width_px(*s, None)).collect();
        assert!(widths.windows(2).all(|p| p[0] > p[1]), "{widths:?}");
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let old = (cell() * f32::from(tokens::space::COLUMN_MEDIUM + 2 * MARGIN)).ceil() as u32;
        assert_eq!(width_px(Size::Medium, None), old);
        let line = tokens::metrics::line_height(tokens::text::size());
        #[allow(clippy::cast_precision_loss)]
        let bar = width_px(Size::Bar, None) as f32;
        assert!(bar > line, "the bar is {bar} px, a line {line}");
        // AND THE BAR'S CELLS ARE ODD, so its one glyph has a middle.
        assert_eq!((tokens::space::COLUMN_BAR + 2 * MARGIN) % 2, 1);
        assert_eq!(Size::Bar.picture_lines(), 0);
        assert_eq!(Size::Medium.picture_lines(), tokens::space::COLUMN_PICTURES);
        assert!(Size::Large.picture_lines() > Size::Medium.picture_lines());
    }

    /// FAILS IF: a size leaks past its share of a screen, falls under the
    /// bar, or a wider size comes out narrower than a smaller one.
    #[test]
    fn nothing_leaks_off_a_small_screen() {
        for output in [1000, 1280, 640, 100] {
            let widths: Vec<u32> = Size::ALL
                .iter()
                .map(|s| width_px(*s, Some(output)))
                .collect();
            let bar = width_px(Size::Bar, None);
            for w in &widths {
                assert!(*w >= bar, "{output}: {widths:?}");
                assert!(
                    *w <= (output * u32::from(tokens::space::COLUMN_MAXPCT) / 100).max(bar),
                    "{output}: {widths:?}"
                );
            }
            assert!(
                widths.windows(2).all(|p| p[0] >= p[1]),
                "{output}: {widths:?}"
            );
        }
    }

    /// FAILS IF: a size the clamp makes as wide as the one below it is still
    /// offered — a glyph that changes nothing — or the bar is offered to a
    /// tenant with nothing made for it.
    #[test]
    fn a_size_equal_to_the_one_below_is_not_offered() {
        let c = cell();
        // TWO SIZES ONE WIDTH: large clamped down to medium's.
        let medium = width_at(Size::Medium, c, None);
        let screen = medium * 100 / u32::from(tokens::space::COLUMN_MAXPCT) + 1;
        assert_eq!(offer_at(Size::Large, true, c, Some(screen)), Offer::NoRoom);
        assert_eq!(offer_at(Size::Medium, true, c, Some(screen)), Offer::Yes);
        assert_eq!(offer_at(Size::Large, true, c, None), Offer::Yes);
        assert_eq!(offer_at(Size::Bar, false, c, None), Offer::NotMade);
    }

    /// FAILS IF: under the screen resizer the clamp is taken of the monitor
    /// and not of the room its curtains leave — a medium column took 77% of
    /// a 700 px room on a 1920 px monitor, and past the room's own width it
    /// hung off its left edge — or the cut of one screen narrows a column
    /// on another.
    #[test]
    fn under_the_resizer_the_clamp_is_taken_of_the_room() {
        let c = 10.8;
        let widths: BTreeMap<String, u32> =
            [("DP-1".to_owned(), 1920), ("HDMI-1".to_owned(), 1920)].into();
        let cut = crate::resizer::State {
            output: "DP-1".to_owned(),
            screen_width_px: 1920,
            screen_height_px: 1080,
            width_px: 700,
            height_px: 900,
        };
        let room = room_of(&widths, "DP-1", Some(&cut));
        assert_eq!(room, Some(700));
        let most = 700 * u32::from(tokens::space::COLUMN_MAXPCT) / 100;
        let medium = width_at(Size::Medium, c, room);
        assert!(
            medium <= most,
            "medium is {medium} px of a 700 px room, over {most}"
        );
        assert!(
            width_at(Size::Medium, c, None) > most,
            "the case proves nothing: medium fits the room unclamped"
        );
        // AND THE ROOM'S OFFER: large clamped to medium's width is not one.
        assert_eq!(offer_at(Size::Large, true, c, room), Offer::NoRoom);
        // THE OTHER SCREEN, and no cut: the monitor.
        assert_eq!(room_of(&widths, "HDMI-1", Some(&cut)), Some(1920));
        assert_eq!(room_of(&widths, "DP-1", None), Some(1920));
        assert_eq!(room_of(&widths, "DP-2", Some(&cut)), None);
        // AT THE RESIZER'S FLOOR, no size but the bar is wider than the
        // room's share, and the bar is never wider than the room.
        let floor = crate::resizer::limits(crate::resizer::Units {
            cell: c,
            line: 26.1,
        });
        let narrow = crate::resizer::State {
            width_px: floor.min_width_px,
            ..cut
        };
        let room = room_of(&widths, "DP-1", Some(&narrow)).unwrap();
        let bar = width_at(Size::Bar, c, None);
        for size in Size::ALL {
            let w = width_at(size, c, Some(room));
            assert!(
                w <= (room * u32::from(tokens::space::COLUMN_MAXPCT) / 100).max(bar),
                "{size:?} is {w} px of a {room} px room"
            );
            assert!(w < room, "{size:?} is {w} px, the whole {room} px room");
        }
    }

    /// FAILS IF: a step past the bar stops going back to the size before it,
    /// a step skips a size that can be taken or takes one that cannot, or
    /// the end of the ring says something.
    #[test]
    fn the_sizes_step_down_to_the_bar_and_the_bar_goes_back() {
        let all = |_: Size| Offer::Yes;
        let mut s = Sizing::at(Size::Medium);
        assert_eq!(s.wider(all), Ok(Size::Large));
        assert_eq!(s.narrower(all), Ok(Size::Small));
        assert!(s.ask(Size::Small));
        assert!(!s.ask(Size::Small), "the same size again is no change");
        assert_eq!(s.narrower(all), Ok(Size::Bar));
        assert!(s.ask(Size::Bar));
        assert_eq!(s.before_bar, Size::Small);
        // FROM THE BAR, BACK — not up the ring.
        assert_eq!(s.wider(all), Ok(Size::Small));
        assert_eq!(s.narrower(all), Err(None), "nothing under the bar");
        // A SIZE THAT CANNOT BE TAKEN IS STEPPED OVER, and said when it is
        // the last.
        let no_small = |x: Size| {
            if x == Size::Small {
                Offer::NoRoom
            } else {
                Offer::Yes
            }
        };
        assert_eq!(Sizing::at(Size::Medium).narrower(no_small), Ok(Size::Bar));
        let no_large = |x: Size| {
            if x == Size::Large {
                Offer::NoRoom
            } else {
                Offer::Yes
            }
        };
        assert_eq!(
            Sizing::at(Size::Medium).wider(no_large),
            Err(Some(Offer::NoRoom))
        );
        assert_eq!(Sizing::at(Size::Large).wider(all), Err(None));
        // OPENED AT THE BAR, it goes back to the usual width.
        assert_eq!(Sizing::at(Size::Bar).wider(all), Ok(Size::Medium));
    }

    /// A surface `width` px wide and tall enough for a bar's two lines.
    fn frame(width: f32) -> crate::chrome::Frame {
        crate::chrome::Frame::of(width, 400.0, 0)
    }

    /// Every size offered.
    fn every_size() -> Vec<(Size, Offer)> {
        Size::ALL.iter().map(|s| (*s, Offer::Yes)).collect()
    }

    /// The sentence a decoration says, if any: the one run in the
    /// explanation's register.
    fn said(d: &Decoration) -> Option<&ronoctua_render::Span> {
        d.spans
            .iter()
            .find(|s| s.color == crate::chrome::Register::Explain.color())
    }

    /// Where `glyph` was placed.
    fn placed(d: &Decoration, glyph: Glyph) -> Placed {
        *d.placed
            .iter()
            .find(|p| p.glyph == glyph)
            .unwrap_or_else(|| panic!("{glyph:?} was not placed"))
    }

    /// FAILS IF: a glyph of the control line is taken a cell away from where
    /// it was drawn — counted from the border, at a width whose grid leaves
    /// a remainder — the cell the rule stands in answers as a glyph, or a
    /// line of the bar stops being its glyph's whole.
    #[test]
    fn a_glyph_is_taken_where_it_is_drawn() {
        let t = texts();
        let c = f64::from(cell());
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let offers = every_size();
        for remainder in [0.1, 0.3, 0.7] {
            let width = c * (50.0 + remainder);
            #[allow(clippy::cast_possible_truncation)]
            let f = frame(width as f32);
            let columns = f.columns();
            let d = decoration(Size::Medium, &offers, f, &Under::default(), false, &t);
            assert_eq!(d.placed.len(), 5, "the four sizes and the close");
            let at = |x: f64, y: f64| glyph_at(&d.placed, Size::Medium, f, x, y);
            for p in &d.placed {
                // WHERE THE RENDERER PUTS AN EDGE RUN: its distance in cells
                // from the border, in pixels.
                let left = width - c * f64::from(columns - p.at);
                assert_eq!(at(left + c * 0.5, line * 0.5), Some(p.glyph), "{remainder}");
                assert_eq!(
                    at(left + c * 1.5, line * 0.5),
                    Some(p.glyph),
                    "the air after it is its own"
                );
                assert_eq!(
                    at(left + c * 0.5, line * 1.5),
                    None,
                    "line 1 is the tenant's"
                );
            }
            let close = placed(&d, Glyph::Close);
            let rule_cell = width - c * f64::from(columns - close.at - GLYPH_CELLS);
            assert_eq!(at(rule_cell + c * 0.5, line * 0.5), None, "the rule's cell");
            let before = width - c * f64::from(columns - close.at) - c * 0.5;
            assert_eq!(at(before, line * 0.5), None);
        }
        // THE BAR: widen on line 0 and close on the last, each line whole.
        let f = frame(54.0);
        let bar = decoration(Size::Bar, &offers, f, &Under::default(), false, &t);
        assert_eq!(
            bar.placed,
            [
                Placed {
                    at: MARGIN + 1,
                    foot: false,
                    glyph: Glyph::Widen
                },
                Placed {
                    at: MARGIN + 1,
                    foot: true,
                    glyph: Glyph::Close
                },
            ]
        );
        let at = |x: f64, y: f64| glyph_at(&bar.placed, Size::Bar, f, x, y);
        assert_eq!(at(1.0, line * 0.5), Some(Glyph::Widen));
        assert_eq!(at(50.0, 399.0), Some(Glyph::Close));
        assert_eq!(at(1.0, 400.0 - line + 0.5), Some(Glyph::Close));
        assert_eq!(at(1.0, 200.0), None, "the tenant's lines");
        // AND THE CLOSE IS DRAWN FROM THE BOTTOM EDGE, on the last line.
        let close = bar
            .spans
            .iter()
            .find(|s| s.text == tokens::icon::COLUMN_CLOSE.to_string())
            .expect("the bar's close");
        assert_eq!((close.line, close.foot), (f.lines() - 1, true));
        assert_eq!(
            Area::of_a_column(Size::Bar, 1, 26.1).end(f.lines()),
            f.lines() - 1
        );
        assert_eq!(
            Area::of_a_column(Size::Small, 1, 26.1).end(f.lines()),
            f.lines()
        );
    }

    /// FAILS IF: the close stops being LEFT of the four sizes, the rule
    /// stops standing where the air on its two sides is equal — measured on
    /// the glyphs' INK, not on their cells — or leaves the control line, or
    /// the ink of either glyph stops being measured.
    #[test]
    fn close_sits_left_of_the_sizes_with_equal_air() {
        let t = texts();
        let line = tokens::metrics::line_height(tokens::text::size());
        assert!(tokens::icon::ink_across(tokens::icon::COLUMN_CLOSE).is_some());
        assert!(tokens::icon::ink_across(Size::Large.glyph()).is_some());
        for width in [540.0, 543.7, 700.0, 811.9] {
            let f = frame(width);
            let d = decoration(Size::Medium, &every_size(), f, &Under::default(), false, &t);
            // THE ORDER, from the border: [close] │ [L][M][S][B].
            let ats: Vec<u16> = [
                Glyph::Close,
                Glyph::Size(Size::Large),
                Glyph::Size(Size::Medium),
                Glyph::Size(Size::Small),
                Glyph::Size(Size::Bar),
            ]
            .into_iter()
            .map(|g| placed(&d, g).at)
            .collect();
            assert!(ats.windows(2).all(|p| p[0] < p[1]), "{ats:?}");
            assert_eq!(ats[1] - ats[0], GLYPH_CELLS + 1, "a cell for the rule");
            // THE RULE, and only it, before the focus line.
            assert_eq!(d.over.len(), 1);
            let rule = d.over[0];
            assert_eq!(
                rule,
                crate::chrome::divider(rule.x, rule.y, rule.y + rule.height),
                "the rule is chrome's"
            );
            assert!(rule.y > 0.0 && rule.y + rule.height < line, "{rule:?}");
            // THE AIR, from the ink.
            let columns = f.columns();
            let (_, close_ends) = ink_of(
                tokens::icon::COLUMN_CLOSE,
                crate::chrome::edge_x(width, columns, ats[0]),
            );
            let (large_begins, _) = ink_of(
                Size::Large.glyph(),
                crate::chrome::edge_x(width, columns, ats[1]),
            );
            let left = rule.x - close_ends;
            let right = large_begins - (rule.x + rule.width);
            assert!(left > 0.0 && right > 0.0, "{left} / {right}");
            assert!(
                (left - right).abs() < 0.01,
                "{left} against {right} at {width}"
            );
            // AND NOT THE CELLS': their middle is more than half a pixel off.
            let cells = f32::midpoint(
                crate::chrome::edge_x(width, columns, ats[0]),
                crate::chrome::edge_x(width, columns, ats[1]),
            ) + cell() / 2.0;
            assert!(
                (rule.x + rule.width / 2.0 - cells).abs() > 0.5,
                "the rule stands on the cells' middle: the ink was not read"
            );
        }
    }

    /// FAILS IF: the focus line moves anything — a span, a glyph or the rule
    /// lands elsewhere with the keyboard on the column — or it is not drawn
    /// over everything while it is.
    #[test]
    fn the_focus_line_moves_nothing() {
        let t = texts();
        let offers = every_size();
        for drawn in Size::ALL {
            let under = one(Some(Glyph::Size(Size::Small)));
            let off = decoration(drawn, &offers, frame(540.0), &under, false, &t);
            let on = decoration(drawn, &offers, frame(540.0), &under, true, &t);
            assert_eq!(seen(&off.spans), seen(&on.spans), "{drawn:?}");
            assert_eq!(off.placed, on.placed, "{drawn:?}");
            assert_eq!(on.over.len(), off.over.len() + 1, "{drawn:?}");
            assert_eq!(on.over[..off.over.len()], off.over[..], "{drawn:?}");
            assert_eq!(on.over.last(), Some(&crate::chrome::focus_line(540.0)));
        }
    }

    /// FAILS IF: a sentence of the control line is cut short in any language
    /// on the narrowest column that has one — a refusal would lose its
    /// reason, the way "This screen has no…" did, and a size its name.
    #[test]
    fn every_sentence_of_the_line_fits_a_small_column_in_every_language() {
        #[allow(clippy::cast_precision_loss)]
        let f = frame(width_px(Size::Small, None) as f32);
        for language in ronoctua_i18n::LANGUAGES {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for offer in [Offer::Yes, Offer::NoRoom, Offer::NotMade] {
                let offers: Vec<(Size, Offer)> = Size::ALL.iter().map(|s| (*s, offer)).collect();
                for glyph in Size::ALL.map(Glyph::Size).into_iter().chain([Glyph::Close]) {
                    let key = sentence_on_the_line(glyph, |_| offer).expect("a sentence");
                    let d = decoration(Size::Small, &offers, f, &one(Some(glyph)), false, &t);
                    let s = said(&d)
                        .unwrap_or_else(|| panic!("{language} {glyph:?} {offer:?}: nothing"));
                    assert_eq!(s.text, t.t(key), "{language} {glyph:?} {offer:?}: cut");
                }
            }
        }
    }

    /// FAILS IF: the glyph under the pointer stops saying what it does — the
    /// close included — a glyph that cannot be taken stops saying why, the
    /// sentence stops ending a cell before the close, counted from the
    /// border, the bar says anything, or the line says something at rest.
    #[test]
    fn a_glyph_under_the_pointer_says_what_it_does_except_in_the_bar() {
        let t = texts();
        let mut offers = every_size();
        offers[3].1 = Offer::NotMade;
        let f = frame(648.0);
        let rest = decoration(Size::Medium, &offers, f, &Under::default(), false, &t);
        assert!(said(&rest).is_none(), "the line speaks at rest");
        let close = placed(&rest, Glyph::Close).at;
        for (glyph, key) in [
            (
                Glyph::Size(Size::Small),
                ronoctua_i18n::key::ABOUT_COLUMN_SMALL,
            ),
            (
                Glyph::Size(Size::Large),
                ronoctua_i18n::key::ABOUT_COLUMN_LARGE,
            ),
            (
                Glyph::Size(Size::Bar),
                ronoctua_i18n::key::ABOUT_COLUMN_BAR_NOT_MADE,
            ),
            (Glyph::Close, ronoctua_i18n::key::ABOUT_COLUMN_CLOSE_GLYPH),
        ] {
            let d = decoration(Size::Medium, &offers, f, &one(Some(glyph)), false, &t);
            let s = said(&d).unwrap_or_else(|| panic!("{glyph:?} said nothing"));
            assert_eq!(s.text, t.t(key), "{glyph:?}");
            // SET AGAINST THE CLOSE, from the border, with its air.
            assert!(s.edge, "{glyph:?}: counted from the left");
            assert_eq!(
                s.column + ronoctua_protocol::cells(&s.text),
                close - tokens::space::COLUMN_SAIDAIR,
                "{glyph:?}"
            );
        }
        let bar = decoration(
            Size::Medium,
            &offers,
            f,
            &one(Some(Glyph::Size(Size::Bar))),
            false,
            &t,
        );
        let inert = bar
            .spans
            .iter()
            .find(|s| s.text == Size::Bar.glyph().to_string())
            .expect("the bar's glyph");
        assert_eq!(inert.color, tokens::color::TEXT_SUBTLE_INERT);
        for glyph in [Glyph::Widen, Glyph::Close] {
            let in_the_bar = decoration(
                Size::Bar,
                &offers,
                frame(54.0),
                &one(Some(glyph)),
                false,
                &t,
            );
            assert!(said(&in_the_bar).is_none(), "the bar explained {glyph:?}");
            assert_eq!(in_the_bar.spans.len(), 2, "the bar's two glyphs");
        }
        // AND THE CURRENT SIZE IS BRIGHTER, never the accent.
        let current = rest
            .spans
            .iter()
            .find(|s| s.text == Size::Medium.glyph().to_string())
            .expect("medium's glyph");
        assert_eq!(current.color, tokens::color::TEXT_PRIMARY);
    }

    /// A tenant with nothing of its own, to see what the column does — it
    /// only remembers being told the column closes.
    #[derive(Default)]
    struct Nobody {
        closed: bool,
        /// Every screen it was told it is on, in order.
        placed: Vec<Option<String>>,
        /// A question of its own waits on its rows.
        asking: bool,
        /// A text of its own is being typed.
        typing: bool,
    }

    impl Tenant for Nobody {
        type Hand = ();
        const WHO: crate::right_column::Tenant = crate::right_column::Tenant::Player;
        fn app_id(&self) -> &'static str {
            "test.nobody"
        }
        fn title(&self, _: &ronoctua_i18n::Texts) -> String {
            String::new()
        }
        fn shape(&self, _: Size) -> Option<Shape> {
            Some(Shape::Adapted)
        }
        fn paint(&mut self, _: &mut Ui, _: &mut Canvas<'_>, _: &Area) {}
        fn pointer(&mut self, _: &mut Ui, _: &mut Hand<()>, _: &[PointerEvent], _: &Area) {}
        fn typed(&mut self, _: &mut Ui, _: &KeyEvent, _: Modifiers, _: &Area) {}
        fn closing(&mut self, _: &mut Ui) {
            self.closed = true;
        }
        fn placed_on(&mut self, _: &mut Ui, on: Option<&str>) {
            self.placed.push(on.map(ToOwned::to_owned));
        }
        fn waits_for_an_answer(&self) -> bool {
            self.asking
        }
        fn types_capitals(&self) -> bool {
            self.typing
        }
    }

    /// FAILS IF: a text the tenant is editing in the column types small
    /// letters with Caps Lock on, where the same tenant in a window types
    /// capitals — the column did not pass the tenant's answer on.
    #[test]
    fn the_column_types_the_capitals_its_tenant_types() {
        let ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").unwrap());
        let mut c = a_column(&ui);
        assert!(!Content::types_capitals(&c));
        c.tenant.typing = true;
        assert!(
            Content::types_capitals(&c),
            "the column kept its tenant's capitals"
        );
    }

    /// FAILS IF: a question of the tenant's own — drawn on its rows, which
    /// the bar does not draw — waits unseen while the column is shrunk to its
    /// bar, instead of the column going back to the size it had; or the bar
    /// is left when nothing waits.
    #[test]
    fn a_question_of_the_tenant_widens_the_bar() {
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").unwrap());
        let mut c = a_column_at(&ui, Size::Small);
        c.set_size(&mut ui, Size::Bar);
        assert_eq!(c.sizing.asked, Size::Bar);
        c.each_turn(&mut ui);
        assert_eq!(c.sizing.asked, Size::Bar, "nothing waits: the bar stays");
        c.tenant.asking = true;
        c.each_turn(&mut ui);
        assert_eq!(c.sizing.asked, Size::Small, "back to the size it had");
    }

    /// FAILS IF: the tenant is not told the screen the column is on each time
    /// it moves — shown, moved, parked — or is told again when it did not.
    #[test]
    fn the_tenant_is_told_the_screen_it_is_on_when_it_moves() {
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").unwrap());
        let mut c = a_column(&ui);
        c.now_on(&mut ui, Some("eDP-1"));
        c.now_on(&mut ui, Some("eDP-1"));
        c.now_on(&mut ui, Some("DP-2"));
        c.now_on(&mut ui, None);
        c.now_on(&mut ui, None);
        assert_eq!(
            c.tenant.placed,
            vec![Some("eDP-1".to_owned()), Some("DP-2".to_owned()), None]
        );
        assert_eq!(c.shown_on, None);
    }

    /// FAILS IF: a column BORN on a screen never tells its tenant which —
    /// the first `place` finds it where it is wanted (`Step::Stay`), and a
    /// tenant that counts an unknown screen as every screen would show what
    /// is routed to another one until the column moved.
    #[test]
    fn a_column_born_on_a_screen_tells_its_tenant_which_once() {
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").unwrap());
        let mut c = a_column(&ui);
        c.shown_on = Some("DP-2".to_owned());
        c.placement_owed = true;
        assert_eq!(
            step(c.shown_on.as_deref(), Some("DP-2")),
            Step::Stay,
            "the first place moves nothing"
        );
        c.tell_where_it_was_born(&mut ui);
        c.tell_where_it_was_born(&mut ui);
        c.now_on(&mut ui, Some("DP-2"));
        assert_eq!(c.tenant.placed, vec![Some("DP-2".to_owned())]);
        c.now_on(&mut ui, Some("eDP-1"));
        assert_eq!(
            c.tenant.placed,
            vec![Some("DP-2".to_owned()), Some("eDP-1".to_owned())]
        );
        // A move before the first turn is the telling: never twice.
        let mut c = a_column(&ui);
        c.shown_on = Some("DP-2".to_owned());
        c.placement_owed = true;
        c.now_on(&mut ui, Some("HDMI-A-1"));
        c.tell_where_it_was_born(&mut ui);
        assert_eq!(c.tenant.placed, vec![Some("HDMI-A-1".to_owned())]);
    }

    /// A column at `size` on no compositor, its glyphs laid out as a frame of
    /// `ui` lays them.
    fn a_column_at(ui: &Ui, size: Size) -> Column<Nobody> {
        let queue = ui.conn.new_event_queue::<Door<Column<Nobody>>>();
        let (ping, _) = ping::make_ping().expect("a ping");
        let placed = decoration(
            size,
            &every_size(),
            ui.frame(0),
            &Under::default(),
            false,
            &ui.texts,
        )
        .placed;
        Column {
            tenant: Nobody::default(),
            sizing: Sizing::at(size),
            workspace: None,
            workspaces: Vec::new(),
            born_shown: true,
            placement_owed: false,
            shown_on: None,
            widths: BTreeMap::new(),
            surf: RefCell::new(None),
            told_width: Cell::new(0),
            holders: BTreeSet::new(),
            tenant_holds: BTreeSet::new(),
            placed,
            under: Under::default(),
            settle_owed: false,
            settle_asked: false,
            wm: None,
            qh: queue.handle(),
            ping,
            resizer: None,
            _resizer_watch: None,
            slot: false,
        }
    }

    /// A column at medium — see [`a_column_at`].
    fn a_column(ui: &Ui) -> Column<Nobody> {
        a_column_at(ui, Size::Medium)
    }

    /// A press of the left button at `(x, y)`.
    fn press_at(ui: &Ui, x: f64, y: f64) -> PointerEvent {
        PointerEvent {
            surface: ui.surface.clone(),
            position: (x, y),
            kind: PointerEventKind::Press {
                time: 0,
                button: LEFT_BUTTON,
                serial: 0,
            },
        }
    }

    /// FAILS IF: the close glyph, the bar's close or `C-q` stop closing the
    /// column — the door quits, and the tenant is told first so it can let
    /// its pictures go — or a size's glyph closes it.
    #[test]
    fn closing_the_column_quits() {
        let mut hand = Hand::default();
        // BY THE GLYPH, where it was drawn.
        let mut ui = Ui::for_tests(texts());
        let mut c = a_column(&ui);
        let f = ui.frame(0);
        let line = f64::from(f.line());
        let x_of = |at: u16| {
            f64::from(crate::chrome::edge_x(f.width, f.columns(), at)) + f64::from(f.cell()) * 0.5
        };
        let large = c
            .placed
            .iter()
            .find(|p| p.glyph == Glyph::Size(Size::Large))
            .expect("large")
            .at;
        let e = press_at(&ui, x_of(large), line * 0.5);
        c.pointer(&mut ui, &mut hand, &[e]);
        assert!(
            !ui.quit && !c.tenant.closed,
            "a size's glyph closed the column"
        );
        let close = c
            .placed
            .iter()
            .find(|p| p.glyph == Glyph::Close)
            .expect("close")
            .at;
        let e = press_at(&ui, x_of(close), line * 0.5);
        c.pointer(&mut ui, &mut hand, &[e]);
        assert!(ui.quit && c.tenant.closed, "the close glyph");
        // BY THE KEY, as the keyboard sends it.
        let mut ui = Ui::for_tests(texts());
        let mut c = a_column(&ui);
        let key = ronoctua_keymap::Key::parse("C-q").expect("parses");
        let (event, mods) = crate::door::keys::event_of(key);
        Content::typed(&mut c, &mut ui, &event, mods);
        assert!(ui.quit && c.tenant.closed, "C-q");
        // AND IN THE BAR, by its last line.
        let mut ui = Ui::for_tests(texts());
        let mut c = a_column_at(&ui, Size::Bar);
        let f = ui.frame(0);
        let e = press_at(&ui, 1.0, f64::from(f.height) - line * 0.5);
        c.pointer(&mut ui, &mut hand, &[e]);
        assert!(ui.quit && c.tenant.closed, "the bar's close");
    }

    /// FAILS IF: one pointer's hover reaches another's — a second cursor
    /// crossing the tenant's rows puts out the glyph the first one rests on
    /// and its sentence, or one leaving takes the other's with it. Every
    /// cursor has its own state (`door::handlers`).
    #[test]
    fn a_glyph_under_one_pointer_is_that_pointers() {
        let t = texts();
        let mut ui = Ui::for_tests(texts());
        let mut c = a_column(&ui);
        let mut hand = Hand::default();
        let f = ui.frame(0);
        // WHERE A GLYPH IS, from what was placed: the pointer and the painter
        // read one answer.
        let x_of = |g: Glyph, placed: &[Placed]| {
            let y = f64::from(f.line()) * 0.5;
            (0..f.columns() * 2)
                .map(|half| f64::from(f.cell()) * (f64::from(half) + 0.5) / 2.0)
                .find(|x| glyph_at(placed, Size::Medium, f, *x, y) == Some(g))
                .expect("the glyph is on the line")
        };
        let surface = ui.surface.clone();
        let at = |x: f64, line: f32| PointerEvent {
            surface: surface.clone(),
            position: (x, f64::from(f.line() * (line + 0.5))),
            kind: PointerEventKind::Motion { time: 0 },
        };
        let small = Glyph::Size(Size::Small);
        let large = Glyph::Size(Size::Large);
        let drawn = |c: &Column<Nobody>| {
            let d = decoration(Size::Medium, &every_size(), f, &c.under, false, &t);
            let lit = |g: Glyph| {
                let Glyph::Size(s) = g else { return false };
                d.spans.iter().any(|x| {
                    x.text == s.glyph().to_string() && x.color == tokens::color::TEXT_SUBTLE_LIT_KEY
                })
            };
            (lit(small), lit(large), said(&d).map(|x| x.text.clone()))
        };
        let about = |s: Size| Some(t.t(s.sentence()));
        // POINTER 1 rests on the small glyph: lit, and said.
        ui.pointer_seat = 1;
        let e = at(x_of(small, &c.placed), 0.0);
        c.pointer(&mut ui, &mut hand, &[e]);
        assert_eq!(drawn(&c), (true, false, about(Size::Small)));
        // POINTER 2 crosses the tenant's rows: pointer 1's glyph stays.
        ui.pointer_seat = 2;
        let e = at(x_of(small, &c.placed), 5.0);
        c.pointer(&mut ui, &mut hand, &[e]);
        assert_eq!(
            drawn(&c),
            (true, false, about(Size::Small)),
            "another cursor put the glyph out"
        );
        // POINTER 2 on the large glyph: both lit, and the one that moved is
        // the one said.
        let e = at(x_of(large, &c.placed), 0.0);
        c.pointer(&mut ui, &mut hand, &[e]);
        assert_eq!(drawn(&c), (true, true, about(Size::Large)));
        // POINTER 2 LEAVES: pointer 1's glyph is said again.
        let leave = PointerEvent {
            surface: surface.clone(),
            position: (0.0, 0.0),
            kind: PointerEventKind::Leave { serial: 0 },
        };
        c.pointer(&mut ui, &mut hand, &[leave]);
        assert_eq!(drawn(&c), (true, false, about(Size::Small)));
    }

    /// FAILS IF: a column stops staying where its workspace is shown, stops
    /// moving with it, is not put away when no screen shows it or when it is
    /// gone, or does not come back under the same name.
    #[test]
    fn a_column_follows_its_workspace_by_name() {
        let two_screens = [
            space("1", true, Some("eDP-1")),
            space("2", true, Some("DP-2")),
            space("3", false, Some("eDP-1")),
        ];
        assert_eq!(wanted("1", &two_screens).as_deref(), Some("eDP-1"));
        assert_eq!(wanted("2", &two_screens).as_deref(), Some("DP-2"));
        assert_eq!(wanted("3", &two_screens), None, "not visible");
        assert_eq!(wanted("9", &two_screens), None, "gone");
        // VISIBLE HERE: it stays.
        assert_eq!(step(Some("eDP-1"), Some("eDP-1")), Step::Stay);
        // VISIBLE ELSEWHERE: it moves.
        assert_eq!(
            step(Some("eDP-1"), Some("DP-2")),
            Step::Move("DP-2".to_owned())
        );
        // NOT VISIBLE, or GONE: parked.
        assert_eq!(step(Some("eDP-1"), None), Step::Park);
        assert_eq!(step(None, None), Step::Stay);
        // THE NAME COMES BACK — `TAB 3` makes "3" again — and so does it.
        let back = [space("3", true, Some("eDP-1"))];
        assert_eq!(
            step(None, wanted("3", &back).as_deref()),
            Step::Show("eDP-1".to_owned())
        );
    }

    /// FAILS IF: an opening stops reading its tenant, its workspace (a name
    /// with spaces whole) or its size, stops defaulting a bare `column` to
    /// the player, or reads a word it does not know as a default.
    #[test]
    fn an_opening_is_read_and_an_unknown_word_refused() {
        let bare = Opening::parse::<&str>(&[]).expect("bare");
        assert_eq!(
            bare,
            Opening {
                tenant: crate::right_column::Tenant::Player,
                on: None,
                size: None,
            }
        );
        let full = Opening::parse(&["player", "--on", "mail box", "--size", "bar"]).expect("full");
        assert_eq!(full.on.as_deref(), Some("mail box"));
        assert_eq!(full.size, Some(Size::Bar));
        assert_eq!(
            Opening::parse(&full.args()[1..]).expect("its own words"),
            full
        );
        for refused in [
            &["somebody"][..],
            &["--size", "huge"],
            &["--size"],
            &["--on"],
            &["--on", ""],
            &["player", "player"],
            &["--wide"],
        ] {
            assert!(Opening::parse(refused).is_err(), "{refused:?}");
        }
    }

    /// FAILS IF: `C-<left>`, `C-<right>` and `C-q` stop reaching the column
    /// as the keyboard sends them — the header's click presses them through
    /// the same reader (`keys::event_of`) — or an id of the table does
    /// nothing of its own, and falls into another key's act.
    #[test]
    fn the_column_keys_are_read_as_they_are_sent() {
        let acts: Vec<Option<Own>> = OF_THE_COLUMN.iter().map(|(_, id)| Own::of(id)).collect();
        assert_eq!(
            acts,
            [Some(Own::Wider), Some(Own::Narrower), Some(Own::Close)]
        );
        for (notation, id) in OF_THE_COLUMN {
            let key = ronoctua_keymap::Key::parse(notation).expect(notation);
            let (event, mods) = crate::door::keys::event_of(key);
            let read = crate::door::keys::key_of(&event, mods).expect("a key");
            assert_eq!(
                OF_THE_COLUMN
                    .iter()
                    .find(|(n, _)| ronoctua_keymap::Key::parse(n).ok() == Some(read))
                    .map(|(_, i)| *i),
                Some(*id)
            );
        }
        // AND A BARE ARROW IS THE TENANT'S.
        let plain = ronoctua_keymap::Key::parse("<left>").expect("parses");
        assert!(
            OF_THE_COLUMN
                .iter()
                .all(|(n, _)| ronoctua_keymap::Key::parse(n).ok() != Some(plain))
        );
    }
}
