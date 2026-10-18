//! THE VOCABULARY A LIST IS DRAWN IN — fields, rows, sections, panes, inks
//! and the hits a click lands on.
//!
//! # Why a module of its own
//!
//! It was the first three thousand lines of `files.rs`, because the file
//! manager was the first view drawn out of [`Field`]s — and then every other
//! view and both doors came to draw the same way, so forty modules reached
//! into the file manager for the words they draw in. A view that names the
//! file manager to say what a cell of its own is recompiles with every line
//! of the file manager, and could never live in a crate the file manager
//! does not depend on. This is the half every door and view shares; what is
//! left in `files.rs` is the manager's own state and keys.
//!
//! Nothing here holds state or reads the disk: it is types and arithmetic,
//! which is what lets it sit under every view (the shell's crate graph,
//! `docs/ARCHITECTURE.md`).

use ronoctua_files::{
    listing::Order,
    mode::{self, Explanation},
};
use ronoctua_tokens::Color;

/// A column of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The glyph for what this is.
    Icon,
    /// The name. Never dropped, whatever the width.
    Name,
    /// `drwxr-xr-x`, one cell per fact.
    Perms,
    /// Who owns it.
    Owner,
    /// Which group.
    Group,
    /// How big.
    Size,
    /// When it last changed.
    Modified,
}

impl Column {
    /// The text key naming this column in the header.
    #[must_use]
    pub const fn label(self) -> &'static str {
        use crate::key;
        match self {
            // The icon column has no header: a one-cell label is not a word,
            // and the drawing under it says what it is.
            Self::Icon => key::FILES_COLUMN_ICON,
            Self::Name => key::FILES_COLUMN_NAME,
            Self::Perms => key::FILES_COLUMN_PERMS,
            Self::Owner => key::FILES_COLUMN_OWNER,
            Self::Group => key::FILES_COLUMN_GROUP,
            Self::Size => key::FILES_COLUMN_SIZE,
            Self::Modified => key::FILES_COLUMN_MODIFIED,
        }
    }

    /// Which sort order clicking this column asks for.
    ///
    /// Three of the seven have no order of their own — one cannot sort by
    /// permission bits in any way a person means — and they fall back to the
    /// kind, which is the closest thing to what somebody clicking there wants:
    /// the files grouped by what they are.
    #[must_use]
    pub const fn sorts_by(self) -> Order {
        match self {
            Self::Size => Order::Size,
            Self::Modified => Order::Modified,
            Self::Icon | Self::Perms | Self::Owner | Self::Group => Order::Kind,
            Self::Name => Order::Name,
        }
    }

    /// The name this column crosses to `ronoctua_design::Table` under.
    ///
    /// A `&'static str` because that crate carries no buffer's and no shell's
    /// vocabulary — it resolves ids, and the caller finds its own field again.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Icon => "icon",
            Self::Name => "name",
            Self::Perms => "perms",
            Self::Owner => "owner",
            Self::Group => "group",
            Self::Size => "size",
            Self::Modified => "modified",
        }
    }

    /// The column an id names, if it is one of ours.
    pub fn of_id(id: &str) -> Option<Self> {
        [
            Self::Icon,
            Self::Name,
            Self::Perms,
            Self::Owner,
            Self::Group,
            Self::Size,
            Self::Modified,
        ]
        .into_iter()
        .find(|c| c.id() == id)
    }

    /// How many cells this column wants.
    ///
    /// Fixed for everything but the name, which takes what is left. `Modified`
    /// is sixteen because that is `2026-08-20 14:32` exactly, and a date column
    /// that elides is a date column that lies about the year.
    pub const fn width(self) -> u16 {
        match self {
            // THE GLYPH, AND NOTHING ELSE. The air is the columns' own `gap`,
            // which `layout_beside` already adds after every column.
            //
            // It used to be two — "the glyph plus one cell of air" — and the
            // gap was added on top, so the icon had TWO cells between its ink
            // and the first letter of the name while the whole table had one
            // between any other pair of columns. Measured off the owner's
            // screen: 25 px on the icon's right against 21 on its left, in a
            // grid whose cell is 10,8. *"The icon spacing is far too wide
            // in the listing."*
            //
            // One cell of air is what every other column gets, and it is what
            // this one gets now. The air itself did not go anywhere; it stopped
            // being paid twice.
            Self::Icon => 1,
            Self::Name => 0,
            Self::Perms => 10,
            Self::Owner | Self::Group => 8,
            // `1023,9 KiB` is the longest a size gets.
            Self::Size => 10,
            Self::Modified => 16,
        }
    }
}

/// The air AFTER the last column.
///
/// Without it the date ends flush against the edge and reads as a column that
/// was cut off rather than one that ended. It is not the modeline's
/// `edgeRight`, which is zero on purpose so the notification icon can sit
/// flush: a table is not a status line.
pub const EDGE_RIGHT: u16 = ronoctua_tokens::space::FILES_EDGERIGHT;

/// The cell the rule between the sidebar and the listing sits in.
///
/// A one-pixel rule and one cell of grid: separation by whitespace and a rule,
/// never by a container (invariant 15).
pub const RULE: u16 = 1;

/// The narrowest a PANEL may be squeezed to.
///
/// `Dimensions  3024 × 4032` is twenty-three cells and it is the widest
/// ordinary fact there is; twenty-four is that plus the air on its right. Below
/// it the value elides on the row that exists to carry the value, which is the
/// same argument [`NAME_FLOOR`] makes about a name and [`SIDE_FLOOR`] about a
/// place.
pub const PANEL_FLOOR: u16 = 24;

/// Which way a bar's track runs.
///
/// A drag has to say so: the preview has two bars, and a pointer holding one of
/// them is not holding the other. Before this the drag was keyed by pane alone,
/// which was enough while every section had exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// The list, top to bottom.
    Down,
    /// The line, left to right.
    Across,
    /// The film, beginning to end.
    ///
    /// A third axis and not a third mechanism: the timeline is a
    /// [`ronoctua_design::Scroll`] whose `total` is the film's length in tenths
    /// of a second and whose `offset` is where the picture is. `bar_across`
    /// draws the cursor and `offset_at` reads it back, so where it is drawn is
    /// where it is grabbed — the law the bars already keep, applied to time.
    Time,
}

/// A scrollbar somebody could grab: whose it is, which way it runs, and where.
///
/// Named because three functions hand it about — each door's "draw them all"
/// and each door's "which one is under the pointer" — and the whole point of
/// the pair is that they are the SAME rectangles. A tuple spelled out four
/// times is four chances for them to stop being the same.
pub type Bar = (Pane, Axis, (f32, f32, f32, f32));

/// How many lines each kind of section is given.
///
/// Two numbers and not one, because the panels and the listing are not the same
/// height whenever the panels are down in the strip — and a bar asked about the
/// wrong one answers "nothing is cut" for a list that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    /// The listing's, which the sidebar shares.
    pub list: usize,
    /// The panels', which is the strip's height when they are down there.
    pub panel: usize,
}

/// What a point resolves to: the section, the row inside THAT section, and the
/// field's target.
///
/// The section comes first: read off a line of the region laid end to end,
/// a pointer resting in the empty part of one section described a row of
/// another.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// Which section owns the cell.
    pub pane: Pane,
    /// The row inside that section. `usize::MAX` is the listing's chrome, which
    /// both doors already spell that way.
    pub row: usize,
    /// What the field under the point does.
    pub hit: Hit,
    /// What it says about itself, for the line above.
    pub explain: Option<Explanation>,
}

/// How many cells a scrollbar takes.
///
/// One, and it is the cell at the section's right-hand edge. Named because
/// three places have to agree about it: where the bar is DRAWN
/// (`Scroll::bar`), where it is GRABBED (the same rectangle, asked again), and
/// where the content must STOP — which is what this constant is for. The third
/// was missing, and the owner photographed a line of code running under the
/// thumb.
pub const BAR: u16 = 1;

/// What tells a moving frame's texture from its file's photograph.
///
/// A suffix and not a second field, because the doors carry ONE key from
/// `View::preview_key` to `Images::upload_it` and back — see that function for
/// why there have to be two textures at all.
pub const FRAME_MARK: &str = ":frame";

/// Where the sound glyph sits, counted from the transport's first cell.
///
/// Two, which is one for the play glyph and one of air. Named for the reason
/// [`BAR`] is: the row is built here and the track is drawn by the doors, and a
/// number written twice is a number that stops agreeing.
pub const TRANSPORT_SOUND: u16 = 2;

/// How many cells the expand button takes on the transport's row.
///
/// The glyph, and one of air between it and the clock. The owner asked for it
/// *"à direita da marcação de tempo"*, so it sits past the clock and the clock
/// moves left by exactly this — and [`track_px`] adds the same to its tail,
/// because the track's two ends have to keep the identical air the owner asked
/// for. The two numbers are one constant for that reason.
pub const TRANSPORT_THEATRE: u16 = 2;

/// The air on EACH SIDE of the timeline's track.
///
/// The owner asked for it in those words — *"com espaços em branco idênticos na
/// esquerda e na direita"* — and identical is what a single constant makes it.
/// Before, the left gap was a fixed cell and the right gap was whatever the
/// clock's own width happened to leave over: one cell against two, on a control
/// whose two ends are a hand's width apart.
pub const TRANSPORT_AIR: u16 = 2;

/// Where the transport's first cell is, counted from the panel's.
///
/// One, and it is the air the owner asks for beside every drawing: a glyph
/// against the panel's own edge reads as a glyph that was cut.
pub const TRANSPORT_LEAD: u16 = 1;

/// The timeline's track inside a panel, in pixels.
///
/// # One answer, three readers
///
/// [`Section::time_bar_box`] draws the cursor with it and both doors turn a
/// pointer into a fraction with it. Written out at each of the three, the day
/// somebody moved the clock a cell the drawing and the grabbing would stop
/// agreeing — which is the defect this manager has already paid for twice, in
/// the facts' bar and in the listing's.
///
/// `clock` is how many cells the time at the right-hand end occupies, MEASURED
/// and not assumed: it is `1:23 / 5:40` on one film and `3:44 / 10:34` on
/// another, and a fixed reserve leaves a different gap on each. The air is
/// [`TRANSPORT_AIR`] on both sides, which is what makes the two identical.
///
/// `None` when the panel is too narrow for a track worth grabbing.
#[must_use]
pub fn track_px(left: f32, wide: f32, cell: f32, clock: u16) -> Option<(f32, f32)> {
    // THE GLYPHS, THEN THE AIR: play, a space, sound, then the air.
    let lead = f32::from(TRANSPORT_LEAD + TRANSPORT_SOUND + 1 + TRANSPORT_AIR) * cell;
    let tail = f32::from(clock + TRANSPORT_THEATRE + TRANSPORT_AIR + TRANSPORT_LEAD) * cell;
    let track = wide - lead - tail;
    (track > cell).then_some((left + lead, track))
}

