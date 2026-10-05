//! THE FOOTER'S MEDIA COLUMN: where the player's picture goes in the region,
//! beside the rows or over all of it, and what stands at its foot — a line
//! of the transport and a time bar, the window's two lines in this door.
//!
//! # A function of the width alone
//!
//! The panel is placed from the region's WIDTH and nothing else
//! ([`footer_panel`]): the rows are laid out at what it leaves, and how many
//! rows there are is what sizes the region — a panel that asked the region's
//! height back would be the footer asking itself
//! (`the-footer-does-not-ask-itself`). Its lines are the region's, whatever
//! they are once the rows have sized it ([`shape`]).
//!
//! # Three widths, the column's
//!
//! It is as wide as the column the player sits in at its three sizes —
//! `space.column.large`, `medium` and `small` — the widest that leaves the
//! rows at least as much as it takes. A region too narrow even for the small
//! one shows the rows whole and says why on a line of the home
//! (`player-panel-no-room`, a refusal is a sentence): `o` shows the picture
//! alone there all the same.
//!
//! # Whatever plays has one
//!
//! The panel stands while something is loaded, not only while a video
//! plays: the picture — a film, or a cover the player decodes as one — and
//! with no picture a TITLE CARD, what plays in words, on the region's own
//! ground (never on `surface.backdrop`, which carries no text).
//!
//! # One geometry for the paint and the pointer
//!
//! The transport's buttons are placed by [`buttons`] and the time bar by
//! [`Track`], and the footer records what the frame placed — as it does the
//! modeline's segments — so a press is taken where it was drawn.
//!
//! # Each screen, its own pictures
//!
//! The footer is a surface on every screen, and each shows the pictures
//! routed to IT ([`placed_on_screen`], `player_layout::tiles_on` by that
//! screen's connector) — placed in the panel as every door places them. The
//! footer keeps one picture a lane for all of them: every lane routed to one
//! of its screens ([`lanes_shown`]), asked once and drawn wherever it goes.

use crate::player_transport::{BUTTONS, shown};
use ronoctua_player::wire::State;
use ronoctua_tokens as tokens;

/// Where the footer puts the picture, in the surface's cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
    /// Its first column.
    pub from: u16,
    /// How many columns it takes.
    pub cells: u16,
}

/// The widths the panel steps through, the widest first: the column's own
/// three sizes, so the picture beside the rows is the one the column shows.
pub const STEPS: [u16; 3] = [
    tokens::space::COLUMN_LARGE,
    tokens::space::COLUMN_MEDIUM,
    tokens::space::COLUMN_SMALL,
];

/// The footer's picture: beside the rows, or the whole region.
///
/// Beside the rows it is the widest of [`STEPS`] that leaves them at least
/// its own width, at the region's right end, and the region's own bar keeps
/// its last cell. Only the picture (`o`) takes the whole region, the window's
/// `Only` in this door. `None` when not even the narrowest fits: the rows
/// whole, and a line saying why.
#[must_use]
pub fn footer_panel(columns: u16, only: bool) -> Option<Panel> {
    let usable = columns.saturating_sub(tokens::space::FILES_EDGERIGHT);
    if only {
        return (usable > 0).then_some(Panel {
            from: 0,
            cells: usable,
        });
    }
    STEPS
        .into_iter()
        .find(|&cells| usable >= cells.saturating_mul(2))
        .map(|cells| Panel {
            from: usable - cells,
            cells,
        })
}

/// How many lines the region keeps while the panel stands beside the rows:
/// a picture of 16:9 on the panel's width — a cell `cell_px` wide, a line
/// `line_px` tall — and the transport and the time bar under it.
///
/// THE FILM IS ONE SIZE ON EVERY PAGE. The region is as tall as its rows, and
/// the panel had what they left: two rows of a page whose subject is gone
/// left the picture none, a question three lines, and the film jumped in
/// size at every page. The rows are padded to this instead — the region's
/// ceiling still binds it — and it is a function of the WIDTH alone, never of
/// what the region holds (`the-footer-does-not-ask-itself`).
#[must_use]
pub fn panel_lines(panel: Panel, cell_px: f32, line_px: f32) -> u16 {
    let picture = f32::from(panel.cells) * cell_px * 9.0 / 16.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lines = (picture / line_px.max(1.0)).ceil() as u16;
    lines.saturating_add(2)
}

/// The panel's lines, in the region's: the picture, then the transport and
/// the time bar at its foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shape {
    /// The picture's first line and how many it takes, when any are left.
    pub picture: Option<(u16, u16)>,
    /// The transport's line.
    pub transport: Option<u16>,
    /// The time bar's line.
    pub bar: Option<u16>,
}

