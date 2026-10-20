//! A FEATURE'S VIEW IN THE FOOTER'S REGION — the [`Region`] a feature writes,
//! the [`Doorway`] it acts through, and the slot host ([`Slots`]) the footer
//! keeps them in.
//!
//! # Why a trait, and not a field and a dozen methods of the footer
//!
//! A view of the footer was spread over a dozen places of one file: a field,
//! its opening, its arm in the command table, its branch of the keys, its
//! rank, its rows, its click, its cursor, its hint, its errands. So a line of
//! the installer stick's view was a line of the footer, and every view
//! recompiled with every other. A region is that view as ONE value the footer
//! holds by its id and asks the same questions of — its rows, its hint, its
//! cursor; a key, a command, a click, an answer, a paste, a drop — and the
//! footer names no feature to ask them.
//!
//! # Why a doorway, and not the footer itself
//!
//! What a view does — say a sentence, ask a question, open a program, close —
//! is the same in both of its doors (the owner's rule: the two doors do the
//! same things). The window's door is [`crate::door::Ui`], the footer's is a
//! [`Recorder`] that only WRITES DOWN what was asked ([`Effect`]) while the
//! region is borrowed, and does it once the region is back in its slot: the
//! footer is never re-entered from inside a region, and the precedence is
//! never asked in the middle of a call. One errand handler of a view, written
//! once over a `&mut dyn Doorway`, is then the same handler in both doors —
//! "the two doors do the same" becomes a fact of the types.
//!
//! # Why the host is generic
//!
//! The footer keeps every region the binary registered ([`Entry`]) in
//! [`Slots`], open or closed, and its only knowledge of one is the id it is
//! ranked under in the footer's precedence: a region registered without a
//! rank, or a rank no region answers, is refused when the footer starts.
//!
//! # Why a region may live hidden
//!
//! Some of a view's work outlives its page: a root gesture of the security
//! centre's asked from the leader with the page closed waits in the sudo bar,
//! and its verdict is said on the footer's line when it comes; a run of the
//! update asked for by hand is followed in the brain's mind, and its question
//! opens the page when the brain holds it. Such a command brings the region
//! to life HIDDEN ([`Closed::Behind`]) — unranked, undrawn, holding no
//! keyboard — and the host keeps it while it says it LINGERS
//! ([`Region::lingers`]): its threads are still heard, its question still
//! answered, and it asks for the page itself ([`Doorway::show`]) when what
//! came is to be seen. A region that closed with work in flight goes hidden
//! the same way, instead of dropping what it waits for.
//!
//! A hidden region that does not take a command of its own — a closed view
//! that only has its work's answers left to say, not a page to bring back —
//! is as closed as a closed one: its entry answers the command
//! ([`Entry::closed`]), and one that opens the view finds its slot free, the
//! hidden one STEPPED ASIDE ([`Slots::set_aside`]): kept beside its slot and
//! told what its threads bring ([`Region::woken`]) until it has nothing out.
//!
//! # Why a region reads the machine through the footer's lines
//!
//! The footer is alive for the whole session and keeps lines to the machine
//! for its own sake — the battery's mark and its alarms, the sound's two
//! marks and the volume keys, the Syncthing mark, a new mouse's cursor. A
//! view of the footer shown as a region reads the SAME lines ([`Kept`],
//! [`Doorway::kept`]): the last reading the footer holds and the line it
//! asks a daemon through, lent by type. So a region opens with what the
//! footer already knows, asks through the footer's thread — whose refusals
//! the footer already says — and never opens a second connection to the same
//! daemon; and a command that works with the view closed (pause every
//! folder, keep the machine awake) reaches the same line while no region is
//! open at all. A window keeps lines of its own and lends nothing.
//!
//! # Why a feature may keep a resident
//!
//! Some of a feature's state outlives its region and is no reading the
//! footer keeps for its own sake: the player's last state and its queue's
//! outline, which a command run with the view closed reads; a question half
//! answered; a scan asked with the view closed. That is the feature's
//! RESIDENT ([`Resident`], registered beside its entry, [`Residing`]): made
//! once when the footer starts, it hears what the footer heard and the
//! region's wake while the region is closed, and the open region holds the
//! same value ([`Residence`], [`Doorway::residence`]) — no copy to go stale.
//! It is not a [`Kept`]: what the footer keeps of the machine is the
//! footer's, lent by type to every region; a resident is one feature's, and
//! only its region and its closed entry read it.

use std::any::{Any, TypeId};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use smithay_client_toolkit::seat::pointer::AxisScroll;

/// Whether a region took what it was handed — a key, a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// It was the region's, and the region did what it does.
    Yes,
    /// Not the region's: whoever asked goes on asking.
    No,
}

impl Taken {
    /// `Yes` when `took`.
    #[must_use]
    pub const fn of(took: bool) -> Self {
        if took { Self::Yes } else { Self::No }
    }

    /// Whether it was taken.
    #[must_use]
    pub const fn took(self) -> bool {
        matches!(self, Self::Yes)
    }
}

/// How a region lays itself on the footer's lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// Rows, one under the other, paged by the footer — every list.
    #[default]
    List,
    /// The whole ceiling, laid out by the region itself (a repository's
    /// history: columns side by side).
    Whole,
    /// The film's stage: the one thing let past the footer's ceilings, to
    /// the share of the screen the footer's own token says
    /// (`expansion.playerStagePct`) — never a number a region chooses
    /// (invariant 9).
    Stage,
    /// Blank rows the region draws its own cells over ([`Region::paint`]) —
    /// a grid of glyphs, a search and its pictures.
    Cells,
    /// Columns side by side, each its own list with its own scroll and bar,
    /// a strip pinned under them, pictures in its panels — laid onto the
    /// lines by the footer's own arithmetic, which asks the region what each
    /// part is ([`Region::sections`], [`crate::sections::Sectioned`]).
    Sections,
    /// Rows in the footer's own two columns ([`Pair`]), laid by the footer's
    /// grid: the colon at the widest key of the whole list, as the
    /// shortcuts are — the manuals.
    Pairs,
}

/// What the pointer resting on a row lights ([`Region::lit`]) — what a
/// click there takes, and nothing else (invariant 19: the pointer lights
/// what the click takes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lit {
    /// The whole row — every list's.
    #[default]
    Row,
    /// Only the cells `[from, to)` of it, counted as its fields are: the
    /// stretch a click there takes — one kind of a strip, one tab.
    Cells(u16, u16),
    /// Nothing: a click there takes nothing.
    Nothing,
}

/// Where a region is on the footer's surface, in the surface's pixels and
/// lines — what it paints at and what a pointer is measured against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// The surface's width.
    pub width: f32,
    /// The surface's bottom edge.
    pub bottom: f32,
    /// The surface's line the region starts on.
    pub first: u16,
    /// How many lines the region has.
    pub lines: u16,
    /// How many cells of text a line holds, net of both margins.
    pub columns: u16,
}

impl Area {
    /// The surface's grid, counted from the region's first line — what
    /// every line and cell of it is placed with (`chrome::Frame`).
    #[must_use]
    pub const fn frame(&self) -> crate::chrome::Frame {
        crate::chrome::Frame::of(self.width, self.bottom, self.first)
    }
}

/// What a region is drawn at: the words, the width and height it has, the
/// zone its dates are in, and whether it is a photograph's.
pub struct Seen<'a> {
    /// The words, in the language chosen.
    pub texts: &'a ronoctua_i18n::Texts,
    /// Cells of text a line holds.
    pub columns: u16,
    /// Lines the region may hold: the ceiling — and, for a region laid out
    /// whole ([`Shape::Whole`]), the page it is drawn on, net of the
    /// explanation's row.
    pub lines: u16,
    /// The zone a date is said in.
    pub zone: &'a ronoctua_files::when::Zone,
    /// A photograph's region reaches nothing of the machine.
    pub photographing: bool,
    /// What the footer holds and lends the region to read ([`Lent`]).
    pub lent: Lent<'a>,
}

/// One row a region draws: its pieces at their cells, and whether a click
/// on it is the region's ([`Region::clicked`]).
#[derive(Debug, Clone, Default)]
pub struct Row {
    /// The pieces, left to right, already at their cells.
    pub fields: Vec<crate::fields::Field>,
    /// A click on it selects it, and a second acts — invariant 19.
    pub takes: bool,
}

/// A ROW AS TOLD TO THE FOOTER ([`Region::told`]): the row, what it does
/// said under the pointer, and — for a row that is no table of fields — the
/// footer's own two columns.
///
/// # Why beside the row, and not in it
///
/// Because most regions draw rows of fields and nothing more, and write a
/// [`Row`] whole; what only some say comes with the row where they build it,
/// and the rest say nothing by default ([`Told::from`]).
#[derive(Debug, Clone, Default)]
pub struct Told {
    /// The row: its fields — none, for a [`Pair`] — and whether it takes a
    /// click.
    pub row: Row,
    /// What it does, said under the pointer — written where the row is
    /// built, which already holds what it is about: asked row by row
    /// ([`Region::about`]), a list would be laid out once per row.
    pub about: Option<String>,
    /// The row in the footer's own two columns, when it is not a table of
    /// fields ([`Pair`]).
    pub pair: Option<Pair>,
    /// A picture drawn in a cell of the row, once it is here ([`RowPicture`]).
    pub picture: Option<RowPicture>,
    /// The colour CHIPS on it, by the cell each starts at in the row's own
    /// cells: the ground under each glyph is `chrome::chip`.
    pub chips: Vec<u16>,
    /// The cell a TEXT CARET stands before, on a line of a text being
    /// edited (`chrome::caret`) — said with the row by a region whose rows
    /// are built whole; one whose rows are cheap to ask one at a time
    /// answers [`Region::caret`] instead.
    pub caret: Option<u16>,
    /// The PICTURES over this line — a cover, a thumbnail: each drawn once,
    /// from its first line on screen, cut where the scroll cuts its lines
    /// ([`cover_place`]), out of [`Region::cover_picture`].
    pub covers: Vec<Cover>,
    /// The line CONTINUES the row above it — a row of several lines: the
    /// pointer lights the row whole, as a click takes it whole.
    pub continues: bool,
    /// What the pointer lights on it, said with the row ([`LineLights`]) —
    /// beside [`Region::lit`], which a region answers for the cell under
    /// the pointer.
    pub lights: LineLights,
    /// In a grid's row, the cell the CURSOR is on, `(from, width)` in the
    /// row's own cells: it wears `surface.selection`, as the file manager's
    /// cursor does.
    pub chosen: Option<(u16, u16)>,
    /// The row this row's click lands on, when it is a line of a control of
    /// several — said with the row by a region that lays every row out to
    /// tell them, so the footer does not ask [`Region::control_of`] row by
    /// row, which lays them all out again for each. `None`: it asks.
    pub control: Option<usize>,
}

impl From<Row> for Told {
    /// A row that says nothing more than its fields.
    fn from(row: Row) -> Self {
        Self {
            row,
            ..Self::default()
        }
    }
}

/// A PICTURE OVER A REGION'S ROWS — a cover, a thumbnail — on one of its
/// lines: placed ONCE, when the rows are built, so every door fits it in the
/// same box ([`cover_place`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    /// Its line of the picture: `0` is the top.
    pub line: u16,
    /// How many lines the picture's box takes.
    pub lines: u16,
    /// The box's first cell, as a field's `at`.
    pub left: u16,
    /// How many cells wide the box is.
    pub cells: u16,
    /// The key the region holds the picture under ([`Region::cover_picture`]).
    pub key: std::rc::Rc<str>,
    /// The picture's own size, in pixels: its proportion.
    pub size: (u32, u32),
}

/// What the pointer lights on a line of a region, said with the row
/// ([`Told::lights`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LineLights {
    /// The band every row of a list lights.
    #[default]
    Row,
    /// Nothing: a line of a figure — a picture, a schematic — or a title.
    Nothing,
    /// The ONE cell under the pointer's `x` of these `(from, width)` cells of
    /// the row (`chrome::Across`), and nothing between two cells — a grid's
    /// row, lit as its click takes it.
    UnderX(Vec<(u16, u16)>),
    /// The cell of these holding the pointer's CELL — a shelf of pictures,
    /// lit over the shelf's lines.
    UnderCell(Vec<(u16, u16)>),
    /// From this cell of the row to the band's end — a line beside a figure:
    /// its words light, wherever on the row the pointer rests.
    From(u16),
}

/// The box a picture of `size` pixels fits in, `cells` by `lines` of
/// `cell_px` by `line_px`, at the picture air's distance from the box's
/// edges — the part of the box from line `first` to `end` drawn, as the
/// picture's own fractions of its height `(top, bottom)`. `None` when
/// nothing of it shows. Every door's drawing of a picture over rows.
#[must_use]
pub fn cover_place(
    (cells, lines): (u16, u16),
    (cell_px, line_px): (f32, f32),
    (width, height): (u32, u32),
    (first, end): (u16, u16),
) -> Option<(ronoctua_render::image::Area, (f32, f32))> {
    #[allow(clippy::cast_precision_loss)]
    let aspect = width as f32 / (height.max(1) as f32);
    let air = crate::sizes::picture_air();
    let (box_w, box_h) = (
        f32::from(cells) * cell_px,
        (f32::from(lines) * line_px - air - air).max(0.0),
    );
    let (mut w, mut h) = (box_h * aspect, box_h);
    if w > box_w {
        (w, h) = (box_w, box_w / aspect.max(f32::MIN_POSITIVE));
    }
    let (top, bottom) = (
        f32::from(first) * line_px,
        f32::from(end.min(lines)) * line_px,
    );
    let (from, to) = (top.max(air), bottom.min(air + h));
    if to <= from || w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some((
        ronoctua_render::image::Area {
            x: 0.0,
            y: from,
            width: w,
            height: to - from,
        },
        ((from - air) / h, (to - air) / h),
    ))
}

/// `rgba`'s rows from `top` to `bottom` (fractions of its height) — the part
/// of a picture on screen when its rows are cut by the scroll.
#[must_use]
pub fn cropped(
    rgba: &ronoctua_render::image::Rgba,
    (top, bottom): (f32, f32),
) -> Option<ronoctua_render::image::Rgba> {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let row = |f: f32| ((f.clamp(0.0, 1.0) * rgba.height as f32).round() as u32).min(rgba.height);
    let (from, to) = (row(top), row(bottom));
    if to <= from {
        return None;
    }
    let stride = rgba.width as usize * 4;
    let pixels = rgba.pixels[from as usize * stride..to as usize * stride].to_vec();
    ronoctua_render::image::Rgba::new(rgba.width, to - from, pixels)
}

/// A row in the footer's own TWO COLUMNS — a key and what it says — for a
/// region whose rows are no table of fields: a shortcut and what it does, a
/// manual's name and its sentence, a gesture beside its drawing.
///
/// # Why the footer lays it, and not the region
///
/// Because the colon of a list of keys is one column for the whole list —
/// the widest key's — and the footer's grid is what measures it
/// ([`Shape::Pairs`]), the same arithmetic the shortcuts' list has. A region
/// laying its own colon would be a second grid, and the two lists side by
/// side in a photograph would disagree by a cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    /// The left column — a key, a name. Empty, the text takes the line.
    pub key: String,
    /// The right column.
    pub text: String,
    /// The key's tone.
    pub key_tone: ronoctua_tokens::Color,
    /// The text's tone.
    pub text_tone: ronoctua_tokens::Color,
    /// Which characters of the key a query matched, to light.
    pub lit: Vec<usize>,
    /// The cursor's row: what matched takes the cursor's own tone there.
    pub here: bool,
    /// The key flush RIGHT in its column (a shortcut, the colons in line), or
    /// left (a name).
    pub flush_right: bool,
    /// The `:` between the two columns — where the left one leads to the
    /// right one.
    pub colon: bool,
    /// The text stays in the RIGHT column with no key — beside a drawing.
    pub aside: bool,
    /// A drawing in the key column, from this line down ([`Picture`]).
    pub picture: Option<Picture>,
    /// The line sits beside a drawing, so its key column is the drawing's
    /// width.
    pub beside_picture: bool,
}

impl Default for Pair {
    /// The footer's standard line: a key in the key's tone, its text in the
    /// secondary one, flush right with its colon.
    fn default() -> Self {
        Self {
            key: String::new(),
            text: String::new(),
            key_tone: ronoctua_tokens::color::TEXT_KEY,
            text_tone: ronoctua_tokens::color::TEXT_SECONDARY,
            lit: Vec::new(),
            here: false,
            flush_right: true,
            colon: true,
            aside: false,
            picture: None,
            beside_picture: false,
        }
    }
}

/// A DRAWING a [`Pair`] puts in the key column: the key its pixels go up
/// under ([`Region::picture`] rasterises them), and the box it is fitted in,
/// in cells and lines — drawn only while every one of its lines is on
/// screen, since nothing clips there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The key of its pixels: what it shows, its size and the palette.
    pub key: String,
    /// The box's width, in cells.
    pub cells: u16,
    /// The box's height, in lines — the lines beside it.
    pub lines: u16,
}

/// WHAT THE FOOTER HOLDS AND LENDS a region for one call, by type — the
/// queue its band draws, the studio's state its marks read, the cameras
/// plugged in, the jobs of the transfer queue.
///
/// # Why lent, and not told
///
/// Because a region that kept its own copy of what the footer already holds
/// would keep a second copy, and the second copy goes stale: the manager of
/// the notifications holds a cursor, never the records (`notifications.rs`).
/// What changes is told ([`Region::heard`]); what is READ is lent, for the
/// call, and read where it lives. A type the footer does not lend reads as
/// `None`.
///
/// # Why beside [`Kept`], and not one of them
///
/// Because they lend different things: [`Kept`] is what the footer keeps of
/// the MACHINE for the session — a reading, the line to a daemon — shared by
/// `Rc` and lent to the gestures ([`Doorway::kept`]); this is the footer's
/// own fields BORROWED for one call, the questions ([`Seen::lent`]) and the
/// painting ([`RegionEasel::lent`]) included, with nothing to share and
/// nothing to clone.
#[derive(Clone, Default)]
pub struct Lent<'a> {
    held: Vec<&'a (dyn Any + 'static)>,
}

impl std::fmt::Debug for Lent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lent")
            .field("held", &self.held.len())
            .finish()
    }
}

impl<'a> Lent<'a> {
    /// Nothing lent — a window, which holds its own.
    #[must_use]
    pub const fn none() -> Self {
        Self { held: Vec::new() }
    }

    /// These, lent.
    #[must_use]
    pub const fn of(held: Vec<&'a (dyn Any + 'static)>) -> Self {
        Self { held }
    }

    /// The lent value of type `T`, when one is.
    #[must_use]
    pub fn get<T: Any>(&self) -> Option<&'a T> {
        self.held.iter().find_map(|h| {
            let held: &'a (dyn Any + 'static) = *h;
            held.downcast_ref::<T>()
        })
    }
}