/// Where the transport's rectangles are measured from, in pixels.
///
/// # Why five numbers travel as ONE value
///
/// Because they were five `f32` parameters, and five `f32` parameters are
/// positionally interchangeable: the compiler has nothing to say about a
/// caller that passes them in the wrong order, and one did. The two sites that
/// offer the timeline as something to GRAB — one in each door — called
/// `time_track_box(x, top, wide, cell, line_px)` against a signature reading
/// `(left, wide, cell, line_px, bottom)`. Every argument after the first was
/// wrong, it compiled without a word, and the grab rectangle landed somewhere
/// nobody could reach.
///
/// The symptom was the owner's: *"todas as linhas de tempo não estão
/// funcionando; ao clicar em uma linha do tempo a mídia pausa/play"* — the
/// press missed the track, fell through to the row's own field, and that field
/// is [`Hit::Play`].
///
/// It is the trap `AGENTS.md` records for `#[repr(C)]`, in ordinary Rust: a
/// layout that is promised rather than checked. Named fields are the check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Foot {
    /// The transport's own left edge — see [`Section::fields_px`].
    pub left: f32,
    /// How wide the transport is.
    pub wide: f32,
    /// One cell.
    pub cell: f32,
    /// One line.
    pub line_px: f32,
    /// The bottom of the band the section was drawn in.
    pub bottom: f32,
}

/// The band the transport is painted on, in pixels — or none.
///
/// # The controls sit ON the film now, and this is what makes them legible
///
/// The owner asked for both halves: *"todos os controles do player apareçam
/// por cima do vídeo/gif, de forma a deixar o vídeo/gif ocupando toda a
/// altura possível"*, and a background *"com semi-transparência, para que
/// seja possível visualizá-los em todas as condições de contraste"*.
///
/// So the picture keeps the whole panel — nothing is subtracted from its
/// box any more — and this rectangle goes over its last line, under the
/// glyphs. It is the one place in this manager that draws over its own
/// content, and it is drawn over a PICTURE rather than over text: a film is
/// not read line by line, and a control one has to move the film to find is
/// not a control.
///
/// `left`, `wide` and `top` are the section's own pixels, as
/// [`Section::time_bar_box`] takes them.
#[must_use]
pub fn transport_band(section: &Section, foot: Foot) -> Option<(f32, f32, f32, f32)> {
    section.transport?;
    Some((
        foot.left,
        transport_top(section, foot),
        foot.wide,
        foot.line_px,
    ))
}

/// Where the transport's line BEGINS, in pixels — counted up from the foot.
///
/// # One answer for the glyphs and the rectangles, or they come apart
///
/// The row is drawn with `Row::foot`, so the text renderer places it counting
/// lines UP from the surface's bottom. The band, the track and the cursor were
/// still counted DOWN from the region's top — and the difference between the
/// two is the surface's remainder, which is exactly what the owner
/// photographed: *"a barra de tempo está em cima, ela está desalinhada do resto
/// dos controles"*. It was my own fix for the row's position producing the next
/// defect, because only half of the row moved.
///
/// # And it counts what is BELOW it, which is the second half of the same rule
///
/// The transport is not always the last line: a sideways scrollbar sits on the
/// band's own last line, and the rows — the transport's among them — stop one
/// short of it. The rectangles did not, so on the one preview that has both,
/// they landed a line below the glyphs. The owner photographed that too, on an
/// `.m3u8`: *"aparece um scrollbar horizontal, assim fazendo que os botões de
/// controle fiquem mais acima, e a linha de tempo mais abaixo"*.
///
/// So the offset is counted rather than assumed, and it is counted HERE — the
/// one place the whole transport asks. Anything else anchored to the foot in
/// future is one more line in this function and nowhere else.
#[must_use]
pub fn transport_top(section: &Section, foot: Foot) -> f32 {
    // WHAT SITS UNDER THE TRANSPORT, in whole lines.
    let below = f32::from(u8::from(section.across.is_some()));
    below
        .mul_add(-foot.line_px, foot.bottom - foot.line_px)
        .max(0.0)
}

/// The time a transport shows: where the film is, and how long it runs.
///
/// One function, because the width of what it returns decides the length of the
/// track — see [`track_px`] — and a string built in one place and measured in
/// another is two answers waiting to disagree.
#[must_use]
pub fn clock_text(time: &ronoctua_design::Scroll) -> String {
    #[allow(clippy::cast_precision_loss)]
    let (at, total) = (time.offset as f64 / 10.0, time.total as f64 / 10.0);
    format!(
        "{} / {}",
        crate::sizes::clock_of(at),
        crate::sizes::clock_of(total)
    )
}

/// What the film's letterbox is filled with: NOTHING.
///
/// # Why it is air and not a token
///
/// A film is letterboxed into the frame the panel asked for, and the bars
/// beside a picture of another shape should be the air the panel would have
/// left anyway. A colour would have had to be one of TWO — the footer's chrome
/// and the window's buffer background are different tokens — and the frame is
/// one texture drawn in both doors, so either door would get the other's.
///
/// Fully transparent removes the question. Measured against the real binary:
/// the bars arrive with alpha 0 and the picture with alpha 255, and the image
/// pipeline blends with `ALPHA_BLENDING` — so each door shows its own
/// background through, and no token value crosses into `ronoctua-inspect`
/// (invariant 11's rule, one crate along).
#[must_use]
pub fn letterbox() -> String {
    "#00000000".to_owned()
}

/// A token's colour in the notation ffmpeg reads.
///
/// The one place a `Color` becomes a string for a child process — the same
/// rule `pixels.rs` keeps for the other direction, where `Rgba` is born in one
/// function. `ronoctua-inspect` receives no token values (invariant 11's rule,
/// one crate along), so whoever holds the tokens does this.
#[must_use]
pub fn ink(colour: ronoctua_tokens::Color) -> String {
    let [r, g, b, a] = colour.to_rgba8();
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

/// What the preview says when there is nothing more of it to reach.
pub const PREVIEW_ALL_SHOWN: &str = crate::key::FILES_PREVIEW_ALL_SHOWN;

/// One of the two panels the manager can put beside — or under — the listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// What the file LOOKS like: a picture, or the type's glyph and the reason
    /// there is none.
    Preview,
    /// What the file IS, in as much detail as the tools on the machine can say.
    Info,
}

/// Where every region of the manager begins, at one width.
///
/// # Why one struct and not four functions
///
/// Because `View::split` already records what happens when the division is
/// computed in more than one place: both painters and both hit-testers asked
/// `sidebar_columns` for themselves, and on a second monitor the rule and the
/// fields landed at different cells. With FOUR regions — the sidebar, the
/// listing and two panels — that argument only gets stronger, so the whole
/// division is decided once and handed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panes {
    /// Where the sidebar ends; zero when there is none.
    pub sidebar: u16,
    /// How many cells the panels take on the right, rules and edge included;
    /// zero when they are not there.
    pub right: u16,
    /// How many LINES the bottom strip takes; zero when there is none.
    pub strip: u16,
    /// Whether the preview panel is on.
    pub preview: bool,
    /// Whether the info panel is on.
    pub info: bool,
    /// Whether the preview has been given the region and everything in it.
    ///
    /// `C-w`. The panels' arithmetic is unchanged — the preview is still a
    /// panel occupying `right` cells — and this says only that `right` is the
    /// whole width, so there is nothing to its LEFT to be separated from and
    /// the leading rule would be drawn against the screen's own edge.
    pub whole: bool,
}

impl Panes {
    /// The listing's first cell — where the sidebar ends plus its rule, or zero.
    #[must_use]
    pub const fn listing_from(self) -> u16 {
        if self.sidebar == 0 {
            0
        } else {
            self.sidebar + RULE
        }
    }

    /// The panels that are on, left to right.
    ///
    /// The preview first, always. It is what answers "is this the file I
    /// meant" in one glance, and the facts are what one then reads.
    #[must_use]
    pub fn on(self) -> Vec<Panel> {
        let mut out = Vec::new();
        if self.preview {
            out.push(Panel::Preview);
        }
        if self.info {
            out.push(Panel::Info);
        }
        out
    }

    /// Where each panel sits: `(which, first cell, cells)`.
    ///
    /// **ONE function for both places.** The right-hand columns and the bottom
    /// strip differ in how wide they are and how many lines they have, and in
    /// nothing else — the panels themselves are the same two panels. Two
    /// builders would be two answers to "what does the info panel say", which
    /// is the divergence the two doors already exist to prevent.
    #[must_use]
    pub fn boxes(self, width: u16) -> Vec<(Panel, u16, u16)> {
        let on = self.on();
        let Ok(n) = u16::try_from(on.len()) else {
            return Vec::new();
        };
        if n == 0 {
            return Vec::new();
        }
        // A RULE BEFORE THE FIRST ONE ONLY ON THE RIGHT. Down in the strip
        // there is nothing to its left to be separated from: what separates the
        // strip from the listing is the rule ABOVE it, which is a line and not a
        // column.
        let (from, room, leading) = if self.right > 0 {
            (
                width.saturating_sub(self.right),
                self.right,
                // NO RULE WHEN THERE IS NOTHING ON THE OTHER SIDE OF IT. A rule
                // is what separates two things; drawn against the screen's own
                // edge it is a line with one thing beside it.
                u16::from(!self.whole),
            )
        } else if self.strip > 0 {
            (0, width, 0u16)
        } else {
            return Vec::new();
        };
        let rules = leading.saturating_add(n - 1);
        let each = room
            .saturating_sub(rules)
            .saturating_sub(EDGE_RIGHT)
            .checked_div(n)
            .unwrap_or(0);
        let mut at = from.saturating_add(leading * RULE);
        let mut out = Vec::with_capacity(on.len());
        let last = on.len().saturating_sub(1);
        for (i, which) in on.into_iter().enumerate() {
            // THE REMAINDER GOES TO THE LAST ONE, and it is not tidiness.
            //
            // An integer division leaves up to `n - 1` cells over, and they
            // were left at the right-hand end where nothing drew them: the
            // owner measured the consequence as *"the big blank space that
            // should not exist between the scrollbar and the window's corner"*,
            // because the facts' bar sits in the LAST cell of its own box and
            // the box stopped short of the edge.
            let mine = if i == last {
                room.saturating_sub(at.saturating_sub(from))
                    .saturating_sub(EDGE_RIGHT)
            } else {
                each
            };
            out.push((which, at, mine));
            at = at.saturating_add(mine).saturating_add(RULE);
        }
        out
    }