/// The panel's lines in a region whose first line is `first` and which is
/// `lines` tall: the foot first — the transport and the bar are the controls
/// — and what is left above them for the picture.
#[must_use]
pub const fn shape(first: u16, lines: u16) -> Shape {
    match lines {
        0 => Shape {
            picture: None,
            transport: None,
            bar: None,
        },
        1 => Shape {
            picture: None,
            transport: Some(first),
            bar: None,
        },
        2 => Shape {
            picture: None,
            transport: Some(first),
            bar: Some(first + 1),
        },
        _ => Shape {
            picture: Some((first, lines - 2)),
            transport: Some(first + lines - 2),
            bar: Some(first + lines - 1),
        },
    }
}

/// A button of the panel's transport, where the frame placed it: its line,
/// its first cell, the cell after it, and the command it runs.
pub type Placed = (u16, u16, u16, &'static str);

/// The transport's buttons on `line` of `panel`: the window's, the ones the
/// state shows, three cells apart from the panel's first column — a glyph
/// and its air, both answering the click — and only the ones whole inside it.
#[must_use]
pub fn buttons(panel: Panel, line: u16, state: Option<&State>) -> Vec<Placed> {
    let end = panel.from + panel.cells;
    let mut at = panel.from;
    let mut out = Vec::new();
    for id in BUTTONS.into_iter().filter(|id| shown(id, state)) {
        if at + 2 > end {
            break;
        }
        out.push((line, at, at + 2, id));
        at += 3;
    }
    out
}

/// The button placed at the cell `column` of `line`, when one was.
#[must_use]
pub fn button_at(placed: &[Placed], line: u16, column: u16) -> Option<&'static str> {
    placed
        .iter()
        .find(|(l, a, b, _)| *l == line && column >= *a && column < *b)
        .map(|(_, _, _, id)| *id)
}

/// Where the clock goes on the transport's line: at the panel's right end,
/// a cell after the last button — `None` when it does not fit there whole,
/// because a clock cut at the edge reads as a wrong time.
#[must_use]
pub fn clock_at(panel: Panel, placed: &[Placed], cells: u16) -> Option<u16> {
    let after = placed.last().map_or(panel.from, |(_, _, b, _)| b + 1);
    let end = panel.from + panel.cells;
    let from = end.checked_sub(cells)?;
    (from >= after).then_some(from)
}

/// The cells a seek beside the clock takes: its glyph and the cell of air
/// after it, both answering the click — a transport button's own box
/// ([`buttons`]).
const SEEK_CELLS: u16 = 2;

/// THE CLOCK BETWEEN ITS TWO SEEKS — `◀◀ 1:23 / 4:56 ▶▶` — at the right end
/// of the transport's line: ten seconds back, the clock, ten seconds ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockGroup {
    /// The seek back, where it was placed: its glyph on its first cell and
    /// a cell of air before the clock.
    pub back: Placed,
    /// The clock's first cell.
    pub clock_from: u16,
    /// The seek ahead: a cell of air after the clock, then its glyph and
    /// the cell of air that keeps it off the panel's edge.
    pub ahead: Placed,
}

/// The clock and its two seeks on `line`, flush with the panel's right end
/// — a cell of air between each piece, and one after the last glyph, which
/// never stands against the edge — after the buttons already `placed`, a
/// cell apart from the last. `None` WHOLE when the three do not fit: a seek
/// without its clock is a glyph nobody can read, and a clock cut reads as a
/// wrong time.
#[must_use]
pub fn clock_group(
    panel: Panel,
    line: u16,
    placed: &[Placed],
    clock_cells: u16,
) -> Option<ClockGroup> {
    let after = placed.last().map_or(panel.from, |(_, _, b, _)| b + 1);
    let end = panel.from + panel.cells;
    let ahead = end.checked_sub(SEEK_CELLS)?;
    let clock_from = ahead.checked_sub(clock_cells.checked_add(1)?)?;
    let back = clock_from.checked_sub(SEEK_CELLS)?;
    (back >= after).then_some(ClockGroup {
        back: (line, back, back + SEEK_CELLS, "player-rewind"),
        clock_from,
        ahead: (line, ahead, ahead + SEEK_CELLS, "player-forward"),
    })
}

/// What the transport's clock says of `s`: where it is, and of how long when
/// it has a length — `None` with nothing loaded.
#[must_use]
pub fn clock_text(s: &State) -> Option<String> {
    s.current?;
    Some(s.duration.map_or_else(
        || crate::player::clock(s.position),
        |d| {
            format!(
                "{} / {}",
                crate::player::clock(s.position),
                crate::player::clock(d)
            )
        },
    ))
}