/// A LIST INSIDE A REGION'S LIST — a band of its rows with a ground, a bar
/// and a pointer's band of its own (the network view's shelves).
///
/// What the footer paints and points at for it is the footer's, as the
/// region's own bar is: a ground under the rows the band shows, the pointer's
/// band cut to the box, and a bar at the box's edge while the band is wholly
/// on screen and holds more than it shows — grabbed where it is drawn, and
/// dragged into [`Region::shelf_scrolled`]. The region says only where each
/// band is and what it holds ([`Region::shelves`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shelf {
    /// Which of the region's bands — its own number, handed back when the
    /// band's bar is dragged.
    pub id: usize,
    /// The row of the region's list the band starts at.
    pub first: usize,
    /// How many rows of the list it takes.
    pub lines: usize,
    /// How many rows it holds.
    pub total: usize,
    /// The first of them it shows.
    pub offset: usize,
}

/// A file every door that draws it watches, just WRITTEN by a view
/// ([`Doorway::written`]): the door takes it now, so the frame after the
/// gesture is already drawn the new way — its watch rings a moment later and
/// finds nothing new.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// The language (`language.json`, `crate::language`).
    Language,
    /// The places the glance draws (`ronoctua_world::prefs`), which only the
    /// footer's door draws beside a view.
    Places,
}

/// A row's picture ([`Told::picture`]): the texture's key
/// ([`Region::held_picture`] has its pixels) and the cell it is drawn in —
/// the glyph's, which the region left out of the row's fields the moment the
/// picture took it: the glyph until then, never both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPicture {
    /// The texture's key; it carries the side the picture was decoded at.
    pub key: String,
    /// The cell it is drawn in, counted as the fields' are.
    pub at: u16,
}

/// A picture a region paints itself ([`RegionEasel::pictures`]): where it
/// goes and where its pixels come from. (A drawing a [`Pair`] puts in its
/// key column is a [`Picture`].)
#[derive(Debug, Clone)]
pub struct Painted {
    /// The texture's key.
    pub key: String,
    /// Where it goes, in logical pixels.
    pub area: ronoctua_render::image::Area,
    /// Its pixels.
    pub source: Source,
}

/// Where a painted picture's pixels come from.
#[derive(Clone)]
pub enum Source {
    /// Pixels at hand, put up when the key is missing.
    Still(std::sync::Arc<ronoctua_render::image::Rgba>),
    /// Pixels that change under the same key (a film playing): put up every
    /// frame.
    Moving(std::sync::Arc<ronoctua_render::image::Rgba>),
    /// Pixels drawn when the key is missing — an SVG rasterised — or none,
    /// when it did not draw.
    Drawn(std::sync::Arc<dyn Fn() -> Option<ronoctua_render::image::Rgba> + Send + Sync>),
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Still(_) => "Still",
            Self::Moving(_) => "Moving",
            Self::Drawn(_) => "Drawn",
        })
    }
}

impl Painted {
    /// The pixels, when they are to go up: a moving picture's always, a
    /// still or a drawn one's when its texture is `missing`.
    #[must_use]
    pub fn pixels(
        &self,
        missing: bool,
    ) -> Option<std::borrow::Cow<'_, ronoctua_render::image::Rgba>> {
        match &self.source {
            Source::Moving(r) => Some(std::borrow::Cow::Borrowed(r.as_ref())),
            Source::Still(r) if missing => Some(std::borrow::Cow::Borrowed(r.as_ref())),
            Source::Drawn(draw) if missing => draw().map(std::borrow::Cow::Owned),
            _ => None,
        }
    }
}

/// What a pointer did over a region that lays out its own cells
/// ([`Shape::Cells`], [`Region::pointer`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pointing {
    /// It came in, or moved.
    Moved,
    /// It left the surface.
    Left,
    /// A button went down — its `BTN_*` code.
    Pressed(u32),
    /// A button came up.
    Released(u32),
    /// The wheel, or a touchpad's two fingers.
    Wheel {
        /// Up and down.
        vertical: AxisScroll,
        /// Left and right.
        horizontal: AxisScroll,
    },
}

/// Which of the bars a region draws itself a pointer took
/// ([`Region::grab`]) — the region's own number for it.
///
/// THE POINTER'S, not the region's: every seat has a pointer of its own, and
/// two of them may hold two bars at once (an open notification has one per
/// half). The footer keeps the grip with the pointer that pressed and hands
/// it back with each of that pointer's motions ([`Region::drag`]), so each
/// drag keeps answering the bar it started on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grip(pub usize);

/// One pointer event, where it happened on the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointed {
    /// The surface's pixels.
    pub at: (f64, f64),
    /// What it was.
    pub pointing: Pointing,
}

/// Where a pointer is over a region that lays itself out as rows, as the
/// footer measured it — what a click, a hover and a drag of the region's
/// own controls are read against ([`Region::hovered`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Under {
    /// The surface's pixels.
    pub at: (f64, f64),
    /// The line of the region's page it is on — `0` the region's first.
    pub line: Option<usize>,
    /// The region's row on that line.
    pub row: Option<usize>,
    /// The cell of the row under it (`chrome::FIELDS_LEAD` taken off).
    pub cell: Option<u16>,
    /// The grid column's first cell the line is drawn from.
    pub base: u16,
    /// `Ctrl` is held.
    pub ctrl: bool,
    /// `Shift` is held.
    pub shift: bool,
    /// The surface's grid, counted from the region's first line — `None`
    /// before a frame was drawn.
    pub frame: Option<crate::chrome::Frame>,
}

/// One line of a region as the footer placed it on its page this frame: the
/// region's row, the surface's line it is on and the grid column's first
/// cell — what a region that paints over its rows paints at
/// ([`Region::paint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// The region's row.
    pub row: usize,
    /// The surface's line.
    pub line: u16,
    /// The grid column's first cell.
    pub base: u16,
}

/// The screen a frame is painted on ([`RegionEasel::screen`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// Its connector, when the compositor named it.
    pub connector: Option<String>,
    /// What its pictures are keyed by: the connector, or its place in the
    /// list while it has none (`screen-<n>`).
    pub key: String,
    /// Physical pixels per logical one.
    pub scale: u32,
}

/// What the footer measured of a frame before it is painted
/// ([`Region::laid_out`]): where the region is, its page, the screens.
#[derive(Debug, Clone)]
pub struct Laid {
    /// The surface's line the region starts on.
    pub first: u16,
    /// How many lines the region has.
    pub lines: u16,
    /// Cells of text a line holds, net of both margins.
    pub columns: u16,
    /// The region's page: its first row on screen…
    pub offset: usize,
    /// …and how many rows are on screen.
    pub room: usize,
    /// Whether the region is drawn at all — with the bars hidden it may be
    /// out of sight.
    pub shown: bool,
    /// Whether it is the view the footer shows — an open region under
    /// another is laid out too, and shows nothing.
    pub ranked: bool,
    /// The screens it is drawn on, in order.
    pub screens: Vec<Screen>,
    /// The surface's grid, counted from the region's first line.
    pub frame: Option<crate::chrome::Frame>,
    /// Where the pointer rests, when it does.
    pub pointer: Option<(f64, f64)>,
}

/// What a region paints over its rows ([`Region::paint`]): the bands under
/// the text, what goes over the pictures, its own spans of text and the
/// tracks of the bars it draws (`crate::bars`, which the pointer reads).
pub struct RegionEasel<'a> {
    /// Where the region is.
    pub area: Area,
    /// The words.
    pub texts: &'a ronoctua_i18n::Texts,
    /// The first of the region's rows on the page — what a picture placed
    /// among them is measured from (the clipboard's preview box).
    pub offset: usize,
    /// Bands, rules and thumbs, under the text.
    pub under: &'a mut Vec<ronoctua_render::quad::Quad>,
    /// What is painted after the pictures.
    pub over: &'a mut Vec<ronoctua_render::quad::Quad>,
    /// Text the region places itself.
    pub spans: &'a mut Vec<ronoctua_render::Span>,
    /// Every bar's track, recorded where it was drawn.
    pub bars: &'a mut crate::bars::Drawn,
    /// What the footer holds and lends the region to read ([`Lent`]).
    pub lent: Lent<'a>,
    /// Every hand's colour, and what the region named the target that hand
    /// rests on ([`Region::rests_on`]): each is lit in its own colour.
    pub lit: &'a [(ronoctua_tokens::Color, usize)],
    /// The pictures it paints, put up by the footer — a still one when its
    /// texture is missing, a moving one every frame — and drawn in order.
    pub pictures: &'a mut Vec<Painted>,
    /// Its lines on this frame's page, where the footer placed them.
    pub placed: &'a [Placed],
    /// The screen this frame is painted on — what a picture's key names
    /// when the same picture is drawn on several.
    pub screen: Screen,
    /// What the pointer of this screen holds over the region
    /// ([`Region::hovered`]).
    pub hand: Option<&'a dyn Any>,
}

/// What the pointer lights on a row of a region laid out by itself
/// ([`Region::band`]) — beside [`Lit`], which says it in the row's own
/// cells: a region laid out whole knows its bands in the surface's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Band {
    /// The footer's band across the row, or what [`Region::lit`] says of
    /// it — every list's.
    Row,
    /// A band the region placed itself, in the surface's pixels — x, y,
    /// width and height (a column of a repository's history).
    Placed(crate::chrome::Rect),
    /// Nothing: a press there takes nothing.
    Nothing,
}

/// What the pointer lights among a region's fields ([`Region::lights`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lights {
    /// The field under it, and the runs of the same thing with it — every
    /// list's.
    #[default]
    Field,
    /// Only the runs of the same thing: the field under the pointer is not
    /// lit apart, because the region's accent is its cursor (the
    /// calculator's mode, its ring's category).
    Kin,
    /// Nothing: the region's pointer mark is its band alone (a repository's
    /// history, whose cursor is the accent).
    Nothing,
}