    /// The cells the VERTICAL rules sit in, for whoever paints them.
    ///
    /// One before the first panel when they are on the right, and one between
    /// them wherever they are. It is a one-pixel rule and a cell of grid:
    /// separation by whitespace and a rule, never by a container (invariant 15).
    #[must_use]
    pub fn rules(self, width: u16) -> Vec<u16> {
        // AND NONE AT ALL WITH THE REGION GIVEN AWAY. A rule separates two
        // things; with one panel filling the width there is nothing to its left
        // to be separated from, and the saturating subtraction below put the
        // line in cell nought — against the screen's own edge. The owner
        // photographed it: *"no modo largo há uma linha vertical na esquerda
        // que continua aparecendo"*.
        if self.whole {
            return Vec::new();
        }
        let boxes = self.boxes(width);
        let mut out = Vec::new();
        for (n, (_, at, _)) in boxes.iter().enumerate() {
            if n == 0 && self.right == 0 {
                continue;
            }
            out.push(at.saturating_sub(RULE));
        }
        out
    }

    /// The cell the LISTING's own scrollbar sits in.
    ///
    /// # Why the bar moved out of the surface's last cell
    ///
    /// Because with the panels on the right that cell is inside the INFO
    /// panel, and a thumb there says "the listing is cut" from a column that is
    /// not the listing — and drags a list it is not sitting on. The owner
    /// photographed the consequence from the other side: two bars in one
    /// column, one under the other, reading as one control.
    ///
    /// So it is the listing's own last cell: just before the panels' first
    /// rule when they are on the right.
    ///
    /// `None` when nothing is beside the listing on the right, and that is not
    /// the same as "the last cell of the region". The region's cells are the
    /// SCREEN's and every row is drawn `chrome::FIELDS_LEAD` cells further in,
    /// so the region's last cell is already past the edge — `files.edgeRight`
    /// is what absorbs it. `None` means the SURFACE's own last cell, which is
    /// exactly where every other view's bar goes and where this one has always
    /// been; the door knows that in pixels and the view does not.
    #[must_use]
    pub const fn listing_bar(self, width: u16) -> Option<u16> {
        if self.right == 0 {
            return None;
        }
        Some(width.saturating_sub(self.right).saturating_sub(RULE))
    }

    /// Which SECTION a cell belongs to.
    ///
    /// # Why this exists at all
    ///
    /// Because until now nothing asked. The manager draws one `Row` per line
    /// carrying every section's fields, and `Row::field_at` walks from the
    /// right — which answers "what did I click" without ever naming a region,
    /// and that was the whole trick. It is not enough for the three things the
    /// owner asked for next, and he named the symptom exactly: *"putting the
    /// cursor over an item of the info column, on the exact row the mouse is
    /// on, ends up giving the hover effect in the listing; that is, right now
    /// it is all one single row"* — and the same missing answer is why a scroll
    /// gesture over the facts scrolled the listing, the sidebar and the facts
    /// together.
    ///
    /// A field can say what it is. A REGION cannot, because the empty cells of
    /// a short section have no field in them at all — and the pointer spends
    /// most of its time over empty cells. So the region is arithmetic, here,
    /// once, and the three callers (the band, the wheel and the focus) ask it
    /// rather than each working it out from `sidebar` and `right`.
    /// `in_strip` is whether the pointer is BELOW the listing, in the band the
    /// panels take when they do not fit beside it. It has to be told, because
    /// down there the panels' boxes span the whole width — the same cell is the
    /// listing's above the rule and a panel's below it, and a cell alone cannot
    /// say which side of a horizontal line it is on.
    #[must_use]
    pub fn section_at(self, cell: u16, width: u16, in_strip: bool) -> Pane {
        if !in_strip && self.sidebar > 0 && cell < self.sidebar {
            return Pane::Sidebar;
        }
        if self.right == 0 && !in_strip {
            return Pane::Listing;
        }
        // THE PANELS FROM THE RIGHT, in the order `boxes` lays them out. A cell
        // on a rule belongs to the section that starts after it, which is the
        // same choice `band_of` makes about the pixel: the rule is drawn on top
        // of whatever the band left there.
        //
        // AND EVERY CELL OF THE RIGHT-HAND REGION HAS AN OWNER, which is what
        // the sentence above PROMISED and the code did not do. `boxes` walks
        // `at += mine + RULE`, so each rule cell — and the leading one, and the
        // `EDGE_RIGHT` reserve past the last box — belonged to no box at all
        // and fell out of this loop to the `Listing` below.
        //
        // The owner photographed the consequence: a scrollbar is drawn at
        // `rule_x`, half a cell past its box's last text cell, so HALF ITS
        // PIXELS answered `Listing` — and the hover band lit a row of the
        // listing while the pointer was over the preview's bar. The same hole
        // silently switched off `band_belongs`, which is the rule that keeps a
        // band off a photograph.
        //
        // So each box claims from where the one before it ended, and the last
        // one claims to the edge: a rule goes to the section after it, and the
        // reserve at the end goes to the section it is the edge OF.
        let boxes = self.boxes(width);
        let last = boxes.len().saturating_sub(1);
        // WHERE THE REGION BEGINS, by the same rule `boxes` uses: the panels'
        // own width on the right, or the whole line down in the strip.
        let mut claim = if self.right > 0 {
            width.saturating_sub(self.right)
        } else {
            0
        };
        for (i, (which, at, room)) in boxes.into_iter().enumerate() {
            let ends = if i == last {
                width
            } else {
                at.saturating_add(room)
            };
            if cell >= claim && cell < ends {
                return match which {
                    Panel::Preview => Pane::Preview,
                    Panel::Info => Pane::Info,
                };
            }
            claim = ends;
        }
        Pane::Listing
    }

    /// The preview's box, in cells: `(first column, columns)`.
    #[must_use]
    pub fn preview_box(self, width: u16) -> Option<(u16, u16)> {
        self.boxes(width)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Preview))
            .map(|(_, at, w)| (at, w))
    }
}

/// Where a RULE's line goes inside the cell reserved for it, in PIXELS.
///
/// # The same lopsidedness, one register down
///
/// [`RULE`] reserves a whole cell of grid for a line one logical pixel wide,
/// and every site drew that line at the cell's LEFT edge. So the cell became
/// air on the line's right and none at all on its left. Measured off the
/// owner's screen, in the footer door at 1280 px: the sidebar's last column
/// ended 10,8 px before the rule and the listing's icon began 21,6 px after
/// it — one cell against two, for one boundary.
///
/// The line goes in the MIDDLE of its own cell, so both neighbours get half of
/// it. Nothing else moves: every hit test, every column and every elision
/// counts the same cells they counted before, because the cell was the rule's
/// already and no field is laid out inside it.
///
/// `at` is the rule's cell, `lead` what the door owes before a row's first
/// cell, and `cell` how wide a cell is in that door.
#[must_use]
pub fn rule_x(at: u16, lead: u16, cell: f32) -> f32 {
    (f32::from(lead.saturating_add(at)) + 0.5) * cell
}

/// A BAND'S RECTANGLE IN PIXELS, from the cells it covers.
///
/// The other half of [`band_for`]: that one answers for a whole SECTION, which
/// is what a row of a list occupies, and this one for the part of a line a
/// shelf's cell does. Both doors call one or the other and neither multiplies a
/// cell width of its own — which is how the two of them came to disagree about
/// where the listing's band ended, twice.
/// HOW BIG A RUN IS DRAWN.
///
/// Three cases and not a number, because they are three different questions and
/// a number cannot be asked which one it is: the ordinary size, a run standing
/// in for a PICTURE, and a run that has to be read in a cell too small for the
/// body size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ink {
    /// The size everything else in this manager is drawn at.
    #[default]
    Grid,
    /// AS TALL AS THIS MANY LINES: a thumbnail's fallback glyph, standing where
    /// a picture would have been.
    Fills(u16),
    /// SMALLER THAN THE BODY, from the one token that already means that.
    ///
    /// The name under a thumbnail at the smallest step: six cells is six
    /// characters of a filename at the body size and a dozen at this one, and
    /// the owner asked for it in those terms — *"além das duas linhas para o
    /// nome, também reduza o tamanho da fonte para um tamanho bem pequeno"*.
    ///
    /// The number is WHICH LINE OF THE NAME it is, and it is here because a
    /// name that wraps is one thing read downwards: leaded by the grid, its
    /// two lines sit a quarter of a line further apart than the size they are
    /// drawn in wants, and the owner read that off the screen too — *"a
    /// distância entre as duas linhas dos nomes de arquivos e pastas está
    /// muito grande"*. It is the door's answer because only the door knows the
    /// two runs are one block; the renderer is told by [`Size::Stacked`].
    Small(u16),
}

/// THE SIZE A RUN IS DRAWN AT.
///
/// `ink` is the field's own (see [`Field::ink`]), `base` the size text is drawn
/// at, and `line` how tall one line is — both in the door's logical pixels.
///
/// # Why `OnTheCell`
///
/// Because a run bigger than the grid has to be CENTRED on the cells it
/// occupies, or it hangs off the left of them: `Size::Px` shares a grid run's
/// left edge, which is right for a title and wrong for a glyph standing in for
/// a picture. It is the same variant the bell and the countdown already use,
/// for the same reason.
///
/// # And the air is the picture's own
///
/// A fallback glyph stands where a picture would be, so it gives up the same
/// `sizes::picture_air` on both sides that `picture_in` sheds — otherwise
/// the one cell that has no picture is the one whose ink touches its
/// neighbour's.
#[must_use]
pub fn run_size(ink: Ink, base: f32, line: f32) -> ronoctua_render::Size {
    match ink {
        Ink::Grid => ronoctua_render::Size::Px(base),
        // THROUGH THE ONE RESOLVER. `one-text-size` lets two files name a fixed
        // size and this is not one of them — `paint::scale_in_px` is where the
        // token is read and scaled, and asking it is how this moves with the
        // size a person chose rather than pinning a second number.
        // AND THE BLOCK IS LEADED BY ITSELF. `Size::Stacked` draws the nth line
        // of a block where the block's own line height puts it rather than
        // where the grid's would; the first line is `Size::Px` exactly, so
        // nothing moves for a name that fits on one.
        Ink::Small(nth) => ronoctua_render::Size::Stacked(
            crate::paint::scale_in_px(ronoctua_protocol::Scale::Small),
            nth,
        ),
        Ink::Fills(lines) => {
            let air = crate::sizes::picture_air() * 2.0;
            ronoctua_render::Size::OnTheCell((f32::from(lines) * line - air).max(base))
        }
    }
}