/// THE TRANSPORT'S LINE, whole — the ONE answer every door paints from and
/// takes a press, a click and a hover by: the buttons ([`buttons`]), then
/// the seeks around the clock ([`clock_group`]) placed among them, and the
/// clock's first cell. Where the group does not fit, the clock alone where
/// it fits ([`clock_at`]), as it always was — and neither where that does
/// not: the seeks are `j` and `l` still.
#[must_use]
pub fn transport(
    panel: Panel,
    line: u16,
    state: Option<&State>,
) -> (Vec<Placed>, Option<(u16, u16)>) {
    let mut placed = buttons(panel, line, state);
    let Some(cells) = state
        .and_then(clock_text)
        .map(|c| ronoctua_protocol::cells(&c))
    else {
        return (placed, None);
    };
    if let Some(group) = clock_group(panel, line, &placed, cells) {
        placed.push(group.back);
        placed.push(group.ahead);
        return (placed, Some((line, group.clock_from)));
    }
    let alone = clock_at(panel, &placed, cells).map(|from| (line, from));
    (placed, alone)
}

/// A third of the picture, across: where a click lands decides what a
/// double click does there ([`PictureClicks`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// The left third: ten seconds back.
    Back,
    /// The middle: the whole screen, as a double click always did.
    Middle,
    /// The right third: ten seconds ahead.
    Ahead,
}

/// Which third of a picture `width` wide the point `x` (from its left
/// edge) is in.
#[must_use]
pub fn picture_zone(width: f64, x: f64) -> Zone {
    let third = width / 3.0;
    if x < third {
        Zone::Back
    } else if x >= width - third {
        Zone::Ahead
    } else {
        Zone::Middle
    }
}

/// What a click on the picture does, as [`PictureClicks`] reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PictureClick {
    /// Play or pause — the first click, as on every player.
    Toggle,
    /// The second, in the middle: play again what the first paused, and the
    /// whole screen — or back from it.
    ToggleAndMaximize,
    /// The second, on a side: play again what the first paused, and seek by
    /// these seconds — back is negative.
    ToggleAndSeek(f64),
    /// A third and on, on the same side while it seeks: seek again, and no
    /// more pausing.
    Seek(f64),
}

/// THE CLICKS ON A PICTURE, one after the other: the first plays or pauses,
/// a second quick enough in the middle maximizes, on a side it seeks ten
/// seconds — and each further one on the same side seeks again. The ONE
/// reading the window, the column and the footer take (`the doors do the
/// same`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PictureClicks {
    /// The last click that counts: when, where, and whether it sought.
    last: Option<(std::time::Instant, Zone, bool)>,
}

impl PictureClicks {
    /// A click at `now` in `zone`; `double` is how quick a second one is.
    pub fn clicked(
        &mut self,
        now: std::time::Instant,
        zone: Zone,
        double: std::time::Duration,
    ) -> PictureClick {
        let step = |zone: Zone| match zone {
            Zone::Back => -crate::player::SEEK_STEP,
            Zone::Middle | Zone::Ahead => crate::player::SEEK_STEP,
        };
        let recent = self
            .last
            .filter(|(then, _, _)| now.saturating_duration_since(*then) <= double);
        match recent {
            // ANOTHER ON THE SAME SIDE while it seeks: again, and only that.
            Some((_, side, true)) if side == zone => {
                self.last = Some((now, zone, true));
                PictureClick::Seek(step(zone))
            }
            // THE SECOND: the middle maximizes, a side seeks.
            Some((_, _, false)) => match zone {
                Zone::Middle => {
                    self.last = None;
                    PictureClick::ToggleAndMaximize
                }
                Zone::Back | Zone::Ahead => {
                    self.last = Some((now, zone, true));
                    PictureClick::ToggleAndSeek(step(zone))
                }
            },
            // A FIRST — nothing before it, too long ago, or the other side
            // after a seek.
            _ => {
                self.last = Some((now, zone, false));
                PictureClick::Toggle
            }
        }
    }
}

/// The time bar: a line of the panel across its whole width, what has
/// played in the accent — pressed, it seeks there, and held it drags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    /// Its line.
    pub line: u16,
    /// Its first cell.
    pub from: u16,
    /// The cell after its last.
    pub to: u16,
}

impl Track {
    /// The time bar of `panel`, on `line`.
    #[must_use]
    pub const fn of(panel: Panel, line: u16) -> Self {
        Self {
            line,
            from: panel.from,
            to: panel.from + panel.cells,
        }
    }

    /// Its ends, in logical pixels, for cells `cell` wide.
    #[must_use]
    pub fn px(self, cell: f32) -> (f32, f32) {
        (f32::from(self.from) * cell, f32::from(self.to) * cell)
    }