/// WHAT A THREAD OF A VIEW HANDS A COMMAND BACK THROUGH, to be run on the
/// door's loop later — a clone that ended comes back as its own command with
/// the answer as its argument ([`Doorway::orders`]). A thread cannot touch
/// the door; it can leave an order for it.
#[derive(Clone)]
pub struct Orders(Arc<dyn Fn(&'static str, String) + Send + Sync>);

impl Orders {
    /// Orders carried by `f` — the door's way of running a command later.
    #[must_use]
    pub fn new(f: impl Fn(&'static str, String) + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// Orders that reach nothing, and say so in the log: a door with no
    /// loop to run them on (a photograph's, a test's).
    #[must_use]
    pub fn nowhere() -> Self {
        Self::new(|command, _| {
            tracing::warn!(command, "an order for a door with no loop; not run");
        })
    }

    /// Runs `command` with `argument` on the door's loop, later.
    pub fn run(&self, command: &'static str, argument: String) {
        (self.0)(command, argument);
    }
}

impl std::fmt::Debug for Orders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Orders")
    }
}

/// The rows a door shows now: the first of them, and how many fit
/// ([`Doorway::page_shown`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Page {
    /// The first row on the page.
    pub offset: usize,
    /// How many rows the page holds.
    pub lines: usize,
}

/// A FEATURE'S VIEW in the footer's region — see the module.
///
/// Three questions are the region's to answer — its rows, its hint and its
/// cursor — and four gestures — a key, a command, a click and its name.
/// Everything else has a default that is the footer's standard, and a region
/// that wants otherwise overrides that one method.
pub trait Region {
    /// The id it is ranked under in the footer's precedence — `stick`.
    fn id(&self) -> &'static str;
    /// Its rows, as the footer's one painter draws them.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row>;
    /// Its rows from `skip`, at most `take` of them, as TOLD to the footer
    /// ([`Told`]) — each with what it says under the pointer and, when it is
    /// no table of fields, the footer's own two columns: what the footer
    /// draws. By default [`Region::rows_from`], each saying nothing more;
    /// `take` is a ceiling, never a count.
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Told> {
        self.rows_from(seen, skip, take)
            .into_iter()
            .map(Told::from)
            .collect()
    }
    /// The keys its hint line teaches (`chrome::with_the_way_back` is the
    /// footer's to add).
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece>;
    /// The row the cursor is on, which the footer's page follows.
    fn cursor(&self) -> Option<usize>;
    /// What row `row` does, said under the pointer.
    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        let _ = (row, seen);
        None
    }
    /// A key, while the region holds the keyboard. `No` swallows it all the
    /// same — a modal view's wrong key loses nothing — and only says so.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken;
    /// A command of the registry, open or not (the leader's, `LEADER :`'s,
    /// a window's order): `No` when it is not one of the region's.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken;
    /// Row `row` was clicked.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway);
    /// Row `row` was clicked at cell `cell` of the line, counted from the
    /// region's left margin as the row's fields are — for a row whose halves
    /// sit side by side (a device's page of the radio), or a row of several
    /// stretches (a strip of kinds, a row of tabs), where the cell says which
    /// one the hand meant. The row alone ([`Region::clicked`]) by default.
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let _ = cell;
        self.clicked(row, way);
    }
    /// What the pointer resting on row `row`, at `cell`, lights — the
    /// stretch a click there takes ([`Region::clicked_at`] reads the same
    /// answer), and the whole row by default.
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Lit {
        let _ = (row, cell, seen);
        Lit::Row
    }
    /// The question it asked ([`Doorway::ask`]) was answered.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let _ = (id, text, way);
    }
    /// The question it asked was let go.
    fn unanswered(&mut self, id: &'static str, way: &mut dyn Doorway) {
        let _ = (id, way);
    }
    /// The clipboard's text it asked for ([`Doorway::paste`]).
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let _ = (text, way);
    }
    /// Whether files dragged over it may land — what the drag's cursor says
    /// before the drop ([`Region::dropped`]).
    fn takes_drops(&self) -> bool {
        false
    }
    /// Files dropped on it — `copies` when the drag asked for a copy.
    /// Whether it took them.
    fn dropped(&mut self, paths: &[PathBuf], copies: bool, way: &mut dyn Doorway) -> bool {
        let _ = (paths, copies, way);
        false
    }
    /// A thread it handed a [`Doorway::wake`] to woke the loop.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let _ = way;
    }
    /// What the footer heard from the machine — a reading of the network,
    /// of the power, of the sound — for whoever knows its type: the footer
    /// tells it by the type it names (`hosted_heard::<T>`), and a region
    /// reads it by the type it downcasts to, which the compiler cannot pair
    /// (`regions-hear-what-the-footer-tells` does). A region that took it
    /// says what changed: [`Doorway::rows_changed`] when rows came or went,
    /// [`Doorway::repaint`] when only what a row says did.
    fn heard(&mut self, news: &dyn Any, way: &mut dyn Doorway) {
        let _ = (news, way);
    }
    /// How it is laid on the lines.
    fn shape(&self) -> Shape {
        Shape::List
    }
    /// What it paints over its rows itself — pictures, a keypad, a globe.
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let _ = easel;
    }
    /// The pixels of a row's picture ([`RowPicture`]) it HOLDS, by key, in a
    /// bounded pool of its own — lent when the footer puts the texture up,
    /// never copied into the footer's map (a drawing rasterised once into
    /// that map is [`Region::picture`]). The pictures of the rows on screen
    /// are asked for before the frame ([`Region::before_frame`]).
    fn held_picture(&self, key: &str) -> Option<&ronoctua_render::image::Rgba> {
        let _ = key;
        None
    }
    /// What the minibuffer's line says while the region is shown — what was
    /// typed into its search, as the launcher's query is drawn there — or
    /// `None` to leave the line to the keys it teaches.
    fn prompt(&self) -> Option<String> {
        None
    }
    /// What the hand whose frame [`Region::pointer`] just took rests on, for
    /// a region that lays out its own cells: `Some(Some(code))` a target the
    /// region named by `code` (handed back in [`RegionEasel::lit`]),
    /// `Some(None)` nothing of the region's, `None` a frame that did not move
    /// the hand — a press, a bar held — whose target stays what it was.
    fn rests_on(&self) -> Option<Option<usize>> {
        None
    }
    /// What the pointer rests on has to add, whole, on the explanation line
    /// — a region that lays out its own cells, at `area`.
    fn about_the_pointer(&self, area: &Area, seen: &Seen<'_>) -> Option<String> {
        let _ = (area, seen);
        None
    }
    /// A frame of pointer events over a region that lays out its own cells.
    /// Whether it took the frame.
    fn pointer(&mut self, events: &[Pointed], area: &Area, way: &mut dyn Doorway) -> bool {
        let _ = (events, area, way);
        false
    }
    /// The footer's second, while it is open. Whether it changed.
    fn tick(&mut self, now: std::time::Instant, way: &mut dyn Doorway) -> bool {
        let _ = (now, way);
        false
    }
    /// The lists inside its list ([`Shelf`]) — none by default.
    fn shelves(&self, seen: &Seen<'_>) -> Vec<Shelf> {
        let _ = seen;
        Vec::new()
    }
    /// Band `id`'s bar was dragged: it shows from row `offset` of what it
    /// holds.
    fn shelf_scrolled(&mut self, id: usize, offset: usize, way: &mut dyn Doorway) {
        let _ = (id, offset, way);
    }
    /// How many lines the footer's region shows, told before a key reaches
    /// it: a file picker over the view walks half of them on `PgUp`/`PgDn`.
    fn page_is(&self, lines: usize) {
        let _ = lines;
    }
    /// Whether it SAYS when its rows came or went ([`Doorway::rows_changed`])
    /// — then a frame it asks for on what the footer heard
    /// ([`Doorway::repaint`]) is only a frame. One that does not (the
    /// default) has the footer's height measured again after every frame it
    /// asks for on news, in case its rows changed with what it heard.
    fn says_when_its_rows_change(&self) -> bool {
        false
    }
    /// Whether it SAYS what a wake of its threads brought
    /// ([`Region::woken`]): a wake after which it asked nothing of its door
    /// (but news for the footer, [`Doorway::news`]) brought nothing to draw,
    /// and costs neither a frame nor a geometry. One that does not (the
    /// default) is owed a frame after every wake: what came may have changed
    /// what it shows without its asking for one.
    fn says_what_a_wake_brought(&self) -> bool {
        false
    }
    /// A row it wants at the TOP of the region, once — a section's title
    /// after `TAB` — which beats following the cursor for one frame. Taken:
    /// asked again, it says `None`.
    fn take_anchor(&mut self) -> Option<usize> {
        None
    }
    /// The language changed — chosen in another door, or here: what it
    /// keeps in words is said again ([`Doorway::texts`] is the new one).
    fn language_changed(&mut self, way: &mut dyn Doorway) {
        let _ = way;
    }
    /// The dates' shape changed (`locale.json`), in another door above all.
    fn locale_changed(&mut self, way: &mut dyn Doorway) {
        let _ = way;
    }
    /// It is about to close: its threads go with it.
    fn closing(&mut self, way: &mut dyn Doorway) {
        let _ = way;
    }
    /// Whether it has work in flight that must still be HEARD once it is no
    /// longer shown — a root gesture waiting in the sudo bar, a check on a
    /// thread, a question on the minibuffer's line, a run it follows, a deed
    /// a closed view told and whose answer is still its door's to say. The
    /// host keeps such a region alive HIDDEN — unranked and undrawn — and
    /// lets it go once it does not; one that does not take a command that
    /// opens it steps aside for a fresh one ([`Slots::set_aside`], see the
    /// module).
    fn lingers(&self) -> bool {
        false
    }
    /// It is SHOWN now — opened, or a hidden one brought into view
    /// ([`Doorway::show`]): what it reads only while it is on screen (its
    /// scene, the folders it watches) starts here, and [`Region::closing`]
    /// stops it.
    fn shown(&mut self, way: &mut dyn Doorway) {
        let _ = way;
    }
    /// How many rows it has, WITHOUT building them — for a list too long to
    /// build on every frame and every pointer motion (an output of thousands
    /// of lines). The default counts [`Region::rows`].
    fn row_count(&self, seen: &Seen<'_>) -> usize {
        self.rows(seen).len()
    }
    /// Its rows from `skip`, at most `take` of them — only those built. The
    /// default cuts [`Region::rows`]; `take` is a ceiling, never a count.
    fn rows_from(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Row> {
        self.rows(seen).into_iter().skip(skip).take(take).collect()
    }
    /// The row whose CONTROL row `row` belongs to: the first row of a
    /// sentence wrapped over several, which are one control — the pointer
    /// on any of them lights them all, and a click on any takes the same
    /// thing. Itself by default.
    fn control_of(&self, row: usize, seen: &Seen<'_>) -> usize {
        let _ = seen;
        row
    }
    /// The row the cursor is on AS DRAWN at `seen` — what the footer's page
    /// follows — for a region whose rows wrap, or whose cursor counts only
    /// the rows one can stand on (titles, blank rows and notes that wrap with
    /// the width sit between them — the language view's), and so needs the
    /// width and the words to find it. [`Region::cursor`] by default.
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        let _ = seen;
        self.cursor()
    }
    /// What a region laid out in sections answers ([`Shape::Sections`]) —
    /// `None` for every other shape.
    fn sections(&self) -> Option<&dyn crate::sections::Sectioned> {
        None
    }
    /// The same, for a gesture of the pointer that moves the region's own
    /// state.
    fn sections_mut(&mut self) -> Option<&mut dyn crate::sections::Sectioned> {
        None
    }
    /// A frame is about to be drawn while it is shown: what its threads are
    /// to be asked for now — once a frame, which is what keeps every way the
    /// cursor can move from being a place to forget to ask.
    fn before_frame(&mut self, framing: &Framing<'_>) {
        let _ = framing;
    }
    /// The frame was painted: the pictures it handed up are up.
    fn after_paint(&mut self) {}
    /// The frame is drawn, or the hand turned what it shows: what the frame
    /// measured (a picture's box) is told to its threads.
    fn after_frame(&mut self) {}
    /// A clock of its own, faster than the footer's second, while it is shown
    /// and wants one — a name walking sideways. `None` drops it.
    fn clock(&self) -> Option<std::time::Duration> {
        None
    }
    /// That clock beat: whether what is drawn changed.
    fn clocked(&mut self, now: std::time::Instant) -> bool {
        let _ = now;
        false
    }
    /// The language changed: these are its words from now on.
    fn speak(&mut self, texts: std::rc::Rc<ronoctua_i18n::Texts>) {
        let _ = texts;
    }
    /// Whether it goes when the footer loses the keyboard — what it holds is
    /// a state a person built with it (a place, marks, an armed question),
    /// and a region on screen that answers no key is worse than none.
    fn closes_without_the_keyboard(&self) -> bool {
        false
    }
    /// Whether the door's line teaches its keys now ([`Region::hint`], with
    /// the way back in front) — `false` while what it shows is a line of its
    /// own (a filter being typed, a menu with its own line): the line then
    /// says what is typed, and teaches nothing.
    fn teaches(&self) -> bool {
        true
    }

    // THE QUESTIONS A REGION ANSWERS WITH WHAT IT WAS LENT OR MEASURED, each
    // with the footer's standard as its default: a region that has nothing
    // to say about one of them says nothing.

    /// What the thing under the pointer says, at cell `cell` of row `row` —
    /// for a row that is two records side by side, where the row alone
    /// cannot say ([`Told::about`] says it for a row that is one thing).
    fn about_at(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Option<String> {
        let _ = (row, cell, seen);
        None
    }
    /// What the row the KEYBOARD is on has to add — a switch that does
    /// nothing, and why — said on the line under the pointer's sentence.
    fn note(&self, seen: &Seen<'_>) -> Option<String> {
        let _ = seen;
        None
    }
    /// The keys of the drawings its rows show ([`Picture`]) that are to be
    /// up before a frame — rasterised once each ([`Region::picture`]).
    fn pictures(&self, seen: &Seen<'_>) -> Vec<String> {
        let _ = seen;
        Vec::new()
    }
    /// The pixels of drawing `key`.
    fn picture(&self, key: &str) -> Option<ronoctua_render::image::Rgba> {
        let _ = key;
        None
    }
    /// The cells of row `row` the pointer's band lights at cell `cell` —
    /// `(from, to)` in the row's own cells — when the band is narrower than
    /// the row: one of two lists side by side.
    fn hover_span(&self, row: usize, cell: u16, seen: &Seen<'_>) -> Option<(u16, u16)> {
        let _ = (row, cell, seen);
        None
    }
    /// The wheel over cell `cell` of row `row`, `lines` of it: whether the
    /// region scrolled something of its own (one of two halves) — the
    /// footer's page does not move then.
    fn wheel(&mut self, row: usize, cell: u16, lines: i64, way: &mut dyn Doorway) -> bool {
        let _ = (row, cell, lines, way);
        false
    }
    /// The text a sweep from cell `cell` of row `row` stays in, when it is
    /// narrower than the region — one half of two.
    fn text_area(
        &self,
        row: usize,
        cell: u16,
        area: &Area,
        seen: &Seen<'_>,
    ) -> Option<crate::text_select::Area> {
        let _ = (row, cell, area, seen);
        None
    }
    /// A press at `at`, on one of the bars the region draws itself
    /// ([`Region::paint`]): which one it took ([`Grip`]), `None` when the
    /// press was on none. The region keeps nothing of it: the footer keeps
    /// the grip for the POINTER that pressed, until its button comes up.
    fn grab(&mut self, at: (f64, f64), area: &Area, way: &mut dyn Doorway) -> Option<Grip> {
        let _ = (at, area, way);
        None
    }
    /// The pointer that holds bar `grip` is at `at`: whether that bar moved.
    fn drag(&mut self, grip: Grip, at: (f64, f64), area: &Area, way: &mut dyn Doorway) -> bool {
        let _ = (grip, at, area, way);
        false
    }
    /// The cell a text CARET stands before on row `row` — a line being typed
    /// (the calculator's expression), painted with `chrome::caret`.
    fn caret(&self, row: usize, seen: &Seen<'_>) -> Option<u16> {
        let _ = (row, seen);
        None
    }
    /// What the pointer lights over row `row` (`None`: a line that takes no
    /// click) at cell `cell`, for a region laid out by itself that places
    /// its own bands in the surface's pixels — [`Band::Row`], the footer's
    /// band and [`Region::lit`]'s answer, unless the region says otherwise.
    /// Asked by the painter only, never under the footer's geometry.
    fn band(&self, row: Option<usize>, cell: Option<u16>, area: &Area, seen: &Seen<'_>) -> Band {
        let _ = (row, cell, area, seen);
        Band::Row
    }
    /// What the pointer lights among the region's fields.
    fn lights(&self) -> Lights {
        Lights::Field
    }

    /// The picture it holds under `key`, the base of its texture's key and
    /// its pixels — what a [`Cover`] of its rows is drawn from.
    fn cover_picture(
        &self,
        key: &str,
    ) -> Option<&(String, std::sync::Arc<ronoctua_render::image::Rgba>)> {
        let _ = key;
        None
    }
    /// Whether it TYPES TEXT where Caps Lock means capitals — a text being
    /// edited. Everywhere else a letter is a key of the view.
    fn types_capitals(&self) -> bool {
        false
    }
    /// Whether a held button sweeps its rows' text (`crate::text_select`) —
    /// a list's; a region whose held button drags its own controls says no.
    fn sweeps_text(&self) -> bool {
        true
    }
    /// Row `row` was clicked, `under` where the pointer was — a grid's cell
    /// is taken by it. By default [`Region::clicked_at`], at the cell under
    /// the pointer.
    fn clicked_under(&mut self, row: usize, under: &Under, way: &mut dyn Doorway) {
        self.clicked_at(row, under.cell, way);
    }
    /// Whether the point is on a control of its own that no row is under —
    /// a picture beside the rows, its transport: no row lights there.
    fn owns_point(&self, at: (f64, f64), frame: Option<crate::chrome::Frame>) -> bool {
        let _ = (at, frame);
        false
    }
    /// The pointer moved over the surface — `hand` what this pointer holds
    /// over the region, kept with the pointer. Whether a frame is owed.
    fn hovered(
        &mut self,
        under: &Under,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        let _ = (under, hand, way);
        false
    }
    /// The pointer moved with something of the region's HELD — a bar, a
    /// band of a figure: whether one is.
    fn dragged(
        &mut self,
        under: &Under,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        let _ = (under, hand, way);
        false
    }
    /// The left button went down on one of its own controls: whether it was
    /// one (no row is clicked then).
    fn pressed(
        &mut self,
        under: &Under,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        let _ = (under, hand, way);
        false
    }
    /// The left button came up: what it held let go.
    fn released(&mut self, hand: &mut Option<Box<dyn Any>>, way: &mut dyn Doorway) {
        let _ = (hand, way);
    }
    /// The pointer left the surface: what it held or lit goes with it.
    fn left(&mut self, hand: &mut Option<Box<dyn Any>>, way: &mut dyn Doorway) {
        let _ = (hand, way);
    }
    /// Whether the wheel over `under` is its own — a control that turns with
    /// it: the footer counts the wheel's lines for it, and nothing else
    /// scrolls.
    fn takes_the_wheel(&self, under: &Under) -> bool {
        let _ = under;
        false
    }
    /// The wheel over one of its own controls — `lines` of it: whether it
    /// took it (a globe brought nearer).
    fn wheeled(
        &mut self,
        under: &Under,
        lines: i64,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        let _ = (under, lines, hand, way);
        false
    }
    /// What the message line says of what is under the pointer — a button,
    /// a control of a figure — before any row's sentence.
    fn said_under(&self, hand: Option<&dyn Any>, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let _ = (hand, texts);
        None
    }
    /// Whether its lines came or went since the last frame — a width, a
    /// state or a stage decides it — before the frame's shape is asked: the
    /// footer measures again. This and the frame's other hooks are told what
    /// the region SEES and reach no doorway ([`Slots::frame`]): a frame asks
    /// nothing of the footer.
    fn before_measuring(&mut self, seen: &Seen<'_>) -> bool {
        let _ = seen;
        false
    }
    /// The frame's layout, before it is painted ([`Laid`]): what it lays out
    /// itself is written down here, for the paint and the pointer.
    fn laid_out(&mut self, laid: &Laid, hand: &mut Option<Box<dyn Any>>, seen: &Seen<'_>) {
        let _ = (laid, hand, seen);
    }
    /// Every screen painted the frame — `presented` when one presented it.
    fn painted(&mut self, presented: bool, seen: &Seen<'_>) {
        let _ = (presented, seen);
    }
    /// After the frame: whether something under a pointer that did not move
    /// (`under`, measured against this frame) changed, and one more frame is
    /// owed — one round, never a loop.
    fn after_the_frame(
        &mut self,
        under: Option<&Under>,
        hand: &mut Option<Box<dyn Any>>,
        seen: &Seen<'_>,
    ) -> bool {
        let _ = (under, hand, seen);
        false
    }
    /// A thread it handed a [`Doorway::frame_wake`] to has a new picture:
    /// whether one more frame is owed — and nothing else is done.
    fn framed(&mut self) -> bool {
        false
    }
}

/// WHAT A FEATURE KEEPS IN THE FOOTER FOR THE WHOLE SESSION, its region open
/// or not ([`Residing`]): what a command asks while the region is closed — a
/// state its daemon last said, a question half answered — and what the
/// region shares when it opens ([`Doorway::residence`]). While the region is
/// open, the region hears the footer's news and its wake; while it is
/// closed, the resident does.
pub trait Resident {
    /// Whether it hears `news` while the region is closed — a question of
    /// its type, asked before anything is made for it: the footer's feeds
    /// pass here at every reading of the machine (ten a second for the sound
    /// levels), and a reading no resident hears costs nothing.
    fn hears(&self, news: &dyn Any) -> bool {
        let _ = news;
        false
    }
    /// What the footer heard from the machine, while the region is closed —
    /// only what it said it hears ([`Resident::hears`]).
    fn heard(&mut self, news: &dyn Any, way: &mut dyn Doorway) {
        let _ = (news, way);
    }
    /// A thread it handed the feature's wake to woke the loop, while the
    /// region is closed.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let _ = way;
    }
    /// The question a command of its asked with the region closed was let
    /// go: what it kept for the answer goes with it.
    fn let_go(&mut self, question: &str) {
        let _ = question;
    }
}

/// THE SCREENS THE FOOTER KNOWS, told to its regions as they change — a
/// screen named, gone, the pointer on another footer: each by its connector,
/// with what the compositor says it is, and the one the pointer is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screens {
    /// Every screen known by name.
    pub known: Vec<(String, Option<String>)>,
    /// The one the pointer last entered.
    pub here: Option<String>,
}

/// A resident, held twice: what the footer calls ([`Resident`]) and what the
/// feature downcasts to its own type ([`Doorway::residence`]) — the same value.
#[derive(Clone)]
pub struct Residence {
    /// The footer's handle.
    pub hooks: std::rc::Rc<std::cell::RefCell<dyn Resident>>,
    /// The feature's handle, `RefCell<T>` underneath.
    pub any: std::rc::Rc<dyn Any>,
}

impl Residence {
    /// `resident`, kept for the session.
    #[must_use]
    pub fn of<T: Resident + 'static>(resident: T) -> Self {
        let typed = std::rc::Rc::new(std::cell::RefCell::new(resident));
        Self {
            hooks: typed.clone(),
            any: typed,
        }
    }

    /// The feature's own handle, when it is a `T`.
    #[must_use]
    pub fn as_a<T: 'static>(&self) -> Option<std::rc::Rc<std::cell::RefCell<T>>> {
        std::rc::Rc::clone(&self.any)
            .downcast::<std::cell::RefCell<T>>()
            .ok()
    }
}

impl std::fmt::Debug for Residence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Residence").finish_non_exhaustive()
    }
}

/// A resident the binary registers, beside its region's [`Entry`]: its id
/// and how it is made, once, when the footer starts.
#[derive(Clone, Copy)]
pub struct Residing {
    /// The region it lives beside.
    pub id: &'static str,
    /// Makes it.
    pub make: fn() -> Residence,
}

impl std::fmt::Debug for Residing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Residing")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// What a region is told before a frame ([`Region::before_frame`]): the
/// width and the listing's lines it is drawn at, the scale its pictures
/// are made for, the rows of its list the page shows and the words they are
/// said in — a list that asks for the pictures of the rows on screen only
/// builds them in the frame's words.
#[derive(Debug, Clone, Copy)]
pub struct Framing<'a> {
    /// Cells of text a line holds.
    pub columns: u16,
    /// The lines its listing has.
    pub room: usize,
    /// Physical pixels per logical one, the largest of the screens.
    pub scale: f32,
    /// The row of its list the page starts at.
    pub first: usize,
    /// How many rows of its list a page shows.
    pub page: usize,
    /// The words.
    pub texts: &'a ronoctua_i18n::Texts,
}

/// ONE THING A DOOR KEEPS OF THE MACHINE, lent by its type: a reading
/// (`ronoctua_power::Snapshot`), a line a daemon is asked through
/// (`ronoctua_power::Handle`), a wish the door follows (a `Cell` of the
/// person's chosen profile) — see the module for why.
pub type Kept = Rc<dyn Any>;

/// What `way` keeps of the machine of type `T` ([`Doorway::kept`]) — `None`
/// for a door that keeps none, or none of that type.
#[must_use]
pub fn kept<T: Any>(way: &dyn Doorway) -> Option<Rc<T>> {
    way.kept(TypeId::of::<T>())
        .and_then(|k| k.downcast::<T>().ok())
}

/// The one of `kept` whose inside is of type `ty` — by the type of what is
/// INSIDE each `Rc`, never the `Rc`'s own, which is the same for all of them.
fn found(kept: &[Kept], ty: TypeId) -> Option<Kept> {
    kept.iter()
        .find(|k| <dyn Any>::type_id(k.as_ref()) == ty)
        .map(Rc::clone)
}

/// Asks `way` to raise its line of type `T` ([`Doorway::want`]).
pub fn want<T: Any>(way: &mut dyn Doorway) {
    way.want(TypeId::of::<T>());
}