/// One offset, moved by `by` and kept inside what there is to show.
///
/// Integer arithmetic and saturating at both ends, like every other offset in
/// this project: a list shorter than its room does not scroll at all, and the
/// last page is the floor rather than the end.
#[must_use]
pub fn slide(from: usize, by: i64, total: usize, room: usize) -> usize {
    let last = total.saturating_sub(room);
    let moved = i64::try_from(from)
        .unwrap_or(i64::MAX)
        .saturating_add(by)
        .max(0);
    usize::try_from(moved).unwrap_or(0).min(last)
}

/// THE SIZE THE GLOBE'S TEXTURE IS ASKED FOR, out of the box it is drawn in.
///
/// # Why it is not the box
///
/// Because [`picture_in`] gives up `sizes::picture_air` on each of the four
/// sides before anything is fitted into it, and `Area::centre` then SHRINKS
/// whatever it is given to fit what is left. A texture asked for at the outer
/// size is therefore always drawn a little smaller than it was painted — by
/// `(h - 2·air) / h`, which is a per cent or two in a tall box and a third of
/// the picture in a short one.
///
/// That is invisible while the box does not move and glaring when it does: the
/// globe's rows SCROLL now, so the box gets shorter as they go, and the owner
/// photographed the map changing size between two scroll positions — twice,
/// because the first repair made the CUT right and left this alone.
///
/// The preview has always subtracted it (`measured_preview`); this is the same
/// subtraction, in one place, for the door that forgot.
///
/// It is in LOGICAL pixels; [`globe_texture_at`] is the same box at a
/// screen's scale, which is what is PAINTED.
#[must_use]
pub fn globe_texture(space: ronoctua_render::image::Area) -> (u32, u32) {
    let air = crate::sizes::picture_air() * 2.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (
        (space.width - air).max(0.0) as u32,
        (space.height - air).max(0.0) as u32,
    )
}

/// What a click on a cell means.
///
/// Every variant is something a key also reaches — that is the invariant this
/// enum exists to make checkable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hit {
    /// The name: open it, as `Enter` does.
    Name(usize),
    /// The icon: mark it, as `m` does.
    Mark(usize),
    /// The address under the globe: open the coordinate in the browser's map.
    ///
    /// Its own variant and not `Nothing`, because a line that says where a
    /// photograph was taken is the line somebody presses to go and look. What
    /// it runs is the menu's `open-map`, which is the keyboard's path to the
    /// same place.
    GlobeMap,
    /// One permission cell: flip that bit, as the menu's `m` reaches.
    Bit(usize, mode::Who, mode::What),
    /// A special bit's cell.
    Special(usize, mode::Special),
    /// The owner: pick another, as the menu's `O` does.
    Owner(usize),
    /// The group: pick another, as the menu's `G` does.
    Group(usize),
    /// The size of a directory: measure it, as `TAB` on a directory does.
    Measure(usize),
    /// A row of the facts that CHANGES the file — the name, the date, a tag,
    /// a detail, the place: ask the question it asks, as `RET` on that row
    /// with the facts focused does (`View::amend`). Carries the listing row
    /// the facts are about and what the row offers.
    Amend(usize, crate::fields::Doable),
    /// The globe at the top of the info panel.
    ///
    /// One target for the whole picture, and the drag is resolved from it: a
    /// press on the globe takes the pointer, exactly as a press on a shape in
    /// the preview does. The sentence it carries teaches the gesture, because
    /// a picture that responds to the hand and says nothing is a picture people
    /// do not try.
    Globe,
    /// The coordinate's own dot, when it is on this side of the world.
    ///
    /// A second target and not the same one: pressing the dot FRAMES on it,
    /// and pressing anywhere else on the disc drags. Where the dot is comes
    /// back with the frame — where a thing is drawn is where it is grabbed.
    GlobeDot,
    /// A piece of the path above: go there, as `-` does one level at a time.
    Crumb(usize),
    /// A column's header: sort by it, as `s` cycles.
    Sort(Column),
    /// A row of the open menu.
    Menu(&'static str),
    /// A row of the SIDEBAR's open menu.
    ///
    /// Its own variant and not `Menu` with a flag: the two tables have separate
    /// ids and separate keys, and one variant carrying "which table" is a place
    /// for the wrong table to be consulted with no error.
    SideMenu(&'static str),
    /// A choice of the open picker.
    Choice(usize),
    /// One entry of a playlist in the preview: play THAT one.
    ///
    /// Reached by the keyboard through `C-{` and `C-}`, which walk the list one
    /// entry at a time — the mouse arrives in one press and the keys arrive in
    /// several, which is what `files-everything-has-a-key` asks for. It is the
    /// argument [`Hit::Sidebar`] already makes.
    Track(usize),
    /// A row of the sidebar: go there, open it, run it — or mount it, when it
    /// is a device that is not mounted. As `Enter` does on a listing row, which
    /// is why it costs no key of its own.
    Sidebar(usize),
    /// The button that gives a PANEL the whole region, or gives it back.
    ///
    /// As `C-w` on the preview and `C-W` on the facts, and through the same
    /// code: the mouse never reaches anything the keyboard cannot, and never by
    /// a second path.
    ///
    /// IT CARRIES WHICH PANEL, and it has to: there are two corners on screen
    /// at once, so a button that only meant "expand" would expand whichever
    /// panel the handler happened to name — and that is one variant with two
    /// meanings, which is the shape this enum exists to refuse.
    Theatre(Panel),
    /// The glyph at the end of the path's line: turn the entries over.
    ///
    /// As `C-g`, and through the same code — the mouse never reaches anything
    /// the keyboard cannot, and never by a second path.
    ///
    /// IT CARRIES NOTHING, unlike [`Hit::Theatre`] beside it: there is one
    /// listing and it has two shapes, so "turn it over" names the whole thing
    /// without ambiguity. A variant that carried which shape to go TO would be
    /// a second place to decide what the glyph already draws.
    Layout,
    /// The glyph beside it: ring the thumbnails' size.
    ///
    /// As `C-G`, and through the same code. It exists only while the entries
    /// are side by side, because there is nothing to size otherwise — and the
    /// cell it occupies is given up with it, so a path may use it again.
    Sizing,
    /// The transport's play glyph: hold the film, or let it go on. As `C-;`.
    ///
    /// A DRAWN target and not a click anywhere on the picture, for the reason
    /// the eject glyph gives below: a click on a picture says nothing about
    /// which of two things it would do.
    Play,
    /// The transport's sound glyph: let the film be heard, or not. As `C-'`.
    Sound,
    /// A mounted device's eject glyph, in the sidebar's last cell. As `C-e`.
    ///
    /// TAKING A ROW OFF THE SIDEBAR HAS NO TARGET HERE, and that is a decision:
    /// a little cross on every row is chrome on a surface that has none
    /// (invariant 14), and `C-u` reaches it. The rule is one-directional —
    /// everything the mouse can do the keyboard must do, never the reverse —
    /// which is why `files-mark-all` has no target either.
    ///
    /// A DRAWN target and not a click on the row, for the reason the
    /// notification manager already gives about its bin: a click on a row says
    /// nothing about which of two things it would do, and a click on an eject
    /// glyph is not a guess.
    Eject(usize),
    /// THE LISTING'S AIR: past the last column of a shelf, below the last
    /// entry, and the blank lines under a short list.
    ///
    /// It carries no index because there is no row there — that is the whole
    /// of what it says. A click on it puts the marks down, which is `C-A`
    /// (`files-unmark-all`) through the key's own code, so the one-directional
    /// rule holds: the pointer reaches nothing new.
    ///
    /// It is NOT produced over a modal. With a menu, a picker or a sheet up
    /// the listing's section carries the modal's rows, and clearing there
    /// would take away the marks the menu's own title is counting.
    Air,
    /// THE GIT LINE under the path (`ronoctua_shell_git_line::git_line`): open the history, as
    /// `C-h` does and through its code.
    Git,
    /// Nothing happens here.
    Nothing,
}

/// Which SECTION of the manager has the keyboard.
///
/// The same word the notification manager uses for the same idea, and
/// deliberately: two names for "which side am I on" is two things to learn.
///
/// # There are four now, and the order is the screen's
///
/// It was two, and the two were the two LISTS — the only regions that had a
/// cursor to move. Then the panels arrived on the right, the facts stopped
/// fitting in the lines they were given, and the owner asked for the obvious
/// consequence: *"each section being independent and having its own scrollbar
/// (sidebar/listing/preview/info). And it has to be possible to move the focus
/// between them with Tab and Shift+Tab."*
///
/// The variants are declared LEFT TO RIGHT as they are drawn, because that is
/// what [`View::cycle_pane`] walks. Down in the bottom strip the two panels are
/// underneath rather than beside, and the order still reads correctly: they
/// come after the listing either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Pane {
    /// The control that opens the line above everything.
    ///
    /// **It is not a section and it is not the manager's**, which is why it
    /// carries no rows: the line it opens belongs to the door and shows an
    /// echo or an `M-x` prompt just as readily (see [`crate::fold`]). What is
    /// the manager's is the FOCUS that reaches it — `Tab` walks one ring in
    /// this system and the owner asked for the control to be on it, so this
    /// is where it had to go.
    ///
    /// First, because it is drawn above every other pane; the order of these
    /// variants is the order `Tab` walks, and it is the order on screen.
    Fold,
    /// The sidebar.
    Sidebar,
    /// The directory. Where the cursor starts, and where typing always goes.
    #[default]
    Listing,
    /// The picture.
    ///
    /// **It has nothing to scroll, and it still takes focus.** A picture is one
    /// thing and it is either in the panel or it is not, so this pane will
    /// almost never show a bar — the arrows in it move the LISTING's cursor,
    /// which is what changes the picture, and that is the honest answer to
    /// "what does moving inside a preview mean". What it must not do is be
    /// SKIPPED: a `Tab` that visits three of four sections is a key one has to
    /// count presses for.
    Preview,
    /// The facts.
    Info,
}

/// One drawn piece of a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// HOW BIG this run is drawn.
    ///
    /// # Why the FIELD and not the row
    ///
    /// It was the row for an hour, and the owner photographed what that costs:
    /// the footer flattens all four sections' fields onto ONE line, so a
    /// thumbnail's glyph asking for four lines made the sidebar's `trash` and
    /// the facts' `owner:group` four lines tall beside it. A line in that door
    /// is not one list's; a field always is.
    pub ink: Ink,
    /// First cell.
    pub at: u16,
    /// What is drawn.
    pub text: String,
    /// Which token colours it.
    pub color: Color,
    /// What a click does.
    pub hit: Hit,
    /// What the pointer resting here says on the message line.
    pub explain: Option<Explanation>,
}