    /// Whether `(x, y)` is on it — `line` the line the pointer is on.
    #[must_use]
    pub fn holds(self, x: f64, line: u16, cell: f32) -> bool {
        let (l, r) = self.px(cell);
        line == self.line && x >= f64::from(l) && x < f64::from(r)
    }

    /// How far along it `x` is, clamped to its ends: a drag that leaves the
    /// bar keeps the end it left by.
    #[must_use]
    pub fn along(self, x: f64, cell: f32) -> f64 {
        let (l, r) = self.px(cell);
        ((x - f64::from(l)) / f64::from((r - l).max(1.0))).clamp(0.0, 1.0)
    }
}

/// What a frame laid out of the panel: ONE answer the painter draws from and
/// the pointer reads back — the footer keeps the last one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drawn {
    /// The panel, when the region has one.
    pub panel: Option<Panel>,
    /// The picture's lines — or the title card's.
    pub picture: Option<(u16, u16)>,
    /// The transport's buttons, where they were placed — the seeks around
    /// the clock among them ([`transport`]).
    pub buttons: Vec<Placed>,
    /// The clock's line and first cell, where it fits ([`transport`]).
    pub clock: Option<(u16, u16)>,
    /// The time bar.
    pub track: Option<Track>,
}

/// The panel laid out in a region whose first line is `first` and which is
/// `lines` tall: [`shape`]'s lines, the [`buttons`] on the transport's and the
/// [`Track`] on the bar's.
///
/// THE TRANSPORT KEEPS THE TEXT'S INSET where the panel begins at the
/// region's first column (Only, `o`): a button's glyph sits on its first
/// cell, and the stage's first one — previous — was drawn ON the screen's
/// left edge, its bar cut at the first pixel (the final battery's
/// `footer-media`, `04-1500-stage`). Beside the rows the rows are its air;
/// the window places its own after its `MARGIN`. The picture and the time
/// bar still take the whole region.
#[must_use]
pub fn drawn(panel: Panel, first: u16, lines: u16, state: Option<&State>) -> Drawn {
    let s = shape(first, lines);
    let transport = if panel.from == 0 {
        Panel {
            from: crate::chrome::INSET,
            cells: panel.cells.saturating_sub(crate::chrome::INSET),
        }
    } else {
        panel
    };
    let (buttons, clock) = s
        .transport
        .map(|line| self::transport(transport, line, state))
        .unwrap_or_default();
    Drawn {
        panel: Some(panel),
        picture: s.picture,
        buttons,
        clock,
        track: s.bar.map(|line| Track::of(panel, line)),
    }
}

/// [`drawn`] with the picture FOLDED away (`player-picture-fold`): the
/// transport and the time bar on the region's last two lines, and no
/// picture — the panel keeps its width, and the rows beside it their room.
#[must_use]
pub fn drawn_folded(panel: Panel, first: u16, lines: u16, state: Option<&State>) -> Drawn {
    let foot = lines.min(FOLDED_LINES);
    drawn(panel, first + lines - foot, foot, state)
}

/// The lines the panel keeps with the picture folded away: the transport
/// and the time bar ([`panel_lines`] without the picture).
pub const FOLDED_LINES: u16 = 2;

/// Where each picture routed to the screen `connector` goes in the panel's
/// `room` — logical pixels of that screen's footer: the ONE placement every
/// door draws with (`player_layout::placed`).
#[must_use]
pub fn placed_on_screen(
    s: &State,
    connector: Option<&str>,
    room: ronoctua_player::arrange::Rect,
) -> ronoctua_player::arrange::Placement {
    let tiles = crate::player_layout::tiles_on(s, connector);
    crate::player_layout::placed(s, &tiles, room)
}

/// The lanes whose pictures a footer on the screens `connectors` shows:
/// every one routed to one of them, each once, in the order the screens
/// come — a screen with no name yet shows them all.
#[must_use]
pub fn lanes_shown<'a>(
    s: &State,
    connectors: impl IntoIterator<Item = Option<&'a str>>,
) -> Vec<ronoctua_player::own::LaneId> {
    let mut lanes = Vec::new();
    for connector in connectors {
        for tile in crate::player_layout::tiles_on(s, connector) {
            if !lanes.contains(&tile.lane) {
                lanes.push(tile.lane);
            }
        }
    }
    lanes
}