/// WHAT A VIEW ACTS THROUGH, in either of its doors — the window's
/// ([`crate::door::Ui`]) and the footer's ([`Recorder`]).
pub trait Doorway {
    /// The words.
    fn texts(&self) -> &ronoctua_i18n::Texts;
    /// The same words, shared — what a view that keeps them holds, to say
    /// later what it reads now.
    fn words(&self) -> std::rc::Rc<ronoctua_i18n::Texts>;
    /// Cells of text a line holds.
    fn columns(&self) -> u16;
    /// Whether the door is a photograph's (or a test's): nothing of the
    /// machine is to be reached.
    fn photographing(&self) -> bool;
    /// Says a sentence, for as long as its class says.
    fn say(&mut self, text: String, class: crate::echo::Echo);
    /// Holds a state on the line, until something replaces it: no timer
    /// takes it off in either door (the window's `Ui::hold`, the footer's
    /// held echo), since nothing would put it back while it is still true.
    fn hold(&mut self, text: String);
    /// Asks a question on the door's line; the answer comes back to
    /// [`Region::answered`] under the line's id.
    fn ask(&mut self, line: crate::line::Line);
    /// Asks for the clipboard's text, read off the loop: [`Region::pasted`].
    fn paste(&mut self);
    /// Starts a program, off the loop; a failure is said on this door.
    fn launch(&mut self, launching: crate::door::Launching);
    /// Opens a window of the shell — its words after the binary's name.
    fn open_window(&mut self, args: Vec<String>);
    /// Tells the transfer queue; a queue that does not answer is said.
    fn queue(&mut self, ask: &ronoctua_transfers::Ask);
    /// The gate a root gesture goes through: the footer's sudo bar.
    fn sudo(&self) -> crate::sudo::Gate;
    /// A wake for a thread of the view's: called, the door's loop wakes and
    /// the view hears [`Region::woken`].
    fn wake(&self) -> crate::wake::Wake;
    /// The view is done: its door closes it.
    fn close(&mut self);
    /// A frame is owed — what a view asks when only what its rows SAY
    /// changed (a meter's bar, a mark moved).
    fn repaint(&mut self);
    /// The view's rows CAME OR WENT — a device unplugged took its row, a
    /// folder was added — so the page clamps to what is left and the height
    /// follows them, and a frame is owed. What a view asks when a reading
    /// may have changed how many rows it has; never the cursor's to follow:
    /// a reading that arrives does not move the page. The door never counts
    /// the rows to find out — a level ten times a second would build them
    /// all twice for a bar that may not have moved a cell. The window lays
    /// its list out again on every frame, and only repaints.
    fn rows_changed(&mut self);
    /// Runs a command of the registry, as `ronoctua-shell command` does.
    fn run(&mut self, command: &'static str, argument: Option<String>);
    /// The view is to be SEEN: a hidden region of the footer
    /// ([`Closed::Behind`]) asks for the footer's region — given when the
    /// region is free, and the refusal said when it is not. A window is
    /// shown already.
    fn show(&mut self);
    /// The page moves as `paging` says — after a gesture that did not come
    /// by a key (a click, a command by name), or to bring a place somebody
    /// asked for into view. The footer pages its region; a window pages its
    /// own list itself, after the call, and its doorway does nothing here.
    fn page(&mut self, paging: Paging);
    /// ANOTHER VIEW TAKES OVER. The footer leaves this view's region first,
    /// so the region is free for the other one, and runs `footer`; a window
    /// STAYS, and asks the footer for `window` — the other view's own window,
    /// or the same command when the footer is where that view is opened.
    fn give_way(&mut self, footer: &'static str, window: &'static str);
    /// Another door is about to take the person's keyboard — a terminal,
    /// another view — so this one steps aside first: the footer gives its
    /// region back (a view left open over the terminal would keep the
    /// keyboard, and another view would find the region taken); a window,
    /// which holds nothing of anyone's, stays.
    ///
    /// # Why it has no default
    ///
    /// Because the two doors answer it in opposite ways, and a default could
    /// only be one of them. A doorway that WRAPS another — one that lends
    /// the footer's fields around its [`Recorder`], a window's around its
    /// `Ui` — must hand it on; with the window's "nothing" as a default, a
    /// wrapper of the footer's that forgot would compile, and the region
    /// would stay open over the terminal, holding the keyboard. Required,
    /// the compiler names every doorway that has not said.
    fn step_aside(&mut self);
    /// Opens, or walks, the door's own line — the footer's minibuffer, a
    /// window's line 0 — when the keyboard is on its control.
    fn fold(&mut self, ask: crate::fold::Ask);
    /// The page follows the view's cursor now, the surface first, as it does
    /// after a person's key — what a view asks after a gesture that may have
    /// moved its cursor: a press (a key, a gesture by name, the click that
    /// acts), or one that reached it by a command (a second open, a question
    /// that came down). A click that only selects, a reading that arrives and
    /// an answer typed do not move the page. The window follows its cursor
    /// after every key and click already, and does nothing here.
    fn follow(&mut self);
    /// The page follows the cursor — when the cursor's row `was`, where it
    /// stood before what just arrived, was on the page: a log followed at its
    /// end keeps its end in sight, and one the person wheeled away from stays
    /// where they put it. Data never moves the page otherwise.
    ///
    /// NO DEFAULT, like [`Doorway::step_aside`]: see there.
    fn follow_if_seen(&mut self, was: usize);
    /// A file every door that draws it watches was just written by the view
    /// — see [`Written`].
    fn written(&mut self, written: Written);
    /// Takes the door's question down, when it is `question` — the view let
    /// go of what it asked (a press reached it while the line asked).
    fn let_go_of(&mut self, question: &'static str);
    /// What the door KEEPS of the machine of type `ty` ([`Kept`], [`kept`]):
    /// the footer lends its own lines to its regions; a window keeps its
    /// own, and lends none.
    fn kept(&self, ty: TypeId) -> Option<Kept> {
        let _ = ty;
        None
    }
    /// Asks the door to raise a line of type `ty` it raises only when a view
    /// first needs it ([`want`]) — the shell's history, whose reading is a
    /// process every thirty seconds and is paid only once somebody looked.
    /// Raised, the footer keeps it for the session; a window raises its own
    /// when it opens, and has nothing to do here.
    fn want(&mut self, ty: TypeId) {
        let _ = ty;
    }
    /// The list starts again at its first page — what a view built again on
    /// a row of its own asks (the power view aimed at its thresholds, or an
    /// armed action, while it was already open), before [`Doorway::follow`]:
    /// the door shows it as it shows a view just opened on that row. The
    /// window's list goes back to its top.
    fn first_page(&mut self);
    /// What the footer holds and lends for the call ([`Lent`]) — nothing in
    /// a window, which holds its own.
    fn lent(&self) -> Lent<'_> {
        Lent::none()
    }
    /// Puts what was copied LAST into the transfer queue, as `wish` says — the
    /// footer's clipboard history is where it is, so a window, which keeps
    /// none, hands the footer `command`, the one that does it
    /// ([`Doorway::run`]).
    fn queue_copied(&mut self, command: &'static str, wish: ronoctua_transfers::where_to::Wish) {
        let _ = wish;
        self.run(command, None);
    }
    /// Asks the notification desk the footer keeps — its queue and the
    /// person's filter book — something only it does ([`crate::desk::Ask`]).
    /// A window keeps its own (its files) and never asks this of its door:
    /// the ask is let go, and said in the log.
    fn desk(&mut self, ask: crate::desk::Ask) {
        tracing::warn!(?ask, "a door with no notification desk was asked for one");
    }
    /// Something only the door's host does anything with — the footer's
    /// modeline marks follow what a view read ([`News`]); a window, which
    /// has no modeline, drops it.
    fn news(&mut self, news: News) {
        let _ = news;
    }
    /// Where the region is on the screen the pointer is on — what a region
    /// that lays out its own cells measures a key against. `None` with no
    /// screen.
    fn area(&self) -> Option<Area> {
        None
    }
    /// The rows the door shows now — what a picture among them, and a
    /// question about whether it fits, is measured against, and what a
    /// region laid out whole walks its keys over. A door that says nothing
    /// of its page shows an empty one at the top.
    fn page_shown(&self) -> Page {
        Page::default()
    }
    /// Where a thread of the view leaves a command for the door's loop to
    /// run later ([`Orders`]) — from a door with no loop, orders that say
    /// in the log that they went nowhere.
    fn orders(&self) -> Orders {
        Orders::nowhere()
    }
    /// Asks the clipboard's history — which the FOOTER keeps, for both of
    /// the clipboard's doors (`crate::clipboard_wire`) — for `act`, aimed at
    /// the history `generation` the view was showing. A door that is neither
    /// of the clipboard's says so in the log and does nothing.
    fn clipboard(&mut self, act: crate::clipboard_wire::Act, generation: u64) {
        let _ = (act, generation);
        tracing::warn!("a clipboard act from a door that keeps no clipboard; not done");
    }

    /// What the feature keeps for the session in this door ([`Resident`]):
    /// the footer's, `None` in a window, which keeps its own.
    fn residence(&self) -> Option<Residence> {
        None
    }
    /// A wake for a thread that brings a new PICTURE: the loop wakes, the
    /// view hears [`Region::framed`], and only a paint is owed — a film's
    /// frames are no news for anything else.
    fn frame_wake(&self) -> crate::wake::Wake {
        self.wake()
    }
    /// The view's lines came or went: the door measures itself again — the
    /// footer's region; a window lays itself out at each frame.
    fn resized(&mut self) {}
    /// What the door's line says is taken back — an order's sentence whose
    /// order was refused, and is asked about on the rows instead.
    fn take_back(&mut self) {}
    /// The workspace the door is on, by name — the footer's, which knows the
    /// compositor's; `None` in a window.
    fn workspace(&self) -> Option<String> {
        None
    }
    /// The screens the door knows by name, each with what the compositor
    /// says it is, and the one the pointer is on.
    fn screens(&self) -> (Vec<(String, Option<String>)>, Option<String>) {
        (Vec::new(), None)
    }
    /// Whether the door runs a LOOP its threads can wake — `false` in a
    /// photograph of the footer and a test's footer, where nothing would
    /// hear a wake: a view reads there in place, and no thread is raised
    /// for a picture or a globe that would never be heard.
    fn has_a_loop(&self) -> bool {
        true
    }
}

/// How the page moves ([`Doorway::page`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paging {
    /// It follows the cursor, as it does after a person's key.
    Follow,
    /// Row `n` first on the page — a place somebody asked for — and the
    /// cursor kept on the page.
    Top(usize),
    /// So many pages towards the end; negative, towards the start.
    Pages(i32),
    /// So many lines towards the end; negative, towards the start — a
    /// manual read with the arrows.
    Lines(i32),
    /// The last page: the end of a log, where a live run writes.
    End,
    /// The last page again, when the page was on it before the list grew by
    /// so many rows — a log the person took to its end stays on its end as
    /// the end moves; anywhere else, data does not move the page.
    Tail(usize),
}

/// What a view tells its door's host ([`Doorway::news`]): any value, for
/// whoever knows its type. Two are the same news when they are one value.
#[derive(Clone)]
pub struct News(pub std::rc::Rc<dyn Any>);

impl News {
    /// News of `value`.
    #[must_use]
    pub fn of<T: Any>(value: T) -> Self {
        Self(std::rc::Rc::new(value))
    }

    /// The value, when it is a `T`.
    #[must_use]
    pub fn read<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl std::fmt::Debug for News {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("News")
    }
}

impl PartialEq for News {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}

/// One thing a region asked of the footer's door, written down while the
/// region was borrowed and done once it is back ([`Recorder`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// [`Doorway::say`].
    Say(String, crate::echo::Echo),
    /// [`Doorway::hold`].
    Hold(String),
    /// [`Doorway::ask`].
    Ask(crate::line::Line),
    /// [`Doorway::paste`].
    Paste,
    /// [`Doorway::launch`].
    Launch(crate::door::Launching),
    /// [`Doorway::open_window`].
    OpenWindow(Vec<String>),
    /// [`Doorway::queue`].
    Queue(ronoctua_transfers::Ask),
    /// [`Doorway::close`].
    Close,
    /// [`Doorway::repaint`].
    Repaint,
    /// [`Doorway::rows_changed`].
    RowsChanged,
    /// [`Doorway::run`].
    Run(&'static str, Option<String>),
    /// [`Doorway::show`].
    Show,
    /// [`Doorway::page`].
    Page(Paging),
    /// [`Doorway::give_way`] — what the footer runs.
    GiveWay(&'static str),
    /// [`Doorway::fold`].
    Fold(crate::fold::Ask),
    /// [`Doorway::follow`].
    Follow,
    /// [`Doorway::written`].
    Written(Written),
    /// [`Doorway::let_go_of`].
    LetGoOf(&'static str),
    /// [`Doorway::want`].
    Want(TypeId),
    /// [`Doorway::first_page`].
    FirstPage,
    /// [`Doorway::queue_copied`]: the command that asked, and the wish.
    QueueCopied(&'static str, ronoctua_transfers::where_to::Wish),
    /// [`Doorway::desk`].
    Desk(crate::desk::Ask),
    /// [`Doorway::news`].
    News(News),
    /// [`Doorway::follow_if_seen`].
    FollowIfSeen(usize),
    /// [`Doorway::step_aside`].
    StepAside,
    /// [`Doorway::clipboard`]: the act, and the history it was aimed at.
    Clipboard(crate::clipboard_wire::Act, u64),
    /// [`Doorway::resized`].
    Resized,
    /// [`Doorway::take_back`].
    TakeBack,
}

/// THE FOOTER'S DOORWAY: it answers what it was given and writes down what
/// it is asked ([`Recorder::owed`]), for the footer to do after the call —
/// see the module for why nothing is done inside it. What the footer LENDS
/// the call of its own fields is the [`Lending`] around it.
pub struct Recorder {
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    columns: u16,
    photographing: bool,
    gate: crate::sudo::Gate,
    wake: crate::wake::Wake,
    /// What the footer lends of the machine for the call ([`Doorway::kept`]).
    kept: Vec<Kept>,
    area: Option<Area>,
    /// The page the footer shows of the region ([`Doorway::page_shown`]).
    page: Page,
    /// The footer's way of running a command later ([`Doorway::orders`]);
    /// none, orders that go nowhere.
    orders: Option<Orders>,
    /// What was asked, in the order it was.
    pub owed: Vec<Effect>,
    /// The feature's resident, when the call is for a region that has one.
    pub residence: Option<Residence>,
    /// The region's wake for its pictures ([`Doorway::frame_wake`]).
    pub frame_wake: Option<crate::wake::Wake>,
    /// The workspace the footer is on.
    pub workspace: Option<String>,
    /// The screens the footer knows by name, and the one the pointer is on.
    pub screens: (Vec<(String, Option<String>)>, Option<String>),
    /// Whether the footer runs a loop its threads can wake
    /// ([`Doorway::has_a_loop`]).
    pub has_a_loop: bool,
}

impl Recorder {
    /// A doorway for one call into one region: the words, the width, whether
    /// it is a photograph's, the footer's sudo gate and the region's wake.
    #[must_use]
    pub const fn new(
        texts: std::rc::Rc<ronoctua_i18n::Texts>,
        columns: u16,
        photographing: bool,
        gate: crate::sudo::Gate,
        wake: crate::wake::Wake,
    ) -> Self {
        Self {
            texts,
            columns,
            photographing,
            gate,
            wake,
            kept: Vec::new(),
            area: None,
            page: Page {
                offset: 0,
                lines: 0,
            },
            orders: None,
            owed: Vec::new(),
            residence: None,
            frame_wake: None,
            workspace: None,
            screens: (Vec::new(), None),
            has_a_loop: true,
        }
    }

    /// The same doorway, lending what the footer keeps of the machine
    /// ([`Doorway::kept`]) — cheap: each is an `Rc` the footer holds.
    #[must_use]
    pub fn keeping(mut self, kept: Vec<Kept>) -> Self {
        self.kept = kept;
        self
    }

    /// The same doorway, with what the footer lends the call of its own
    /// fields ([`Lent`]).
    #[must_use]
    pub const fn lending(self, lent: Lent<'_>) -> Lending<'_> {
        Lending { way: self, lent }
    }

    /// The same doorway, answering that the region is at `area` on the
    /// screen the pointer is on ([`Doorway::area`]).
    #[must_use]
    pub const fn at(mut self, area: Option<Area>) -> Self {
        self.area = area;
        self
    }

    /// The same doorway, with the footer's way of running a command later
    /// ([`Doorway::orders`]).
    #[must_use]
    pub fn with_orders(mut self, orders: Orders) -> Self {
        self.orders = Some(orders);
        self
    }

    /// The same doorway, saying the footer shows `page` of the region's
    /// rows ([`Doorway::page_shown`]); with none, an empty page at the top.
    #[must_use]
    pub const fn with_page(mut self, page: Page) -> Self {
        self.page = page;
        self
    }
}

impl Doorway for Recorder {
    fn texts(&self) -> &ronoctua_i18n::Texts {
        &self.texts
    }
    fn words(&self) -> std::rc::Rc<ronoctua_i18n::Texts> {
        std::rc::Rc::clone(&self.texts)
    }
    fn columns(&self) -> u16 {
        self.columns
    }
    fn photographing(&self) -> bool {
        self.photographing
    }
    fn say(&mut self, text: String, class: crate::echo::Echo) {
        self.owed.push(Effect::Say(text, class));
    }
    fn hold(&mut self, text: String) {
        self.owed.push(Effect::Hold(text));
    }
    fn ask(&mut self, line: crate::line::Line) {
        self.owed.push(Effect::Ask(line));
    }
    fn paste(&mut self) {
        self.owed.push(Effect::Paste);
    }
    fn launch(&mut self, launching: crate::door::Launching) {
        self.owed.push(Effect::Launch(launching));
    }
    fn open_window(&mut self, args: Vec<String>) {
        self.owed.push(Effect::OpenWindow(args));
    }
    fn queue(&mut self, ask: &ronoctua_transfers::Ask) {
        self.owed.push(Effect::Queue(ask.clone()));
    }
    fn sudo(&self) -> crate::sudo::Gate {
        self.gate.clone()
    }
    fn wake(&self) -> crate::wake::Wake {
        self.wake.clone()
    }
    fn close(&mut self) {
        self.owed.push(Effect::Close);
    }
    fn repaint(&mut self) {
        self.owed.push(Effect::Repaint);
    }
    fn rows_changed(&mut self) {
        self.owed.push(Effect::RowsChanged);
    }
    fn run(&mut self, command: &'static str, argument: Option<String>) {
        self.owed.push(Effect::Run(command, argument));
    }
    fn kept(&self, ty: TypeId) -> Option<Kept> {
        found(&self.kept, ty)
    }
    fn want(&mut self, ty: TypeId) {
        self.owed.push(Effect::Want(ty));
    }
    fn first_page(&mut self) {
        self.owed.push(Effect::FirstPage);
    }
    fn news(&mut self, news: News) {
        self.owed.push(Effect::News(news));
    }
    fn area(&self) -> Option<Area> {
        self.area
    }
    fn follow(&mut self) {
        self.owed.push(Effect::Follow);
    }
    fn written(&mut self, written: Written) {
        self.owed.push(Effect::Written(written));
    }
    fn let_go_of(&mut self, question: &'static str) {
        self.owed.push(Effect::LetGoOf(question));
    }
    fn show(&mut self) {
        self.owed.push(Effect::Show);
    }
    fn page(&mut self, paging: Paging) {
        self.owed.push(Effect::Page(paging));
    }
    fn give_way(&mut self, footer: &'static str, _: &'static str) {
        self.owed.push(Effect::GiveWay(footer));
    }
    fn fold(&mut self, ask: crate::fold::Ask) {
        self.owed.push(Effect::Fold(ask));
    }
    fn queue_copied(&mut self, command: &'static str, wish: ronoctua_transfers::where_to::Wish) {
        self.owed.push(Effect::QueueCopied(command, wish));
    }
    fn desk(&mut self, ask: crate::desk::Ask) {
        self.owed.push(Effect::Desk(ask));
    }
    fn follow_if_seen(&mut self, was: usize) {
        self.owed.push(Effect::FollowIfSeen(was));
    }
    fn step_aside(&mut self) {
        self.owed.push(Effect::StepAside);
    }
    fn page_shown(&self) -> Page {
        self.page
    }
    fn orders(&self) -> Orders {
        self.orders.clone().unwrap_or_else(Orders::nowhere)
    }
    fn clipboard(&mut self, act: crate::clipboard_wire::Act, generation: u64) {
        self.owed.push(Effect::Clipboard(act, generation));
    }

    fn residence(&self) -> Option<Residence> {
        self.residence.clone()
    }
    fn frame_wake(&self) -> crate::wake::Wake {
        self.frame_wake.clone().unwrap_or_else(|| self.wake.clone())
    }
    fn resized(&mut self) {
        self.owed.push(Effect::Resized);
    }
    fn take_back(&mut self) {
        self.owed.push(Effect::TakeBack);
    }
    fn workspace(&self) -> Option<String> {
        self.workspace.clone()
    }
    fn screens(&self) -> (Vec<(String, Option<String>)>, Option<String>) {
        self.screens.clone()
    }
    fn has_a_loop(&self) -> bool {
        self.has_a_loop
    }
}

/// THE FOOTER'S DOORWAY WITH WHAT IT LENDS the call ([`Lent`]): the
/// [`Recorder`] it holds writes down what is asked, and it answers
/// [`Doorway::lent`] with what the footer lent.
///
/// # Why a wrapper, and not a field of the recorder
///
/// Because what is lent is BORROWED from the footer's fields for the call,
/// and a borrow is a lifetime: on the recorder it would be one on every
/// doorway the footer builds and every test that builds one, for the few
/// calls that read something lent. The recorder stays what it is, and the
/// borrow lives only where it is made.
pub struct Lending<'a> {
    /// The doorway that writes down what is asked.
    pub way: Recorder,
    lent: Lent<'a>,
}

impl std::ops::Deref for Lending<'_> {
    type Target = Recorder;
    fn deref(&self) -> &Recorder {
        &self.way
    }
}

impl std::ops::DerefMut for Lending<'_> {
    fn deref_mut(&mut self) -> &mut Recorder {
        &mut self.way
    }
}

/// EVERY ASK GOES TO THE RECORDER — what the footer keeps of the machine
/// ([`Doorway::kept`]) included: lending its fields hides none of its lines.
impl Doorway for Lending<'_> {
    fn texts(&self) -> &ronoctua_i18n::Texts {
        self.way.texts()
    }
    fn words(&self) -> std::rc::Rc<ronoctua_i18n::Texts> {
        self.way.words()
    }
    fn columns(&self) -> u16 {
        self.way.columns()
    }
    fn photographing(&self) -> bool {
        self.way.photographing()
    }
    fn say(&mut self, text: String, class: crate::echo::Echo) {
        self.way.say(text, class);
    }
    fn hold(&mut self, text: String) {
        self.way.hold(text);
    }
    fn ask(&mut self, line: crate::line::Line) {
        self.way.ask(line);
    }
    fn paste(&mut self) {
        self.way.paste();
    }
    fn launch(&mut self, launching: crate::door::Launching) {
        self.way.launch(launching);
    }
    fn open_window(&mut self, args: Vec<String>) {
        self.way.open_window(args);
    }
    fn queue(&mut self, ask: &ronoctua_transfers::Ask) {
        self.way.queue(ask);
    }
    fn sudo(&self) -> crate::sudo::Gate {
        self.way.sudo()
    }
    fn wake(&self) -> crate::wake::Wake {
        self.way.wake()
    }
    fn close(&mut self) {
        self.way.close();
    }
    fn repaint(&mut self) {
        self.way.repaint();
    }
    fn rows_changed(&mut self) {
        self.way.rows_changed();
    }
    fn run(&mut self, command: &'static str, argument: Option<String>) {
        self.way.run(command, argument);
    }
    fn show(&mut self) {
        self.way.show();
    }
    fn page(&mut self, paging: Paging) {
        self.way.page(paging);
    }
    fn give_way(&mut self, footer: &'static str, window: &'static str) {
        self.way.give_way(footer, window);
    }
    fn fold(&mut self, ask: crate::fold::Ask) {
        self.way.fold(ask);
    }
    fn follow(&mut self) {
        self.way.follow();
    }
    fn written(&mut self, written: Written) {
        self.way.written(written);
    }
    fn let_go_of(&mut self, question: &'static str) {
        self.way.let_go_of(question);
    }
    fn kept(&self, ty: TypeId) -> Option<Kept> {
        self.way.kept(ty)
    }
    fn want(&mut self, ty: TypeId) {
        self.way.want(ty);
    }
    fn first_page(&mut self) {
        self.way.first_page();
    }
    fn lent(&self) -> Lent<'_> {
        self.lent.clone()
    }
    fn queue_copied(&mut self, command: &'static str, wish: ronoctua_transfers::where_to::Wish) {
        self.way.queue_copied(command, wish);
    }
    fn desk(&mut self, ask: crate::desk::Ask) {
        self.way.desk(ask);
    }
    fn news(&mut self, news: News) {
        self.way.news(news);
    }
    fn area(&self) -> Option<Area> {
        self.way.area()
    }
    fn follow_if_seen(&mut self, was: usize) {
        self.way.follow_if_seen(was);
    }
    fn step_aside(&mut self) {
        self.way.step_aside();
    }
    fn page_shown(&self) -> Page {
        self.way.page_shown()
    }
    fn orders(&self) -> Orders {
        self.way.orders()
    }
    fn clipboard(&mut self, act: crate::clipboard_wire::Act, generation: u64) {
        self.way.clipboard(act, generation);
    }

    fn residence(&self) -> Option<Residence> {
        self.way.residence()
    }
    fn frame_wake(&self) -> crate::wake::Wake {
        self.way.frame_wake()
    }
    fn resized(&mut self) {
        self.way.resized();
    }
    fn take_back(&mut self) {
        self.way.take_back();
    }
    fn workspace(&self) -> Option<String> {
        self.way.workspace()
    }
    fn screens(&self) -> (Vec<(String, Option<String>)>, Option<String>) {
        self.way.screens()
    }
    fn has_a_loop(&self) -> bool {
        self.way.has_a_loop()
    }
}

