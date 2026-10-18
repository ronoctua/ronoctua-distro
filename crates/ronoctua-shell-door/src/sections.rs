//! A REGION LAID OUT IN SECTIONS — what the footer asks of a region whose
//! shape is [`crate::region::Shape::Sections`]: columns side by side, each
//! with its own rows, its own scroll and its own bar, a strip pinned under
//! them, pictures in its panels, a band under the pointer per section, and a
//! drag that carries what is under it out of the footer.
//!
//! # Why a second trait, and not more of `Region`
//!
//! Because a list is one column of rows the footer pages — every view but
//! one — and a region in sections is the other shape the footer draws: the
//! footer's own arithmetic lays the sections onto its lines, places their
//! pictures, paints their bands and bars, and resolves a point to the section
//! under it first. That arithmetic is the footer's (`surface/sections.rs`),
//! and what it asks of the region is here, by what each answer is — never by
//! which feature gives it. A region that is a list never sees any of it.
//!
//! # What is asked here, and what is not
//!
//! The QUESTIONS of the layout, the pointer, the bands and the pictures —
//! each a value, so a frame can ask them any number of times — and the
//! pointer's gestures that only move the region's own state (a press, a
//! drag, the wheel), which answer whether anything moved. What a gesture
//! DOES — a click, the menu — is carried out like everything a region does,
//! over its doorway ([`Sectioned::click`], [`Sectioned::menu_at`]).

use std::path::PathBuf;

use ronoctua_files::mode::Explanation;
use ronoctua_render::image::{Area, Rgba};

use crate::fields::{Axis, Pane, Panes, Room, Row, Section, Target};
use crate::region::Doorway;

/// THE QUESTIONS A REGION IN SECTIONS ANSWERS THE FOOTER — see the module.
///
/// `width` is the region's width in cells and `room` the lines its listing
/// has; both are the footer's, asked once per question.
pub trait Sectioned {
    // --- the layout --------------------------------------------------------