/// The TITLE CARD: what plays, in words, in a picture's place `cells` wide
/// and `lines` tall — each line set in the middle of the width and the block
/// in the middle of the height, as `(line, column, text)` from the place's
/// corner. A title longer than the place keeps the lines it has, the last
/// one cut at its end: a name, not a note.
#[must_use]
pub fn title_card(title: &str, cells: u16, lines: u16) -> Vec<(u16, u16, String)> {
    let room = cells.saturating_sub(2).max(1);
    let most = usize::from(lines);
    if most == 0 || title.trim().is_empty() {
        return Vec::new();
    }
    let mut wrapped = ronoctua_render::layout::wrap(title, room, most + 1);
    if wrapped.len() > most {
        wrapped.truncate(most);
        if let Some(last) = wrapped.last_mut() {
            // ONE CELL FOR THE ELLIPSIS, taken off the end of the last line.
            *last = ronoctua_render::layout::elide_end(&format!("{last} …"), room);
        }
    }
    let n = u16::try_from(wrapped.len()).unwrap_or(lines);
    let top = (lines - n.min(lines)) / 2;
    wrapped
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let wide = ronoctua_render::layout::width(&text).min(cells);
            let line = top + u16::try_from(i).unwrap_or(0);
            (line, (cells - wide) / 2, text)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a screen's footer shows a picture routed to another screen,
    /// stops showing one routed to it, places them anywhere but in its panel,
    /// or the footer keeps a picture for a lane no screen of it shows — or
    /// two for one lane shown on two screens.
    #[test]
    fn only_lanes_routed_here_are_tiles_in_the_footer() {
        let s = crate::player::fixture::routed();
        let room = ronoctua_player::arrange::Rect {
            x: 600.0,
            y: 100.0,
            width: 480.0,
            height: 240.0,
        };
        let on = |c| {
            placed_on_screen(&s, c, room)
                .iter()
                .map(|(l, _)| l.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(on(Some("eDP-1")), vec![1, 3]);
        assert_eq!(on(Some("DP-2")), vec![1, 2]);
        assert_eq!(on(None), vec![1, 2, 3]);
        for (_, r) in placed_on_screen(&s, Some("DP-2"), room) {
            assert!(
                r.x >= room.x - 0.01
                    && r.right() <= room.right() + 0.01
                    && r.y >= room.y - 0.01
                    && r.bottom() <= room.bottom() + 0.01,
                "{r:?} outside the panel"
            );
        }
        let ids =
            |cs: Vec<Option<&str>>| lanes_shown(&s, cs).iter().map(|l| l.0).collect::<Vec<_>>();
        assert_eq!(ids(vec![Some("eDP-1"), Some("DP-2")]), vec![1, 3, 2]);
        assert_eq!(ids(vec![Some("HDMI-A-1")]), vec![1]);
        assert_eq!(ids(vec![]), Vec::<u32>::new());
    }

    /// FAILS IF: the lines the panel keeps beside the rows stop being a
    /// 16:9 picture on its width and the two lines of its foot — or come to
    /// depend on anything but the width.
    #[test]
    fn the_panel_keeps_a_picture_s_lines_on_every_page() {
        let panel = |cells| Panel { from: 0, cells };
        // 32 cells of 10 px: 320 × 9/16 = 180 px, nine lines of 20 px.
        assert_eq!(panel_lines(panel(32), 10.0, 20.0), 9 + 2);
        assert_eq!(panel_lines(panel(48), 10.0, 20.0), 14 + 2);
        assert!(panel_lines(panel(72), 10.0, 20.0) > panel_lines(panel(48), 10.0, 20.0));
    }
    use ronoctua_player::wire::{Track as MediaTrack, TrackKind};

    /// FAILS IF: the panel stops stepping through the column's three widths —
    /// the widest that leaves the rows at least its own — or stops leaving
    /// the region's bar its cell, or appears where not even the narrowest
    /// fits, or Only stops taking the whole region. And it is a function of
    /// the width alone: the same width, the same answer.
    #[test]
    fn the_panel_steps_through_the_columns_widths() {
        let edge = tokens::space::FILES_EDGERIGHT;
        for cells in STEPS {
            // EXACTLY THE WIDTH that leaves the rows as much as it takes…
            let columns = 2 * cells + edge;
            let p = footer_panel(columns, false).expect("room for the picture");
            assert_eq!(p.cells, cells, "at {columns} columns");
            assert_eq!(p.from + p.cells, columns - edge, "the bar's cell is taken");
            assert_eq!(p.from, cells, "the rows keep less than the panel");
            // …AND ONE CELL LESS takes the next step down.
            let narrower = footer_panel(columns - 1, false);
            assert!(
                narrower.is_none_or(|n| n.cells < cells),
                "{narrower:?} at {}",
                columns - 1
            );
        }
        // BELOW THE NARROWEST, no panel — the home says why.
        let least = STEPS[STEPS.len() - 1];
        assert_eq!(footer_panel(2 * least + edge - 1, false), None);
        // A 1024 PX SCREEN at the text's size is the small step, a 1500 PX
        // one the medium, and 1920 the large.
        let at = |px: f32| {
            footer_panel(tokens::metrics::columns(px, tokens::text::size()), false).map(|p| p.cells)
        };
        assert_eq!(at(1024.0), Some(tokens::space::COLUMN_SMALL));
        assert_eq!(at(1500.0), Some(tokens::space::COLUMN_MEDIUM));
        assert_eq!(at(1920.0), Some(tokens::space::COLUMN_LARGE));
        // ONLY THE PICTURE: the whole region, at any width.
        for columns in [10, 94, 200] {
            let only = footer_panel(columns, true).expect("the whole region");
            assert_eq!((only.from, only.cells), (0, columns - edge));
        }
    }

    /// FAILS IF: the stage's transport (Only: the panel from the region's
    /// first column) puts its first glyph on the screen's edge again, a
    /// panel beside the rows loses a cell of its own to an inset it does not
    /// need, or the stage's time bar stops going edge to edge.
    #[test]
    fn the_stage_transport_keeps_the_inset() {
        let only = footer_panel(94, true).expect("the whole region");
        let d = drawn(only, 0, 20, None);
        assert_eq!(d.buttons.first().map(|b| b.1), Some(crate::chrome::INSET));
        assert_eq!(d.track.map(|t| (t.from, t.to)), Some((0, only.cells)));
        let side = footer_panel(94, false).expect("beside the rows");
        let d = drawn(side, 0, 20, None);
        assert_eq!(d.buttons.first().map(|b| b.1), Some(side.from));
    }

    /// FAILS IF: the transport and the time bar stop being the panel's last
    /// two lines, or the picture takes them, or a short region loses the
    /// transport before the picture.
    #[test]
    fn the_foot_of_the_panel_is_the_transport_and_the_bar() {
        let s = shape(4, 15);
        assert_eq!(s.picture, Some((4, 13)));
        assert_eq!((s.transport, s.bar), (Some(17), Some(18)));
        assert_eq!(shape(4, 2).picture, None);
        assert_eq!(shape(4, 2).transport, Some(4));
        assert_eq!(shape(4, 1).bar, None);
        assert_eq!(shape(4, 1).transport, Some(4));
        assert_eq!(shape(4, 0), Shape::default());
    }

    fn playing(tracks: Vec<MediaTrack>) -> State {
        State {
            position: 30.0,
            duration: Some(120.0),
            ..crate::player::fixture::alone("/f/a.mkv", tracks)
        }
    }

    fn audio(id: i64) -> MediaTrack {
        crate::player::fixture::track(id, TrackKind::Audio, None, id == 1)
    }

    /// FAILS IF: a press on the panel's transport is taken somewhere else
    /// than where the frame put the button — every cell of every button
    /// answers its own, the gap between two answers none — or the media's
    /// glyph is placed with nothing to choose, or a button is placed cut
    /// at the panel's edge.
    #[test]
    fn a_panel_button_is_taken_where_it_was_placed() {
        let small = Panel {
            from: 40,
            cells: tokens::space::COLUMN_SMALL,
        };
        let s = playing(vec![audio(1), audio(2)]);
        let placed = buttons(small, 9, Some(&s));
        // ELEVEN IN THE SMALL STEP, with a choice: the room is exact.
        assert_eq!(placed.len(), BUTTONS.len());
        assert!(placed.iter().any(|(_, _, _, id)| *id == "player-tracks"));
        let last = placed.last().expect("a button");
        assert_eq!(last.2, small.from + small.cells, "not flush with the edge");
        for (line, a, b, id) in &placed {
            for column in *a..*b {
                assert_eq!(button_at(&placed, *line, column), Some(*id));
            }
            assert_eq!(button_at(&placed, *line, *b), None, "the gap after {id}");
            assert_eq!(button_at(&placed, line + 1, *a), None, "another line");
        }
        // NO CHOICE, NO GLYPH — and the ones after it close up.
        let plain = buttons(small, 9, Some(&playing(vec![audio(1)])));
        assert_eq!(plain.len(), BUTTONS.len() - 1);
        assert!(!plain.iter().any(|(_, _, _, id)| *id == "player-tracks"));
        // A PANEL NARROWER THAN THE TRANSPORT keeps the ones that are whole.
        let narrow = Panel { from: 0, cells: 10 };
        let few = buttons(narrow, 0, Some(&s));
        assert_eq!(few.len(), 3);
        assert!(few.iter().all(|(_, _, b, _)| *b <= 10));
        // THE CLOCK after the last button, or not at all.
        let medium = Panel {
            from: 0,
            cells: tokens::space::COLUMN_MEDIUM,
        };
        let placed = buttons(medium, 0, Some(&s));
        let at = clock_at(medium, &placed, 11).expect("room for the clock");
        assert!(at > placed.last().map_or(0, |p| p.2));
        assert_eq!(at + 11, medium.cells);
        assert_eq!(clock_at(small, &buttons(small, 0, Some(&s)), 5), None);
    }

    /// FAILS IF: the seeks stop standing around the clock at the transport's
    /// right end — a cell of air between the pieces and after the last glyph
    /// — or come to overlap the buttons, or one is placed without the other
    /// or without the clock; or the clock alone stops being kept where the
    /// three do not fit and it does. At the column's three widths.
    #[test]
    fn the_clock_stands_between_its_seeks() {
        let s = playing(vec![audio(1), audio(2)]);
        let clock = clock_text(&s).expect("something plays");
        assert_eq!(clock, "0:30 / 2:00");
        let cells = ronoctua_protocol::cells(&clock);
        let panel = |cells| Panel { from: 0, cells };
        // 32, the small step: eleven buttons fill it, and neither fits.
        let small = panel(tokens::space::COLUMN_SMALL);
        let placed = buttons(small, 3, Some(&s));
        assert_eq!(clock_group(small, 3, &placed, cells), None);
        let (all, at) = transport(small, 3, Some(&s));
        assert_eq!(all, placed, "no seek in the small step");
        assert_eq!(at, None, "no clock in the small step");
        // 72, the large: the three, flush right, and nothing overlaps.
        let large = panel(tokens::space::COLUMN_LARGE);
        let placed = buttons(large, 3, Some(&s));
        let g = clock_group(large, 3, &placed, cells).expect("room for the three");
        assert_eq!(g.back.3, "player-rewind");
        assert_eq!(g.ahead.3, "player-forward");
        assert_eq!(g.ahead.2, large.cells, "the ahead's air is the last cell");
        assert_eq!(
            g.clock_from + cells + 1,
            g.ahead.1,
            "a cell of air after the clock"
        );
        assert_eq!(g.back.2, g.clock_from, "the back's air is before the clock");
        let last = placed.last().expect("buttons").2;
        assert!(g.back.1 > last, "the back seek overlaps the buttons");
        let (all, at) = transport(large, 3, Some(&s));
        assert_eq!(at, Some((3, g.clock_from)));
        assert_eq!(all.len(), placed.len() + 2);
        // PAINT, CLICK AND HOVER READ THE SAME BOXES: every cell of a seek
        // answers it, the gap before the clock answers nothing.
        for column in g.back.1..g.back.2 {
            assert_eq!(button_at(&all, 3, column), Some("player-rewind"));
        }
        for column in g.ahead.1..g.ahead.2 {
            assert_eq!(button_at(&all, 3, column), Some("player-forward"));
        }
        assert_eq!(
            button_at(&all, 3, g.clock_from),
            None,
            "the clock is no button"
        );
        // 48, the medium: with the media's glyph the three do not fit, and
        // the clock alone does — as it always did; without it, the three.
        let medium = panel(tokens::space::COLUMN_MEDIUM);
        let placed = buttons(medium, 3, Some(&s));
        assert_eq!(clock_group(medium, 3, &placed, cells), None);
        let (all, at) = transport(medium, 3, Some(&s));
        assert_eq!(all, placed);
        assert_eq!(at, clock_at(medium, &placed, cells).map(|c| (3, c)));
        assert!(at.is_some(), "the clock alone fits the medium step");
        let plain = playing(vec![audio(1)]);
        let (all, at) = transport(medium, 3, Some(&plain));
        assert!(all.iter().any(|p| p.3 == "player-forward"));
        assert!(at.is_some());
        // NOTHING LOADED: no clock, no seek.
        let (all, at) = transport(large, 3, Some(&crate::player::fixture::idle()));
        assert!(at.is_none() && !all.iter().any(|p| p.3 == "player-rewind"));
    }

    /// FAILS IF: a picture's thirds stop being where a double click seeks
    /// back, maximizes and seeks ahead — or a double click on a side stops
    /// playing again what its first click paused, or maximizes; or a third
    /// on the same side pauses again instead of seeking again; or a click
    /// after the window of a double click is anything but a first.
    #[test]
    fn a_double_click_on_a_side_of_the_picture_seeks() {
        assert_eq!(picture_zone(300.0, 0.0), Zone::Back);
        assert_eq!(picture_zone(300.0, 99.0), Zone::Back);
        assert_eq!(picture_zone(300.0, 100.0), Zone::Middle);
        assert_eq!(picture_zone(300.0, 199.0), Zone::Middle);
        assert_eq!(picture_zone(300.0, 200.0), Zone::Ahead);
        assert_eq!(picture_zone(300.0, 299.0), Zone::Ahead);
        let quick = std::time::Duration::from_millis(400);
        let step = std::time::Duration::from_millis(100);
        let t0 = std::time::Instant::now();
        let mut c = PictureClicks::default();
        let step10 = crate::player::SEEK_STEP;
        assert_eq!(c.clicked(t0, Zone::Back, quick), PictureClick::Toggle);
        assert_eq!(
            c.clicked(t0 + step, Zone::Back, quick),
            PictureClick::ToggleAndSeek(-step10)
        );
        assert_eq!(
            c.clicked(t0 + step * 2, Zone::Back, quick),
            PictureClick::Seek(-step10)
        );
        assert_eq!(
            c.clicked(t0 + step * 3, Zone::Back, quick),
            PictureClick::Seek(-step10)
        );
        // THE OTHER SIDE after a seek is a first click.
        assert_eq!(
            c.clicked(t0 + step * 4, Zone::Ahead, quick),
            PictureClick::Toggle
        );
        assert_eq!(
            c.clicked(t0 + step * 5, Zone::Ahead, quick),
            PictureClick::ToggleAndSeek(step10)
        );
        // TOO LATE: a first again.
        let late = t0 + step * 5 + quick + step;
        assert_eq!(c.clicked(late, Zone::Ahead, quick), PictureClick::Toggle);
        // THE MIDDLE maximizes, as a double click always did, and the next
        // is a first.
        assert_eq!(
            c.clicked(late + step, Zone::Middle, quick),
            PictureClick::ToggleAndMaximize
        );
        assert_eq!(
            c.clicked(late + step * 2, Zone::Middle, quick),
            PictureClick::Toggle
        );
    }

    /// FAILS IF: the picture folded away stops leaving the panel the
    /// transport and the time bar on the region's last two lines, or keeps
    /// a picture.
    #[test]
    fn a_folded_picture_leaves_the_transport_and_the_bar() {
        let panel = footer_panel(200, false).expect("beside the rows");
        let s = playing(vec![audio(1)]);
        let d = drawn_folded(panel, 4, 15, Some(&s));
        assert_eq!(d.picture, None);
        assert_eq!(d.buttons.first().map(|b| b.0), Some(4 + 15 - 2));
        assert_eq!(d.track.map(|t| t.line), Some(4 + 15 - 1));
        assert!(d.clock.is_some_and(|(line, _)| line == 4 + 15 - 2));
    }

    /// FAILS IF: the time bar is taken anywhere but where it is drawn, or a
    /// press along it seeks to another share than the one under it, or a
    /// drag past its end leaves the end.
    #[test]
    fn the_panel_time_bar_is_taken_where_it_is_drawn() {
        let cell = 10.0;
        let track = Track::of(
            Panel {
                from: 50,
                cells: 32,
            },
            12,
        );
        let (l, r) = track.px(cell);
        assert_eq!((l, r), (500.0, 820.0));
        assert!(track.holds(500.0, 12, cell));
        assert!(!track.holds(499.0, 12, cell));
        assert!(!track.holds(820.0, 12, cell));
        assert!(!track.holds(600.0, 11, cell));
        assert!((track.along(660.0, cell) - 0.5).abs() < 1e-9);
        assert!((track.along(2000.0, cell) - 1.0).abs() < f64::EPSILON);
        assert!(track.along(0.0, cell).abs() < f64::EPSILON);
    }

    /// FAILS IF: the title card stops being in the middle of its place, or a
    /// line runs past it, or a title too long for it spills lines it has not
    /// got.
    #[test]
    fn a_title_card_is_set_in_the_middle_and_stays_inside() {
        let card = title_card("Marta Quintela — The Lighthouse Keeper", 32, 9);
        assert!(!card.is_empty());
        for (line, column, text) in &card {
            assert!(*line < 9);
            let wide = ronoctua_render::layout::width(text);
            assert!(column + wide <= 32, "{text:?} runs out");
            // IN THE MIDDLE, to the cell.
            assert!((32 - wide) / 2 == *column, "{text:?} at {column}");
        }
        let n = u16::try_from(card.len()).unwrap();
        assert_eq!(card[0].0, (9 - n) / 2, "not in the middle of the height");
        let long = "word ".repeat(80);
        let cut = title_card(&long, 20, 3);
        assert_eq!(cut.len(), 3);
        assert!(cut[2].2.ends_with('…'), "{:?}", cut[2].2);
        assert!(title_card("", 20, 3).is_empty());
        assert!(title_card("a", 20, 0).is_empty());
    }
}