/// One line of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right.
    pub fields: Vec<Field>,
    /// Whether this row is anchored to the surface's BOTTOM edge.
    ///
    /// Only the film's transport is. A surface is not a whole number of lines,
    /// and the leftover under the last one is where the owner found the
    /// controls sitting high: *"os controles continuam elevados (com espaço
    /// abaixo deles) quando há apenas um espaço menor que uma linha"*. Anchored
    /// to the foot, the remainder falls above them — inside the picture, where
    /// nothing is looking for it. It is `Span::edge`'s decision on the other
    /// axis, and the same argument.
    pub foot: bool,
    /// Whether this entry is marked for a batch operation.
    pub marked: bool,
    /// Whether the cursor is on it.
    pub focused: bool,
}

impl Field {
    /// Does this field own that cell?
    ///
    /// **THE ONE RULER**, and there were three: `Row::field_at` counted the gap
    /// after a field as belonging to it (`+ 1`), while both painters' "is the
    /// pointer on this field" test did not. So the last cell of a field lit
    /// nothing and yet clicked something — one cell wide, on every field, in
    /// both doors.
    ///
    /// The `+ 1` is the right answer and it is the one kept: a one-cell dead
    /// zone between columns is invisible and feels like a broken click.
    #[must_use]
    pub fn covers(&self, cell: u16) -> bool {
        cell >= self.at && cell < self.at + width_of(&self.text) + 1
    }
}

/// WHICH of these fields owns that cell — the one walk, for every row type.
///
/// A free function and not a method, because two different types carry the
/// same `Field`s: the manager's [`Row`] and the footer's own expansion row,
/// which is what the footer's manager is drawn out of. A method on one of them
/// would have been a second ruler for the other, and this whole file is the
/// record of what two rulers cost.
///
/// See [`Row::field_index_at`] for why it is `rposition` and not `position`.
#[must_use]
pub fn lit_field(fields: &[Field], cell: u16) -> Option<usize> {
    fields.iter().rposition(|f| f.covers(cell))
}

/// WHETHER THIS FIELD LIGHTS with the one the pointer is on.
///
/// # Why a field is not always alone
///
/// [`lit_field`] says which field the pointer is ON, walking from the right so
/// that two packed fields do not both claim the cell between them. This says
/// which OTHER fields are part of the same THING and light with it.
///
/// In a list that is only ever the field itself: each of a row's fields carries
/// its own `Hit`, and the type glyph is the MARK's rather than the name's. In a
/// GRID a name is up to two runs on two lines of ONE cell (`layout::wrap` breaks
/// it by cluster), and lighting only the run the hand happens to be over lights
/// half a name — the owner read it on screen, `sample-` quiet above an accented
/// `aud.ogg`. It is the law the SENTENCE already follows one field along: one
/// name, said once.
///
/// TWO THINGS ARE LEFT OUT, and each is load-bearing:
///
///   - the glyph that FILLS the picture's room, because its colour is a fact
///     about the file — `glyph_colour_of` paints the executable bit there — and
///     a highlight over it would hide the one thing that glyph says;
///   - `Hit::Nothing`, which is not an identity: the permission column's kind
///     character carries it on every row, so matching by it would light a
///     column of the whole listing at once.
#[must_use]
pub fn lit_with(field: &Field, hit: &Hit) -> bool {
    !matches!(hit, Hit::Nothing) && field.hit == *hit && !matches!(field.ink, Ink::Fills(_))
}

/// ONE COLUMN OF THE MANAGER, and everything that is true about it.
///
/// # Why this type exists
///
/// Because the manager draws four lists and had no name for any of them. Each
/// painter, each hit test and each gesture worked out where a column began and
/// what it held, and they disagreed — every one of the owner's reports was two
/// of those answers differing:
///
/// - the sidebar's first two rows were unreachable, because the LISTING's
///   chrome was refused for the whole line;
/// - clicking a folder went to the parent, because the listing's cell was
///   resolved against the path's crumbs;
/// - the preview's top moved when the listing scrolled, because the panels'
///   band was computed as "the region, minus the listing's chrome, minus how
///   much of it is still on screen";
/// - and a gesture over the facts scrolled everything, because there was one
///   offset.
///
/// A section says where it is (`from`..`to` in cells, `top` for `lines`), what
/// it holds (`rows`, already at its own `offset`), and how much of it there is
/// (`total`). Everything else asks. Nothing else decides.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Which column this is.
    pub pane: Pane,
    /// Its first cell.
    pub from: u16,
    /// One past its last cell.
    pub to: u16,
    /// Its first line of the region.
    pub top: usize,
    /// How many lines it occupies.
    pub lines: usize,
    /// Its own first visible row.
    pub offset: usize,
    /// How many rows it has in all.
    pub total: usize,
    /// How many of `content`'s first lines are CHROME — the listing's path and
    /// header, which do not scroll. Zero for every other section.
    pub above: usize,
    /// The shelf geometry, when this section lays its rows out SIDE BY SIDE.
    ///
    /// `None` is one row per line, which is every section there is but the
    /// listing in thumbnail mode — and it is what keeps the eighteen places
    /// that turn a line into a row unchanged.
    ///
    /// # Why the whole grid and not a pair of numbers
    ///
    /// Because [`ronoctua_design::Grid`] is the one arithmetic this project has
    /// for items side by side, and its own doc says why: the three icon
    /// browsers each had their own, and *"the symptom was the click landing on
    /// one item while the screen showed another"*. `Grid::at` and
    /// `Grid::index_at` are the same sum in both directions, so
    /// [`Section::line_of`] and [`Section::row_at`] cannot drift apart the way
    /// the drawing and the hit test in this file already have, twice.
    ///
    /// `offset` and `total` stay in ENTRIES with a grid, not in shelves. That
    /// is deliberate: the wheel, the drag, `follow_within` and
    /// `scroll_listing_to` all speak entries, and giving them a second unit to
    /// know about is how a number comes to mean two things.
    pub grid: Option<ronoctua_design::Grid>,
    /// Its own lines, from `offset`, at most `lines` of them. The listing's
    /// chrome is the first of these and does not scroll.
    ///
    /// A section's lines are ITS OWN. Nothing composes a line out of several
    /// sections' fields: that arrangement is what made a column's length, its
    /// offset and its top reach into its neighbours, and every defect the
    /// owner reported was one of those reaches.
    pub content: Vec<Row>,
    /// The same list read SIDEWAYS, when its lines are wider than the section.
    ///
    /// `None` for a section whose rows are cut to fit and never move — which
    /// is every one of them but the text preview, where a line of code is
    /// longer than a twenty-four-cell panel and the window over it moves.
    pub across: Option<ronoctua_design::Scroll>,
    /// The film's timeline, when this section is showing one that has a length.
    ///
    /// The SAME [`ronoctua_design::Scroll`] the other two axes are, counted in
    /// tenths of a second: `total` is how long the film runs, `offset` is where
    /// the picture is, and `lines` is one tenth — which is what makes the
    /// cursor a mark rather than a page. `bar_across` draws it and `offset_at`
    /// reads it back, so grabbing it lands where it looks.
    ///
    /// `None` for a GIF, and that is not an omission: a GIF loops and declares
    /// no duration, and its frames are inter-dependent, so seeking into one
    /// means composing every frame before it — 707 ms on the owner's own
    /// forty-four-frame sample. No duration is what draws that line, with no
    /// branch anywhere else.
    pub time: Option<ronoctua_design::Scroll>,
    /// How many cells the clock at the transport's right-hand end occupies.
    ///
    /// It travels with the section because the TRACK's length depends on it and
    /// three places compute that: the cursor's rectangle, and each door's drag.
    /// `1:23 / 5:40` and `3:44 / 10:34` are not the same width, so a reserve
    /// guessed once is a gap that differs per film.
    pub clock: u16,
    /// Which of `content`'s rows is the transport, when there is one.
    ///
    /// The controls are a ROW and not a rectangle — two of their cells are
    /// clickable, so `Section::line_of` and `View::hit` resolve them with the
    /// arithmetic they already have. What needs a rectangle is the timeline,
    /// because where a bar is drawn is where it is grabbed; this is the line it
    /// is drawn on, and [`Section::time_bar_box`] is the rectangle.
    pub transport: Option<usize>,
}

impl Section {
    /// WHICH REGION LINE carries `row` of this section, if it is on screen.
    ///
    /// Each section counts from its own `top` and its own `offset`, and the
    /// listing's chrome sits in front of its rows. Doing this arithmetic
    /// anywhere else is how a pointer resting on the third fact lit the third
    /// row of the listing: the row number was right and the line it was turned
    /// into belonged to another column.
    #[must_use]
    pub fn line_of(&self, row: usize) -> Option<usize> {
        let i = row.checked_sub(self.offset)?;
        // SIDE BY SIDE: the shelf this row sits on, from the one arithmetic.
        //
        // `Grid::at` already answers "where does item `i` of the page go", and
        // it answers `None` past the last one that fits — which is exactly
        // "scrolled out of sight", the same thing the line below says. Working
        // it out here with a division would be the second copy its own doc
        // warns about.
        if let Some(g) = self.grid {
            let (line, _) = g.at(i)?;
            let at = usize::from(line);
            return (at < self.content.len()).then_some(self.top + at);
        }
        let at = self.above.saturating_add(i);
        (at < self.content.len()).then_some(self.top + at)
    }

    /// WHICH ROW of this section a point is on — the other direction of
    /// [`Section::line_of`], and the only one there is.
    ///
    /// `inside` counts from this section's own `top`; `cell` is absolute.
    /// `None` means the point is on no row of this section: the listing's
    /// chrome, a panel's reserve, the air past the last shelf of a grid, or
    /// past the last entry.
    ///
    /// # Why the CELL is an argument, and was not
    ///
    /// Because with rows side by side the line alone cannot answer. [`View::hit`]
    /// worked the row out as `offset + inside - chrome`, which is a sentence
    /// with no cell in it — right for one row per line and wrong for every
    /// other arrangement. A grid resolved that way would light the first
    /// thumbnail of a shelf wherever on the shelf the pointer was.
    ///
    /// In one-row-per-line the cell is unused, and that is not a wasted
    /// argument: it is what makes the two modes the same call, so no caller
    /// has to know which one it is looking at.
    #[must_use]
    pub fn row_at(&self, inside: usize, cell: u16) -> Option<usize> {
        if let Some(g) = self.grid {
            let line = u16::try_from(inside).ok()?;
            let i = g.index_at(line, cell.checked_sub(self.from)?)?;
            let row = self.offset.checked_add(i)?;
            return (row < self.total).then_some(row);
        }
        if inside < self.above {
            return None;
        }
        let row = self.offset.checked_add(inside)?.checked_sub(self.above)?;
        (row < self.total).then_some(row)
    }