    /// How the width is divided between the sections.
    fn panes(&self, width: u16) -> Panes;
    /// Whether the sections are shown as themselves — no menu, no sheet, no
    /// picker covering them.
    fn plain(&self) -> bool;
    /// The lines above the listing (the path, the header), as rows.
    fn chrome_rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row>;
    /// How many of them there are.
    fn chrome_lines(&self) -> usize;
    /// Every section, laid out under the chrome, `take` lines at most.
    fn sections(
        &self,
        chrome: &[Row],
        take: usize,
        width: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Section>;
    /// The strip pinned under the sections, as rows (none when the panels
    /// are beside the listing).
    fn strip_rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row>;
    /// The length of what covers the sections (a menu, a sheet), when one
    /// does.
    fn modal_len(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Option<usize>;
    /// How many lines the region scrolls through, the chrome counted in.
    fn region_len(&self, chrome: usize, width: u16) -> Option<usize>;
    /// The listing's first line on screen — the offset is the region's own.
    fn list_offset(&self) -> usize;
    /// Moves the listing's first line on screen.
    fn scroll_listing_to(&mut self, to: usize);

    // --- the pointer -------------------------------------------------------

    /// What is under line `line`, cell `cell`: the section first, then its
    /// row and field.
    fn hit(
        &self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Target>;
    /// Which section a line and a cell are in, field or no field.
    fn pane_at(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<Pane>;
    /// The section and its own line the band under the pointer lights.
    fn band_row(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<(Pane, usize)>;
    /// Whether a band may light that section — never over a picture.
    fn band_belongs(&self, pane: Pane) -> bool;
    /// The cells the band under the pointer covers, when they are not the
    /// section's whole width.
    fn band_cells_at(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<(u16, u16)>;
    /// How many lines the band under the pointer covers.
    fn band_lines_at(&self, width: u16, room: usize, line: usize, cell: u16) -> usize;
    /// Which section the pointer is in, told for what it lights.
    fn pointing(&self, pane: Option<Pane>);
    /// Which row of the listing the pointer is on.
    fn pointing_at(&self, row: Option<usize>);
    /// Whether the pointer is on the globe.
    fn pointing_globe(&self, on: bool);
    /// Which picture of a grid the pointer is over (`at`: line and cell).
    fn hover_thumb(&mut self, width: u16, room: usize, at: Option<(usize, u16)>);
    /// A section's bar dragged to `fraction` of its track: whether it moved.
    fn drag_to(&mut self, pane: Pane, axis: Axis, fraction: f64, width: u16, room: Room) -> bool;
    /// The wheel over section `which`, in lines: whether it moved.
    fn wheel(&mut self, which: Pane, by: i64, width: u16, room: usize) -> bool;
    /// A button down on the listing: whether the region took it (a click is
    /// then decided on the release, [`Sectioned::let_go`]).
    fn pressed(
        &mut self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        mods: ronoctua_keymap::Mods,
        texts: &ronoctua_i18n::Texts,
    ) -> bool;
    /// The button still down, moved: whether it became a gesture.
    fn moved_to(&mut self, width: u16, room: usize, line: usize, cell: u16) -> bool;
    /// The button came up: the target, when it was a click.
    fn let_go(&mut self) -> Option<Target>;
    /// The gesture ends where it is: whether it took or gave back anything.
    fn gave_up(&mut self) -> bool;
    /// Whether a shape is on show, which a press turns.
    fn shape_here(&self) -> bool;
    /// The shape turned by the hand, in radians: whether it turned.
    fn spin(&mut self, right: f32, down: f32) -> bool;
    /// The globe panned by the hand, in pixels: whether it moved.
    fn pan_globe(&mut self, right: f32, down: f32) -> bool;
    /// A click on `target`, with the modifiers `held` — done over `way`.
    fn click(&mut self, target: &Target, held: ronoctua_keymap::Mods, way: &mut dyn Doorway);
    /// The menu, on what is under the pointer (`None`: on nothing) — done
    /// over `way`.
    fn menu_at(&mut self, target: Option<&Target>, way: &mut dyn Doorway);

    // --- the drag ----------------------------------------------------------

    /// A drag that has just picked something up: what it carries and
    /// whether as copies.
    fn begun_carrying(&mut self) -> Option<(Vec<PathBuf>, bool)>;
    /// A drag over line `line`, cell `cell`: whether what is lit changed.
    fn carry_to(
        &mut self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        copies: bool,
        texts: &ronoctua_i18n::Texts,
    ) -> bool;
    /// A drag from somewhere else came over the region.
    fn carrying_from_afar(&mut self, copies: bool);
    /// The drag is gone: whether something lit is not any more.
    fn carried_away(&mut self) -> bool;
    /// Whether a drop would land where the drag is now.
    fn landing_now(&self) -> bool;
    /// The rubber band being swept: first and last cell, first and last line.
    fn sweep_box(&self, width: u16, room: usize) -> Option<(u16, u16, usize, usize)>;
    /// The folder a drop would land in, the same way.
    fn landing_box(&self, width: u16, room: usize) -> Option<(u16, u16, usize, usize)>;

    // --- the bands ---------------------------------------------------------

    /// The section with the keyboard, and its line.
    fn focus_line(&self, width: u16, room: usize) -> Option<(Pane, usize)>;
    /// The cells the focus band covers, when they are not the section's.
    fn focus_cells(&self, width: u16, room: usize) -> Option<(u16, u16)>;
    /// How many lines the focus band covers.
    fn focus_lines(&self, width: u16, room: usize) -> usize;
    /// The listing's line that keeps a quieter band while the keyboard is
    /// elsewhere.
    fn resting_line(&self, width: u16, room: usize) -> Option<usize>;
    /// Its cells, when they are not the listing's.
    fn resting_cells(&self, width: u16, room: usize) -> Option<(u16, u16)>;
    /// Its lines.
    fn resting_lines(&self, width: u16, room: usize) -> usize;
    /// The section with the keyboard.
    fn pane(&self) -> Pane;

    // --- the line ----------------------------------------------------------

    /// What the row under the cursor does, when a list covers the sections.
    fn cursor_explains(&self) -> Option<Explanation>;
    /// What went wrong with a film.
    fn film_trouble(&self) -> Option<Explanation>;
    /// What went wrong with a shape.
    fn shape_said(&self) -> Option<Explanation>;
    /// Whether the door's line can be opened — the keyboard's `Tab` lands
    /// on its control only then.
    fn fold_is_shown(&self, shown: bool);
    /// Whether the keyboard is on that control.
    fn fold_focused(&self) -> bool;

    // --- the pictures ------------------------------------------------------

    /// Whether a film's controls are drawn over its picture.
    fn transport_showing(&self) -> bool;
    /// The preview's still picture.
    fn preview_pixels(&self) -> Option<&Rgba>;
    /// The picture that redraws itself (a film's frame, a turned shape).
    fn live_frame(&self) -> Option<&Rgba>;
    /// The preview is drawn at this scale.
    fn preview_at_scale(&self, scale: i32);
    /// The preview's box, as it was measured, in pixels.
    fn measured_preview(&self, wide: f32, tall: f32);
    /// The key the preview's picture is uploaded under.
    fn preview_key(&self) -> Option<String>;
    /// The preview's picture as it is drawn now.
    fn preview_now(&self) -> Option<&Rgba>;
    /// Where the globe goes: its lines, its cells and how much of its top
    /// scrolled away.
    fn globe_place(&self) -> Option<(usize, u16, usize)>;
    /// The globe's box, as it was measured: its size in pixels, the pixels
    /// scrolled off its top, and the palette it is painted in.
    fn measured_globe(&self, size: (u32, u32), from: u32, theme: usize);
    /// The key the globe's picture is uploaded under.
    fn globe_key(&self) -> Option<String>;
    /// The globe's picture.
    fn globe_frame(&self) -> Option<Rgba>;
    /// Where each picture of a grid goes, by its key.
    #[allow(clippy::too_many_arguments)]
    fn grid_pictures(
        &self,
        width: u16,
        room: usize,
        lead: u16,
        cell: f32,
        line: f32,
        top: f32,
        page: ronoctua_tokens::theme::Ground,
    ) -> Vec<(String, Area)>;
    /// The keys of the grid's pictures in hand.
    fn thumb_keys(&self) -> Vec<String>;
    /// Whether the picture under `key` moves (a new frame every time).
    fn thumb_moves(&self, key: &str) -> bool;
    /// The picture under `key`.
    fn thumb_pixels(&self, key: &str) -> Option<&Rgba>;
}