/// A DOORWAY OF THE FOOTER'S the host fits to the region it asks — the
/// feature's resident and the region's wakes ([`Slots::closed_command`]):
/// the [`Recorder`], and the [`Lending`] around it.
pub trait Fits: Doorway {
    /// The recorder underneath.
    fn recorder(&mut self) -> &mut Recorder;
}

impl Fits for Recorder {
    fn recorder(&mut self) -> &mut Recorder {
        self
    }
}

impl Fits for Lending<'_> {
    fn recorder(&mut self) -> &mut Recorder {
        &mut self.way
    }
}

/// What a command does to a CLOSED region ([`Entry::closed`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// Not one of its commands.
    NotMine,
    /// Done without opening it — a window of the view, a deed of its daemon.
    Taken,
    /// It opens the region — when the region is free — and is then handed to
    /// the open region ([`Region::command`]) as any other of its commands.
    Opens,
    /// It brings the region to life HIDDEN — unranked, undrawn, the region
    /// that holds the footer's lines left where it is — and is handed to it
    /// ([`Region::command`]); the region lives while it lingers
    /// ([`Region::lingers`]) and asks to be seen itself ([`Doorway::show`]).
    /// A leader command that acts on the view's behalf without its page.
    Behind,
}

/// A region the binary registers: its id, what its commands do while it is
/// closed, and how it opens.
#[derive(Clone, Copy)]
pub struct Entry {
    /// The id it is ranked under — what [`Region::id`] of what `open` makes
    /// answers.
    pub id: &'static str,
    /// What command `id` (with its argument) does while the region is closed.
    ///
    /// IT ACTS ONLY THROUGH `way`: the host also asks it, with the region
    /// open and a doorway whose asks are dropped, whether a command the open
    /// region did not take is one that OPENS it (`Closed::Opens`) — and
    /// refuses that opening as every opening is refused while the region is
    /// held. A `closed` that did something of its own would do it then too.
    /// For a HIDDEN region that did not take a command, it answers as for a
    /// closed one, with a doorway whose asks are done.
    pub closed: fn(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed,
    /// Opens it, for the command that asked — the footer asked whether the
    /// region is free first.
    pub open: fn(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// What starts an argument carrying a path that is not text
/// ([`path_argument`]): no path holds a NUL, so no folder is taken for one.
const NOT_TEXT: char = '\u{0}';

/// A PATH AS A COMMAND'S ARGUMENT, every byte kept — [`argument_path`]
/// reads it back. An argument is text and a path is bytes: a folder whose
/// name is not UTF-8, sent lossily, comes back as another one (what was not
/// text turned into U+FFFD) and opens a folder that is not there. So a path
/// that is text goes as itself, and one that is not as a NUL and its bytes
/// in hex.
#[must_use]
pub fn path_argument(path: &std::path::Path) -> String {
    use std::fmt::Write as _;
    use std::os::unix::ffi::OsStrExt as _;
    if let Some(text) = path.to_str() {
        return text.to_owned();
    }
    let mut said = String::from(NOT_TEXT);
    for byte in path.as_os_str().as_bytes() {
        let _ = write!(said, "{byte:02x}");
    }
    said
}

/// The path an argument carries ([`path_argument`]): the argument itself,
/// or the bytes written after a NUL. `None` for what follows a NUL that is
/// not those bytes — refused, never read as some other folder.
#[must_use]
pub fn argument_path(argument: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStringExt as _;
    let Some(hex) = argument.strip_prefix(NOT_TEXT) else {
        return Some(std::path::PathBuf::from(argument));
    };
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    Some(std::path::PathBuf::from(std::ffi::OsString::from_vec(
        bytes,
    )))
}

/// One slot: a registered region, and the region while it is open.
struct Slot {
    entry: Entry,
    open: Option<Box<dyn Region>>,
    /// The region is out of its slot for one call ([`Slots::lend`]): open,
    /// with nothing to show.
    lent: bool,
    /// Its shape as it was when it last came back ([`Slots::kept_shape`]).
    shape: Shape,
    /// Whether the live region is SHOWN — ranked and drawn — or lives
    /// hidden while it lingers ([`Closed::Behind`]).
    shown: bool,
    /// Closed regions of this id that stepped aside for a fresh one with
    /// work still out ([`Slots::linger`]).
    lingering: Vec<Box<dyn Region>>,
    /// Its wakes while they are hung: what its threads call, and what one
    /// that brings a picture calls ([`Slots::set_wakes`]).
    wakes: Option<(crate::wake::Wake, crate::wake::Wake)>,
}

impl Slot {
    /// Whether a region is in it, or lent out of it for a call.
    const fn live(&self) -> bool {
        self.open.is_some() || self.lent
    }

    /// Whether the region in it is open on the screen.
    const fn showing(&self) -> bool {
        self.live() && self.shown
    }
}

/// THE SLOT HOST: every region the binary registered, by id, open or closed.
///
/// It knows nothing of any of them but the id: the footer asks it which are
/// open ([`Slots::is_open`]), lends one out for a call ([`Slots::lend`],
/// [`Slots::give_back`]) and asks the closed ones what a command does to
/// them ([`Slots::closed_command`]). A region may also be LIVE and hidden
/// ([`Slots::keep_hidden`], [`Slots::is_live`]): kept, heard and asked, never
/// ranked nor drawn.
#[derive(Default)]
pub struct Slots {
    slots: Vec<Slot>,
    /// The residents, by their region's id ([`Slots::with_residents`]).
    residents: Vec<(&'static str, Residence)>,
}

impl std::fmt::Debug for Slots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.slots.iter().map(|s| (s.entry.id, s.open.is_some())))
            .finish()
    }
}

impl Slots {
    /// A host for these regions — every one closed. Two entries under one id
    /// are refused, by the id.
    ///
    /// # Errors
    ///
    /// An id registered twice.
    pub fn of(entries: &[Entry]) -> Result<Self, String> {
        let mut slots: Vec<Slot> = Vec::with_capacity(entries.len());
        for e in entries {
            if slots.iter().any(|s| s.entry.id == e.id) {
                return Err(format!("two regions are registered as `{}`", e.id));
            }
            slots.push(Slot {
                entry: *e,
                open: None,
                lent: false,
                shape: Shape::List,
                shown: false,
                lingering: Vec::new(),
                wakes: None,
            });
        }
        Ok(Self {
            slots,
            residents: Vec::new(),
        })
    }

    /// The host, with the residents the binary registered, each made now and
    /// kept for the session. A resident with no region, or two for one, is
    /// refused, by its id.
    ///
    /// # Errors
    ///
    /// A resident whose id no region has, or one registered twice.
    pub fn with_residents(mut self, residents: &[Residing]) -> Result<Self, String> {
        for r in residents {
            if !self.slots.iter().any(|s| s.entry.id == r.id) {
                return Err(format!(
                    "a resident is registered as `{}`, and no region is",
                    r.id
                ));
            }
            if self.residents.iter().any(|(id, _)| *id == r.id) {
                return Err(format!("two residents are registered as `{}`", r.id));
            }
            self.residents.push((r.id, (r.make)()));
        }
        Ok(self)
    }

    /// Region `id`'s resident, when it has one.
    #[must_use]
    pub fn residence(&self, id: &str) -> Option<Residence> {
        self.residents
            .iter()
            .find(|(of, _)| *of == id)
            .map(|(_, k)| k.clone())
    }

    /// The ids that have a resident.
    pub fn resident_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.residents.iter().map(|(id, _)| *id)
    }

    /// Region `id`'s wakes, hung by the door's loop — or none, unhung.
    pub fn set_wakes(&mut self, id: &str, wakes: Option<(crate::wake::Wake, crate::wake::Wake)>) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.entry.id == id) {
            slot.wakes = wakes;
        }
    }

    /// Region `id`'s wakes, while they are hung: what its threads call, and
    /// what one that brings a picture calls.
    #[must_use]
    pub fn wakes(&self, id: &str) -> Option<(crate::wake::Wake, crate::wake::Wake)> {
        self.slots
            .iter()
            .find(|s| s.entry.id == id)
            .and_then(|s| s.wakes.clone())
    }

    /// Every registered id, in the order they were registered.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.slots.iter().map(|s| s.entry.id)
    }

    /// Whether region `id` is open — SHOWN: ranked and drawn. A hidden one
    /// is not ([`Slots::is_live`]).
    #[must_use]
    pub fn is_open(&self, id: &str) -> bool {
        self.slots.iter().any(|s| s.entry.id == id && s.showing())
    }

    /// The open regions' ids.
    pub fn open_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.slots
            .iter()
            .filter(|s| s.showing())
            .map(|s| s.entry.id)
    }

    /// Whether region `id` is LIVE — open, or kept hidden while it lingers.
    #[must_use]
    pub fn is_live(&self, id: &str) -> bool {
        self.slots.iter().any(|s| s.entry.id == id && s.live())
    }

    /// Whether region `id` is live and HIDDEN.
    #[must_use]
    pub fn is_hidden(&self, id: &str) -> bool {
        self.slots
            .iter()
            .any(|s| s.entry.id == id && s.live() && !s.shown)
    }

    /// The live regions' ids, shown or hidden — every one whose threads are
    /// still heard.
    pub fn live_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.slots.iter().filter(|s| s.live()).map(|s| s.entry.id)
    }

    /// The live region `id`, to be asked a question.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn Region> {
        self.slots
            .iter()
            .find(|s| s.entry.id == id)
            .and_then(|s| s.open.as_deref())
    }

    /// The open region `id`, for a gesture that only moves its own state —
    /// a bar dragged, the wheel, the row it wants at the top
    /// ([`Region::take_anchor`]) — and asks nothing of a door: what a region
    /// DOES goes through [`Slots::call`], with a doorway.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut (dyn Region + 'static)> {
        self.slots
            .iter_mut()
            .find(|s| s.entry.id == id)
            .and_then(|s| s.open.as_deref_mut())
    }

    /// The registered entry `id`.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<Entry> {
        self.slots
            .iter()
            .find(|s| s.entry.id == id)
            .map(|s| s.entry)
    }

    /// The open region `id`, LENT OUT of its slot for one call — the slot
    /// stays, and stays open ([`Slots::give_back`]). While it is lent the
    /// host answers that it is open and has nothing to show.
    pub fn lend(&mut self, id: &str) -> Option<Box<dyn Region>> {
        let slot = self.slots.iter_mut().find(|s| s.entry.id == id)?;
        let region = slot.open.take()?;
        slot.lent = true;
        Some(region)
    }

    /// A region back in its slot — after a call ([`Slots::lend`]), or opened
    /// ([`Entry::open`]): SHOWN, as a region opened is, or as it was when it
    /// was lent. A region whose id was never registered is refused, and
    /// handed back.
    ///
    /// # Errors
    ///
    /// No slot answers to the region's id.
    pub fn give_back(&mut self, region: Box<dyn Region>) -> Result<(), Box<dyn Region>> {
        self.put(region, true)
    }

    /// A region kept in its slot HIDDEN — brought to life by a command that
    /// acts without its page ([`Closed::Behind`]), or closed with work in
    /// flight ([`Region::lingers`]). Refused as [`Slots::give_back`] refuses.
    ///
    /// # Errors
    ///
    /// No slot answers to the region's id.
    pub fn keep_hidden(&mut self, region: Box<dyn Region>) -> Result<(), Box<dyn Region>> {
        self.put(region, false)
    }

    /// The region in its slot; a lent one keeps whether it was shown.
    fn put(&mut self, region: Box<dyn Region>, shown: bool) -> Result<(), Box<dyn Region>> {
        match self.slots.iter_mut().find(|s| s.entry.id == region.id()) {
            Some(s) => {
                s.shape = region.shape();
                s.shown = if s.lent { s.shown } else { shown };
                s.open = Some(region);
                s.lent = false;
                Ok(())
            }
            None => Err(region),
        }
    }

    /// The hidden region `id` SHOWN — ranked and drawn from now on.
    /// `false` when there is no live region under `id`.
    pub fn show(&mut self, id: &str) -> bool {
        match self.slots.iter_mut().find(|s| s.entry.id == id && s.live()) {
            Some(s) => {
                s.shown = true;
                true
            }
            None => false,
        }
    }

    /// CALLS region `id`, lent out of its slot for the call and given back
    /// after it — to the same slot, whatever id it answers — with `way` as
    /// its doorway. `None` when it is not open.
    pub fn call<T>(
        &mut self,
        id: &str,
        way: &mut dyn Doorway,
        f: impl FnOnce(&mut dyn Region, &mut dyn Doorway) -> T,
    ) -> Option<T> {
        let at = self.slots.iter().position(|s| s.entry.id == id)?;
        let mut region = self.slots[at].open.take()?;
        self.slots[at].lent = true;
        let out = f(region.as_mut(), way);
        let slot = &mut self.slots[at];
        slot.shape = region.shape();
        slot.open = Some(region);
        slot.lent = false;
        Some(out)
    }

    /// TOUCHES region `id` for the FRAME's own hooks — lent and given back as
    /// [`Slots::call`] does, with NO doorway: what the frame tells a region
    /// (`Region::before_measuring`, `laid_out`, `painted`, `after_the_frame`,
    /// `framed`) may ask the footer nothing, so a paint can never run a
    /// command that paints (`the-footer-does-not-ask-itself`). `None` when it
    /// is not open.
    pub fn frame<T>(&mut self, id: &str, f: impl FnOnce(&mut dyn Region) -> T) -> Option<T> {
        let at = self.slots.iter().position(|s| s.entry.id == id)?;
        let mut region = self.slots[at].open.take()?;
        self.slots[at].lent = true;
        let out = f(region.as_mut());
        let slot = &mut self.slots[at];
        slot.shape = region.shape();
        slot.open = Some(region);
        slot.lent = false;
        Some(out)
    }

    /// Region `id`'s shape as it was KEPT when it last came back to its slot
    /// — a stored fact, which the footer's geometry may read without asking
    /// the region anything (`the-footer-does-not-ask-itself`). `List` for a
    /// region that is not open.
    #[must_use]
    pub fn kept_shape(&self, id: &str) -> Shape {
        self.slots
            .iter()
            .find(|s| s.entry.id == id && s.showing())
            .map_or(Shape::List, |s| s.shape)
    }

    /// Region `id` closed: it leaves its slot, and is handed over to be told
    /// ([`Region::closing`]) and dropped — or kept hidden
    /// ([`Slots::keep_hidden`]) while it lingers.
    pub fn take(&mut self, id: &str) -> Option<Box<dyn Region>> {
        self.slots
            .iter_mut()
            .find(|s| s.entry.id == id)
            .and_then(|s| {
                s.lent = false;
                s.shown = false;
                s.open.take()
            })
    }

    /// A CLOSED region kept BESIDE its slot for the work it still has out
    /// ([`Region::lingers`]) — a hidden one that did not take a command that
    /// opens it, stepping aside for a fresh one: unshown, unranked and not
    /// live — [`Slots::is_open`] and [`Slots::is_live`] do not see it — and
    /// told only what its threads bring ([`Slots::call_lingering`]) until it
    /// has none. One that does not linger, or whose id has no slot, is handed
    /// back.
    ///
    /// # Errors
    ///
    /// The region has nothing out, or no slot answers to its id.
    pub fn linger(&mut self, region: Box<dyn Region>) -> Result<(), Box<dyn Region>> {
        if !region.lingers() {
            return Err(region);
        }
        match self.slots.iter_mut().find(|s| s.entry.id == region.id()) {
            Some(s) => {
                s.lingering.push(region);
                Ok(())
            }
            None => Err(region),
        }
    }

    /// The HIDDEN region `id` STEPS ASIDE for a fresh one: out of its slot,
    /// kept beside it while it lingers ([`Slots::linger`]) and dropped when it
    /// does not — so the slot is closed, and its opening command opens it
    /// anew. `false` when no hidden region is under `id`.
    pub fn set_aside(&mut self, id: &str) -> bool {
        if !self.is_hidden(id) {
            return false;
        }
        let Some(region) = self.take(id) else {
            return false;
        };
        // ONE WITH NOTHING OUT is dropped here: nothing of it is left to say.
        drop(self.linger(region));
        true
    }

    /// Whether a closed region of `id` still lingers beside its slot.
    #[must_use]
    pub fn lingers(&self, id: &str) -> bool {
        self.slots
            .iter()
            .any(|s| s.entry.id == id && !s.lingering.is_empty())
    }

    /// CALLS every lingering region of `id` with `way`, and lets go of each
    /// that has nothing out any more.
    pub fn call_lingering(
        &mut self,
        id: &str,
        way: &mut dyn Doorway,
        mut f: impl FnMut(&mut dyn Region, &mut dyn Doorway),
    ) {
        let Some(slot) = self.slots.iter_mut().find(|s| s.entry.id == id) else {
            return;
        };
        for region in &mut slot.lingering {
            f(region.as_mut(), way);
        }
        slot.lingering.retain(|r| r.lingers());
    }

    /// What command `command` does to the first CLOSED region it is one of —
    /// its id and the answer — or `None` when it is no closed region's. A
    /// live region, shown or hidden, is asked as a region
    /// ([`Region::command`]), never here.
    ///
    /// Each entry is asked with ITS resident and ITS wakes on the doorway
    /// ([`Doorway::residence`], [`Doorway::wake`]): a command of a closed
    /// region reads what the feature keeps for the session — its daemon's
    /// last state, a question half answered — and a thread it starts wakes
    /// the feature's resident. An entry with no wakes hung is asked with the
    /// ones the doorway came with, never those the entry before it left.
    pub fn closed_command(
        &self,
        command: &str,
        argument: Option<&str>,
        way: &mut impl Fits,
    ) -> Option<(&'static str, Closed)> {
        let (wake, frame_wake) = {
            let r = way.recorder();
            (r.wake.clone(), r.frame_wake.clone())
        };
        self.slots.iter().filter(|s| !s.live()).find_map(|s| {
            {
                let r = way.recorder();
                r.residence = self.residence(s.entry.id);
                match &s.wakes {
                    Some((w, f)) => {
                        r.wake = w.clone();
                        r.frame_wake = Some(f.clone());
                    }
                    None => {
                        r.wake = wake.clone();
                        r.frame_wake.clone_from(&frame_wake);
                    }
                }
            }
            match (s.entry.closed)(command, argument, way) {
                Closed::NotMine => None,
                answer => Some((s.entry.id, answer)),
            }
        })
    }
}

