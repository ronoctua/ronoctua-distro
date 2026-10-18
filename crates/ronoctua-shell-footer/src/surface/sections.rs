//! THE HOST'S HALF OF A REGION LAID OUT IN SECTIONS (`region::Shape::
//! Sections`): the footer's arithmetic that lays the sections onto its lines,
//! measures their bars and their strip, resolves a point to the section under
//! it, and hands a gesture of the pointer to the region — asking the region
//! what each part is (`sections::Sectioned`) and never which feature it is.
//!
//! # Why the footer keeps this arithmetic
//!
//! Because it is the footer's lines, cells and pixels it is about: where a
//! section's bar is drawn is where the pointer grabs it, and both answers are
//! the footer's. The region answers questions in its own terms — its panes,
//! its rows, the section and row under a cell — and the footer turns them
//! into its surface. The window door has its own twin of all of this, in its
//! own geometry; the region is the same in both.
//!
//! It names no feature: what it knows of a region is that it is shown and
//! laid out in sections.

use super::*;
use crate::region::Shape;
use crate::sections::Sectioned;

impl Footer {
    /// The region SHOWN — what `view()` ranks — when it is laid out in
    /// sections: its id. The shape is the one the host keeps of it, a stored
    /// fact.
    pub(super) fn sectioned(&self) -> Option<&'static str> {
        match self.view() {
            ViewState::Region(id) if self.regions.kept_shape(id) == Shape::Sections => Some(id),
            _ => None,
        }
    }

    /// What the shown region in sections answers.
    pub(super) fn sections(&self) -> Option<&dyn Sectioned> {
        let id = self.sectioned()?;
        self.regions
            .get(id)
            .and_then(crate::region::Region::sections)
    }

    /// The same, for a gesture of the pointer that only moves the region's
    /// own state.
    pub(super) fn sections_mut(&mut self) -> Option<&mut dyn Sectioned> {
        let id = self.sectioned()?;
        self.regions
            .get_mut(id)
            .and_then(crate::region::Region::sections_mut)
    }

    /// Hands a gesture to the shown region in sections, with a doorway of its
    /// own, and does what it asked once it is back.
    fn with_sections(
        &mut self,
        f: impl FnOnce(&mut dyn Sectioned, &mut dyn crate::region::Doorway),
    ) {
        let Some(id) = self.sectioned() else {
            return;
        };
        let _ = self.with_region(id, |r, w| {
            if let Some(s) = r.sections_mut() {
                f(s, w);
            }
        });
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Tells the region which picture of its grid the pointer is over.
    ///
    /// The window door does this inline; here it is a method because the
    /// footer's pointer arrives in three places — entering, moving and leaving
    /// — and three copies of two lines is how two of them come to disagree.
    pub(super) fn hover_a_thumbnail(&mut self) {
        let columns = self.screen_columns();
        let room = self.grid().lines;
        let at = self
            .here
            .row_under_pointer
            .map(|line| (line, self.here.cell_under_pointer.unwrap_or(0)));
        if let Some(s) = self.sections_mut() {
            s.hover_thumb(columns, room, at);
        }
    }

    /// How many lines the strip pinned under the sections takes — zero when
    /// there is none.
    ///
    /// It is a HEIGHT and never a scroll: the strip is pinned under the last
    /// line of the list, so it costs the listing rows rather than following it.
    /// See `expansion_len` for the audit of who reads which.
    pub(super) fn sections_strip(&self) -> usize {
        self.sections()
            .map_or(0, |v| usize::from(v.panes(self.screen_columns()).strip))
    }

    /// The strip's own rows, built by the same region that builds the
    /// sections.
    pub(super) fn sections_strip_rows(&self) -> Vec<ExpansionRow> {
        let Some(view) = self.sections() else {
            return Vec::new();
        };
        view.strip_rows(self.screen_columns(), &self.texts)
            .into_iter()
            .map(|r| ExpansionRow {
                // THE SAME RULE AS A LINE OF THE LISTING: a row with nothing in
                // it is not a target. The strip is four lines and the info
                // panel may have two facts.
                click: (!r.fields.is_empty()).then_some(Click::Sections),
                // AND THE TRANSPORT'S ANCHOR TRAVELS WITH IT — see
                // `fields::Row::foot`: the strip's last line is the one the
                // remainder must fall above rather than below.
                foot: r.foot,
                fields: r.fields,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    /// The lines above the listing — the path and the header — through the
    /// region's own chrome, and only while it is plain: a menu, a picker or a
    /// sheet has no columns, and chrome over them would name nothing.
    ///
    /// THROUGH THE REGION'S OWN CHROME, and that is a repair: this door built
    /// the two rows by hand once, a SECOND builder for lines the region
    /// already assembles, and the day the row that names the columns became a
    /// choice the footer went on drawing it while the region counted one.
    ///
    /// The CLICK is the door's: the first line is the path and the rest is the
    /// listing's own, and both resolve through the region's `hit`.
    pub(super) fn sections_chrome(
        &self,
        view: &dyn Sectioned,
        width: u16,
        plain: bool,
    ) -> Vec<ExpansionRow> {
        if !plain {
            return Vec::new();
        }
        view.chrome_rows(width, &self.texts)
            .into_iter()
            .map(|row| ExpansionRow {
                click: Some(Click::Sections),
                fields: row.fields,
                foot: false,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    /// The two rooms this door gives the sections: the listing's lines, and
    /// the panels' — the strip's height when they are down there.
    pub(super) fn sections_rooms(&self) -> crate::fields::Room {
        let lines = self.grid().lines;
        crate::fields::Room {
            list: lines,
            panel: if self.sections_strip() > 0 {
                self.sections_strip()
            } else {
                lines
            },
        }
    }

    /// Drags one section's bar to where the pointer is.
    pub(super) fn drag_section_to(
        &mut self,
        pane: crate::fields::Pane,
        axis: crate::fields::Axis,
        x: f64,
        y: f64,
    ) {
        // ALONG THE TRACK THAT WAS GRABBED. The sideways bar's is the panel's
        // own pixels, and it answers first because the pane it belongs to also
        // has one going down.
        // AND THE FILM'S AXIS RUNS ACROSS TOO, over the track's own pixels
        // rather than the panel's — `fields::track_px` is the one answer, asked
        // here and by the rectangle that was drawn.
        if matches!(
            axis,
            crate::fields::Axis::Across | crate::fields::Axis::Time
        ) {
            let columns = self.screen_columns();
            let cell = tokens::metrics::cell_width(tokens::text::size());
            let edge = self
                .screens
                .iter()
                .find(|t| t.configured && t.width > 0)
                .map_or(0.0, |t| {
                    f32::from(u16::try_from(t.width).unwrap_or(u16::MAX))
                });
            let rooms = self.sections_rooms();
            let panes = self.sections().map(|v| v.panes(columns));
            let Some(panes) = panes else { return };
            let Some((left, wide)) = crate::fields::section_px(
                panes,
                pane,
                crate::chrome::FIELDS_LEAD,
                columns,
                cell,
                edge,
            ) else {
                return;
            };
            if wide <= 0.0 {
                return;
            }
            let Some((left, wide)) = (if matches!(axis, crate::fields::Axis::Time) {
                // THE CLOCK'S WIDTH IS THE SECTION'S — it decides where the
                // track ends, so grabbing it by any other arithmetic would read
                // a position the cursor was never drawn at.
                let clock = self.sections().map_or(0, |v| {
                    let chrome = v.chrome_rows(columns, &self.texts);
                    v.sections(&chrome, self.grid().lines, columns, &self.texts)
                        .iter()
                        .find(|s| s.pane == pane)
                        .map_or(0, |s| s.clock)
                });
                crate::fields::track_px(left, wide, cell, clock)
            } else {
                Some((left, wide))
            }) else {
                return;
            };
            if let Some(v) = self.sections_mut()
                && v.drag_to(
                    pane,
                    axis,
                    (x - f64::from(left)) / f64::from(wide),
                    columns,
                    rooms,
                )
            {
                self.owes_paint = true;
            }
            return;
        }
        let line_height = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let top = f64::from(self.first_of_expansion()) * line_height;
        let rooms = self.sections_rooms();
        let columns = self.screen_columns();
        // THE SECTION'S OWN BAND, and not the region's height for all four.
        //
        // The window door had this defect and the owner watched it: given the
        // whole surface, the listing's thumb ran down into the facts' box. The
        // strip's rule is where the listing and the sidebar end; the panels
        // down in the strip end at the surface. `Scroll::track` decides which,
        // and it is the same call the thumb was drawn with.
        #[allow(clippy::cast_precision_loss)]
        let tall = {
            let lines = match pane {
                crate::fields::Pane::Info => rooms.panel,
                _ => rooms.list,
            };
            let bar = ronoctua_design::Scroll {
                total: usize::MAX,
                lines,
                offset: 0,
                top: 0,
            };
            let bottom = self
                .screens
                .iter()
                .find(|t| t.configured && t.height > 0)
                .map_or(0.0, |t| t.height as f32 - self.keyboard_px());
            f64::from(bar.track(top as f32, bottom, line_height as f32)).max(1.0)
        };
        if let Some(v) = self.sections_mut()
            && v.drag_to(pane, axis, (y - top) / tall, columns, rooms)
        {
            // OWED, not drawn: see `Footer::owes_paint`. A drag sends a
            // `Motion` per pixel of pointer travel, and presenting waits for
            // the vblank — a hundred of them is a hundred frames, and the
            // listing is SEEN walking to where the thumb already is.
            self.owes_paint = true;
        }
    }

    /// EVERY SECTION'S SCROLLBAR, as rectangles — drawn here and tested for
    /// there. See `fields::Section::bar_box`.
    pub(super) fn sections_bars(
        &self,
        columns: u16,
        cell: f32,
        line_height: f32,
        edge: f32,
    ) -> Vec<crate::fields::Bar> {
        let Some(v) = self.sections() else {
            return Vec::new();
        };
        let panes = v.panes(columns);
        let top = f32::from(self.first_of_expansion()) * line_height;
        // THE SURFACE'S OWN BOTTOM, for the same reason as the window's — see
        // `fields::Section::bar_box`.
        let bottom = self.metric().map_or(top, crate::chrome::Frame::bottom);
        let chrome = v.chrome_rows(columns, &self.texts);
        v.sections(&chrome, self.grid().lines, columns, &self.texts)
            .iter()
            .filter_map(|sec| {
                let (x, wide) = crate::fields::section_px(
                    panes,
                    sec.pane,
                    crate::chrome::FIELDS_LEAD,
                    columns,
                    cell,
                    edge,
                )?;
                // BOTH OF A SECTION'S BARS. See the window door's twin: a
                // pane may have one going down and one going across, and a
                // pointer holding one is not holding the other.
                let down = sec
                    .bar_box(x + wide, top, bottom, cell, line_height)
                    .map(|r| (sec.pane, crate::fields::Axis::Down, r));
                let across = sec
                    .wide_bar_box(x, top, bottom, wide, cell, line_height)
                    .map(|r| (sec.pane, crate::fields::Axis::Across, r));
                // AND THE FILM'S TRACK — see the window door's twin.
                let time = sec
                    .time_track_box(crate::fields::Foot {
                        left: x,
                        wide,
                        cell,
                        line_px: line_height,
                        bottom,
                    })
                    .map(|r| (sec.pane, crate::fields::Axis::Time, r));
                Some([down, across, time])
            })
            .flatten()
            .flatten()
            .collect()
    }

    /// The film's controls, as rectangles that go ON the picture.
    ///
    /// Three of them and in this order: the half-transparent band, the
    /// timeline's track, and the cursor on it. They are painted after the
    /// pictures — see the `draw_range` pair in `paint` — because the film keeps
    /// the whole panel and anything drawn with the other backgrounds would be
    /// underneath it.
    ///
    /// `Section::time_bar_box` is where the cursor's rectangle lives, which is
    /// what keeps this in step with what the drag reads back.
    pub(super) fn sections_transport(
        &self,
        columns: u16,
        cell: f32,
        line_height: f32,
        edge: f32,
    ) -> Vec<ronoctua_render::quad::Quad> {
        let Some(v) = self.sections() else {
            return Vec::new();
        };
        if !v.transport_showing() {
            return Vec::new();
        }
        let panes = v.panes(columns);
        let top = f32::from(self.first_of_expansion()) * line_height;
        let chrome = v.chrome_rows(columns, &self.texts);
        let mut out = Vec::new();
        for sec in &v.sections(&chrome, self.grid().lines, columns, &self.texts) {
            let Some((_x, _wide)) = crate::fields::section_px(
                panes,
                sec.pane,
                crate::chrome::FIELDS_LEAD,
                columns,
                cell,
                edge,
            ) else {
                continue;
            };
            let mut band = |rect: (f32, f32, f32, f32), color| {
                out.push(ronoctua_render::quad::Quad {
                    x: rect.0,
                    y: rect.1,
                    width: rect.2,
                    height: rect.3,
                    color,
                });
            };
            // EVERY PART OF THE TRANSPORT FROM THE SAME FOOT — see
            // `fields::transport_top`, and the window door's twin.
            let foot = crate::fields::Foot {
                left: 0.0,
                wide: 0.0,
                cell,
                line_px: line_height,
                bottom: self.metric().map_or(top, crate::chrome::Frame::bottom),
            };
            // THE FIELDS' OWN CELLS, and not the section's rails — see
            // `Section::fields_px`: the glyphs and the clock sit at `from`..`to`
            // and the rails are wider, which is the gap the owner photographed
            // between the timeline and the numbers.
            let (fx, fwide) = sec.fields_px(crate::chrome::FIELDS_LEAD, cell);
            let foot = crate::fields::Foot {
                left: fx,
                wide: fwide,
                ..foot
            };
            if let Some(r) = crate::fields::transport_band(sec, foot) {
                band(r, tokens::color::SURFACE_TRANSPORT);
            }
            if let Some(r) = sec.time_track_box(foot) {
                band(r, tokens::color::RULE_STRONG);
            }
            if let Some(r) = sec.time_bar_box(foot) {
                band(r, tokens::color::TEXT_PRIMARY);
            }
        }
        out
    }

    /// Which section's bar a POINT is on — the same rectangles that were drawn.
    pub(super) fn sections_bar_at_point(
        &self,
        x: f64,
        y: f64,
    ) -> Option<(crate::fields::Pane, crate::fields::Axis)> {
        let width = self
            .screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map(|t| f32::from(u16::try_from(t.width).unwrap_or(u16::MAX)))?;
        let cell = tokens::metrics::cell_width(tokens::text::size());
        let line_height = tokens::metrics::line_height(tokens::text::size());
        self.sections_bars(self.screen_columns(), cell, line_height, width)
            .into_iter()
            .find(|(_, _, r)| {
                let (bx, by, bw, bh) = *r;
                x >= f64::from(bx)
                    && x < f64::from(bx + bw)
                    && y >= f64::from(by)
                    && y < f64::from(by + bh)
            })
            .map(|(p, axis, _)| (p, axis))
    }

    /// WHAT IS UNDER THE POINTER in the sections, asked of the region — the
    /// one question the window door asks too (`Sectioned::hit`), section
    /// first. This door answered it in four places once (the path's crumbs,
    /// the header, the listing's row and the strip's), each with its own idea
    /// of which list a `y` belonged to, and the owner found every one of them.
    pub(super) fn sections_target(&self) -> Option<crate::fields::Target> {
        let view = self.sections()?;
        let line = self.here.row_under_pointer?;
        let cell = self.here.cell_under_pointer.unwrap_or(0);
        view.hit(
            self.screen_columns(),
            self.grid().lines,
            line,
            cell,
            &self.texts,
        )
    }

    /// WHERE A DRAG WOULD LAND, resolved through the pointer's own path: the
    /// same `row_at`/`cell_at` and the same `hit`, so what lights under a drag
    /// and what a drop takes are one answer.
    pub(super) fn sections_carried_to(&mut self, x: f64, y: f64, copies: bool) {
        let (columns, room) = (self.screen_columns(), self.grid().lines);
        let texts = std::rc::Rc::clone(&self.texts);
        let (Some(line), Some(cell)) = (self.row_at(x, y), self.cell_at(x)) else {
            return;
        };
        if self
            .sections_mut()
            .is_some_and(|v| v.carry_to(columns, room, line, cell, copies, &texts))
        {
            self.draw_and_paint();
        }
    }

    /// A drag from somewhere else is over the sections.
    pub(super) fn sections_carrying_from_afar(&mut self, copies: bool) {
        if let Some(v) = self.sections_mut() {
            v.carrying_from_afar(copies);
        }
    }

    /// That drag is gone. `true` when something was lit and is not any more.
    pub(super) fn sections_carried_away(&mut self) -> bool {
        self.sections_mut().is_some_and(Sectioned::carried_away)
    }

    /// A drag of OURS ended, however it ended.
    pub(super) fn let_the_drag_go(&mut self) {
        if let Some(c) = self.carrying.as_mut() {
            c.done();
        }
        if self.sections_carried_away() {
            self.draw_and_paint();
        }
    }

    /// The right button: the region's menu, on whatever is under the
    /// pointer — through the same resolution the left button uses
    /// (`sections_target`). `None` is an answer and not a failure: the empty
    /// directory has no field anywhere and still has a menu.
    pub(super) fn sections_menu(&mut self) {
        let target = self.sections_target();
        self.with_sections(|s, w| s.menu_at(target.as_ref(), w));
    }

    /// A click on a line of the sections — the shown region's own line of
    /// `Footer::click_at`.
    ///
    /// EVERYTHING THE REGION OWNS, through ONE resolution: the target says
    /// the section, the row inside it and the field, and the region presses
    /// the key that goes with it. Nothing here decides anything the keyboard
    /// cannot reach.
    pub(super) fn sections_click(&mut self) {
        let Some(target) = self.sections_target() else {
            return;
        };
        // WHAT IS HELD, taken before the region is borrowed — see
        // `chrome::held`.
        let held = crate::chrome::held(self.pointer_mods());
        self.with_sections(|s, w| s.click(&target, held, w));
    }

    /// The pointer moved over the footer: which of the sections and rows it
    /// points at, and a shape or a globe turned with the hand — whether one
    /// turned.
    pub(super) fn sections_pointer_moved(&mut self, at: (f64, f64)) -> bool {
        let mut spun = false;
        // AND WHICH SECTION, which is what decides the film's two buttons are
        // drawn — see the window door's twin. Their CELLS are reserved
        // either way, so nothing moves when they arrive.
        // AND WHICH ROW OF THE LISTING, which is what decides whose name
        // walks: the owner asked for the hover to move a name too long for
        // its column, as well as the cursor.
        let aimed = self.sections_target();
        if let Some(v) = self.sections() {
            v.pointing_globe(matches!(
                aimed.as_ref().map(|t| &t.hit),
                Some(crate::fields::Hit::Globe | crate::fields::Hit::GlobeDot)
            ));
        }
        let row = aimed
            .as_ref()
            .filter(|t| t.pane == crate::fields::Pane::Listing && t.row != usize::MAX)
            .map(|t| t.row);
        // WHICH SECTION, ASKED OF THE ARITHMETIC and not of the target. A
        // target needs a FIELD, and most of a facts panel has none — the space
        // past a short value, every line below the last fact — so the pointer
        // crossing those answered "nowhere", and the expand button in the
        // corner went out before the hand could reach it (`Sectioned::pane_at`).
        let where_ = self.here.row_under_pointer;
        let cell = self.here.cell_under_pointer.unwrap_or(0);
        let wide = self.screen_columns();
        let deep = self.grid().lines;
        if let Some(v) = self.sections() {
            v.pointing(where_.and_then(|line| v.pane_at(wide, deep, line, cell)));
            v.pointing_at(row);
        }
        // AND A SHAPE TURNS WITH THE HAND. The deltas are in PIXELS and become
        // radians here, because the region has no idea how big a pixel is; a
        // whole screen's width is about half a turn, which is the ratio that
        // makes a piece feel attached to the pointer rather than geared to it.
        if let Some((was_x, was_y)) = self.here.dragging_globe {
            #[allow(clippy::cast_possible_truncation)]
            let moved = self
                .sections_mut()
                .is_some_and(|v| v.pan_globe((at.0 - was_x) as f32, (at.1 - was_y) as f32));
            self.here.dragging_globe = Some(at);
            if moved {
                spun = true;
            }
        }
        if let Some((was_x, was_y)) = self.here.spinning {
            let wide = self
                .screens
                .iter()
                .find(|t| t.configured && t.width > 0)
                .map_or(1.0, |t| f64::from(t.width));
            let by = std::f64::consts::PI / wide;
            // WHERE THE HAND WENT, in radians of turn. Which way a yaw counts
            // is the region's business (`Sectioned::spin`) — a convention
            // spelled at two call sites is a convention that gets one of them
            // backwards, which is what happened.
            #[allow(clippy::cast_possible_truncation)]
            let turned = self.sections_mut().is_some_and(|v| {
                v.spin(((at.0 - was_x) * by) as f32, ((at.1 - was_y) * by) as f32)
            });
            self.here.spinning = Some(at);
            if turned {
                spun = true;
            }
        }
        spun
    }

    /// A section's bar, held and dragged to `at`: whether one is held.
    pub(super) fn sections_bar_dragged(&mut self, at: (f64, f64)) -> bool {
        let Some((pane, axis)) = self.here.dragging_section else {
            return false;
        };
        self.drag_section_to(pane, axis, at.0, at.1);
        true
    }

    /// A button down on the listing, moved: whether it became a gesture (a
    /// band), and what a drag that has just picked something up carries —
    /// `None` when the pointer is on no line and cell.
    #[allow(clippy::type_complexity)]
    pub(super) fn sections_listing_moved(
        &mut self,
    ) -> Option<(bool, Option<(Vec<std::path::PathBuf>, bool)>)> {
        let (Some(line), Some(cell)) = (self.here.row_under_pointer, self.here.cell_under_pointer)
        else {
            return None;
        };
        // AND A BUTTON THAT IS DOWN ON THE LISTING IS A GESTURE NOW, or still
        // a click — see `Sectioned::moved_to`.
        let (columns, room) = (self.screen_columns(), self.grid().lines);
        let moved = self
            .sections_mut()
            .is_some_and(|v| v.moved_to(columns, room, line, cell));
        // AND A DRAG THAT HAS JUST PICKED SOMETHING UP HANDS IT TO THE
        // COMPOSITOR. From here on the pointer is its, and what this surface
        // hears is `wl_data_device`.
        let picked = self.sections_mut().and_then(Sectioned::begun_carrying);
        Some((moved, picked))
    }

    /// The pointer left the footer: a band ends where the hand left it, and
    /// the sections point at nothing — whether the band took or gave back
    /// marks.
    pub(super) fn sections_pointer_left(&mut self) -> bool {
        // AND A BAND ENDS WHERE THE HAND LEFT IT. What it took was drawn,
        // entry by entry, the whole way — undoing that silently is the worse
        // of the two answers.
        let swept = self.sections_mut().is_some_and(Sectioned::gave_up);
        if let Some(v) = self.sections() {
            v.pointing(None);
            v.pointing_at(None);
        }
        swept
    }

    /// The right button went down: a band the left one was sweeping ends —
    /// the menu is about what is chosen, and a set still moving is not one.
    pub(super) fn sections_right_pressed(&mut self) {
        if let Some(v) = self.sections_mut() {
            v.gave_up();
        }
    }

    /// A press on one of the sections' bars, which takes the pointer —
    /// whether it was one. The sections' first line of the press in
    /// `Footer::pointer_frame_here`.
    pub(super) fn sections_bar_pressed(&mut self, at: (f64, f64)) -> bool {
        if let Some((pane, axis)) = self.sections_bar_at_point(at.0, at.1) {
            self.here.dragging_section = Some((pane, axis));
            self.drag_section_to(pane, axis, at.0, at.1);
        } else {
            return false;
        }
        true
    }

    /// A press on the preview, the globe or the listing — whether it took the
    /// pointer (a click on the listing fires on the release,
    /// `Sectioned::pressed`). The sections' second line of the press, after
    /// the region's own bar (which they never draw: `has_bar`).
    pub(super) fn sections_listing_pressed(
        &mut self,
        at: (f64, f64),
        columns: u16,
        room: usize,
        held: ronoctua_keymap::Mods,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        if self
            .sections_target()
            .is_some_and(|t| matches!(t.pane, crate::fields::Pane::Preview))
            && self.sections().is_some_and(Sectioned::shape_here)
        {
            // A PRESS ON THE PIECE TAKES THE POINTER. After the bars, because
            // a bar drawn over the panel is still a control; before the row,
            // because a shape has no rows and the click would otherwise open
            // the file — which is what pressing a picture does, and is exactly
            // what somebody about to turn it does not want.
            self.here.spinning = Some(at);
        } else if self
            .sections_target()
            .is_some_and(|t| matches!(t.hit, crate::fields::Hit::Globe))
        {
            // A PRESS ON THE GLOBE TAKES THE POINTER TOO, for the reason just
            // above — and in a field of its own, not `spinning`: the two turn
            // pixels into different quantities (radians of yaw against degrees
            // of centre).
            //
            // THE PICTURE, AND NOT THE DOT. The dot is the one place on this
            // figure with a meaning of its own — it frames on the coordinate —
            // and a press that started a drag there could never become that
            // press. It is the scrollbar's rule: the thumb is a target, the
            // trough is a drag.
            self.here.dragging_globe = Some(at);
        } else if let (Some(line), Some(cell)) =
            (self.here.row_under_pointer, self.here.cell_under_pointer)
            && self
                .sections_mut()
                .is_some_and(|v| v.pressed(columns, room, line, cell, held, texts))
        {
            // THE REGION TOOK IT, and a press on the listing is no longer a
            // click: until the pointer moves or the button comes up there is
            // nothing that says which of the three gestures it is. The click
            // fires on the RELEASE.
        } else {
            return false;
        }
        true
    }

    /// The left button came up over the sections: whether a press that never
    /// became a gesture is a click.
    pub(super) fn sections_released(&mut self) -> bool {
        self.sections_mut().is_some_and(|v| v.let_go().is_some())
    }
}