    /// WHICH CELLS one row of this section occupies, when it is narrower than
    /// the section itself.
    ///
    /// `None` with one row per line, and that is the answer rather than a
    /// failure: a row of a list runs the whole width of its column, which every
    /// band already knew. Only a shelf holds several rows side by side and so
    /// only a shelf has to say which part of it a row is.
    #[must_use]
    pub fn cells_of(&self, row: usize) -> Option<(u16, u16)> {
        let g = self.grid?;
        let i = row.checked_sub(self.offset)?;
        let (_, column) = g.at(i)?;
        // THE WHOLE STEP, air included — the same claim `Grid::index_at` makes
        // about which item a cell belongs to. A band narrower than the target
        // says the wrong thing about where a click would land.
        let at = self.from.saturating_add(column);
        Some((at, at.saturating_add(g.step).min(self.to)))
    }

    /// THE SCROLLBAR'S RECTANGLE, in pixels — or none when nothing is cut.
    ///
    /// One rectangle, drawn by the painter and tested by the pointer. They used
    /// to be two answers: a cell index for the drawing and another arithmetic
    /// for the hit test, and the owner found every place they disagreed — the
    /// facts' bar that could not be grabbed in one door, the listing's that
    /// could not be grabbed in the other, and one drawn a cell past the border.
    ///
    /// `right` is the section's own right edge and `top` the region's, both in
    /// pixels; `Section::line_of` and [`section_px`] are where those come from.
    /// The thumb is at least one line: with twenty facts in four lines the
    /// proportional answer is nought.
    #[must_use]
    pub fn bar_box(
        &self,
        right: f32,
        top: f32,
        bottom: f32,
        cell: f32,
        line_px: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        // THE TRACK IS PIXELS, AND IT ENDS WHERE THE BAND DOES.
        //
        // It used to be `lines × line_px`, and a surface is not a whole number
        // of lines: `lines` is `floor(height / line_px)`, so the track stopped
        // short by the residue — up to one line of it — and at the last offset
        // the thumb stopped there too. The owner photographed the gap under
        // the facts' bar twice and said, correctly, that it was still there.
        //
        // AND THE BAND IS THE SECTION'S OWN, never the whole surface's.
        //
        // Reaching for `bottom` unconditionally was the first shape of that
        // fix, and it was too wide: down in the STRIP the panels have lines of
        // their own BELOW the listing, so the listing's band ends where the
        // strip begins. Handed the surface, the listing's thumb walked out of
        // its column and into the facts' box, which the owner watched it do.
        //
        // The residue is at most one line by construction, so a band ending
        // within one line of the surface is the LAST band in its column and
        // takes what is left over; one ending a whole line or more above it
        // has something underneath.
        // AND THE BAND BEGINS BELOW THE CHROME, which is the listing's alone.
        //
        // A scrollbar measures the rows that SCROLL, and the path and the
        // header do not: a track drawn beside them says two lines are part of
        // a journey they take no part in. Starting the band under them makes
        // the drawing and the arithmetic the same sentence — the thumb at
        // offset nought sits level with the first ENTRY, and the drag has the
        // whole track to spend on entries.
        //
        // It also keeps ONE rectangle: `Scroll::bar` is still the only thing
        // that builds one — see the `one-scrollbar` check, which refuses a
        // second thumb worked out by hand and refused this when it was written
        // that way.
        let above = f32::from(u16::try_from(self.top + self.above).unwrap_or(0));
        let band_top = top + above * line_px;
        ronoctua_design::Scroll {
            lines: self.visible(),
            ..self.scroll()
        }
        .bar(right, band_top, bottom, cell, line_px)
    }

    /// The sideways bar's rectangle, when this section has one.
    ///
    /// `left` and `wide` are the section's own pixels, `top` is where the
    /// region's first line begins, and `bottom` is where the surface ends. The
    /// twin of [`Section::bar_box`], and it asks the same
    /// [`ronoctua_design::Scroll`] — where a bar is drawn is where it is
    /// grabbed, whichever way its track runs.
    ///
    /// # Why it needs the SURFACE's bottom and not only its own lines
    ///
    /// Because a surface is not a whole number of lines. `lines` is
    /// `floor(height / line_px)`, so a band that ends at its last whole line
    /// stops short of the surface by the residue — and the owner photographed
    /// exactly that: twenty pixels of background under the bar, between it and
    /// the modeline.
    ///
    /// [`ronoctua_design::Scroll::track`] already decides this for the bar
    /// going down — the last band in a column takes the leftover, and one with
    /// something underneath does not — so this asks the same question rather
    /// than answering it a second way.
    #[must_use]
    pub fn wide_bar_box(
        &self,
        left: f32,
        top: f32,
        bottom: f32,
        wide: f32,
        cell: f32,
        line_px: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        let across = self.across?;
        let band_top = top + f32::from(u16::try_from(self.top).unwrap_or(0)) * line_px;
        let band_bottom = band_top + self.scroll().track(band_top, bottom, line_px);
        across.bar_across(left, band_bottom, wide, cell)
    }

    /// The cells the transport's own fields occupy, in pixels.
    ///
    /// # Why not the section's rails
    ///
    /// Because the glyphs and the clock are placed at `from`..`to` — the CELLS
    /// the layout gave the panel — while `section_px` answers with the RAILS,
    /// which run from rule to rule and are wider. The two nearly coincide in a
    /// narrow panel and do not in a wide one: the owner photographed the
    /// timeline touching the clock, and that gap is exactly the difference.
    #[must_use]
    pub fn fields_px(&self, lead: u16, cell: f32) -> (f32, f32) {
        (
            f32::from(lead + self.from) * cell,
            f32::from(self.to.saturating_sub(self.from)) * cell,
        )
    }

    /// The timeline's rectangle, when this section is showing a film with one.
    ///
    /// `left` and `wide` are the transport's own pixels — see
    /// [`Section::fields_px`] — and `top` is where the region's first line
    /// begins. The THIRD axis, on the same [`ronoctua_design::Scroll`] the
    /// other two are — so the cursor is drawn by `bar_across` and read back by
    /// `offset_at`, and grabbing it lands where it looks.
    ///
    /// The track stops short of the clock at the right and starts after the two
    /// buttons at the left. Those cells are reserved WHATEVER the pointer is
    /// doing: had the track grown when the buttons went away, the cursor would
    /// slide out from under the pointer in the instant of the click.
    #[must_use]
    pub fn time_bar_box(&self, foot: Foot) -> Option<(f32, f32, f32, f32)> {
        let time = self.time?;
        self.transport?;
        let (from, track) = track_px(foot.left, foot.wide, foot.cell, self.clock)?;
        let line_top = transport_top(self, foot);
        let (cell, line_px) = (foot.cell, foot.line_px);
        // CENTRED IN THE LINE, level with the glyphs beside it.
        //
        // It used to sit at the line's FOOT, which is where a scrollbar belongs
        // and where a timeline does not: the owner photographed the play glyph
        // and the track on two different heights and asked for them level.
        // `bar_across` measures up from what it is given, so what it is given
        // is the bar's own bottom.
        time.bar_across(from, line_top + (line_px + cell) / 2.0, track, cell)
    }

    /// The timeline's TRACK, which is the part one grabs.
    ///
    /// # The track and not the cursor is the hit target
    ///
    /// Because [`ronoctua_design::Scroll::offset_at`] maps any point of the
    /// track to a position — that is what it is for. A timeline one could only
    /// grab by its one-cell cursor would be a control one has to aim at, and
    /// clicking half way along a film is the ordinary way to use one.
    ///
    /// The cursor is drawn from [`Section::time_bar_box`] on top of this, in a
    /// stronger tone. Two rectangles, one arithmetic.
    #[must_use]
    pub fn time_track_box(&self, foot: Foot) -> Option<(f32, f32, f32, f32)> {
        self.time?;
        self.transport?;
        let (from, track) = track_px(foot.left, foot.wide, foot.cell, self.clock)?;
        let line_top = transport_top(self, foot);
        Some((
            from,
            line_top + (foot.line_px - foot.cell) / 2.0,
            track,
            foot.cell,
        ))
    }

    /// This section as a list that may not fit.
    #[must_use]
    /// How many of this section's OWN rows are on screen.
    ///
    /// # It is not `lines`, and the difference is two
    ///
    /// `lines` is how tall the section is; `above` is the part of it that does
    /// not scroll — the listing's path and header. What every question about
    /// scrolling wants is the remainder: how many entries a person can see, so
    /// how far the offset can go (`total - visible`) and how long the thumb is.
    ///
    /// Handed `lines` instead, the arithmetic stops two entries early. The
    /// owner found it with the mouse: *"ao deslizar a scrollbar da listagem …
    /// os dois últimos arquivos não apareceram, mas com as setas do teclado eu
    /// consegui ir mais para baixo"*. The keys reach them because they ask a
    /// different question — and in the footer they are right by CANCELLATION,
    /// their total carrying the same two lines their room does.
    ///
    /// # And with a grid it is still ENTRIES, not shelves
    ///
    /// `total` and `offset` are entries (see [`Section::grid`]), so the third
    /// number of that scroll has to be one too, or the thumb measures a
    /// fraction with two different units in it. A grid of four across and five
    /// lines deep in a twenty-line band shows four shelves, which is sixteen
    /// entries.
    pub const fn visible(&self) -> usize {
        let plain = self.lines.saturating_sub(self.above);
        match self.grid {
            Some(g) => (plain / g.down as usize) * g.across as usize,
            None => plain,
        }
    }

    /// The scroll this listing is at, as the bar's arithmetic reads it.
    pub const fn scroll(&self) -> ronoctua_design::Scroll {
        ronoctua_design::Scroll {
            total: self.total,
            lines: self.lines,
            offset: self.offset,
            top: self.top,
        }
    }
}