/// WHAT A VIEW'S THREADS HAND BACK to its door's loop, in the order they
/// sent it, and the wake that tells the loop ([`Doorway::wake`]) — in either
/// door, so one view's threads are written once.
///
/// # One wake while anything waits unread
///
/// A burst — a rebuild's journal prints thousands of lines at once — is
/// one turn of the loop, not one per line: a post wakes the loop only when
/// nothing posted before it is still waiting, and [`Inbox::take`] takes all
/// of it. Nothing is lost between the two: the inbox is marked read BEFORE
/// it is emptied, so a post that lands in between wakes the loop again.
pub struct Inbox<T> {
    shared: Arc<Shared<T>>,
}

/// What an inbox and its posts share.
struct Shared<T> {
    items: Mutex<Vec<T>>,
    /// A wake was sent and its loop has not taken what waits yet.
    waiting: AtomicBool,
    /// The inbox is gone: a post tells its thread to stop.
    gone: AtomicBool,
    wake: crate::wake::Wake,
}

/// Where a thread posts what it hands back ([`Inbox::post`]).
pub struct Post<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Clone for Post<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> std::fmt::Debug for Post<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Post")
    }
}

impl<T> std::fmt::Debug for Inbox<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Inbox")
    }
}

impl<T> Inbox<T> {
    /// An empty inbox, whose posts wake the loop through `wake`.
    #[must_use]
    pub fn new(wake: crate::wake::Wake) -> Self {
        Self {
            shared: Arc::new(Shared {
                items: Mutex::new(Vec::new()),
                waiting: AtomicBool::new(false),
                gone: AtomicBool::new(false),
                wake,
            }),
        }
    }

    /// Where a thread posts.
    #[must_use]
    pub fn post(&self) -> Post<T> {
        Post {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Everything posted since the last take, in the order it was posted.
    #[must_use]
    pub fn take(&self) -> Vec<T> {
        self.shared.waiting.store(false, Ordering::SeqCst);
        self.shared
            .items
            .lock()
            .map(|mut items| std::mem::take(&mut *items))
            .unwrap_or_default()
    }
}

impl<T> Drop for Inbox<T> {
    fn drop(&mut self) {
        self.shared.gone.store(true, Ordering::SeqCst);
    }
}

impl<T> Post<T> {
    /// Hands `item` back, and wakes the loop when nothing was waiting.
    /// `false` when the inbox is gone — its door closed — and the thread is
    /// to stop.
    pub fn send(&self, item: T) -> bool {
        if self.shared.gone.load(Ordering::SeqCst) {
            return false;
        }
        if let Ok(mut items) = self.shared.items.lock() {
            items.push(item);
        }
        if !self.shared.waiting.swap(true, Ordering::SeqCst) {
            self.shared.wake.call();
        }
        true
    }
}

// ── THE LIGHT HELD TO THE CLICK, for every view's tests ─────────────────────
//
// Invariant 19: the pointer lights what the click takes. A view's test builds
// the view fresh once per row, clicks the row ONCE — the click that selects —
// and asks whether anything a person or the machine sees moved; the row was to
// be lit exactly then. Once, and not the second click that is `RET`: a row the
// first click does not take, the second does not either (the cursor did not
// move), and a `RET` run on every row of a test reaches the machine — a
// consent written, a clock turned. The row the cursor already stands on is
// the one where the first click IS the `RET`, and it is not clicked
// ([`Held::here`]). The window's half is `crate::door::held_to_the_click`.

/// ONE ROW HELD TO THE RULE THAT THE POINTER LIGHTS WHAT THE CLICK TAKES:
/// whether the door lights it under a resting pointer, and whether a click on
/// it, in a view of its own, did anything.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    /// The row, as drawn.
    pub row: usize,
    /// Whether a resting pointer lights it.
    pub lit: bool,
    /// Whether a click on it did anything that is seen.
    pub did: bool,
    /// Whether the fresh view's cursor was on it, or on the control it is a
    /// line of — where the click is already the `RET`, and so it is not
    /// clicked: lit, it is the click's all the same, since from any other
    /// row the first click selects it.
    pub here: bool,
}

/// Every row of a region `fresh` opens, [`Held`] to its click: lit by its
/// [`Row::takes`] at `seen`, and clicked once through [`Region::clicked`] on a
/// view of its own each time — but the row the cursor is on — and `witness`
/// is what a click is seen by (the rows, the cursor, [`deeds`]).
#[cfg(any(test, feature = "test-support"))]
pub fn held_to_the_click<R: Region, D: Doorway, W: PartialEq>(
    fresh: impl Fn() -> (R, D),
    seen: &Seen<'_>,
    witness: impl Fn(&R, &D) -> W,
) -> Vec<Held> {
    let first = fresh().0;
    let lit: Vec<bool> = first.rows(seen).iter().map(|r| r.takes).collect();
    // THE CURSOR'S CONTROL, every line of it (`Region::control_of`): a click
    // on any of them lands where the cursor is, and is its `RET`.
    let cursor = first.cursor_in(seen).map(|c| first.control_of(c, seen));
    lit.iter()
        .enumerate()
        .map(|(row, &lit)| {
            let here = cursor == Some(first.control_of(row, seen));
            if here {
                return Held {
                    row,
                    lit,
                    did: false,
                    here,
                };
            }
            let (mut r, mut way) = fresh();
            let before = witness(&r, &way);
            r.clicked(row, &mut way);
            Held {
                row,
                lit,
                did: witness(&r, &way) != before,
                here,
            }
        })
        .collect()
}

/// What a region asked of its door that is SEEN — `owed` without the frames
/// it asked for (a repaint, rows changed, the page following a cursor that
/// did not move, a height measured again): a click that asks only those did
/// nothing.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn deeds(owed: &[Effect]) -> Vec<Effect> {
    owed.iter()
        .filter(|e| {
            !matches!(
                e,
                Effect::Repaint
                    | Effect::RowsChanged
                    | Effect::Follow
                    | Effect::FollowIfSeen(_)
                    | Effect::Page(Paging::Follow)
                    | Effect::Resized
            )
        })
        .cloned()
        .collect()
}

/// What [`held_to_the_click`] sees of a region by default: its rows' fields
/// at `at`, its cursor as drawn, and the [`deeds`] of what it asked (`owed`,
/// the [`Recorder`]'s).
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn seen<R: Region>(
    r: &R,
    owed: &[Effect],
    at: &Seen<'_>,
) -> (Vec<Vec<crate::fields::Field>>, Option<usize>, Vec<Effect>) {
    (
        r.rows(at).into_iter().map(|row| row.fields).collect(),
        r.cursor_in(at),
        deeds(owed),
    )
}

/// The rows [`held_to_the_click`] found lit without a click that does
/// anything, or a click that does something without the light — empty when
/// the light is the click. The row the cursor stood on is lit by being the
/// cursor's ([`Held::here`]). Said whole, for an assertion's message.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn unheld(held: &[Held]) -> Vec<String> {
    held.iter()
        .filter(|h| h.lit != h.did && !(h.lit && h.here))
        .map(|h| {
            if h.lit {
                format!("row {} is lit and a click on it does nothing", h.row)
            } else {
                format!("row {} is not lit and a click on it does something", h.row)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A region that remembers what it was handed, and answers its own two
    /// commands: `fake-down` moves the cursor, `fake-close` closes it.
    struct Fake {
        cursor: usize,
        heard: Vec<String>,
    }

    impl Region for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
            Vec::new()
        }
        fn hint(&self, _: &Seen<'_>) -> Vec<crate::chrome::Piece> {
            Vec::new()
        }
        fn cursor(&self) -> Option<usize> {
            Some(self.cursor)
        }
        fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
            self.heard.push(key.notation());
            way.repaint();
            Taken::Yes
        }
        fn command(&mut self, id: &str, _: Option<&str>, way: &mut dyn Doorway) -> Taken {
            match id {
                "fake-open" => Taken::Yes,
                "fake-down" => {
                    self.cursor += 1;
                    way.say("down".to_owned(), crate::echo::Echo::Brief);
                    Taken::Yes
                }
                "fake-close" => {
                    way.close();
                    Taken::Yes
                }
                _ => Taken::No,
            }
        }
        fn clicked(&mut self, row: usize, _: &mut dyn Doorway) {
            self.cursor = row;
        }
    }

    fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
        match command {
            "fake-open" | "fake-down" => Closed::Opens,
            "fake-window" => {
                way.open_window(vec!["fake".to_owned()]);
                Closed::Taken
            }
            _ => Closed::NotMine,
        }
    }

    fn open(_: &str, _: Option<&str>, _: &mut dyn Doorway) -> Box<dyn Region> {
        Box::new(Fake {
            cursor: 0,
            heard: Vec::new(),
        })
    }

    const FAKE: Entry = Entry {
        id: "fake",
        closed,
        open,
    };

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: a region registered twice under one id is taken, so two
    /// slots would answer to one rank.
    #[test]
    fn an_id_registered_twice_is_refused() {
        let refused = Slots::of(&[FAKE, FAKE]).expect_err("two regions under one id");
        assert!(refused.contains("`fake`"), "{refused}");
        assert_eq!(Slots::of(&[FAKE]).map(|s| s.ids().count()), Ok(1));
    }

    /// FAILS IF: a closed region's commands are not asked of its entry, or
    /// one that is not its own is answered for it — the host would open a
    /// view for a command nobody gave it.
    #[test]
    fn a_closed_region_says_what_a_command_does_to_it() {
        let slots = Slots::of(&[FAKE]).expect("one region");
        let mut w = way();
        assert_eq!(
            slots.closed_command("fake-down", None, &mut w),
            Some(("fake", Closed::Opens))
        );
        assert_eq!(
            slots.closed_command("fake-window", None, &mut w),
            Some(("fake", Closed::Taken))
        );
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["fake".to_owned()])]);
        assert_eq!(slots.closed_command("stick-open", None, &mut w), None);
    }

    /// FAILS IF: a region lent out for a call reads as closed — the footer
    /// would rank the next view in its place in the middle of the call — or
    /// does not come back open, or an open region is asked what a command
    /// does to it CLOSED.
    #[test]
    fn a_lent_region_stays_open_and_comes_back() {
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        assert!(!slots.is_open("fake"));
        let mut w = way();
        slots
            .give_back(open("fake-open", None, &mut w))
            .map_err(|_| "refused")
            .expect("its slot");
        assert!(slots.is_open("fake"));
        assert_eq!(slots.get("fake").and_then(Region::cursor), Some(0));
        let mut lent = slots.lend("fake").expect("open");
        assert!(slots.is_open("fake"), "lent reads as closed");
        assert!(slots.get("fake").is_none(), "a lent region is still shown");
        assert_eq!(slots.open_ids().collect::<Vec<_>>(), vec!["fake"]);
        assert_eq!(slots.closed_command("fake-down", None, &mut w), None);
        assert_eq!(lent.command("fake-down", None, &mut w), Taken::Yes);
        slots
            .give_back(lent)
            .map_err(|_| "refused")
            .expect("its slot");
        assert_eq!(slots.get("fake").and_then(Region::cursor), Some(1));
        assert_eq!(
            w.owed,
            vec![Effect::Say("down".to_owned(), crate::echo::Echo::Brief)]
        );
        assert!(slots.take("fake").is_some());
        assert!(!slots.is_open("fake"));
    }

    /// FAILS IF: a call does not lend the region out (it would be asked
    /// while the host still showed it) or does not give it back open, with
    /// its shape kept for the footer's geometry.
    #[test]
    fn a_call_lends_the_region_and_keeps_its_shape() {
        struct Whole;
        impl Region for Whole {
            fn id(&self) -> &'static str {
                "fake"
            }
            fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
                Vec::new()
            }
            fn hint(&self, _: &Seen<'_>) -> Vec<crate::chrome::Piece> {
                Vec::new()
            }
            fn cursor(&self) -> Option<usize> {
                None
            }
            fn key(&mut self, _: ronoctua_keymap::Key, _: &mut dyn Doorway) -> Taken {
                Taken::No
            }
            fn command(&mut self, _: &str, _: Option<&str>, _: &mut dyn Doorway) -> Taken {
                Taken::No
            }
            fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}
            fn shape(&self) -> Shape {
                Shape::Whole
            }
        }
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        let mut w = way();
        assert_eq!(
            slots.call("fake", &mut w, |_, _| ()),
            None,
            "a closed region was called"
        );
        slots
            .give_back(Box::new(Whole))
            .map_err(|_| "refused")
            .expect("its slot");
        assert_eq!(slots.kept_shape("fake"), Shape::Whole);
        let shown_inside = slots.call("fake", &mut w, |r, _| r.cursor());
        assert_eq!(shown_inside, Some(None));
        assert!(slots.is_open("fake") && slots.get("fake").is_some());
        assert!(slots.take("fake").is_some());
        assert_eq!(
            slots.kept_shape("fake"),
            Shape::List,
            "a closed region keeps a shape"
        );
    }

    /// FAILS IF: a frame's hook reaches a closed region, or the shape the
    /// hook left — a stage that ended with the frame — is not the one kept:
    /// the footer's geometry would read a stage that is gone. The hook is
    /// handed what the region sees, and no doorway: a frame asks nothing.
    #[test]
    fn a_frame_touches_an_open_region_and_keeps_the_shape_it_left() {
        struct Staged(bool);
        impl Region for Staged {
            fn id(&self) -> &'static str {
                "fake"
            }
            fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
                Vec::new()
            }
            fn hint(&self, _: &Seen<'_>) -> Vec<crate::chrome::Piece> {
                Vec::new()
            }
            fn cursor(&self) -> Option<usize> {
                None
            }
            fn key(&mut self, _: ronoctua_keymap::Key, _: &mut dyn Doorway) -> Taken {
                Taken::No
            }
            fn command(&mut self, _: &str, _: Option<&str>, _: &mut dyn Doorway) -> Taken {
                Taken::No
            }
            fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}
            fn shape(&self) -> Shape {
                if self.0 { Shape::Stage } else { Shape::List }
            }
            fn before_measuring(&mut self, seen: &Seen<'_>) -> bool {
                self.0 = seen.columns == 0;
                true
            }
        }
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 80,
            lines: 10,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        assert_eq!(slots.frame("fake", |r| r.before_measuring(&seen)), None);
        slots
            .give_back(Box::new(Staged(true)))
            .map_err(|_| "refused")
            .expect("its slot");
        assert_eq!(slots.kept_shape("fake"), Shape::Stage);
        assert_eq!(
            slots.frame("fake", |r| r.before_measuring(&seen)),
            Some(true)
        );
        assert_eq!(
            slots.kept_shape("fake"),
            Shape::List,
            "the shape the frame left"
        );
        assert!(slots.is_open("fake") && slots.get("fake").is_some());
    }

    /// FAILS IF: a region whose id has no slot is kept — a view the
    /// precedence cannot rank would be open.
    #[test]
    fn a_region_with_no_slot_is_handed_back() {
        let mut slots = Slots::of(&[]).expect("no region");
        let mut w = way();
        assert!(slots.give_back(open("fake-open", None, &mut w)).is_err());
        assert!(!slots.is_open("fake"));
    }

    /// FAILS IF: the footer's doorway does a thing while the region is
    /// borrowed instead of writing it down, in order, for after the call.
    #[test]
    fn the_footers_doorway_writes_down_what_it_is_asked() {
        let mut w = way();
        let mut fake = Fake {
            cursor: 0,
            heard: Vec::new(),
        };
        let key = ronoctua_keymap::Key::parse("j").expect("a key");
        assert!(fake.key(key, &mut w).took());
        assert_eq!(fake.command("fake-close", None, &mut w), Taken::Yes);
        assert_eq!(fake.command("other", None, &mut w), Taken::No);
        w.run("transfers-open", Some("x".to_owned()));
        w.queue(&ronoctua_transfers::Ask::List);
        // AND WHAT A REGION ASKS OF THE FOOTER'S PAGE: the first one, the
        // cursor followed, measured again, the line taken back.
        w.first_page();
        w.follow();
        w.resized();
        w.take_back();
        assert_eq!(
            w.owed,
            vec![
                Effect::Repaint,
                Effect::Close,
                Effect::Run("transfers-open", Some("x".to_owned())),
                Effect::Queue(ronoctua_transfers::Ask::List),
                Effect::FirstPage,
                Effect::Follow,
                Effect::Resized,
                Effect::TakeBack,
            ]
        );
        assert_eq!(fake.heard, vec!["j".to_owned()]);
        assert!(w.photographing());
        assert_eq!(w.columns(), 80);
    }

    /// FAILS IF: the door's line and its page — what a region in sections
    /// asks after a key moved its cursor — are not written down in the order
    /// asked, or a region cannot be touched in its slot (a bar dragged, the
    /// wheel) without being lent out, or is shown as closed while touched, or
    /// a region that says nothing of it does not teach its keys.
    #[test]
    fn the_line_and_the_page_are_asked_of_the_door() {
        let mut w = way();
        w.fold(crate::fold::Ask::By(-1));
        w.page(Paging::Follow);
        w.fold(crate::fold::Ask::Toggle);
        assert_eq!(
            w.owed,
            vec![
                Effect::Fold(crate::fold::Ask::By(-1)),
                Effect::Page(Paging::Follow),
                Effect::Fold(crate::fold::Ask::Toggle),
            ]
        );
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        assert!(
            slots.get_mut("fake").is_none(),
            "a closed region was touched"
        );
        slots
            .give_back(open("fake-open", None, &mut w))
            .map_err(|_| "refused")
            .expect("its slot");
        slots.get_mut("fake").expect("open").clicked(3, &mut w);
        assert!(slots.is_open("fake"));
        assert_eq!(slots.get("fake").and_then(Region::cursor), Some(3));
        assert!(Region::sections(slots.get("fake").expect("open")).is_none());
        // AND A REGION TEACHES ITS KEYS unless it says otherwise.
        assert!(slots.get("fake").expect("open").teaches());
    }

    /// FAILS IF: the window's doorway does not do what a window does — its
    /// line, its question, its launches, its orders for the footer.
    #[test]
    fn the_windows_doorway_is_the_windows_ui() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        let way: &mut dyn Doorway = &mut ui;
        assert!(way.photographing(), "a detached door is a photograph's");
        way.say("said".to_owned(), crate::echo::Echo::Long);
        way.launch(crate::door::Launching::Entry("app.desktop".to_owned()));
        way.run("transfers-open", None);
        way.wake().call();
        way.close();
        assert_eq!(ui.message.as_deref(), Some("said"));
        assert_eq!(
            ui.launches(),
            &[crate::door::Launching::Entry("app.desktop".to_owned())]
        );
        assert_eq!(ui.orders(), &["command transfers-open".to_owned()]);
        assert!(ui.quit);
    }

    /// FAILS IF: a window asked to show itself or to page does anything, or
    /// one giving way to another view closes — the window stays, and the
    /// other view is the footer's to open.
    #[test]
    fn a_window_gives_way_and_stays() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        let way: &mut dyn Doorway = &mut ui;
        way.show();
        way.page(Paging::End);
        way.give_way("belongings-open", "belongings-window");
        assert_eq!(ui.orders(), &["command belongings-window".to_owned()]);
        assert!(!ui.quit, "a window giving way closed");
        assert_eq!(ui.offset, 0, "a window paged from a view's errand");
    }

    /// A region closed with a deed out: it lingers while `out` says so.
    struct Lingering {
        out: std::rc::Rc<std::cell::Cell<bool>>,
        told: std::rc::Rc<std::cell::Cell<u32>>,
    }

    impl Region for Lingering {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
            Vec::new()
        }
        fn hint(&self, _: &Seen<'_>) -> Vec<crate::chrome::Piece> {
            Vec::new()
        }
        fn cursor(&self) -> Option<usize> {
            None
        }
        fn key(&mut self, _: ronoctua_keymap::Key, _: &mut dyn Doorway) -> Taken {
            Taken::No
        }
        fn command(&mut self, _: &str, _: Option<&str>, _: &mut dyn Doorway) -> Taken {
            Taken::No
        }
        fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}
        fn woken(&mut self, way: &mut dyn Doorway) {
            self.told.set(self.told.get() + 1);
            way.say("answered".to_owned(), crate::echo::Echo::Normal);
        }
        fn lingers(&self) -> bool {
            self.out.get()
        }
    }

    /// FAILS IF: a closed region with work out is dropped (its answer would
    /// never be said), is ranked as open, or is kept after the work came
    /// back — or one with nothing out is kept at all.
    #[test]
    fn a_closed_region_lingers_while_its_work_is_out() {
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        let (out, told) = (
            std::rc::Rc::new(std::cell::Cell::new(true)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        );
        let region = Box::new(Lingering {
            out: std::rc::Rc::clone(&out),
            told: std::rc::Rc::clone(&told),
        });
        assert!(slots.linger(region).is_ok());
        assert!(slots.lingers("fake"));
        assert!(!slots.is_open("fake"), "a lingering region is not shown");
        assert!(
            !slots.is_live("fake"),
            "a region aside is heard as a live one"
        );
        assert!(slots.open_ids().next().is_none());
        let mut w = way();
        slots.call_lingering("fake", &mut w, |r, w| r.woken(w));
        assert_eq!(told.get(), 1);
        assert!(slots.lingers("fake"), "still out");
        out.set(false);
        slots.call_lingering("fake", &mut w, |r, w| r.woken(w));
        assert_eq!(told.get(), 2, "told once more, the answer said");
        assert!(!slots.lingers("fake"), "kept after its work came back");
        assert_eq!(
            w.owed,
            vec![
                Effect::Say("answered".to_owned(), crate::echo::Echo::Normal),
                Effect::Say("answered".to_owned(), crate::echo::Echo::Normal),
            ]
        );
        let idle = Box::new(Lingering {
            out: std::rc::Rc::new(std::cell::Cell::new(false)),
            told,
        });
        assert!(slots.linger(idle).is_err(), "nothing out, nothing kept");
    }

    /// FAILS IF: a hidden region set aside stays in its slot (its opening
    /// command would reach the hidden one again, and nothing would open), is
    /// dropped with work still out (its answer would never be said), or is
    /// kept beside its slot with nothing out — or a region that is not hidden
    /// is set aside at all.
    #[test]
    fn a_hidden_region_steps_aside_for_a_fresh_one() {
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        let mut w = way();
        assert!(!slots.set_aside("fake"), "nothing hidden was set aside");
        let out = std::rc::Rc::new(std::cell::Cell::new(true));
        let told = std::rc::Rc::new(std::cell::Cell::new(0));
        slots
            .keep_hidden(Box::new(Lingering {
                out: std::rc::Rc::clone(&out),
                told: std::rc::Rc::clone(&told),
            }))
            .map_err(|_| "refused")
            .expect("its slot");
        assert!(slots.set_aside("fake"));
        assert!(!slots.is_live("fake"), "the slot still holds it");
        assert!(slots.lingers("fake"), "dropped with its work out");
        assert_eq!(
            slots.closed_command("fake-open", None, &mut w),
            Some(("fake", Closed::Opens)),
            "the opening command does not reach the closed slot"
        );
        slots
            .give_back(open("fake-open", None, &mut w))
            .map_err(|_| "refused")
            .expect("its slot");
        assert!(slots.is_open("fake") && slots.lingers("fake"));
        out.set(false);
        slots.call_lingering("fake", &mut w, |r, w| r.woken(w));
        assert_eq!(told.get(), 1);
        assert!(!slots.lingers("fake") && slots.is_open("fake"));
        assert!(!slots.set_aside("fake"), "a shown region was set aside");
        // ONE WITH NOTHING OUT goes, and nothing is kept beside the slot.
        let idle = slots.take("fake").expect("open");
        drop(idle);
        slots
            .keep_hidden(Box::new(Lingering {
                out: std::rc::Rc::new(std::cell::Cell::new(false)),
                told,
            }))
            .map_err(|_| "refused")
            .expect("its slot");
        assert!(slots.set_aside("fake"));
        assert!(!slots.is_live("fake") && !slots.lingers("fake"));
    }

    /// FAILS IF: what the footer lends a call does not read back by its type,
    /// a type it does not lend reads as something, the doorway that lends it
    /// forgets what was already asked, or lending the footer's fields hides
    /// what it keeps of the machine (`Doorway::kept`).
    #[test]
    fn what_is_lent_reads_back_by_its_type() {
        let jobs: Vec<u64> = vec![3, 4];
        let zone = String::from("UTC");
        let lent = Lent::of(vec![&jobs, &zone]);
        assert_eq!(lent.get::<Vec<u64>>(), Some(&jobs));
        assert_eq!(lent.get::<String>().map(String::as_str), Some("UTC"));
        assert_eq!(lent.get::<u8>(), None, "a type nobody lent");
        assert!(Lent::none().get::<Vec<u64>>().is_none());
        let mut w = way();
        w.repaint();
        let w = w.lending(Lent::of(vec![&jobs]));
        assert_eq!(
            w.owed,
            vec![Effect::Repaint],
            "lending forgot what was asked"
        );
        assert_eq!(Doorway::lent(&w).get::<Vec<u64>>(), Some(&jobs));
        let line: Kept = Rc::new(7_u32);
        let w = way().keeping(vec![line]).lending(Lent::of(vec![&jobs]));
        assert_eq!(
            kept::<u32>(&w).as_deref(),
            Some(&7),
            "lending hid what the footer keeps"
        );
        let ui = crate::door::Ui::for_tests(
            ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
        );
        assert!(
            Doorway::lent(&ui).get::<Vec<u64>>().is_none(),
            "a window lends nothing"
        );
    }

    /// FAILS IF: the footer's doorway stops writing down a page's move by
    /// lines, a queued copy or a desk's ask, or writes them out of order.
    #[test]
    fn the_footers_doorway_writes_down_a_copy_queued_and_a_desks_ask() {
        let mut w = way();
        w.page(Paging::Lines(-2));
        w.queue_copied("transfers-add", ronoctua_transfers::where_to::Wish::plain());
        w.desk(crate::desk::Ask::ReadAll);
        assert_eq!(
            w.owed,
            vec![
                Effect::Page(Paging::Lines(-2)),
                Effect::QueueCopied("transfers-add", ronoctua_transfers::where_to::Wish::plain()),
                Effect::Desk(crate::desk::Ask::ReadAll),
            ]
        );
    }

    /// FAILS IF: the window's doorway stops handing the footer what only the
    /// footer does — what was copied into the queue, by the command that does
    /// it — or a desk's ask, which a window never makes, closes it or reaches
    /// the footer.
    #[test]
    fn the_windows_doorway_hands_the_footer_a_copy_queued() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        let way: &mut dyn Doorway = &mut ui;
        way.queue_copied("transfers-add", ronoctua_transfers::where_to::Wish::plain());
        way.desk(crate::desk::Ask::ReadAll);
        assert!(!ui.quit, "the window closed");
        assert_eq!(ui.orders(), &["command transfers-add".to_owned()]);
    }

    /// FAILS IF: a pair's default stops being the footer's standard line —
    /// the key's tone, the secondary text, flush right with its colon — a
    /// region that answers the two plain questions stops being asked them by
    /// the defaults of the two that take the width and the cell, or a row
    /// told by default says more than the row.
    #[test]
    fn the_defaults_are_the_footers_standard() {
        let p = Pair::default();
        assert_eq!(p.key_tone, ronoctua_tokens::color::TEXT_KEY);
        assert_eq!(p.text_tone, ronoctua_tokens::color::TEXT_SECONDARY);
        assert!(p.flush_right && p.colon && !p.aside && !p.here);
        let mut fake = Fake {
            cursor: 2,
            heard: Vec::new(),
        };
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 80,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        assert_eq!(fake.cursor_in(&seen), Some(2));
        let mut w = way();
        fake.clicked_at(5, Some(9), &mut w);
        assert_eq!(fake.cursor, 5);
        assert_eq!(fake.row_count(&seen), 0);
        assert!(!fake.closes_without_the_keyboard());
        assert!(!fake.wheel(0, 0, 1, &mut w));
        // AND A ROW TOLD BY DEFAULT is the row, saying nothing more.
        assert!(fake.told(&seen, 0, 12).is_empty(), "told is not the rows");
        let told = Told::from(Row {
            fields: Vec::new(),
            takes: true,
        });
        assert!(told.row.takes && told.about.is_none() && told.pair.is_none());
    }

    /// FAILS IF: a painted picture goes up when it should not — a still one
    /// every frame, tens of mebibytes a second across the bus — or does not
    /// when it should: a moving one, every frame; a still or a drawn one,
    /// when its texture is missing.
    #[test]
    fn a_picture_goes_up_by_its_rule() {
        let rgba = std::sync::Arc::new(ronoctua_render::image::Rgba {
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0, 255],
        });
        let at = ronoctua_render::image::Area {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let of = |source| Painted {
            key: "k".to_owned(),
            area: at,
            source,
        };
        let still = of(Source::Still(std::sync::Arc::clone(&rgba)));
        assert!(still.pixels(true).is_some());
        assert!(still.pixels(false).is_none());
        let moving = of(Source::Moving(std::sync::Arc::clone(&rgba)));
        assert!(moving.pixels(false).is_some());
        let drawn = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&drawn);
        let svg = of(Source::Drawn(std::sync::Arc::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            None
        })));
        assert!(svg.pixels(false).is_none());
        assert_eq!(
            drawn.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "drawn with the texture up"
        );
        assert!(svg.pixels(true).is_none(), "a drawing that did not draw");
        assert_eq!(drawn.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    /// FAILS IF: news is read as another type, or two values are one news.
    #[test]
    fn news_is_a_value_read_by_its_type() {
        let a = News::of(7_u32);
        assert_eq!(a.read::<u32>(), Some(&7));
        assert_eq!(a.read::<u8>(), None);
        assert_eq!(a.clone(), a);
        assert_ne!(News::of(7_u32), a);
    }

    /// FAILS IF: the footer's doorway does not write down a page that starts
    /// again or what a view tells the footer, or answers another place than
    /// it was given.
    #[test]
    fn the_footers_doorway_writes_down_a_new_page_and_news() {
        let area = Area {
            width: 800.0,
            bottom: 600.0,
            first: 3,
            lines: 9,
            columns: 60,
        };
        let mut w = way().at(Some(area));
        assert_eq!(w.area(), Some(area));
        let news = News::of("read");
        w.first_page();
        w.news(news.clone());
        assert_eq!(w.owed, vec![Effect::FirstPage, Effect::News(news)]);
        assert!(std::rc::Rc::ptr_eq(&w.words(), &w.texts));
        assert_eq!(way().area(), None);
    }

    /// FAILS IF: the window's list does not go back to its top for a page that
    /// opened, or the window keeps a view's news for a footer it is not.
    #[test]
    fn the_windows_doorway_starts_a_page_at_its_top() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        ui.offset = 12;
        let way: &mut dyn Doorway = &mut ui;
        way.first_page();
        way.news(News::of(1_u8));
        assert!(way.area().is_some());
        assert_eq!(ui.offset, 0);
    }

    /// FAILS IF: a region that writes no click of its own by the cell loses
    /// its clicks, or one that says nothing of its stretches lights less than
    /// the whole row — the defaults are every list's.
    #[test]
    fn a_row_is_one_thing_unless_the_region_says_otherwise() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 80,
            lines: 10,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let mut fake = Fake {
            cursor: 0,
            heard: Vec::new(),
        };
        let mut w = way();
        fake.clicked_at(4, Some(12), &mut w);
        assert_eq!(fake.cursor, 4, "the cell's click is the row's");
        assert_eq!(fake.lit(4, Some(12), &seen), Lit::Row);
        assert_eq!(Lit::default(), Lit::Row);
    }

    /// FAILS IF: the footer's doorway does the follow or the stepping aside
    /// inside the call instead of writing them down — or the window's
    /// follows a cursor that was off its page, or closes for a terminal.
    #[test]
    fn following_and_stepping_aside_are_each_doors() {
        let mut w = way();
        w.follow_if_seen(3);
        w.step_aside();
        assert_eq!(w.owed, vec![Effect::FollowIfSeen(3), Effect::StepAside]);
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        let room = ui.room_for_rows();
        assert!(room > 2, "a page to follow on");
        ui.offset = 2;
        let way: &mut dyn Doorway = &mut ui;
        way.follow_if_seen(0);
        way.step_aside();
        assert!(!ui.follow_owed, "row 0 was above the page");
        assert!(!ui.quit, "a window stays for a terminal");
        let way: &mut dyn Doorway = &mut ui;
        way.follow_if_seen(2 + room - 1);
        assert!(ui.follow_owed, "the last row of the page was on it");
    }

    /// What a fake feature keeps for the session: how many of its commands
    /// it was asked with the region closed, and whether it heard a wake.
    #[derive(Default)]
    struct Counted {
        asked: u32,
        woken: bool,
    }

    impl Resident for Counted {
        fn woken(&mut self, _: &mut dyn Doorway) {
            self.woken = true;
        }
    }

    fn counted() -> Residence {
        Residence::of(Counted::default())
    }

    /// `fake-count` counts on the resident it is handed, and nothing else.
    fn counting(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
        if command != "fake-count" {
            return Closed::NotMine;
        }
        if let Some(c) = way.residence().and_then(|k| k.as_a::<Counted>()) {
            c.borrow_mut().asked += 1;
        }
        way.wake().call();
        Closed::Taken
    }

    const COUNTING: Entry = Entry {
        id: "fake",
        closed: counting,
        open,
    };

    /// FAILS IF: a resident is kept for a region nobody registered, or two
    /// for one region — a feature whose commands would read the wrong one.
    #[test]
    fn a_resident_stands_beside_its_region() {
        let beside = |id| Residing { id, make: counted };
        let refused = Slots::of(&[FAKE])
            .and_then(|s| s.with_residents(&[beside("other")]))
            .expect_err("a resident with no region");
        assert!(refused.contains("`other`"), "{refused}");
        let refused = Slots::of(&[FAKE])
            .and_then(|s| s.with_residents(&[beside("fake"), beside("fake")]))
            .expect_err("two residents");
        assert!(refused.contains("two residents"), "{refused}");
        let slots = Slots::of(&[FAKE])
            .and_then(|s| s.with_residents(&[beside("fake")]))
            .expect("one resident");
        assert_eq!(slots.resident_ids().collect::<Vec<_>>(), vec!["fake"]);
        assert!(slots.residence("fake").is_some() && slots.residence("other").is_none());
    }

    /// FAILS IF: a closed region's command is not handed its own resident
    /// and its own wakes — a scan asked with the view closed would start a
    /// thread whose answer wakes nothing, and its state would be nobody's.
    #[test]
    fn a_closed_command_reads_its_resident_and_wakes_its_loop() {
        let mut slots = Slots::of(&[COUNTING])
            .and_then(|s| {
                s.with_residents(&[Residing {
                    id: "fake",
                    make: counted,
                }])
            })
            .expect("one resident");
        let heard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mark = std::sync::Arc::clone(&heard);
        slots.set_wakes(
            "fake",
            Some((
                crate::wake::Wake::new(move || {
                    mark.store(true, std::sync::atomic::Ordering::SeqCst);
                }),
                crate::wake::Wake::new(|| {}),
            )),
        );
        let mut w = way();
        assert_eq!(
            slots.closed_command("fake-count", None, &mut w),
            Some(("fake", Closed::Taken))
        );
        assert!(
            heard.load(std::sync::atomic::Ordering::SeqCst),
            "its wake was not its own"
        );
        let kept = slots
            .residence("fake")
            .and_then(|k| k.as_a::<Counted>())
            .expect("the resident");
        assert_eq!(kept.borrow().asked, 1);
        // THE RESIDENT IS THE FOOTER'S TOO, the same value.
        let hooks = slots.residence("fake").expect("the resident").hooks;
        hooks.borrow_mut().woken(&mut w);
        assert!(kept.borrow().woken, "two values, not one");
        slots.set_wakes("fake", None);
        assert!(slots.wakes("fake").is_none());
    }

    /// FAILS IF: a picture over rows is placed outside its box, or a cut
    /// keeps rows the scroll took.
    #[test]
    fn a_picture_over_rows_fits_its_box_and_is_cut_by_the_scroll() {
        let (area, crop) = cover_place((40, 8), (10.0, 20.0), (600, 600), (0, 8)).expect("drawn");
        assert!(area.width <= 400.0 && area.height <= 160.0, "{area:?}");
        assert!(
            crop.0 <= f32::EPSILON && (crop.1 - 1.0).abs() < 1e-4,
            "{crop:?}"
        );
        let (_, half) = cover_place((40, 8), (10.0, 20.0), (600, 600), (4, 8)).expect("drawn");
        assert!(half.0 > 0.4 && half.1 > 0.99, "{half:?}");
        assert!(cover_place((40, 8), (10.0, 20.0), (600, 600), (8, 8)).is_none());
        let rgba = ronoctua_render::image::Rgba::new(2, 4, vec![9; 2 * 4 * 4]).expect("pixels");
        let cut = cropped(&rgba, (0.5, 1.0)).expect("the lower half");
        assert_eq!((cut.width, cut.height), (2, 2));
        assert!(cropped(&rgba, (0.6, 0.6)).is_none());
        assert_eq!(LineLights::default(), LineLights::Row);
    }

    /// FAILS IF: a closed region with no wakes hung is asked with the wake
    /// the entry before it was given — a thread it starts would wake another
    /// feature's resident — or the doorway's own wake is lost on the way.
    #[test]
    fn a_closed_command_with_no_wakes_keeps_the_doorways_own() {
        fn later(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
            if command != "later-count" {
                return Closed::NotMine;
            }
            way.wake().call();
            Closed::Taken
        }
        const LATER: Entry = Entry {
            id: "later",
            closed: later,
            open,
        };
        let mut slots = Slots::of(&[COUNTING, LATER])
            .and_then(|s| {
                s.with_residents(&[Residing {
                    id: "fake",
                    make: counted,
                }])
            })
            .expect("two regions, one resident");
        let resident = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mark = std::sync::Arc::clone(&resident);
        slots.set_wakes(
            "fake",
            Some((
                crate::wake::Wake::new(move || {
                    mark.store(true, std::sync::atomic::Ordering::SeqCst);
                }),
                crate::wake::Wake::new(|| {}),
            )),
        );
        let own = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mine = std::sync::Arc::clone(&own);
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut w = Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(move || {
                mine.store(true, std::sync::atomic::Ordering::SeqCst);
            }),
        );
        assert_eq!(
            slots.closed_command("later-count", None, &mut w),
            Some(("later", Closed::Taken))
        );
        assert!(
            !resident.load(std::sync::atomic::Ordering::SeqCst),
            "the resident before it was woken"
        );
        assert!(
            own.load(std::sync::atomic::Ordering::SeqCst),
            "the doorway's own wake was lost"
        );
        assert!(w.residence.is_none(), "a resident of another region's");
    }

    /// FAILS IF: `Taken` reads the wrong way round.
    #[test]
    fn taken_is_what_it_says() {
        assert!(Taken::of(true).took());
        assert!(!Taken::of(false).took());
        assert_eq!(Shape::default(), Shape::List);
    }

    /// FAILS IF: a region kept hidden reads as open — the footer would rank
    /// a view nobody sees, and hand it the keyboard — or is not live, so its
    /// threads go unheard; or a closed command is asked of it as if it were
    /// closed; or showing it does not put it on the screen.
    #[test]
    fn a_hidden_region_lives_unranked_until_it_is_shown() {
        let mut slots = Slots::of(&[FAKE]).expect("one region");
        let mut w = way();
        slots
            .keep_hidden(open("fake-down", None, &mut w))
            .map_err(|_| "refused")
            .expect("its slot");
        assert!(!slots.is_open("fake"), "a hidden region is ranked");
        assert!(slots.is_live("fake") && slots.is_hidden("fake"));
        assert_eq!(slots.open_ids().count(), 0);
        assert_eq!(slots.live_ids().collect::<Vec<_>>(), vec!["fake"]);
        assert_eq!(slots.kept_shape("fake"), Shape::List);
        assert_eq!(
            slots.closed_command("fake-down", None, &mut w),
            None,
            "a live region was asked as a closed one"
        );
        // A CALL KEEPS IT HIDDEN: lent and given back as it was.
        assert_eq!(slots.call("fake", &mut w, |r, _| r.cursor()), Some(Some(0)));
        assert!(slots.is_hidden("fake"));
        assert!(slots.show("fake"));
        assert!(slots.is_open("fake") && !slots.is_hidden("fake"));
        assert!(slots.take("fake").is_some());
        assert!(!slots.is_live("fake") && !slots.show("fake"));
    }

    /// FAILS IF: the footer's doorway does the three newer asks inside the
    /// call instead of writing them down, in order.
    #[test]
    fn showing_paging_and_giving_way_are_written_down() {
        let mut w = way();
        w.show();
        w.page(Paging::Top(3));
        w.give_way("belongings-open", "belongings-window");
        assert_eq!(
            w.owed,
            vec![
                Effect::Show,
                Effect::Page(Paging::Top(3)),
                Effect::GiveWay("belongings-open"),
            ]
        );
    }

    /// FAILS IF: the defaults of the counted, windowed rows disagree with
    /// the rows themselves, or a row is not its own control.
    #[test]
    fn the_windowed_rows_are_the_rows_by_default() {
        struct Three;
        impl Region for Three {
            fn id(&self) -> &'static str {
                "three"
            }
            fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
                (0..3)
                    .map(|n| Row {
                        fields: Vec::new(),
                        takes: n != 1,
                    })
                    .collect()
            }
            fn hint(&self, _: &Seen<'_>) -> Vec<crate::chrome::Piece> {
                Vec::new()
            }
            fn cursor(&self) -> Option<usize> {
                Some(2)
            }
            fn key(&mut self, _: ronoctua_keymap::Key, _: &mut dyn Doorway) -> Taken {
                Taken::No
            }
            fn command(&mut self, _: &str, _: Option<&str>, _: &mut dyn Doorway) -> Taken {
                Taken::No
            }
            fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}
        }
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 80,
            lines: 10,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let r = Three;
        assert_eq!(r.row_count(&seen), 3);
        let window = r.rows_from(&seen, 1, usize::MAX);
        assert_eq!(
            window.iter().map(|w| w.takes).collect::<Vec<_>>(),
            vec![false, true]
        );
        assert_eq!(r.control_of(2, &seen), 2);
        assert_eq!(r.cursor_in(&seen), Some(2));
        assert!(!r.lingers());
    }

    /// FAILS IF: a burst of posts wakes the loop once per post (a rebuild's
    /// journal would be a turn of the loop per line), a post is lost between
    /// a take and the next wake, the order is not kept, or a thread is not
    /// told its door is gone.
    #[test]
    fn an_inbox_wakes_once_per_batch_and_loses_nothing() {
        let woke = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&woke);
        let inbox = Inbox::new(crate::wake::Wake::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        }));
        let post = inbox.post();
        let twin = post.clone();
        assert!(post.send(1) && post.send(2) && twin.send(3));
        assert_eq!(woke.load(Ordering::SeqCst), 1, "a wake per post");
        assert_eq!(inbox.take(), vec![1, 2, 3]);
        assert!(inbox.take().is_empty());
        assert!(post.send(4));
        assert_eq!(woke.load(Ordering::SeqCst), 2, "the next post did not wake");
        assert_eq!(inbox.take(), vec![4]);
        let from_a_thread = std::thread::spawn(move || (0..100).all(|n| post.send(n)));
        assert!(from_a_thread.join().expect("the thread"));
        assert_eq!(inbox.take(), (0..100).collect::<Vec<_>>());
        let last = inbox.post();
        drop(inbox);
        assert!(!last.send(0), "a thread is not told its door is gone");
    }

    /// FAILS IF: the footer's doorway lends a kept line by the `Rc`'s own
    /// type — the same for every one of them, so the first kept thing would
    /// answer for every type asked — or lends one of another type, or a
    /// window's doorway lends anything.
    #[test]
    fn the_footers_doorway_lends_what_it_keeps_by_the_type_inside() {
        let kept: Vec<Kept> = vec![
            Rc::new(7_u32),
            Rc::new(String::from("the reading")),
            Rc::new(std::cell::Cell::new(3_u8)),
        ];
        let w = way().keeping(kept.clone());
        assert_eq!(super::kept::<u32>(&w).as_deref(), Some(&7));
        assert_eq!(
            super::kept::<String>(&w).as_deref().map(String::as_str),
            Some("the reading")
        );
        assert_eq!(super::kept::<u64>(&w), None, "a type nothing keeps");
        // A WISH IS SHARED, not copied: what a region sets is what the
        // footer reads back.
        if let Some(wish) = super::kept::<std::cell::Cell<u8>>(&w) {
            wish.set(9);
        }
        let footers = kept[2]
            .clone()
            .downcast::<std::cell::Cell<u8>>()
            .expect("the footer's own cell");
        assert_eq!(footers.get(), 9, "the region set a copy");
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let ui = crate::door::Ui::for_tests(texts);
        assert_eq!(super::kept::<u32>(&ui), None, "a window lends nothing");
    }

    /// FAILS IF: the footer's doorway moves the page inside the call instead
    /// of writing it down for after it, in the order asked, or the window's
    /// list does not go back to its top — or following, which the window
    /// does after every key and click already, moves it.
    #[test]
    fn the_page_is_written_down_in_the_footer_and_done_in_the_window() {
        let mut w = way();
        w.first_page();
        w.follow();
        assert_eq!(w.owed, vec![Effect::FirstPage, Effect::Follow]);
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        ui.offset = 12;
        Doorway::follow(&mut ui);
        assert_eq!(ui.offset, 12, "the window's follow moved its list");
        Doorway::first_page(&mut ui);
        assert_eq!(ui.offset, 0);
    }

    /// FAILS IF: rows that came or went are done inside the call instead of
    /// written down for after it, or written down as a bare frame — the
    /// footer's height would stay the old list's — or the window does more
    /// than paint again.
    #[test]
    fn rows_that_came_or_went_are_written_down_for_after_the_call() {
        let mut w = way();
        w.rows_changed();
        w.repaint();
        assert_eq!(w.owed, vec![Effect::RowsChanged, Effect::Repaint]);
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        ui.offset = 12;
        Doorway::rows_changed(&mut ui);
        assert_eq!(ui.offset, 12, "the window's rows moved its list");
        assert!(ui.orders().is_empty() && ui.launches().is_empty() && !ui.quit);
    }

    /// FAILS IF: a region's wish for a line is done inside the call instead
    /// of written down for after it, or written down as something else — or
    /// a window's door does anything with it.
    #[test]
    fn a_wish_for_a_line_is_written_down_for_after_the_call() {
        let mut w = way();
        want::<u32>(&mut w);
        assert_eq!(w.owed, vec![Effect::Want(TypeId::of::<u32>())]);
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        want::<u32>(&mut ui);
        assert!(ui.orders().is_empty() && ui.launches().is_empty());
    }

    /// FAILS IF: a region that says nothing of its band, its lights or its
    /// caret is not the plain list the footer draws — the band would vanish
    /// or a field would stop lighting.
    #[test]
    fn a_region_that_says_nothing_more_is_a_plain_list() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 80,
            lines: 10,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let area = Area {
            width: 800.0,
            bottom: 300.0,
            first: 2,
            lines: 10,
            columns: 80,
        };
        let fake = Fake {
            cursor: 3,
            heard: Vec::new(),
        };
        assert_eq!(fake.band(Some(5), Some(40), &area, &seen), Band::Row);
        assert_eq!(fake.band(None, None, &area, &seen), Band::Row);
        assert_eq!(fake.lights(), Lights::Field);
        assert_eq!(fake.caret(5, &seen), None);
    }

    /// FAILS IF: the footer's doorway forgets the page it was told, or does
    /// the clipboard's act while the region is borrowed instead of writing
    /// it down with the history it was aimed at — or the doorway that lends
    /// the footer's fields hides either.
    #[test]
    fn the_footers_doorway_knows_its_page_and_writes_down_a_clipboard_act() {
        let w = way();
        assert_eq!(w.page_shown(), Page::default());
        let mut w = w.with_page(Page {
            offset: 4,
            lines: 9,
        });
        assert_eq!(
            w.page_shown(),
            Page {
                offset: 4,
                lines: 9
            }
        );
        w.clipboard(crate::clipboard_wire::Act::Pin { n: 2 }, 11);
        assert_eq!(
            w.owed,
            vec![Effect::Clipboard(
                crate::clipboard_wire::Act::Pin { n: 2 },
                11
            )]
        );
        let mut lending = way()
            .with_page(Page {
                offset: 1,
                lines: 3,
            })
            .lending(Lent::none());
        assert_eq!(lending.page_shown().offset, 1);
        lending.clipboard(crate::clipboard_wire::Act::Clear, 5);
        assert_eq!(
            lending.way.owed,
            vec![Effect::Clipboard(crate::clipboard_wire::Act::Clear, 5)]
        );
    }

    /// FAILS IF: the window's doorway does the clipboard's act itself — the
    /// history is the footer's, and the act goes down its socket with the
    /// generation it was aimed at — or forgets its own page.
    #[test]
    fn the_windows_clipboard_act_is_an_order_for_the_footer() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut ui = crate::door::Ui::for_tests(texts);
        ui.offset = 3;
        let page = Doorway::page_shown(&ui);
        assert_eq!(page.offset, 3);
        assert_eq!(page.lines, ui.room_for_rows());
        Doorway::clipboard(&mut ui, crate::clipboard_wire::Act::Remove { n: 1 }, 6);
        let orders = ui.orders().to_vec();
        assert_eq!(orders.len(), 1, "{orders:?}");
        let rest = orders[0]
            .strip_prefix(crate::clipboard_wire::ACT)
            .expect("an act");
        assert_eq!(
            crate::clipboard_wire::Act::parse(rest),
            Some((6, crate::clipboard_wire::Act::Remove { n: 1 }))
        );
    }

    /// FAILS IF: an order a thread leaves through the footer's doorway is not
    /// the one the footer gave it — the clone's answer would be lost, or run
    /// as another command — or the doorway that lends the footer's fields
    /// hands the region orders that go nowhere.
    #[test]
    fn a_threads_order_goes_where_the_door_said() {
        let left = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let kept = Arc::clone(&left);
        let w = way().with_orders(Orders::new(move |command, argument| {
            if let Ok(mut l) = kept.lock() {
                l.push((command.to_owned(), argument));
            }
        }));
        let lending = w.lending(Lent::none());
        let orders = lending.orders();
        std::thread::spawn(move || orders.run("dev-clone", "\u{0}ok".to_owned()))
            .join()
            .expect("the thread");
        assert_eq!(
            left.lock().map(|l| l.clone()).unwrap_or_default(),
            vec![("dev-clone".to_owned(), "\u{0}ok".to_owned())]
        );
        assert!(
            lending.way.owed.is_empty(),
            "an order is not an effect of the call"
        );
    }

    /// FAILS IF: a path sent as an argument comes back as another — a
    /// folder whose name is not UTF-8 lossily turned into one that is not
    /// there — or a path that is text is sent as anything but itself (what
    /// a person types and a photograph passes), or what follows a NUL that
    /// `path_argument` did not write is read as some folder.
    #[test]
    fn a_path_goes_through_an_argument_whole() {
        use std::os::unix::ffi::OsStrExt as _;
        let text = std::path::Path::new("/home/ada/repositório");
        assert_eq!(path_argument(text), "/home/ada/repositório");
        assert_eq!(argument_path(&path_argument(text)).as_deref(), Some(text));
        let bytes = std::path::Path::new(std::ffi::OsStr::from_bytes(b"/home/ada/caf\xe9/\x7f"));
        assert!(bytes.to_str().is_none(), "the sample is text");
        let sent = path_argument(bytes);
        assert!(!sent.contains('\u{fffd}'), "{sent:?}");
        assert_eq!(argument_path(&sent).as_deref(), Some(bytes));
        for wrong in ["\u{0}", "\u{0}2f6", "\u{0}zz", "\u{0}+f2f"] {
            assert_eq!(argument_path(wrong), None, "{wrong:?}");
        }
    }
}