impl Row {
    /// WHICH field a cell belongs to, as an index.
    ///
    /// Walks from the right, so a click in the gap after a field belongs to
    /// that field rather than to nothing: a one-cell dead zone between columns
    /// is invisible and feels like a broken click.
    ///
    /// # Why the index, and why the painters ask for it
    ///
    /// Because [`Field::covers`] answers "MAY this field own the cell" and
    /// only walking from the right answers "does it". The two painters asked
    /// the first question once per field and lit every field that said yes —
    /// so where fields are packed with no gap between them, the `+ 1` of the
    /// gap rule gave cell `k + 1` to field `k` as well as to field `k + 1`,
    /// and BOTH lit.
    ///
    /// The owner read it off the screen on the one row of this manager where
    /// ten fields sit shoulder to shoulder: *"ao colocar o cursor sobre uma das
    /// letras de permissão, mais de uma (duas) letras estão alterando para a
    /// cor de destaque do hover, mas deveria ser apenas uma"*. It is also true
    /// of `owner:group`, which is packed the same way.
    ///
    /// `Field::covers`' own doc already promised this — *"THE ONE RULER, and
    /// there were three"* — and the wave that wrote it unified the wrong half,
    /// by teaching the painters to call `covers`. What the eye sees lit and
    /// what the hand would click are now the same walk, by construction.
    #[must_use]
    pub fn field_index_at(&self, cell: u16) -> Option<usize> {
        lit_field(&self.fields, cell)
    }

    /// Which field a cell belongs to.
    #[must_use]
    pub fn field_at(&self, cell: u16) -> Option<&Field> {
        self.fields.get(self.field_index_at(cell)?)
    }
}

/// Text width in CELLS, by cluster.
pub fn width_of(s: &str) -> u16 {
    ronoctua_render::layout::width(s)
}

/// The first cell that puts `what` cells in the middle of `room` cells at `at`.
///
/// Integer arithmetic, and it rounds DOWN: half a cell does not exist on this
/// grid, and a glyph one cell left of centre reads as centred while a glyph
/// half a cell anywhere reads as blurred.
pub const fn centred(at: u16, room: u16, what: u16) -> u16 {
    at.saturating_add(room.saturating_sub(what) / 2)
}

/// An action's technical answer, as an [`Explanation`] with nothing to fill in.
///
/// A function and not an inline literal because it is written at five sites,
/// and the empty `args` is the half that would drift: an `Explanation` whose
/// key expects an argument and whose `args` is empty draws the placeholder in
/// front of somebody, which is the shape invariant 8 exists to refuse.
pub fn does(key: &'static str) -> mode::Explanation {
    mode::Explanation {
        key,
        args: Vec::new(),
    }
}

/// What a row of the info panel offers, if anything.
///
/// # Everything that can be edited, is
///
/// The owner's rule for this panel, and the enum is its inventory: the name,
/// the permission, the owner, when it was modified, a song's tags, a
/// picture's or a PDF's details, and where a photograph was taken. Each is
/// reached by pressing the row — anywhere on it — or by `RET` on it with the
/// facts focused, and each asks in the manager's own picker, in both doors
/// (`ronoctua_shell_inspect::inspection_edit` says who writes what).
///
/// The first four reach what the LISTING already reaches — the rename
/// question, the permission cells, the owner's picker, the walk — through
/// the same code; the rest exist only here because only here is the file's
/// content read. A row that [`Doable::edits`] is drawn as a SETTING
/// (`chrome::Kind::Setting`), every other one as a FACT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doable {
    /// Nothing. A path is not somewhere one types, a file's size is what the
    /// kernel says it is, and a camera's ISO is what the shutter did. A row
    /// that lit up and then refused would be worse than one that never
    /// promised — `menu::Applies` states that rule and this is it, one
    /// register down.
    Nothing,
    /// The name: the rename question, prefilled, as the menu's `r`.
    Rename,
    /// The permission triplet: one target per character, as `Column::Perms`.
    Mode,
    /// `owner:group`: two targets, as `Column::Owner` and `Column::Group`.
    Ownership,
    /// When it was modified: a typed date (`ronoctua_files::act::set_modified`).
    Modified,
    /// A directory's size, which is a walk — as `TAB` on a directory.
    Measure,
    /// A song's or a film's tag, written by the `ronoctua-tags` program.
    Tag(ronoctua_tags::Field),
    /// A picture's or a PDF's detail, written by `ronoctua_inspect::amend`.
    Meta(ronoctua_inspect::amend::Meta),
    /// Where the file says it was made: taken out, after a question.
    RemoveLocation,
}

impl Doable {
    /// Whether the row CHANGES the file — what makes it a setting. A walk
    /// measures and changes nothing, so it is drawn as the fact it reports.
    #[must_use]
    pub const fn edits(self) -> bool {
        !matches!(self, Self::Nothing | Self::Measure)
    }
}

/// Where the panels' own rows sit inside a region: `(first line, how many).`
///
/// # Why this is not simply "the region"
///
/// Because the panels' fields ride the LISTING's rows, and every door puts
/// lines of its own above those — the footer two, the path and the header; the
/// window three, with the message line between them. The picture was fitted
/// into a box that started at the region's first line and ran its whole
/// height, so it was two lines taller than the panel it was in, centred in the
/// wrong box, and the difference came out as air underneath. Photographed and
/// reported in one sentence: *"the preview's picture is touching the top
/// line and has a lot of extra space underneath"*.
///
/// One function, four readers — each door's picture and each door's facts bar
/// — because that defect was one door's arithmetic written out by hand, and
/// the second door had written its own and got it right. Two answers to one
/// question is how the doors come to disagree.
///
/// `first` is the region's first line, `lines` its height INCLUDING the
/// chrome, `chrome` how many of those lines are the door's own, and `strip`
/// how many lines the bottom strip has — because down there the panels are not
/// in the listing's rows at all, they are underneath them.
#[must_use]
pub const fn panel_band(first: u16, lines: u16, strip: u16) -> (u16, u16) {
    if strip > 0 {
        return (first.saturating_add(lines), strip);
    }
    // THE REGION'S FIRST LINE, and never "minus the listing's chrome".
    //
    // It used to subtract it, and the doors computed the subtraction as
    // `2 - offset` — what was on screen right then. So the picture's box moved
    // two lines UP the moment somebody scrolled the listing, and the owner saw
    // exactly that: *"the same extra-spacing defect at the top of the preview
    // column is still happening, and it goes away only when the listing's
    // scrollbar is scrolled"*. A column whose top is a function of another column's offset
    // is not a column.
    //
    // The chrome does not scroll any more (see `View::region_rows`), and it is
    // the LISTING's: the panels start where every other section starts.
    (first, lines)
}

/// The rule COLUMN that marks the section holding the keyboard.
///
/// # Why the focus mark is a rule
///
/// Two earlier answers were refused from the screen: a one-line TITLE per
/// panel, which cost a line of the panel and was text where a tone would do,
/// and the panel's own SCROLLBAR lit, which was a whole cell of colour. What
/// is left that is already drawn beside a section is the rule it begins at —
/// see [`crate::chrome::divider_lit`].
///
/// # And the one the strip does not have is drawn
///
/// [`Panes::rules`] skips the rule before the FIRST panel when the panels sit
/// BELOW the listing: there is nothing to its left to be separated from, and a
/// line at cell nought with nothing behind it is a defect the owner has
/// already photographed once. That leaves a hole exactly where the preview
/// would be marked, so this answers for every panel and the door draws what
/// comes back — a boundary that exists because the section is FOCUSED is not
/// the same thing as one that is always there.
///
/// `None` for the listing, the sidebar and the fold: those three already light
/// something of their own — a row, or a glyph.
#[must_use]
pub fn focus_rule(panes: Panes, width: u16, pane: Pane) -> Option<u16> {
    if !matches!(pane, Pane::Preview | Pane::Info) {
        return None;
    }
    // AND NOTHING AT ALL WITH THE REGION GIVEN AWAY.
    //
    // It is the argument [`Panes::rules`] already makes one type along: a rule
    // separates two things, and with one panel filling the width there is
    // nothing to its left to be separated from — `boxes` puts that panel at
    // cell nought, so the rule would be drawn against the screen's own edge.
    // The owner photographed that line once already: *"no modo largo há uma
    // linha vertical na esquerda que continua aparecendo"*.
    //
    // `rules` refusing was not enough, and this is the second door into it:
    // both painters draw the focused section's rule EVEN WHEN `rules` did not
    // give them one, which is what puts the strip's first panel back on the
    // screen. With the keyboard in the wide panel — which is the only place
    // `Tab` can leave it, since `panes_on` has one stop — that arm drew the
    // line at cell nought again.
    if panes.whole {
        return None;
    }
    let want = if matches!(pane, Pane::Preview) {
        Panel::Preview
    } else {
        Panel::Info
    };
    panes
        .boxes(width)
        .into_iter()
        .find(|(p, _, _)| *p == want)
        .map(|(_, at, _)| at.saturating_sub(RULE))
}

/// Where the pointer's band begins and how wide it is, in PIXELS.
///
/// # Why one function and not one per door
///
/// Because the two doors had written it out separately and had arrived at
/// three different answers between them, and every one of the three was the
/// same defect: the band began one CELL to the right of the rule it should
/// have started at, so the row lit up with a strip of unlit background between
/// it and the line beside it. The owner reported it in those words — *"the
/// item's selection background does not touch the vertical separating line
/// on the listing's left-hand side"* — about the footer, and the window door
/// had it in both of its arms: the sidebar's band started at the margin
/// instead of at the screen's edge, and the listing's ran to the window's
/// right edge, straight under the preview.
///
/// # Why PIXELS, when everything else here is cells
///
/// Because the band's two edges are RULES, and a rule sits in the middle of
/// its own cell (see [`rule_x`]). Half a cell is not a thing the integer grid
/// can say, and a band that stopped at the cell's edge would be back to a gap
/// on one side. The `cell` and the `edge` come from the door, so this is still
/// one answer for both of them and not a pixel decided here.
///
/// The band stops exactly ON the rule, and that is deliberate: the rules are
/// painted after it, so the line covers the last pixel of the band rather than
/// leaving one of its own.
///
/// `at` is the cell the pointer is on, in the ROW's own cells — `None` when it
/// is over the row but not over a field, which lights the listing, because the
/// listing is what a row mostly is.
/// The rectangle a band takes for a NAMED pane.
///
/// # Why the doors cannot work this out from a cell
///
/// They used to, and it was `match pane { Sidebar => 0, _ => split }` — written
/// twice, once per door — handed to [`band_of`], which resolves a cell to a
/// section. That was right while only the listing and the sidebar could hold
/// the keyboard, and it broke the moment the `TAB` ring reached the two
/// panels: with the panels BELOW the listing the same cells belong to both,
/// and `band_of` answers for the listing. Photographed on the first try — the
/// cursor was on a fact and the band ran the width of the list above it.
///
/// A cell cannot answer, because the question is not "what is at this cell" but
/// "where is this pane" — and [`section_px`] has always answered that one. The
/// listing and the sidebar still come through `band_of`, whose two edges are
/// tuned against the rules drawn beside them.
#[must_use]
pub fn band_for(
    panes: Panes,
    pane: Pane,
    lead: u16,
    width: u16,
    cell: f32,
    edge: f32,
) -> (f32, f32) {
    match pane {
        Pane::Preview | Pane::Info => {
            section_px(panes, pane, lead, width, cell, edge).unwrap_or((0.0, 0.0))
        }
        Pane::Sidebar => band_of(panes, lead, width, Some(0), cell, edge),
        Pane::Listing | Pane::Fold => band_of(panes, lead, width, Some(panes.sidebar), cell, edge),
    }
}

/// Where a run of cells starts and how wide it is, in pixels: `cells` is the
/// half-open span from the row's first cell, `lead` the cells before it.
#[must_use]
pub fn band_px(cells: (u16, u16), lead: u16, cell: f32) -> (f32, f32) {
    let from = f32::from(lead.saturating_add(cells.0)) * cell;
    let wide = f32::from(cells.1.saturating_sub(cells.0)) * cell;
    (from, wide)
}

/// THE RUBBER BAND'S RECTANGLE, from what [`View::sweep_box`] answered.
///
/// # Why the conversion is here and not in the two doors
///
/// Because it is the SAME conversion, and a rectangle worked out twice is a
/// rectangle that disagrees with itself the first time one of the two is
/// edited — the law this manager's bars are already held to. The doors bring
/// only the three numbers the view cannot have: how wide a cell is, how tall a
/// line is, and where the region's first line sits on their own surface.
///
/// `cells` and `lines` are half-open, exactly as `sweep_box` gives them, and
/// the colour is `surface.sweep` — an alpha, because this is painted OVER the
/// thumbnails and what is underneath has to go on being visible.
#[must_use]
pub fn sweep_px(
    cells: (u16, u16),
    lines: (usize, usize),
    lead: u16,
    cell: f32,
    line_px: f32,
    first: f32,
) -> ronoctua_render::quad::Quad {
    let (x, width) = band_px(cells, lead, cell);
    #[allow(clippy::cast_precision_loss)]
    let y = first + lines.0 as f32 * line_px;
    #[allow(clippy::cast_precision_loss)]
    let height = lines.1.saturating_sub(lines.0) as f32 * line_px;
    ronoctua_render::quad::Quad {
        x,
        y,
        width,
        height,
        color: ronoctua_tokens::color::SURFACE_SWEEP,
    }
}

/// The x and the width of the hover band of a listing row, in pixels: from
/// the column `at` (the sidebar's split, or the surface's start) to where the
/// listing ends on the right — the panels' first rule, or the surface's edge.
/// A panel lights itself, and never the listing beside it.
#[must_use]
pub fn band_of(
    panes: Panes,
    lead: u16,
    width: u16,
    at: Option<u16>,
    cell: f32,
    edge: f32,
) -> (f32, f32) {
    let split = panes.sidebar;
    // WHERE THE LISTING ENDS ON THE RIGHT: the panels' first rule, or the
    // surface's own edge. A band running under the preview would light the
    // picture, and the pointer is not on the picture — it is on a row beside it.
    let right = if panes.right > 0 {
        rule_x(width.saturating_sub(panes.right), lead, cell)
    } else {
        edge
    };
    // A PANEL LIGHTS ITSELF, and never the listing beside it.
    //
    // The owner photographed this one: *"putting the cursor over an item
    // of the info column, on the exact row the mouse is on, ends up giving
    // the hover effect in the listing"*. The band was bounded on the right by the panels'
    // first rule and then drawn for whatever cell the pointer was in, so a
    // pointer inside a panel lit a row of a list it was not over.
    if let Some(c) = at
        && panes.right > 0
    {
        let which = panes.section_at(c, width, false);
        if matches!(which, Pane::Preview | Pane::Info) {
            let panel = match which {
                Pane::Preview => Panel::Preview,
                _ => Panel::Info,
            };
            return panel_px(panes, panel, lead, width, cell, edge).unwrap_or((0.0, 0.0));
        }
    }
    if split == 0 {
        return (0.0, right.max(0.0));
    }
    let line = rule_x(split, lead, cell);
    match at {
        // THE SIDEBAR'S BAND RUNS FROM THE SCREEN'S OWN EDGE. It is the one of
        // the four the owner called right, and it is right for a reason worth
        // keeping: the lead is not the sidebar's air, it is the door's, and a
        // band that began after it would leave a lit row with a dark margin.
        Some(c) if c < split => (0.0, line),
        _ => (line, (right - line).max(0.0)),
    }
}

/// Where a panel's REGION begins and ends, in pixels.
///
/// # Why this is not `at * cell` and `room * cell`
///
/// Because those are the TEXT's rails, and a picture is not text. It is the
/// two-rails rule this design system already keeps for the rule and the
/// selection band, applied to the third thing that fills a region: what fills
/// one runs from the rule on its left to the rule on its right, and where there
/// is no rule it runs to the surface's own edge.
///
/// The difference is not academic — it is half a cell at each end, and the
/// owner measured it as a picture that was not centred. In the window's strip
/// the preview's box began at the door's text margin (one cell in) while its
/// region began at the window's edge, so the air came out 60 px on the left
/// against 54 on the right for a box the code believed was centred.
#[must_use]
pub fn panel_px(
    panes: Panes,
    which: Panel,
    lead: u16,
    width: u16,
    cell: f32,
    edge: f32,
) -> Option<(f32, f32)> {
    let boxes = panes.boxes(width);
    let i = boxes.iter().position(|(p, _, _)| *p == which)?;
    let (_, at, _) = boxes[i];
    // A RULE ON THE LEFT, or the surface. Down in the strip the first panel has
    // nothing to its left — what separates it from the listing is the rule
    // ABOVE it — and `Panes::boxes` says so by giving it no leading rule.
    let left = if at >= RULE && (panes.right > 0 || i > 0) {
        rule_x(at.saturating_sub(RULE), lead, cell) + 1.0
    } else {
        0.0
    };
    let right = boxes.get(i + 1).map_or(edge, |(_, next, _)| {
        rule_x(next.saturating_sub(RULE), lead, cell)
    });
    Some((left, (right - left).max(0.0)))
}

/// ANY SECTION'S BAND, in pixels: where it begins and how wide it is.
///
/// # Why the pixels, and why one function
///
/// Because a column's edges are RULES and the surface's border, and neither is
/// on a cell boundary: `columns` is `floor(width / cell)`, so the last cell ends
/// before the border and a rule sits in the middle of its own cell. Anything
/// placed by cell index inherits that difference — and the owner measured every
/// one of its consequences: a scrollbar drawn a cell past the border and gone
/// off screen, another that answered for a cell the pointer never reached, and
/// air to the right of one column that changed with the window's width.
///
/// So the band is arithmetic in pixels, here, once. The hover's band asks it,
/// the scrollbar asks it, and the picture's box asks it — three readers, one
/// answer, which is the only arrangement in which they cannot disagree.
#[must_use]
pub fn section_px(
    panes: Panes,
    pane: Pane,
    lead: u16,
    width: u16,
    cell: f32,
    edge: f32,
) -> Option<(f32, f32)> {
    match pane {
        // THE LINE'S CONTROL IS NOT IN THE REGION at all — it sits on the line
        // above it, which belongs to the door. No column, so no box.
        Pane::Fold => None,
        Pane::Sidebar => (panes.sidebar > 0).then(|| (0.0, rule_x(panes.sidebar, lead, cell))),
        Pane::Listing => {
            let from = if panes.sidebar == 0 {
                0.0
            } else {
                rule_x(panes.sidebar, lead, cell) + 1.0
            };
            // WHERE THE LISTING ENDS: the panels' first rule when they sit
            // beside it, and the surface's own border when they do not.
            let to = if panes.right > 0 {
                rule_x(width.saturating_sub(panes.right), lead, cell)
            } else {
                edge
            };
            Some((from, (to - from).max(0.0)))
        }
        Pane::Preview => panel_px(panes, Panel::Preview, lead, width, cell, edge),
        Pane::Info => panel_px(panes, Panel::Info, lead, width, cell, edge),
    }
}

/// WHERE THE GLOBE GOES inside the info panel, minus the address.
///
/// The same arithmetic the preview's box gets, one panel along — `panel_px` for
/// the column and `panel_band` for the line — and then through `picture_in`, so
/// the air and the SHRINK-NEVER-GROW rule hold for the globe exactly as they do
/// for a photograph.
///
/// `place` is [`View::globe_place`]: how many LINES the picture was given and
/// how many CELLS wide it is. Two shapes go through it — the band across the
/// panel's top, which is the panel's whole width, and the leftmost COLUMN of
/// the expanded sheet — and neither door decides which, because the picture
/// that is drawn has to be the picture that was measured.
#[must_use]
pub fn globe_px(
    panes: Panes,
    lead: u16,
    width: u16,
    // THE THREE MEASUREMENTS THAT ARE PIXELS, together: a cell, a line and the
    // surface's right edge. One tuple and not three arguments, because seven is
    // where a signature stops being read and starts being counted.
    px: (f32, f32, f32),
    top: u16,
    place: (usize, u16),
) -> Option<ronoctua_render::image::Area> {
    let (cell, line_px, edge) = px;
    let (lines, cells) = place;
    if lines < 2 {
        return None;
    }
    let (x, wide) = panel_px(panes, Panel::Info, lead, width, cell, edge)?;
    // THE LAST OF THEM IS THE ADDRESS, so the picture stops one line short.
    let tall = line_px * (lines.saturating_sub(1)) as f32;
    Some(ronoctua_render::image::Area {
        x,
        y: f32::from(top) * line_px,
        // AND NEVER WIDER THAN THE PANEL, whatever the cells say: `min` is what
        // keeps a stale width from one frame ago inside the column it belongs
        // to, and the band's own answer is the panel's width already.
        width: wide.min(f32::from(cells) * cell),
        height: tall,
    })
}

/// [`globe_texture`] at a screen's scale — the texture PAINTED: `picture_in`
/// fits it back into the logical box, so the globe is as sharp as the letters
/// beside it (`crate::hidpi`).
#[must_use]
pub fn globe_texture_at(space: ronoctua_render::image::Area, scale: i32) -> (u32, u32) {
    let by = u32::try_from(scale.max(1)).unwrap_or(1);
    let (w, h) = globe_texture(space);
    (w.saturating_mul(by), h.saturating_mul(by))
}
