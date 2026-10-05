//! THE DECK — a set's lanes on one time axis, laid out once.
//!
//! # What this is
//!
//! A pure layout, with no Wayland and no door in it: the lanes of what plays
//! (`ronoctua_player::own::Lane`), how long each source is, the set's length
//! and the clock go in, and a [`Deck`] comes out — a time ruler and one line
//! per lane, every line on ONE track, so the playhead is one x on every line.
//! The painter draws it ([`Deck::quads`]) and the pointer reads it
//! ([`Deck::hit`]), and both read the same numbers: a control is taken where
//! it is drawn.
//!
//! # One lane is today's bar
//!
//! A single lane is laid out as the time bar every door already has: the
//! track from the rows' first cell, what has played in the accent, and the
//! player's mute and volume at its right end ([`one_lane_is_the_old_bar`]
//! keeps the numbers). Two lanes and more get the ruler and a line each: the
//! kind's glyph (which switches the lane on and off), its name, its band from
//! its start to its end — a live one to the track's end —, its mute and its
//! meter — only a lane that SOUNDS has those two: a picture plays no sound
//! of its own (the daemon tells its voice `aid=no`) and subtitles have none;
//! the small column keeps the glyph, the band and the mute.
//!
//! # Where it is drawn
//!
//! - **The window off its stage and the column at large, medium and small**:
//!   a block in place of the time bar, under the transport — a SET's lanes
//!   (a file alone is one lane: the old bar), `space.player.deckLanes` of them
//!   at most and never more than a quarter of the surface's lines, the rest
//!   scrolling under ONE bar ([`Deck::bar`]).
//! - **On a stage** — the picture alone, the whole screen, the corner — the
//!   one global bar, and nothing else ([`of_state`] with no lanes).
//! - **As rows** — the footer's "Now", while a set plays, and the Timeline
//!   page in every door: the ruler and every lane, uncapped, each a row
//!   carrying its line of the deck (`player_rows::DeckRow`), so the keys walk
//!   them.
//!
//! # One gesture, every door
//!
//! What a press does is decided HERE ([`Deck::press`]): it answers what to
//! ask now and what the pointer holds until it is let go ([`Grip`]) — the
//! geometry it was pressed on, so a drag keeps the track it started on. A
//! band held and moved leaves a ghost ([`Deck::holding`]) and asks nothing
//! until it is let go; let go without moving, it opens the lane's page. The
//! doors only turn a pointer into a line and a column of the deck.
//!
//! [`one_lane_is_the_old_bar`]: tests

use ronoctua_player::own::{Lane, LaneId, LaneKind};
use ronoctua_player::wire::{Ask, LaneChange, State};
use ronoctua_render::quad::Quad;
use ronoctua_tokens as tokens;

use crate::chrome::Frame;

/// The cells a glyph takes on a deck's line (`space.player.deckGlyph`): the
/// window's mute on its time bar has always been these two.
pub const GLYPH_CELLS: u16 = tokens::space::PLAYER_DECKGLYPH;

/// The cell of air between two things on a deck's line
/// (`space.player.deckAir`) — after a glyph, after a name, before the mute.
pub const AIR_CELLS: u16 = tokens::space::PLAYER_DECKAIR;

/// How far `←` and `→` move a lane's start, in seconds.
pub const OFFSET_STEP_SECONDS: f64 = 0.5;

/// How far past the latest thing it knows a deck with no known length looks
/// (`space.player.liveRoomPct`, a quarter more): a live lane has somewhere to
/// run and the playhead is never at the edge.
fn live_room() -> f64 {
    1.0 + f64::from(tokens::space::PLAYER_LIVEROOMPCT) / 100.0
}

/// The latest second a deck's clock is laid out to, and the latest start a
/// person may type: ten thousand hours, past any film, set or live stream a
/// person watches in one sitting.
///
/// A CEILING, and the reason is arithmetic: a start of 309 digits is a
/// finite `f64`, a quarter more than it is infinite, and a ruler whose span
/// is infinite has a step that is infinite and ticks that never pass it — a
/// loop that fills the memory of the door that drew it.
pub const SECONDS_MOST: f64 = 10_000.0 * 3600.0;

/// The least a deck with no known length shows, in seconds: a minute, so the
/// first seconds of a live stream do not stretch across the whole track.
const LIVE_FLOOR_SECONDS: f64 = 60.0;

/// The steps a ruler's ticks may be apart, in seconds: the ones a clock is
/// read in — 1, 2, 5, 10, 15 and 30 seconds and minutes, then hours.
const STEPS_SECONDS: [f64; 18] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0, 7200.0,
    10800.0, 21600.0, 43200.0, 86400.0,
];

/// How much room a deck has, and what it shows in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    /// The width, in cells, from the rows' first cell to the last.
    pub columns: u16,
    /// How many lane lines at most before the lanes scroll; `None` shows
    /// every lane (the Timeline page).
    pub most_lanes: Option<u16>,
    /// The cells a lane's name gets beside its glyph; none in the small
    /// column.
    pub names: u16,
    /// Whether each lane has its own volume meter, and the ruler the
    /// player's — not in the small column.
    pub meters: bool,
}

impl Room {
    /// A window's or a large or medium column's: names and meters, and
    /// `space.player.deckLanes` lanes before it scrolls.
    #[must_use]
    pub const fn wide(columns: u16) -> Self {
        Self {
            columns,
            most_lanes: Some(tokens::space::PLAYER_DECKLANES),
            names: tokens::space::PLAYER_DECKNAMES,
            meters: true,
        }
    }

    /// The small column's: the glyph, the band and the mute.
    #[must_use]
    pub const fn narrow(columns: u16) -> Self {
        Self {
            columns,
            most_lanes: Some(tokens::space::PLAYER_DECKLANES),
            names: 0,
            meters: false,
        }
    }

    /// The same room, with every lane shown — the Timeline page's, and the
    /// footer's "Now": a ruler and a line for every lane, one lane included.
    #[must_use]
    pub const fn every_lane(self) -> Self {
        Self {
            most_lanes: None,
            ..self
        }
    }

    /// The same room, with at most `most` lanes before they scroll — never
    /// fewer than one.
    #[must_use]
    pub fn at_most(self, most: u16) -> Self {
        Self {
            most_lanes: Some(most.max(1)),
            ..self
        }
    }
}

/// A band being dragged: nothing is asked until it is let go, and a ghost
/// shows where it would land.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    /// The lane.
    pub lane: LaneId,
    /// Where on the band it was pressed, in seconds from the band's start
    /// ([`Hit::Band`]).
    pub grab: f64,
    /// The seconds under the pointer now ([`Deck::seconds_at`]).
    pub at: f64,
    /// The seconds the track showed when the band was pressed — what `at`
    /// was read against, and what its ghost is drawn against: a deck of no
    /// known length widens as it plays, and a ghost drawn on the new width
    /// slid away from the hand holding it.
    pub span: f64,
}

impl Held {
    /// The start the lane would have if let go now, never before the set's
    /// first second: a start below zero is clamped, not refused.
    #[must_use]
    pub fn start(&self) -> f64 {
        (self.at - self.grab).max(0.0)
    }
}

/// A tick of the ruler: where it is and the time written from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    /// Where the time is, in cells from the rows' first cell.
    pub at: f32,
    /// The cell its label is written from.
    pub label: u16,
    /// The label (`m:ss`, `h:mm:ss`).
    pub text: String,
}

/// The ruler — or, in a deck of one lane, the time bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Ruler {
    /// The track every line shares, in cells: `(first, end)`.
    pub track: (u16, u16),
    /// The ticks — none on the time bar of one lane.
    pub ticks: Vec<Tick>,
    /// The player's mute glyph's first cell.
    pub mute: u16,
    /// The player's volume track, `(first, end)`, when the room has meters.
    pub volume: Option<(u16, u16)>,
    /// A deck of one lane: the share of it played, drawn in the accent as
    /// the time bar always was — `None` while its length is not known.
    pub played: Option<f32>,
}

/// One lane's line.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneLine {
    /// The lane.
    pub lane: LaneId,
    /// Its kind — the glyph, and the band's colour.
    pub kind: LaneKind,
    /// Whether it plays.
    pub on: bool,
    /// Whether it is muted.
    pub muted: bool,
    /// Its own volume.
    pub volume_percent: u8,
    /// Its start, on the set's clock — what a drag of its band moves.
    pub start_seconds: f64,
    /// Where it ends on the set's clock, when that is known.
    pub end_seconds: Option<f64>,
    /// Whether it SOUNDS — a sound lane: only it has a mute and a meter of
    /// its own. A picture's voice plays no sound (`aid=no`), and subtitles
    /// have none.
    pub sounds: bool,
    /// The track, as the ruler's.
    pub track: (u16, u16),
    /// The glyph's first cell — a click switches the lane on or off.
    pub toggle: u16,
    /// Its name's cells, when the room has names.
    pub name: Option<(u16, u16)>,
    /// Its band, from its start to its end, in cells.
    pub band: (f32, f32),
    /// Whether its end is not known — it runs to the track's end.
    pub live: bool,
    /// Where it would land if let go, while it is dragged.
    pub ghost: Option<(f32, f32)>,
    /// Its mute glyph's first cell — drawn, and answering a press, only on
    /// a lane that [`sounds`](Self::sounds).
    pub mute: u16,
    /// Its meter, `(first, end)`, when the room has meters — and, as the
    /// mute, only on a lane that sounds.
    pub meter: Option<(u16, u16)>,
}

/// One line of a deck.
#[derive(Debug, Clone, PartialEq)]
pub enum DeckLine {
    /// The first: the time ruler, or the time bar of one lane.
    Ruler(Ruler),
    /// A lane.
    Lane(LaneLine),
}

/// What a press on a deck takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// A lane's glyph: on or off.
    Toggle(LaneId),
    /// A lane's name: its page.
    Name(LaneId),
    /// A lane's band, `grab` seconds after its start: a drag moves the start,
    /// a click without moving opens its page.
    Band {
        /// The lane.
        lane: LaneId,
        /// How far into the band, in seconds.
        grab: f64,
    },
    /// The track, where no band is: a seek, and holding drags it.
    Track {
        /// The seconds there, on the set's clock.
        seconds: f64,
    },
    /// Within half a cell of the playhead: dragging it seeks.
    Playhead,
    /// A lane's mute.
    LaneMute(LaneId),
    /// A lane's meter.
    LaneVolume {
        /// The lane.
        lane: LaneId,
        /// How far along the meter, from nought to one.
        fraction: f64,
    },
    /// The player's mute.
    Mute,
    /// The player's volume.
    Volume {
        /// How far along the track, from nought to one.
        fraction: f64,
    },
    /// The bar of lanes that scroll.
    Bar,
}

/// The player's own volume, which the ruler's track is filled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sound {
    /// The volume, in percent.
    pub volume_percent: u8,
    /// Whether it is muted.
    pub muted: bool,
}

/// A deck, laid out: ONE answer the painter and the pointer read.
#[derive(Debug, Clone, PartialEq)]
pub struct Deck {
    /// The ruler first, then the lanes shown.
    pub lines: Vec<DeckLine>,
    /// Where now is, in cells — `None` in a deck of one lane, whose time bar
    /// already says it.
    pub playhead: Option<f32>,
    /// How many seconds the track shows.
    pub span: f64,
    /// The lanes, as a list that may scroll: `total` lanes, `lines` shown
    /// from `offset`.
    pub scroll: ronoctua_design::Scroll,
    /// The room's width, in cells — the bar's cell is its last, while the
    /// lanes scroll.
    pub columns: u16,
}

/// The step between two ticks of a ruler `cells` wide that shows `span`
/// seconds: the smallest a clock is read in whose labels, written from their
/// tick, leave a cell of air before the next one. Every label is at most as
/// wide as the span's own, since `m:ss` only grows.
#[must_use]
pub fn nice_step(span: f64, cells: u16) -> f64 {
    if cells == 0 || span <= 0.0 {
        return span.max(1.0);
    }
    let widest = f64::from(ronoctua_protocol::cells(&crate::player::clock(span)));
    let apart = |step: f64| step / span * f64::from(cells) >= widest + 1.0;
    if let Some(step) = STEPS_SECONDS.iter().copied().find(|s| apart(*s)) {
        return step;
    }
    // PAST A DAY APART: whole days, doubled until they fit.
    let mut step = STEPS_SECONDS[STEPS_SECONDS.len() - 1];
    while !apart(step) && step < span {
        step *= 2.0;
    }
    step
}

/// Why a typed start is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing was typed — the question is let go.
    Empty,
    /// Not a time: `m:ss`, `h:mm:ss`, seconds, or `+`/`-` seconds.
    NotATime,
}

/// A start typed into the lane's question, on the set's clock: `m:ss`,
/// `h:mm:ss` (the last field may carry a fraction), plain seconds, or a move
/// from `now` — `+1.5`, `-0.3`, `+0:05`. The decimal mark is either, a point
/// or a comma, since the question is typed in the person's language. A start
/// before the set's first second is clamped to it, never refused.
///
/// # Errors
///
/// [`Refusal::Empty`] for nothing typed and [`Refusal::NotATime`] for anything
/// else that is not one of the four shapes, or that lands past
/// [`SECONDS_MOST`].
pub fn parse_start(text: &str, now: f64) -> Result<f64, Refusal> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Refusal::Empty);
    }
    // THE MINUS SIGN AS WELL AS THE HYPHEN: a sentence prints one, and a
    // person copies what the sentence printed.
    let (sign, rest) = if let Some(rest) = text.strip_prefix('+') {
        (Some(1.0), rest)
    } else if let Some(rest) = text.strip_prefix(['-', '\u{2212}']) {
        (Some(-1.0), rest)
    } else {
        (None, text)
    };
    let amount = clock_seconds(rest.trim()).ok_or(Refusal::NotATime)?;
    let start = match sign {
        Some(s) => now + s * amount,
        None => amount,
    };
    if !start.is_finite() || start > SECONDS_MOST {
        return Err(Refusal::NotATime);
    }
    Ok(start.max(0.0))
}

/// `h:mm:ss`, `m:ss` or seconds, as seconds. The fields after the first are
/// one or two digits under sixty; only the last may carry a fraction.
fn clock_seconds(text: &str) -> Option<f64> {
    let fields: Vec<&str> = text.split(':').collect();
    if fields.is_empty() || fields.len() > 3 {
        return None;
    }
    let last = fields.len() - 1;
    let mut seconds = 0.0_f64;
    for (i, field) in fields.iter().enumerate() {
        let (whole, fraction) = match field.split_once(['.', ',']) {
            Some((w, f)) if i == last => (w, Some(f)),
            Some(_) => return None,
            None => (*field, None),
        };
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(whole) || fraction.is_some_and(|f| !digits(f)) {
            return None;
        }
        if i > 0 && whole.len() > 2 {
            return None;
        }
        let mut value: f64 = whole.parse().ok()?;
        if i > 0 && value >= 60.0 {
            return None;
        }
        if let Some(f) = fraction {
            value += format!("0.{f}").parse::<f64>().ok()?;
        }
        seconds = seconds * 60.0 + value;
    }
    seconds.is_finite().then_some(seconds)
}

/// The colour of a lane's band: its kind's, or the quiet grey of a lane
/// that is off.
#[must_use]
pub const fn band_color(kind: LaneKind, on: bool) -> tokens::Color {
    if !on {
        return tokens::color::PLAYER_LANE_OFF;
    }
    match kind {
        LaneKind::Video => tokens::color::PLAYER_LANE_VIDEO,
        LaneKind::Audio => tokens::color::PLAYER_LANE_AUDIO,
        LaneKind::Subtitle => tokens::color::PLAYER_LANE_SUBTITLE,
    }
}

/// Where on a track `(first, end)` the time `seconds` of `span` is, in cells.
fn column_of(track: (u16, u16), span: f64, seconds: f64) -> f32 {
    let share = if span > 0.0 {
        (seconds / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    #[allow(clippy::cast_possible_truncation)]
    let along = (share * f64::from(track.1.saturating_sub(track.0))) as f32;
    f32::from(track.0) + along
}

/// Where a lane `start_seconds`..`end` long would be drawn if it began where
/// `held` says — the band moved by what the pointer moved, a live one still
/// to the track's end. [`lay`]'s arithmetic and [`Deck::holding`]'s, one:
/// on the seconds the track showed when the band was pressed
/// ([`Held::span`]), which is where the pointer's column was read, so the
/// ghost stays under the hand however the deck has widened since.
fn ghost_of(track: (u16, u16), start_seconds: f64, end: Option<f64>, held: Held) -> (f32, f32) {
    let scale = held.span;
    let start = match held.start() {
        s if s.is_nan() => 0.0,
        s => s.clamp(0.0, SECONDS_MOST),
    };
    let to = end.map_or(f32::from(track.1), |e| {
        column_of(track, scale, start + (e - start_seconds))
    });
    (column_of(track, scale, start), to)
}

/// Lays a deck out.
///
/// `length_of` says how long each lane's source is (the daemon's word for
/// it; `None` while unknown, or for a stream), `span` the set's length on
/// its clock (`None` when something in it has no end), `position` where the
/// clock is, `offset` the first lane shown when they scroll, and `held` the
/// band being dragged, when one is.
#[must_use]
pub fn lay(
    lanes: &[Lane],
    length_of: impl Fn(LaneId) -> Option<f64>,
    span: Option<f64>,
    position: f64,
    room: Room,
    offset: usize,
    held: Option<Held>,
) -> Deck {
    // EVERY TIME WITHIN THE CEILING, before anything is scaled by it: a start
    // or a length the daemon sent that is not finite, or past
    // [`SECONDS_MOST`], would give the ruler an infinite span.
    let within = |s: f64| {
        if s.is_nan() {
            0.0
        } else {
            s.clamp(0.0, SECONDS_MOST)
        }
    };
    let ends: Vec<Option<f64>> = lanes
        .iter()
        .map(|lane| ronoctua_player::session::end_seconds(lane, length_of(lane.id)).map(within))
        .collect();
    let position = within(position);
    let span = span.map(within);
    // HOW MUCH THE TRACK SHOWS: the set's length, stretched to every band
    // whole — a lane that is off, or a subtitle, can run past what the set
    // counts. With no length, past the latest thing known, for a live lane
    // to run into.
    let latest = lanes
        .iter()
        .zip(&ends)
        .map(|(lane, end)| end.unwrap_or_else(|| within(lane.start_seconds)))
        .fold(0.0_f64, f64::max);
    let scale = match span.filter(|s| *s > 0.0) {
        Some(s) => s.max(latest),
        None => (latest.max(position) * live_room()).max(LIVE_FLOOR_SECONDS),
    };
    let known = span.is_some_and(|s| s > 0.0);

    // ONE LANE IS THE OLD BAR — in a room that caps its lanes: the block of a
    // door. Where every lane is shown (the Timeline page, the footer's "Now")
    // one lane is still a lane, with its line, its glyph and its start.
    if lanes.len() <= 1 && room.most_lanes.is_some() {
        return one_lane(room, scale, known.then_some(position), lanes.first(), &ends);
    }

    let total = lanes.len();
    let most = room.most_lanes.map_or(total, usize::from);
    let shown = total.min(most.max(1));
    let offset = offset.min(total.saturating_sub(shown));
    let scroll = ronoctua_design::Scroll {
        total,
        lines: shown,
        offset,
        top: 0,
    };
    let columns = room.columns.saturating_sub(if scroll.cut() {
        crate::chrome::BAR_CELLS
    } else {
        0
    });

    // THE PREFERENCE, and then nothing leaks: the track is never shorter than
    // a name, so a narrow door gives up the meters first and then the names.
    let floor = tokens::space::PLAYER_DECKNAMES;
    let mut names = room.names;
    let mut meters = room.meters;
    let track_of = |names: u16, meters: bool| {
        let first = GLYPH_CELLS + AIR_CELLS + if names > 0 { names + AIR_CELLS } else { 0 };
        let mute = right_block(columns, meters).0;
        (first, mute.saturating_sub(AIR_CELLS).max(first))
    };
    if track_of(names, meters).1 - track_of(names, meters).0 < floor && meters {
        meters = false;
    }
    if track_of(names, meters).1 - track_of(names, meters).0 < floor {
        names = 0;
    }
    let track = track_of(names, meters);
    let (mute, meter) = right_block(columns, meters);

    let mut lines = Vec::with_capacity(shown + 1);
    lines.push(DeckLine::Ruler(Ruler {
        track,
        ticks: ticks(track, scale),
        mute,
        volume: meter,
        played: None,
    }));
    for (lane, end) in lanes.iter().zip(&ends).skip(offset).take(shown) {
        let start_seconds = within(lane.start_seconds);
        let from = column_of(track, scale, start_seconds);
        let to = end.map_or(f32::from(track.1), |e| column_of(track, scale, e));
        let ghost = held
            .filter(|h| h.lane == lane.id)
            .map(|h| ghost_of(track, start_seconds, *end, h));
        lines.push(DeckLine::Lane(LaneLine {
            lane: lane.id,
            kind: lane.kind,
            on: lane.enabled,
            muted: lane.muted,
            volume_percent: lane.volume_percent,
            start_seconds,
            end_seconds: *end,
            sounds: lane.kind == LaneKind::Audio,
            track,
            toggle: 0,
            name: (names > 0).then_some((GLYPH_CELLS + AIR_CELLS, GLYPH_CELLS + AIR_CELLS + names)),
            band: (from, to.max(from)),
            live: end.is_none(),
            ghost,
            mute,
            meter,
        }));
    }
    Deck {
        lines,
        playhead: Some(column_of(track, scale, position)),
        span: scale,
        scroll,
        columns: room.columns,
    }
}

/// The right end of every line: the mute glyph's first cell and, with
/// meters, the volume's track after a cell of air — the time bar's own
/// arithmetic (`player_window::Player::volume_cells`).
fn right_block(columns: u16, meters: bool) -> (u16, Option<(u16, u16)>) {
    if meters {
        let first = columns.saturating_sub(tokens::space::PLAYER_VOLUMEBAR);
        (
            first.saturating_sub(GLYPH_CELLS + AIR_CELLS),
            Some((first, columns)),
        )
    } else {
        (columns.saturating_sub(GLYPH_CELLS), None)
    }
}

/// A deck of one lane — or of none: the time bar, with the player's mute and
/// volume at its right end, as every door has always drawn it.
fn one_lane(
    room: Room,
    scale: f64,
    position: Option<f64>,
    lane: Option<&Lane>,
    ends: &[Option<f64>],
) -> Deck {
    let (mute, volume) = right_block(room.columns, true);
    let track = (0, mute.saturating_sub(AIR_CELLS));
    // PLAYED only over a known length, as the bar had: a stream has none.
    let known = lane.is_none_or(|_| ends.first().is_some_and(Option::is_some));
    #[allow(clippy::cast_possible_truncation)]
    let played = position
        .filter(|_| known && scale > 0.0)
        .map(|p| (p / scale).clamp(0.0, 1.0) as f32);
    Deck {
        lines: vec![DeckLine::Ruler(Ruler {
            track,
            ticks: Vec::new(),
            mute,
            volume,
            played,
        })],
        playhead: None,
        span: scale,
        scroll: ronoctua_design::Scroll {
            total: 0,
            lines: 0,
            offset: 0,
            top: 0,
        },
        columns: room.columns,
    }
}

/// The ruler's ticks over `track`, `nice_step` apart, each written from its
/// own cell — and only the ones whose label ends before the track does, but
/// THE LAST: a label that would run past the track is moved back to end where
/// the track ends, when that leaves a cell of air after the label before it —
/// on a short track, where only `0:00` fits from its own cell, the ruler gets
/// its second mark (its line stays where the time is).
///
/// NEVER MORE THAN ONE A CELL, whatever the span says: a span that is not
/// finite has no ticks at all, and the walk stops at the track's width even
/// if the step came out too small — a ruler is drawn in cells, and a loop
/// that trusted the arithmetic alone once had no end.
fn ticks(track: (u16, u16), span: f64) -> Vec<Tick> {
    let cells = track.1.saturating_sub(track.0);
    if !span.is_finite() || span <= 0.0 {
        return Vec::new();
    }
    let step = nice_step(span, cells);
    if !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let mut out: Vec<Tick> = Vec::new();
    for k in 0..=u32::from(cells) {
        let t = f64::from(k) * step;
        if t > span {
            break;
        }
        let at = column_of(track, span, t);
        let text = crate::player::clock(t);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let label = at.floor() as u16;
        let wide = ronoctua_protocol::cells(&text);
        if label.saturating_add(wide) <= track.1 {
            out.push(Tick { at, label, text });
            continue;
        }
        // THE LAST, moved back to end at the track's end — only past the
        // label before it with a cell of air, and never before the track.
        let moved = track.1.saturating_sub(wide);
        let clear = out
            .last()
            .is_none_or(|p| moved > p.label + ronoctua_protocol::cells(&p.text));
        if clear && moved >= track.0 {
            out.push(Tick {
                at,
                label: moved,
                text,
            });
        }
        break;
    }
    out
}

/// Whether `column` is on the glyph whose first cell is `first`.
fn on_glyph(column: f64, first: u16) -> bool {
    column >= f64::from(first) && column < f64::from(first + GLYPH_CELLS)
}

/// How far along `(first, end)` `column` is, from nought to one — or `None`
/// off it.
fn along(column: f64, (first, end): (u16, u16)) -> Option<f64> {
    (column >= f64::from(first) && column < f64::from(end)).then(|| {
        ((column - f64::from(first)) / f64::from(end.saturating_sub(first).max(1))).clamp(0.0, 1.0)
    })
}

impl Deck {
    /// The ruler, which every deck has.
    fn ruler(&self) -> Option<&Ruler> {
        match self.lines.first() {
            Some(DeckLine::Ruler(r)) => Some(r),
            _ => None,
        }
    }

    /// The track every line shares, in cells.
    #[must_use]
    pub fn track(&self) -> (u16, u16) {
        self.ruler().map_or((0, 0), |r| r.track)
    }

    /// The seconds at `column` of the track, on the set's clock — clamped to
    /// its ends, so a drag that leaves the track keeps the end it left by.
    #[must_use]
    pub fn seconds_at(&self, column: f64) -> f64 {
        seconds_on(self.track(), self.span, column)
    }

    /// Whether a press on the track says a time to go to: a ruler of the
    /// set's seconds, or the bar of one lane whose length is known — a
    /// stream alone has none, and a press there seeks nowhere.
    #[must_use]
    pub fn seekable(&self) -> bool {
        self.playhead.is_some() || self.ruler().is_some_and(|r| r.played.is_some())
    }

    /// The lane on `line`, when that line is one.
    #[must_use]
    pub fn lane_line(&self, line: usize) -> Option<&LaneLine> {
        match self.lines.get(line) {
            Some(DeckLine::Lane(l)) => Some(l),
            _ => None,
        }
    }

    /// The same deck with `held`'s band's GHOST where it would land — what a
    /// door paints while a band is dragged, from the deck it laid out once.
    /// A lane not on it, or not shown, changes nothing.
    #[must_use]
    pub fn holding(&self, held: Option<Held>) -> Self {
        let mut out = self.clone();
        let Some(held) = held else {
            return out;
        };
        for line in &mut out.lines {
            if let DeckLine::Lane(l) = line {
                l.ghost = (l.lane == held.lane)
                    .then(|| ghost_of(l.track, l.start_seconds, l.end_seconds, held));
            }
        }
        out
    }

    /// Where a press at height `y` on the lanes' bar puts them — the bar
    /// placed as [`Deck::bar`] places it, grabbed by its whole track.
    #[must_use]
    pub fn offset_at(
        &self,
        f: Frame,
        first_line: u16,
        left: u16,
        shown: std::ops::Range<usize>,
        y: f32,
    ) -> usize {
        match self.bar(f, first_line, left, shown) {
            Some(((_, top, _, tall), _)) => self.scroll.offset_at(y - top, tall, f.line()),
            None => self.scroll.offset,
        }
    }

    /// A PRESS at `column` of line `line`: what to ask now, and what the
    /// pointer holds until it is let go — the ONE answer every door reads,
    /// so a control does in the window what it does in the footer.
    ///
    /// - a lane's glyph switches it on or off, its mute mutes it, and its
    ///   name opens its page;
    /// - a band is HELD and nothing is asked: moved, it leaves a ghost and
    ///   asks its new start when let go; let go where it was pressed, it
    ///   opens the lane's page ([`Grip::released`]);
    /// - the track seeks there, and holding drags; the playhead is grabbed
    ///   where it is, and seeks as it is dragged;
    /// - a meter — a lane's, or the player's on the ruler — is set there,
    ///   and holding drags it;
    /// - the lanes' bar is held, and the door scrolls it.
    #[must_use]
    pub fn press(&self, line: usize, column: f64) -> (Option<Grip>, Act) {
        let Some(hit) = self.hit(line, column) else {
            return (None, Act::Nothing);
        };
        let track = self.track();
        let lane_act = |lane: LaneId, change: LaneChange| Act::Tell(Ask::Lane { lane, change });
        match hit {
            Hit::Toggle(id) => (
                None,
                self.lane_line(line).map_or(Act::Nothing, |l| {
                    lane_act(id, LaneChange::Enabled { on: !l.on })
                }),
            ),
            Hit::LaneMute(id) => (
                None,
                self.lane_line(line).map_or(Act::Nothing, |l| {
                    lane_act(id, LaneChange::Mute { muted: !l.muted })
                }),
            ),
            Hit::Name(id) => (None, Act::Choose(id)),
            Hit::Band { lane, grab } => (
                Some(Grip::Band {
                    lane,
                    picture: self
                        .lane_line(line)
                        .is_some_and(|l| l.kind == LaneKind::Video),
                    grab,
                    from: column,
                    moved: false,
                    track,
                    span: self.span,
                }),
                Act::Nothing,
            ),
            Hit::Track { seconds } if self.seekable() => (
                Some(Grip::Seek {
                    track,
                    span: self.span,
                }),
                Act::Tell(Ask::Seek {
                    seconds,
                    relative: false,
                }),
            ),
            Hit::Playhead if self.seekable() => (
                Some(Grip::Seek {
                    track,
                    span: self.span,
                }),
                Act::Nothing,
            ),
            Hit::Track { .. } | Hit::Playhead => (None, Act::Nothing),
            Hit::LaneVolume { lane, fraction } => {
                match self.lane_line(line).and_then(|l| l.meter) {
                    Some(meter) => (
                        Some(Grip::LaneVolume { lane, meter }),
                        lane_act(
                            lane,
                            LaneChange::Volume {
                                percent: percent_of(
                                    fraction,
                                    ronoctua_player::own::VOLUME_MOST_PERCENT,
                                ),
                            },
                        ),
                    ),
                    None => (None, Act::Nothing),
                }
            }
            Hit::Mute => (None, Act::Tell(Ask::Mute)),
            Hit::Volume { fraction } => match self.ruler().and_then(|r| r.volume) {
                Some(meter) => (
                    Some(Grip::Volume { meter }),
                    Act::Tell(Ask::Volume {
                        percent: percent_of(fraction, crate::player::VOLUME_MOST),
                    }),
                ),
                None => (None, Act::Nothing),
            },
            Hit::Bar => (Some(Grip::Bar), Act::Nothing),
        }
    }

    /// The WHEEL over line `line`, `by` lines (down is positive, and
    /// quieter): a lane that sounds turns its own volume, a step a line, and
    /// the ruler — which carries the player's meter and mute — the player's,
    /// `volume_percent`, as the wheel over the transport always did. A lane
    /// that plays NO SOUND has no volume to turn: `None`, and the wheel does
    /// what it does over any list, it scrolls the lanes.
    #[must_use]
    pub fn wheel(&self, line: usize, by: i64, volume_percent: u8) -> Option<Ask> {
        let stepped = |now: u8, most: u8| {
            let step = i64::from(crate::player::VOLUME_STEP);
            u8::try_from((i64::from(now) - by * step).clamp(0, i64::from(most))).unwrap_or(most)
        };
        match self.lines.get(line)? {
            DeckLine::Lane(l) if l.sounds => Some(Ask::Lane {
                lane: l.lane,
                change: LaneChange::Volume {
                    percent: stepped(l.volume_percent, ronoctua_player::own::VOLUME_MOST_PERCENT),
                },
            }),
            DeckLine::Lane(_) => None,
            DeckLine::Ruler(_) => Some(Ask::Volume {
                percent: stepped(volume_percent, crate::player::VOLUME_MOST),
            }),
        }
    }

    /// What is written on line `line`, in the cells of the deck's own origin
    /// — the ticks' times on the ruler and the player's mute; a lane's glyph,
    /// its name and its mute — each in its tone: `lit` is the glyph under the
    /// pointer, which lights as a header's key does under it, and `focused`
    /// says the cursor is on the line's row. The shapes are
    /// [`Deck::quads`]'.
    ///
    /// THE POINTER'S LIGHT IS NOT THE CURSOR (the rule every list keeps): the
    /// cursor wears the accent — on a lane with no name, its glyph — so the
    /// glyph under a resting pointer takes the tone a key the pointer rests
    /// on takes (`text.subtle.lit.key`), and a lane under the hand never
    /// reads as the lane the keys are on.
    #[must_use]
    pub fn words(
        &self,
        line: usize,
        state: &State,
        texts: &ronoctua_i18n::Texts,
        lit: Option<Part>,
        focused: bool,
    ) -> Vec<Word> {
        let glyph = |part: Part, dim: bool| {
            if lit == Some(part) {
                tokens::color::TEXT_SUBTLE_LIT_KEY
            } else if dim {
                tokens::color::TEXT_DIM
            } else {
                tokens::color::TEXT_PRIMARY
            }
        };
        let mut out = Vec::new();
        match self.lines.get(line) {
            None => {}
            Some(DeckLine::Ruler(r)) => {
                // THE CURSOR IS A COLOUR, and a ruler has no label to wear
                // it: its times do.
                let times = if focused {
                    crate::chrome::Kind::Setting.tones(true).0
                } else {
                    tokens::color::TEXT_SECONDARY
                };
                for t in &r.ticks {
                    out.push(Word {
                        at: t.label,
                        text: t.text.clone(),
                        color: times,
                    });
                }
                out.push(Word {
                    at: r.mute,
                    text: crate::player_transport::glyph_of(
                        "player-mute",
                        Some(state),
                        crate::player_window::Stage::Off,
                        crate::player_window::Door::Window,
                    )
                    .to_string(),
                    color: glyph(Part::Mute, false),
                });
            }
            Some(DeckLine::Lane(l)) => {
                // WITH NO NAME — the small column's — the cursor is worn by
                // the glyph, the one word the line has.
                out.push(Word {
                    at: l.toggle,
                    text: crate::player_queue::lane_glyph(l.kind).to_string(),
                    color: if focused && l.name.is_none() {
                        crate::chrome::Kind::Setting.tones(true).0
                    } else {
                        glyph(Part::Toggle, !l.on)
                    },
                });
                if let (Some((a, b)), Some(lane)) =
                    (l.name, state.lanes.iter().find(|x| x.id == l.lane))
                {
                    let room = b.saturating_sub(a);
                    let name = if lane.kind == LaneKind::Subtitle {
                        crate::player_subtitles::name(lane, state, texts, room)
                    } else {
                        crate::player_queue::lane_name(lane, &state.lanes, texts, room)
                    };
                    out.push(Word {
                        at: a,
                        text: ronoctua_render::layout::elide_end(&name, room),
                        color: crate::chrome::Kind::Setting.tones(focused).0,
                    });
                }
                if l.sounds {
                    out.push(Word {
                        at: l.mute,
                        text: if l.muted {
                            tokens::icon::VOLUME_OFF
                        } else {
                            tokens::icon::VOLUME
                        }
                        .to_string(),
                        color: glyph(Part::Mute, l.muted),
                    });
                }
            }
        }
        out
    }
}

/// A piece of text on a line of the deck: where, what, and in which tone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// Its first cell, from the deck's origin.
    pub at: u16,
    /// What it says.
    pub text: String,
    /// Its tone.
    pub color: tokens::Color,
}

/// A glyph of a deck's line that LIGHTS under the pointer, as a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// A lane's glyph, which switches it.
    Toggle,
    /// A mute — a lane's, or the player's on the ruler.
    Mute,
}

/// What the pointer resting on `hit` says on the door's line: its control's
/// sentence (`about-<id>`) — with, on a lane's control, the lane's NAME in
/// front: the small column draws a lane by its kind's glyph alone, and two
/// lanes of a kind side by side could not be told apart. `None` for the bar.
#[must_use]
pub fn hover_sentence(
    hit: Hit,
    state: Option<&State>,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    let about = crate::chrome::hint_sentence(hit.about()?, texts)?;
    let lane = match hit {
        Hit::Toggle(id)
        | Hit::Name(id)
        | Hit::LaneMute(id)
        | Hit::Band { lane: id, .. }
        | Hit::LaneVolume { lane: id, .. } => {
            state.and_then(|s| s.lanes.iter().find(|l| l.id == id).map(|l| (l, s)))
        }
        _ => None,
    };
    Some(match lane {
        Some((lane, s)) => {
            let name = if lane.kind == LaneKind::Subtitle {
                crate::player_subtitles::name(lane, s, texts, crate::player::NAME_IN_A_SENTENCE)
            } else {
                crate::player_queue::lane_name(
                    lane,
                    &s.lanes,
                    texts,
                    crate::player::NAME_IN_A_SENTENCE,
                )
            };
            texts.t_with(
                ronoctua_i18n::key::PLAYER_LANE_ABOUT,
                &[("name", name.as_str()), ("about", about.as_str())],
            )
        }
        None => about,
    })
}

impl Hit {
    /// The id its sentence is read by under the pointer (`chrome::
    /// hint_sentence`: `about-<id>`, or the command's `cmd-<id>`) — `None`
    /// for the bar, which is the bar.
    #[must_use]
    pub const fn about(self) -> Option<&'static str> {
        Some(match self {
            Self::Toggle(_) => "player-lane-toggle",
            Self::Name(_) => "player-lane-name",
            Self::Band { .. } => "player-lane-band",
            Self::Track { .. } => "player-deck-seek",
            Self::Playhead => "player-playhead",
            Self::LaneMute(_) => "player-lane-mute",
            Self::LaneVolume { .. } => "player-lane-volume",
            Self::Mute => "player-mute",
            Self::Volume { .. } => "player-deck-volume",
            Self::Bar => return None,
        })
    }

    /// The glyph it lights, when it is one.
    #[must_use]
    pub const fn part(self) -> Option<Part> {
        match self {
            Self::Toggle(_) => Some(Part::Toggle),
            Self::LaneMute(_) | Self::Mute => Some(Part::Mute),
            _ => None,
        }
    }
}

/// How far a held band has to move, in cells, before it is DRAGGED rather
/// than clicked (`space.player.dragSlack`): less is the hand's own tremble on
/// a click, which opens the lane's page — a click that nudged a lane's start
/// would move a sync nobody meant to move.
#[allow(clippy::cast_lossless)] // `f64::from` is no `const fn`
pub const DRAG_SLACK_CELLS: f64 = tokens::space::PLAYER_DRAGSLACK as f64;

/// What a door does after a press, a drag or a release on a deck.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Nothing — draw again.
    Nothing,
    /// Tell the player.
    Tell(Ask),
    /// A lane CHOSEN — its band let go where it was pressed, or its name on
    /// the block pressed: the first click chooses it, a second on the lane
    /// already chosen opens its page (invariant 19). The door knows which
    /// lane is chosen — its cursor's row, or the one its block lit.
    Choose(LaneId),
}

/// What the pointer HOLDS after a press on a deck, until it is let go: the
/// geometry it was pressed on, so the drag keeps the track it started on
/// whatever the next state lays out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grip {
    /// The track, or the playhead: every motion seeks there.
    Seek {
        /// The track pressed.
        track: (u16, u16),
        /// The seconds it shows.
        span: f64,
    },
    /// A lane's band.
    Band {
        /// The lane.
        lane: LaneId,
        /// Whether it is a PICTURE: its drag takes its voice's sound along.
        picture: bool,
        /// How far into the band it was pressed, in seconds.
        grab: f64,
        /// The column it was pressed at.
        from: f64,
        /// Whether it moved past [`DRAG_SLACK_CELLS`]: a drag, not a click.
        moved: bool,
        /// The track pressed.
        track: (u16, u16),
        /// The seconds it shows.
        span: f64,
    },
    /// A lane's meter.
    LaneVolume {
        /// The lane.
        lane: LaneId,
        /// The meter pressed.
        meter: (u16, u16),
    },
    /// The player's volume, on the ruler.
    Volume {
        /// The meter pressed.
        meter: (u16, u16),
    },
    /// The lanes' bar — the door scrolls it, by the height.
    Bar,
}

impl Grip {
    /// The pointer MOVED to `column`, the button held: what to ask now, and
    /// the band's ghost — where it would land — once it is a drag.
    #[must_use]
    pub fn moved(&mut self, column: f64) -> (Act, Option<Held>) {
        match self {
            Self::Seek { track, span } => (
                Act::Tell(Ask::Seek {
                    seconds: seconds_on(*track, *span, column),
                    relative: false,
                }),
                None,
            ),
            Self::Band {
                lane,
                grab,
                from,
                moved,
                track,
                span,
                ..
            } => {
                if !*moved && (column - *from).abs() < DRAG_SLACK_CELLS {
                    return (Act::Nothing, None);
                }
                *moved = true;
                (
                    Act::Nothing,
                    Some(Held {
                        lane: *lane,
                        grab: *grab,
                        at: seconds_on(*track, *span, column),
                        span: *span,
                    }),
                )
            }
            Self::LaneVolume { lane, meter } => (
                Act::Tell(Ask::Lane {
                    lane: *lane,
                    change: LaneChange::Volume {
                        percent: percent_of(
                            along_clamped(column, *meter),
                            ronoctua_player::own::VOLUME_MOST_PERCENT,
                        ),
                    },
                }),
                None,
            ),
            Self::Volume { meter } => (
                Act::Tell(Ask::Volume {
                    percent: percent_of(along_clamped(column, *meter), crate::player::VOLUME_MOST),
                }),
                None,
            ),
            Self::Bar => (Act::Nothing, None),
        }
    }

    /// The button came up. A band let go where it was pressed CHOOSES the
    /// lane — a second click opens its page, the door's to tell
    /// ([`Act::Choose`]); one DRAGGED asks its new start — `ghost`, where it
    /// was drawn to land, and nothing when the drag was called off (`Esc`
    /// took the ghost away).
    #[must_use]
    pub fn released(self, ghost: Option<Held>) -> Act {
        match self {
            Self::Band {
                lane, moved: false, ..
            } => Act::Choose(lane),
            Self::Band { lane, picture, .. } => {
                ghost.filter(|h| h.lane == lane).map_or(Act::Nothing, |h| {
                    // A PICTURE DRAGGED ALONG THE TIMELINE takes its own
                    // sound with it, in step (`voice`); a SOUND dragged moves
                    // alone — that is how it is set against its picture's
                    // lips, as its typed start is.
                    Act::Tell(Ask::Lane {
                        lane,
                        change: LaneChange::Start {
                            seconds: h.start(),
                            voice: picture,
                        },
                    })
                })
            }
            Self::Seek { .. } | Self::LaneVolume { .. } | Self::Volume { .. } | Self::Bar => {
                Act::Nothing
            }
        }
    }
}

/// The seconds at `column` of `track`, `span` seconds long — clamped to its
/// ends.
fn seconds_on(track: (u16, u16), span: f64, column: f64) -> f64 {
    let (first, end) = track;
    let wide = f64::from(end.saturating_sub(first).max(1));
    ((column - f64::from(first)) / wide).clamp(0.0, 1.0) * span
}

/// How far along `(first, end)` `column` is, clamped to its two ends: a drag
/// that leaves a meter keeps the end it left by.
fn along_clamped(column: f64, (first, end): (u16, u16)) -> f64 {
    ((column - f64::from(first)) / f64::from(end.saturating_sub(first).max(1))).clamp(0.0, 1.0)
}

/// `fraction` of a meter up to `most` per cent.
fn percent_of(fraction: f64, most: u8) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (fraction.clamp(0.0, 1.0) * f64::from(most)).round() as u8;
    percent
}

/// The lanes a door's DECK shows of `state`: a SET's — what the queue's
/// outline says the current entry is — and none for a file that plays alone,
/// whose own lanes (its picture, its sound, its subtitles) are one file on
/// one clock: its deck is the time bar it always had. The Timeline page shows
/// every lane either way.
#[must_use]
pub fn set_lanes<'a>(
    state: &'a State,
    outline: Option<&ronoctua_player::queue::Outline>,
) -> &'a [Lane] {
    let set = state
        .current
        .zip(outline)
        .and_then(|(id, o)| crate::player::node_of(o, id))
        .is_some_and(|n| n.kind == ronoctua_player::queue::NodeKind::Set);
    if set { &state.lanes } else { &[] }
}

/// A deck of `lanes`, laid out from what `state` says: each lane's length as
/// the daemon last measured it, the set's length and where its clock is —
/// nothing loaded is the bar with nothing played.
#[must_use]
pub fn of_state(lanes: &[Lane], state: Option<&State>, room: Room, offset: usize) -> Deck {
    // A LANE THAT IS OFF ends where its file does, as one that is on: its
    // band never runs to the track's end like a live one's.
    let length_of = |id: LaneId| state.and_then(|s| crate::player::lane_length(s, id));
    lay(
        lanes,
        length_of,
        state.and_then(|s| s.duration),
        state.map_or(0.0, |s| s.position),
        room,
        offset,
        None,
    )
}

impl Deck {
    /// Whether the lanes scroll.
    #[must_use]
    pub const fn scrolls(&self) -> bool {
        self.scroll.cut()
    }

    /// What a press at `column` (in cells, fractional, from the rows' first
    /// cell) on the deck's `line` (0 is the ruler) takes — read off the same
    /// numbers [`Deck::quads`] draws from.
    #[must_use]
    pub fn hit(&self, line: usize, column: f64) -> Option<Hit> {
        let drawn = self.lines.get(line)?;
        let (first, end) = self.track();
        let on_track = column >= f64::from(first) && column < f64::from(end);
        let near_now = self
            .playhead
            .is_some_and(|p| on_track && (column - f64::from(p)).abs() <= 0.5);
        match drawn {
            DeckLine::Ruler(r) => {
                if near_now {
                    Some(Hit::Playhead)
                } else if on_track {
                    Some(Hit::Track {
                        seconds: self.seconds_at(column),
                    })
                } else if on_glyph(column, r.mute) {
                    Some(Hit::Mute)
                } else {
                    r.volume
                        .and_then(|v| along(column, v))
                        .map(|fraction| Hit::Volume { fraction })
                }
            }
            DeckLine::Lane(l) => {
                if self.scrolls()
                    && column >= f64::from(self.columns.saturating_sub(crate::chrome::BAR_CELLS))
                {
                    return Some(Hit::Bar);
                }
                if on_glyph(column, l.toggle) {
                    return Some(Hit::Toggle(l.lane));
                }
                if l.name
                    .is_some_and(|(a, b)| column >= f64::from(a) && column < f64::from(b))
                {
                    return Some(Hit::Name(l.lane));
                }
                if near_now {
                    return Some(Hit::Playhead);
                }
                if on_track {
                    let (a, b) = l.band;
                    return Some(if column >= f64::from(a) && column < f64::from(b) {
                        Hit::Band {
                            lane: l.lane,
                            grab: self.seconds_at(column) - l.start_seconds,
                        }
                    } else {
                        Hit::Track {
                            seconds: self.seconds_at(column),
                        }
                    });
                }
                if !l.sounds {
                    return None;
                }
                if on_glyph(column, l.mute) {
                    return Some(Hit::LaneMute(l.lane));
                }
                l.meter
                    .and_then(|m| along(column, m))
                    .map(|fraction| Hit::LaneVolume {
                        lane: l.lane,
                        fraction,
                    })
            }
        }
    }

    /// The bar of the lanes that scroll, drawn over the lane lines of `shown`
    /// placed from `first_line` — its track (where it is grabbed) and its
    /// thumb (what is painted) — or `None` while they fit. `left` is the
    /// rows' first cell. What [`Deck::quads`] paints and what a door records
    /// with `canvas.bar`.
    #[must_use]
    pub fn bar(
        &self,
        f: Frame,
        first_line: u16,
        left: u16,
        shown: std::ops::Range<usize>,
    ) -> Option<(crate::chrome::Rect, Option<crate::chrome::Rect>)> {
        if !self.scrolls() {
            return None;
        }
        let lanes = shown.start.max(1)..shown.end.min(self.lines.len());
        if lanes.is_empty() {
            return None;
        }
        let line_of = |i: usize| first_line + u16::try_from(i - shown.start).unwrap_or(u16::MAX);
        let top = f.y_of(line_of(lanes.start));
        let bottom = f.y_of(line_of(lanes.end));
        let right = f.x_of(left) + f32::from(self.columns) * f.cell();
        Some(crate::chrome::band_bar(
            self.scroll,
            right,
            top,
            bottom,
            f.cell(),
            f.line(),
        ))
    }

    /// Every rectangle of the deck's lines `shown`, the first of them on
    /// `first_line` of a surface measured by `f`, from the rows' first cell
    /// `left`: the tracks, the bands and the ghost, the ticks, the meters, the
    /// playhead over every line shown and the bar. `sound` is the player's
    /// own volume, which the ruler's track is filled to. Text — the glyphs,
    /// the names, the ticks' times — is the door's, from the same lines.
    #[must_use]
    pub fn quads(
        &self,
        f: Frame,
        first_line: u16,
        left: u16,
        shown: std::ops::Range<usize>,
        sound: Option<Sound>,
    ) -> Vec<Quad> {
        use crate::chrome;
        let shown = shown.start..shown.end.min(self.lines.len());
        let origin = f.x_of(left);
        let cell = f.cell();
        let x = |column: f32| origin + column * cell;
        let at = |column: u16| x(f32::from(column));
        let h = f.line();
        let meter = |range: (u16, u16), top: f32, percent: u8, muted: bool, most: u8| {
            chrome::track_bar(
                at(range.0),
                at(range.1),
                top,
                h,
                Some((
                    f32::from(percent.min(most)) / f32::from(most.max(1)),
                    if muted {
                        tokens::color::TEXT_DIM
                    } else {
                        tokens::color::STATE_ACCENT
                    },
                )),
            )
        };
        let mut out = Vec::new();
        for (i, line) in self.lines[shown.clone()].iter().enumerate() {
            let top = f.y_of(first_line + u16::try_from(i).unwrap_or(u16::MAX));
            match line {
                DeckLine::Ruler(r) => {
                    if self.playhead.is_none() {
                        out.extend(chrome::time_bar(
                            at(r.track.0),
                            at(r.track.1),
                            top,
                            h,
                            r.played,
                        ));
                    }
                    for t in &r.ticks {
                        out.push(chrome::tick(x(t.at), top, h));
                    }
                    if let Some(v) = r.volume {
                        out.extend(match sound {
                            Some(s) => meter(
                                v,
                                top,
                                s.volume_percent,
                                s.muted,
                                crate::player::VOLUME_MOST,
                            ),
                            None => chrome::track_bar(at(v.0), at(v.1), top, h, None),
                        });
                    }
                }
                DeckLine::Lane(l) => {
                    out.extend(chrome::track_bar(
                        at(l.track.0),
                        at(l.track.1),
                        top,
                        h,
                        None,
                    ));
                    out.push(chrome::lane_band(
                        x(l.band.0),
                        x(l.band.1),
                        top,
                        h,
                        band_color(l.kind, l.on),
                    ));
                    // THE GHOST ON THE BAND, thinner than it: where the band
                    // ends and where it would land both show.
                    if let Some((a, b)) = l.ghost {
                        out.push(chrome::lane_ghost(x(a), x(b), top, h));
                    }
                    if let Some(m) = l.meter.filter(|_| l.sounds) {
                        out.extend(meter(
                            m,
                            top,
                            l.volume_percent,
                            l.muted,
                            ronoctua_player::own::VOLUME_MOST_PERCENT,
                        ));
                    }
                }
            }
        }
        // THE PLAYHEAD over every line shown — and on the ruler only as high
        // as a tick rises, so it never strikes through a time written there.
        if let Some(p) = self.playhead
            && !shown.is_empty()
        {
            let lines = u16::try_from(shown.len()).unwrap_or(u16::MAX);
            let ruler_shown =
                shown.start == 0 && matches!(self.lines.first(), Some(DeckLine::Ruler(_)));
            let from = if ruler_shown {
                f.y_of(first_line) + h - chrome::tick_rise(h)
            } else {
                f.y_of(first_line)
            };
            out.push(chrome::playhead(
                x(p),
                from,
                f.y_of(first_line.saturating_add(lines)),
            ));
        }
        if let Some((_, Some(thumb))) = self.bar(f, first_line, left, shown) {
            out.push(chrome::thumb(thumb));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Deck, DeckLine, Held, Hit, Refusal, Room, Sound, lay, nice_step, parse_start};
    use ronoctua_player::own::{Lane, LaneId, LaneKind};
    use ronoctua_tokens as tokens;

    /// A surface of 80 columns, measured as every door measures one.
    fn frame() -> crate::chrome::Frame {
        let cell = tokens::metrics::cell_width(tokens::text::size());
        let line = tokens::metrics::line_height(tokens::text::size());
        crate::chrome::Frame::of(cell * 82.0, line * 20.0, 0)
    }

    /// Three lanes: a picture of a minute from 0, a sound of forty seconds
    /// from 10 (muted, at 60 %), and subtitles that are off, from 5 to 50.
    fn three() -> (Vec<Lane>, impl Fn(LaneId) -> Option<f64>) {
        let mut video = Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv");
        video.title = Some("One".into());
        let mut audio = Lane::new(LaneId(2), LaneKind::Audio, "/films/one.mkv");
        audio.start_seconds = 10.0;
        audio.muted = true;
        audio.volume_percent = 60;
        let mut words = Lane::new(LaneId(3), LaneKind::Subtitle, "/films/one.srt");
        words.start_seconds = 5.0;
        words.enabled = false;
        let lengths = |id: LaneId| match id.0 {
            1 => Some(60.0),
            2 => Some(40.0),
            3 => Some(45.0),
            _ => None,
        };
        (vec![video, audio, words], lengths)
    }

    fn lane_line(deck: &Deck, n: usize) -> &super::LaneLine {
        match &deck.lines[n] {
            DeckLine::Lane(l) => l,
            DeckLine::Ruler(_) => panic!("line {n} is the ruler"),
        }
    }

    fn ruler(deck: &Deck) -> &super::Ruler {
        match &deck.lines[0] {
            DeckLine::Ruler(r) => r,
            DeckLine::Lane(_) => panic!("the first line is a lane"),
        }
    }

    /// The pixel a column of the rows is at, for `left` = 1.
    fn px(column: f32) -> f32 {
        let f = frame();
        f.x_of(1) + column * f.cell()
    }

    /// FAILS IF: a band stops running from its lane's start to its end on the
    /// shared track — in the layout AND in the rectangle painted.
    #[test]
    fn a_band_runs_from_its_start_to_its_end() {
        let (lanes, lengths) = three();
        let deck = lay(&lanes, lengths, Some(60.0), 0.0, Room::wide(80), 0, None);
        let (a, b) = deck.track();
        let wide = f32::from(b - a);
        let sound = lane_line(&deck, 2);
        // FROM 10 TO 50 OF 60.
        let want = (
            f32::from(a) + wide * 10.0 / 60.0,
            f32::from(a) + wide * 50.0 / 60.0,
        );
        assert!((sound.band.0 - want.0).abs() < 1e-3 && (sound.band.1 - want.1).abs() < 1e-3);
        let quads = deck.quads(frame(), 2, 1, 0..deck.lines.len(), None);
        let colour = super::band_color(LaneKind::Audio, true);
        let q = quads
            .iter()
            .find(|q| q.color == colour)
            .expect("the sound's band");
        assert!((q.x - px(want.0)).abs() < 0.01 && (q.x + q.width - px(want.1)).abs() < 0.01);
        // A LANE THAT IS OFF keeps its band, in the grey of one that is off.
        let off = quads
            .iter()
            .filter(|q| q.color == tokens::color::PLAYER_LANE_OFF)
            .count();
        assert_eq!(off, 1);
    }

    /// FAILS IF: a lane with no known end stops at a guess instead of the
    /// track's end, or a deck with no length puts now at its edge.
    #[test]
    fn a_live_lane_runs_to_the_edge() {
        let (mut lanes, _) = three();
        lanes[0].until_seconds = None;
        let live = |id: LaneId| (id.0 != 1).then_some(40.0);
        let deck = lay(&lanes, live, None, 30.0, Room::wide(80), 0, None);
        let picture = lane_line(&deck, 1);
        assert!(picture.live);
        assert!((picture.band.1 - f32::from(deck.track().1)).abs() < f32::EPSILON);
        let now = deck.playhead.expect("a playhead");
        assert!(
            now < f32::from(deck.track().1) - 1.0,
            "now is at the edge: {now}"
        );
    }

    /// FAILS IF: the playhead is two lines, or a line's press within half a
    /// cell of it is not the playhead's.
    #[test]
    fn the_playhead_is_one_x_on_every_line() {
        let (lanes, lengths) = three();
        let deck = lay(&lanes, lengths, Some(60.0), 30.0, Room::wide(80), 0, None);
        let now = deck.playhead.expect("a playhead");
        let f = frame();
        let quads = deck.quads(f, 2, 1, 0..deck.lines.len(), None);
        let heads: Vec<_> = quads
            .iter()
            .filter(|q| q.color == tokens::color::PLAYER_PLAYHEAD)
            .collect();
        assert_eq!(heads.len(), 1, "one playhead across every lane");
        let head = heads[0];
        assert!((head.x + head.width / 2.0 - px(now)).abs() < 0.01);
        // ON THE RULER only as high as a tick rises: the times written there
        // are never struck through.
        let rise = crate::chrome::tick_rise(f.line());
        assert!(
            (head.y - (f.y_of(3) - rise)).abs() < 0.01,
            "the playhead starts at {} and the ruler's ticks at {}",
            head.y,
            f.y_of(3) - rise
        );
        let tick = quads
            .iter()
            .find(|q| q.color == tokens::color::RULE_DEFAULT && (q.height - rise).abs() < 0.01)
            .expect("a tick");
        assert!(
            (tick.y - head.y).abs() < 0.01,
            "as high as a tick, no higher"
        );
        let lines = u16::try_from(deck.lines.len()).unwrap();
        assert!((head.y + head.height - f.y_of(2 + lines)).abs() < 0.01);
        for line in 0..deck.lines.len() {
            for by in [-0.45, 0.0, 0.45] {
                assert_eq!(
                    deck.hit(line, f64::from(now) + by),
                    Some(Hit::Playhead),
                    "line {line}, {by} cells from now"
                );
            }
        }
    }

    /// FAILS IF: any control of the deck answers a press anywhere but where
    /// it is drawn — every `Hit`, from the numbers the painter used.
    #[test]
    fn a_press_is_taken_where_it_was_drawn() {
        let (mut lanes, lengths) = three();
        lanes.push(Lane::new(LaneId(4), LaneKind::Audio, "/films/two.flac"));
        let lengths = move |id: LaneId| lengths(id).or(Some(20.0));
        let room = Room {
            most_lanes: Some(2),
            ..Room::wide(80)
        };
        let deck = lay(&lanes, lengths, Some(60.0), 55.0, room, 0, None);
        assert!(deck.scrolls());
        let f = frame();
        let quads = deck.quads(f, 2, 1, 0..deck.lines.len(), None);
        let at = |line: u16, column: f64| {
            #[allow(clippy::cast_possible_truncation)]
            let x = px(column as f32);
            let y = f.y_of(2 + line) + f.line() / 2.0;
            (x, y)
        };
        let painted = |(x, y): (f32, f32), color: tokens::Color| {
            quads.iter().any(|q| {
                q.color == color
                    && x >= q.x
                    && x <= q.x + q.width
                    && y >= q.y
                    && y <= q.y + q.height
            })
        };
        let r = ruler(&deck).clone();
        let picture = lane_line(&deck, 1).clone();
        let sound = lane_line(&deck, 2).clone();

        // THE PICTURE'S BAND, a cell into it.
        let c = f64::from(picture.band.0) + 1.0;
        assert!(painted(at(1, c), super::band_color(LaneKind::Video, true)));
        match deck.hit(1, c) {
            Some(Hit::Band { lane, grab }) => {
                assert_eq!(lane, LaneId(1));
                assert!((grab - deck.seconds_at(c)).abs() < 1e-9);
            }
            other => panic!("the band answered {other:?}"),
        }
        // THE TRACK WHERE NO BAND IS: before the sound's start.
        let c = f64::from(sound.track.0) + 0.5;
        assert!(c < f64::from(sound.band.0));
        assert!(painted(at(2, c), tokens::color::RULE_DEFAULT));
        assert!(matches!(deck.hit(2, c), Some(Hit::Track { .. })));
        // THE PLAYHEAD.
        let now = f64::from(deck.playhead.unwrap());
        assert!(painted(at(1, now), tokens::color::PLAYER_PLAYHEAD));
        assert_eq!(deck.hit(1, now), Some(Hit::Playhead));
        // THE GLYPHS AND THE NAME, which the door writes into these cells.
        assert_eq!(
            deck.hit(1, f64::from(picture.toggle) + 0.5),
            Some(Hit::Toggle(LaneId(1)))
        );
        let (n0, _) = picture.name.expect("a name");
        assert_eq!(deck.hit(1, f64::from(n0) + 1.0), Some(Hit::Name(LaneId(1))));
        assert_eq!(
            deck.hit(2, f64::from(sound.mute) + 0.5),
            Some(Hit::LaneMute(LaneId(2)))
        );
        assert_eq!(deck.hit(0, f64::from(r.mute) + 0.5), Some(Hit::Mute));
        // THE METER, three quarters along, where its track is painted.
        let (m0, m1) = sound.meter.expect("a meter");
        let c = f64::from(m0) + f64::from(m1 - m0) * 0.75;
        assert!(painted(at(2, c), tokens::color::RULE_DEFAULT));
        match deck.hit(2, c) {
            Some(Hit::LaneVolume { lane, fraction }) => {
                assert_eq!(lane, LaneId(2));
                assert!((fraction - 0.75).abs() < 1e-9);
            }
            other => panic!("the meter answered {other:?}"),
        }
        // THE PLAYER'S VOLUME on the ruler, a quarter along.
        let (v0, v1) = r.volume.expect("a volume");
        let c = f64::from(v0) + f64::from(v1 - v0) * 0.25;
        assert!(painted(at(0, c), tokens::color::RULE_DEFAULT));
        assert!(
            matches!(deck.hit(0, c), Some(Hit::Volume { fraction }) if (fraction - 0.25).abs() < 1e-9)
        );
        // THE RULER'S TRACK is a seek.
        let c = f64::from(r.track.0) + 2.0;
        assert!(
            matches!(deck.hit(0, c), Some(Hit::Track { seconds }) if (seconds - deck.seconds_at(c)).abs() < 1e-9)
        );
        // AND THE BAR, in its own cell, on every lane line and not the ruler.
        let (track, thumb) = deck.bar(f, 2, 1, 0..deck.lines.len()).expect("a bar");
        let bar = f64::from(deck.columns) - 0.5;
        for line in 1..deck.lines.len() {
            assert_eq!(deck.hit(line, bar), Some(Hit::Bar));
            let (x, y) = at(u16::try_from(line).unwrap(), bar);
            assert!(
                x >= track.0 && x <= track.0 + track.2 && y >= track.1 && y <= track.1 + track.3
            );
        }
        assert_ne!(deck.hit(0, bar), Some(Hit::Bar));
        let thumb = thumb.expect("a thumb");
        assert!(quads.iter().any(|q| q.color == tokens::color::RULE_STRONG
            && (q.x - thumb.0).abs() < 0.01
            && (q.height - thumb.3).abs() < 0.01));
        // NOTHING PAST THE EDGE of a line answers.
        assert_eq!(deck.hit(0, f64::from(deck.columns) + 3.0), None);
        assert_eq!(deck.hit(9, 10.0), None);
    }

    /// FAILS IF: a dragged band's ghost does not move by what the pointer
    /// moved, or it can be dragged before the set's first second.
    #[test]
    fn a_drag_moves_the_start_by_what_the_pointer_moved_and_never_below_zero() {
        let (lanes, lengths) = three();
        let deck = lay(&lanes, &lengths, Some(60.0), 0.0, Room::wide(80), 0, None);
        let sound = lane_line(&deck, 2).clone();
        let pressed = f64::from(sound.band.0) + 2.0;
        let Some(Hit::Band { lane, grab }) = deck.hit(2, pressed) else {
            panic!("no band under the press");
        };
        // FIVE CELLS RIGHT: the start moves by the seconds five cells are.
        let moved = pressed + 5.0;
        let held = Held {
            lane,
            grab,
            at: deck.seconds_at(moved),
            span: deck.span,
        };
        let delta = deck.seconds_at(moved) - deck.seconds_at(pressed);
        assert!((held.start() - (10.0 + delta)).abs() < 1e-9);
        let dragged = lay(
            &lanes,
            &lengths,
            Some(60.0),
            0.0,
            Room::wide(80),
            0,
            Some(held),
        );
        let ghost = lane_line(&dragged, 2).ghost.expect("a ghost");
        assert!(
            (ghost.0 - (sound.band.0 + 5.0)).abs() < 1e-3,
            "{ghost:?} {:?}",
            sound.band
        );
        assert!((ghost.1 - ghost.0 - (sound.band.1 - sound.band.0)).abs() < 1e-3);
        // THE BAND ITSELF STAYS until it is let go.
        assert_eq!(lane_line(&dragged, 2).band, sound.band);
        // AND FAR LEFT, the start is the set's first second.
        let held = Held {
            at: deck.seconds_at(-40.0),
            ..held
        };
        assert!(held.start().abs() < f64::EPSILON);
        let dragged = lay(
            &lanes,
            &lengths,
            Some(60.0),
            0.0,
            Room::wide(80),
            0,
            Some(held),
        );
        let ghost = lane_line(&dragged, 2).ghost.expect("a ghost");
        assert!((ghost.0 - f32::from(dragged.track().0)).abs() < 1e-3);
        // ONLY THE HELD LANE has a ghost.
        assert!(lane_line(&dragged, 1).ghost.is_none());
    }

    /// FAILS IF: on a short track — where only `0:00` fits from its own
    /// cell — the last label is dropped instead of moved back to end where
    /// the track ends, or is moved onto the label before it.
    #[test]
    fn the_last_label_is_moved_back_to_fit() {
        let mut moved_some = 0;
        for width in 10_u16..=40 {
            let track = (4_u16, 4 + width);
            for n in 1..=400 {
                let span = f64::from(n) * 9.5;
                let ticks = super::ticks(track, span);
                let mut end = 0_u16;
                for (i, t) in ticks.iter().enumerate() {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let own = t.at.floor() as u16;
                    let wide = ronoctua_protocol::cells(&t.text);
                    assert!(i == 0 || t.label > end, "{span} s over {width}: {ticks:?}");
                    assert!(t.label + wide <= track.1, "{span} s over {width}: past");
                    if t.label != own {
                        moved_some += 1;
                        assert_eq!(i, ticks.len() - 1, "only the last is moved");
                        assert_eq!(t.label + wide, track.1, "it ends at the track's end");
                        assert!(t.label < own, "it is moved back, never forward");
                    }
                    end = t.label + wide;
                }
            }
        }
        assert!(moved_some > 0, "a short track gets its second mark");
        // THE PENDING'S CASE: 17 cells, where `0:00` alone fitted.
        let ticks = super::ticks((0, 17), 3601.0);
        assert!(ticks.len() >= 2, "{ticks:?}");
    }

    /// FAILS IF: two labels of the ruler run into each other or past its
    /// track, at any length and any width.
    #[test]
    fn ticks_never_collide() {
        for span in [
            5.0,
            59.0,
            61.0,
            600.0,
            3599.0,
            3601.0,
            7.5 * 3600.0,
            3.0 * 86400.0,
        ] {
            for columns in [32_u16, 48, 64, 80, 120, 200] {
                let mut lanes = Vec::new();
                for id in 1..=2 {
                    lanes.push(Lane::new(LaneId(id), LaneKind::Audio, "/a.flac"));
                }
                let deck = lay(
                    &lanes,
                    |_| Some(span),
                    Some(span),
                    0.0,
                    Room::wide(columns),
                    0,
                    None,
                );
                let r = ruler(&deck);
                let mut end = 0_u16;
                for (i, t) in r.ticks.iter().enumerate() {
                    let wide = ronoctua_protocol::cells(&t.text);
                    assert!(
                        i == 0 || t.label > end,
                        "{span} s over {columns}: `{}` touches the label before",
                        t.text
                    );
                    assert!(
                        t.label + wide <= r.track.1,
                        "{span} s over {columns}: past the track"
                    );
                    end = t.label + wide;
                }
                assert!(
                    !r.ticks.is_empty(),
                    "{span} s over {columns}: no tick at all"
                );
                assert_eq!(r.ticks[0].text, "0:00");
            }
        }
        // THE STEP IS ONE A CLOCK IS READ IN.
        assert!((nice_step(600.0, 60) - 60.0).abs() < f64::EPSILON);
        assert!((nice_step(60.0, 60) - 5.0).abs() < f64::EPSILON);
    }

    /// FAILS IF: lanes past the room stop scrolling under ONE bar, or the
    /// bar's cell is taken out of one line's track and not the others'.
    #[test]
    fn lanes_past_the_room_scroll_with_one_bar() {
        let lanes: Vec<Lane> = (1..=6)
            .map(|i| Lane::new(LaneId(i), LaneKind::Audio, format!("/{i}.flac")))
            .collect();
        let room = Room {
            most_lanes: Some(4),
            ..Room::wide(80)
        };
        let deck = lay(&lanes, |_| Some(30.0), Some(30.0), 0.0, room, 9, None);
        assert_eq!(deck.lines.len(), 5, "the ruler and four lanes");
        assert!(deck.scrolls());
        assert_eq!(
            deck.scroll.offset, 2,
            "an offset past the end is the last page"
        );
        assert_eq!(lane_line(&deck, 1).lane, LaneId(3));
        let track = deck.track();
        for n in 1..5 {
            assert_eq!(lane_line(&deck, n).track, track);
            let meter = lane_line(&deck, n).meter.unwrap();
            assert!(meter.1 < deck.columns, "a meter under the bar");
        }
        let quads = deck.quads(frame(), 2, 1, 0..deck.lines.len(), None);
        let thumbs = quads
            .iter()
            .filter(|q| q.color == tokens::color::RULE_STRONG)
            .count();
        assert_eq!(thumbs, 1, "one bar for the lanes");
        // AND WITH ROOM FOR EVERY LANE, no bar and the whole width.
        let all = lay(
            &lanes,
            |_| Some(30.0),
            Some(30.0),
            0.0,
            room.every_lane(),
            9,
            None,
        );
        assert_eq!(all.lines.len(), 7);
        assert!(!all.scrolls());
        assert_eq!(all.scroll.offset, 0);
        assert_eq!(
            all.track().1,
            track.1 + 1,
            "the bar's cell comes back to the track"
        );
    }

    /// FAILS IF: the small column loses a lane's glyph, its band or its mute,
    /// or keeps a name or a meter it has no room for.
    #[test]
    fn the_small_column_keeps_glyph_band_mute() {
        let (lanes, lengths) = three();
        let deck = lay(&lanes, lengths, Some(60.0), 20.0, Room::narrow(30), 0, None);
        let r = ruler(&deck);
        assert!(r.volume.is_none());
        for n in 1..deck.lines.len() {
            let l = lane_line(&deck, n);
            assert!(l.name.is_none() && l.meter.is_none());
            assert_eq!(l.toggle, 0);
            assert!(l.band.1 > l.band.0);
            assert_eq!(l.mute, 30 - super::GLYPH_CELLS);
            // ONLY A LANE THAT SOUNDS has a mute: the picture and the
            // subtitles answer nothing there.
            assert_eq!(
                deck.hit(n, f64::from(l.mute) + 0.5),
                l.sounds.then_some(Hit::LaneMute(l.lane)),
                "lane {:?}",
                l.lane
            );
        }
        assert_eq!(
            (1..deck.lines.len())
                .filter(|&n| lane_line(&deck, n).sounds)
                .count(),
            1,
            "the sound, and nothing else, sounds"
        );
        assert_eq!(
            deck.track(),
            (super::GLYPH_CELLS + 1, 30 - super::GLYPH_CELLS - 1)
        );
        // A WIDE ROOM TOO NARROW for its names gives them up before the track
        // is shorter than a name.
        let squeezed = lay(
            &lanes,
            |_| Some(60.0),
            Some(60.0),
            0.0,
            Room::wide(34),
            0,
            None,
        );
        let (a, b) = squeezed.track();
        assert!(b - a >= tokens::space::PLAYER_DECKNAMES, "{a}..{b}");
    }

    /// FAILS IF: a set of one lane stops being today's time bar — the track
    /// from the rows' first cell, the mute and the volume where the window
    /// has always had them (`Player::volume_cells`), what played in the
    /// accent, and no playhead.
    #[test]
    fn one_lane_is_the_old_bar() {
        let vb = tokens::space::PLAYER_VOLUMEBAR;
        for columns in [30_u16, 46, 80] {
            let lane = Lane::new(LaneId(1), LaneKind::Video, "/film.mkv");
            let deck = lay(
                &[lane],
                |_| Some(100.0),
                Some(100.0),
                25.0,
                Room::wide(columns),
                0,
                None,
            );
            assert_eq!(deck.lines.len(), 1);
            assert_eq!(deck.playhead, None);
            let r = ruler(&deck);
            // THE WINDOW'S NUMBERS: the glyph three cells before the volume,
            // the bar a cell before the glyph.
            let first = columns - vb;
            assert_eq!(r.volume, Some((first, columns)));
            assert_eq!(r.mute, first - 3);
            assert_eq!(r.track, (0, first - 4));
            assert!(r.ticks.is_empty());
            assert!((r.played.unwrap() - 0.25).abs() < 1e-6);
            let f = frame();
            let quads = deck.quads(
                f,
                2,
                1,
                0..1,
                Some(Sound {
                    volume_percent: 100,
                    muted: false,
                }),
            );
            let bar = crate::chrome::time_bar(
                px(0.0),
                px(f32::from(first - 4)),
                f.y_of(2),
                f.line(),
                Some(0.25),
            );
            assert_eq!(&quads[..2], &bar[..]);
            assert_eq!(
                deck.hit(0, 3.0),
                Some(Hit::Track {
                    seconds: deck.seconds_at(3.0)
                })
            );
        }
        // AND NOTHING LOADED is the bar with nothing played.
        let empty = lay(&[], |_| None, None, 0.0, Room::wide(80), 0, None);
        assert_eq!(empty.lines.len(), 1);
        assert_eq!(ruler(&empty).played, None);
        // A STREAM ALONE has no length, and nothing is filled.
        let stream = Lane::new(LaneId(1), LaneKind::Audio, "https://radio.example/live");
        let live = lay(&[stream], |_| None, None, 40.0, Room::wide(80), 0, None);
        assert_eq!(ruler(&live).played, None);
    }

    /// FAILS IF: a deck whose times are past any clock — a start of 309
    /// digits, a length or a position that is not finite — never finishes
    /// laying out: an infinite span made a ruler whose ticks never passed it,
    /// and the door that drew it ran out of memory.
    #[test]
    fn a_deck_past_any_clock_is_laid_out_and_its_ruler_ends() {
        let (mut lanes, _) = three();
        lanes[1].start_seconds = 1.44e308;
        let lengths = |id: LaneId| match id.0 {
            1 => Some(f64::INFINITY),
            2 => None,
            _ => Some(f64::NAN),
        };
        for span in [None, Some(f64::INFINITY), Some(f64::NAN), Some(1e308)] {
            for position in [f64::INFINITY, f64::NAN, 1e308, 30.0] {
                let deck = lay(&lanes, lengths, span, position, Room::wide(80), 0, None);
                assert!(
                    deck.span.is_finite() && deck.span <= super::SECONDS_MOST * super::live_room(),
                    "{span:?}, {position}: a span of {}",
                    deck.span
                );
                let (a, b) = deck.track();
                assert!(ruler(&deck).ticks.len() <= usize::from(b - a) + 1);
                assert!(deck.playhead.is_some_and(f32::is_finite));
                assert!(lane_line(&deck, 2).start_seconds <= super::SECONDS_MOST);
            }
        }
        // THE LIVE LANE PAST THE CLOCK is drawn from the ceiling, a fifth of
        // the track, and not squeezed to nothing at its end.
        let deck = lay(&lanes, lengths, None, 30.0, Room::wide(80), 0, None);
        let far = lane_line(&deck, 2);
        let (a, b) = deck.track();
        let fifth = f32::from(b - a) / 5.0;
        assert!(
            (far.band.1 - far.band.0 - fifth).abs() < 0.01,
            "{:?} on {:?}",
            far.band,
            (a, b)
        );
        for span in [f64::INFINITY, f64::NAN, -1.0, 0.0] {
            assert!(super::ticks((0, 60), span).is_empty(), "{span}");
        }
        assert!(super::ticks((0, 60), 1e300).len() <= 61);
        assert!(!super::ticks((0, 60), 60.0).is_empty());
    }

    /// FAILS IF: a press on a deck with no room at all panics — the bar's
    /// cell counted back from zero columns.
    #[test]
    fn a_deck_with_no_room_answers_a_press_without_panicking() {
        let (lanes, lengths) = three();
        let room = Room {
            most_lanes: Some(1),
            ..Room::wide(0)
        };
        let deck = lay(&lanes, lengths, Some(60.0), 0.0, room, 0, None);
        assert!(deck.scrolls());
        for line in 0..deck.lines.len() {
            for column in [-1.0, 0.0, 0.5, 3.0] {
                let _ = deck.hit(line, column);
            }
        }
    }

    /// FAILS IF: the ghost of a dragged band hides where the band ends — it
    /// is drawn ON the band, thinner, so both show.
    #[test]
    fn a_ghost_is_thinner_than_its_band_and_drawn_over_it() {
        let (lanes, lengths) = three();
        let deck = lay(&lanes, &lengths, Some(60.0), 0.0, Room::wide(80), 0, None);
        let held = Held {
            lane: LaneId(2),
            grab: 0.0,
            at: 20.0,
            span: deck.span,
        };
        let dragged = lay(
            &lanes,
            &lengths,
            Some(60.0),
            0.0,
            Room::wide(80),
            0,
            Some(held),
        );
        let quads = dragged.quads(frame(), 2, 1, 0..dragged.lines.len(), None);
        let band_colour = super::band_color(LaneKind::Audio, true);
        let band = quads
            .iter()
            .position(|q| q.color == band_colour)
            .expect("the band");
        let ghost = quads
            .iter()
            .position(|q| q.color == tokens::color::PLAYER_LANE_HELD)
            .expect("the ghost");
        assert!(ghost > band, "the ghost is drawn over the band");
        let (b, g) = (&quads[band], &quads[ghost]);
        assert!(g.height < b.height, "{} against {}", g.height, b.height);
        let middle = |q: &ronoctua_render::quad::Quad| q.y + q.height / 2.0;
        assert!((middle(g) - middle(b)).abs() < 0.01, "on the band's middle");
        assert_eq!(lane_line(&dragged, 2).band, lane_line(&deck, 2).band);
    }

    /// FAILS IF: the lane's typed start stops reading a clock, seconds or a
    /// move from where it is — or reads something else as one.
    #[test]
    fn a_typed_start_reads_clock_seconds_and_relative() {
        let ok = |t: &str, now: f64, want: f64| {
            let got = parse_start(t, now).unwrap_or_else(|e| panic!("`{t}` refused: {e:?}"));
            assert!((got - want).abs() < 1e-9, "`{t}` read as {got}, not {want}");
        };
        ok("1:05", 0.0, 65.0);
        ok("0:05.5", 0.0, 5.5);
        // THE MINUS SIGN the refusal's sentence prints, as the hyphen.
        ok("\u{2212}0,5", 3.0, 2.5);
        ok("1:02:03", 0.0, 3723.0);
        ok("90", 0.0, 90.0);
        ok("12.5", 0.0, 12.5);
        ok("12,5", 0.0, 12.5);
        ok(" +1.5 ", 10.0, 11.5);
        ok("-0,3", 10.0, 9.7);
        ok("+0:05", 10.0, 15.0);
        // BEFORE THE SET'S FIRST SECOND is its first second.
        ok("-20", 10.0, 0.0);
        // AND PAST THE CEILING is no time: a number of 309 digits is finite,
        // and a quarter more than it is not.
        ok("9999:59:59", 0.0, 35_999_999.0);
        ok("10000:00:00", 0.0, super::SECONDS_MOST);
        for past in [
            "10000:00:01".to_owned(),
            "9".repeat(309),
            "+36000001".to_owned(),
        ] {
            assert_eq!(
                parse_start(&past, 0.0),
                Err(Refusal::NotATime),
                "`{past}` was read"
            );
        }
        assert_eq!(parse_start("  ", 3.0), Err(Refusal::Empty));
        for bad in [
            "abc", "1:60", "1:2:3:4", "+", "1.5:00", "1:5x", "::", "-1:-5", "1:005", "1e3",
        ] {
            assert_eq!(
                parse_start(bad, 3.0),
                Err(Refusal::NotATime),
                "`{bad}` was read"
            );
        }
    }

    /// Three lanes as a SET plays them — the state the daemon says, with
    /// each lane's length measured.
    fn playing_three() -> ronoctua_player::wire::State {
        let (lanes, lengths) = three();
        let mut s = crate::player::fixture::idle();
        s.current = Some(ronoctua_player::queue::EntryId(1));
        s.duration = Some(60.0);
        s.position = 20.0;
        s.lane_states = lanes
            .iter()
            .map(|l| ronoctua_player::wire::LaneState {
                lane: l.id,
                duration_seconds: lengths(l.id),
                status: ronoctua_player::wire::LaneStatus::Playing,
                picture: false,
                drift_ms: 0,
                master: false,
                size: None,
                cover: false,
                languages: Vec::new(),
            })
            .collect();
        s.lanes = lanes;
        s
    }

    /// FAILS IF: a band's ghost is drawn on the deck's CURRENT width while it
    /// is held — a deck of no known length widens four times a second as it
    /// plays, and the ghost slid away from the hand holding it — or the
    /// start asked on letting go is not the one the ghost was drawn at.
    #[test]
    fn a_ghost_stays_under_the_hand_while_the_deck_widens() {
        use super::Act;
        use ronoctua_player::wire::{Ask, LaneChange};
        let (lanes, lengths) = three();
        let deck = lay(&lanes, &lengths, Some(60.0), 0.0, Room::wide(80), 0, None);
        let sound = lane_line(&deck, 2).clone();
        let pressed = f64::from(sound.band.0) + 2.0;
        let (grip, _) = deck.press(2, pressed);
        let mut grip = grip.expect("held");
        let (_, ghost) = grip.moved(pressed + 5.0);
        let held = ghost.expect("a drag");
        // THE DECK WIDENS while the hand holds still: twice the seconds.
        let wider = lay(&lanes, &lengths, Some(120.0), 0.0, Room::wide(80), 0, None);
        assert_eq!(wider.track(), deck.track(), "the same cells");
        let drawn = lane_line(&wider.holding(Some(held)), 2)
            .ghost
            .expect("a ghost");
        assert!(
            (drawn.0 - (sound.band.0 + 5.0)).abs() < 1e-3,
            "the ghost left the hand: {drawn:?}, pressed on {:?}",
            sound.band
        );
        let Act::Tell(Ask::Lane {
            change: LaneChange::Start { seconds, .. },
            ..
        }) = grip.released(Some(held))
        else {
            panic!("a drag asks its start");
        };
        assert!((seconds - held.start()).abs() < 1e-9);
        let where_drawn = deck.seconds_at(f64::from(drawn.0));
        assert!(
            (seconds - where_drawn).abs() < 0.1,
            "{seconds} asked, {where_drawn} drawn"
        );
    }

    /// FAILS IF: a band held and moved stops leaving a ghost where it would
    /// land and asking that start when let go — or a band let go where it
    /// was pressed stops choosing its lane, or one nudged by less than
    /// the slack is taken as a drag, or a drag called off (`Esc`) still asks.
    #[test]
    fn a_band_is_dragged_by_its_ghost_and_a_click_opens_its_page() {
        use super::{Act, DRAG_SLACK_CELLS, Grip};
        use ronoctua_player::wire::{Ask, LaneChange};
        let (lanes, lengths) = three();
        let deck = lay(&lanes, &lengths, Some(60.0), 0.0, Room::wide(80), 0, None);
        let sound = lane_line(&deck, 2).clone();
        let pressed = f64::from(sound.band.0) + 2.0;
        let (grip, act) = deck.press(2, pressed);
        assert_eq!(act, Act::Nothing, "a band asks nothing on the press");
        let mut grip = grip.expect("the band is held");
        assert!(matches!(grip, Grip::Band { moved: false, .. }));
        // A TREMBLE is no drag: no ghost, and let go it is a click.
        let (act, ghost) = grip.moved(pressed + DRAG_SLACK_CELLS / 2.0);
        assert_eq!((act, ghost), (Act::Nothing, None));
        assert_eq!(
            grip.released(None),
            Act::Choose(LaneId(2)),
            "a click on a band chooses its lane; the door opens it on the second"
        );
        // FIVE CELLS: a ghost, and the start it lands on asked when let go.
        let (grip, _) = deck.press(2, pressed);
        let mut grip = grip.expect("held");
        let (_, ghost) = grip.moved(pressed + 5.0);
        let ghost = ghost.expect("a ghost");
        let delta = deck.seconds_at(pressed + 5.0) - deck.seconds_at(pressed);
        assert!((ghost.start() - (10.0 + delta)).abs() < 1e-9);
        // THE GHOST PAINTED is the one `lay` would draw for it.
        let drawn = deck.holding(Some(ghost));
        let laid = lay(
            &lanes,
            &lengths,
            Some(60.0),
            0.0,
            Room::wide(80),
            0,
            Some(ghost),
        );
        assert_eq!(drawn, laid);
        assert!(lane_line(&drawn, 2).ghost.is_some());
        match grip.released(Some(ghost)) {
            Act::Tell(Ask::Lane {
                lane,
                change: LaneChange::Start { seconds, voice },
            }) => {
                assert_eq!(lane, LaneId(2));
                assert!((seconds - ghost.start()).abs() < 1e-9);
                assert!(!voice, "a sound dragged moves alone, against the lips");
            }
            other => panic!("the drag asked {other:?}"),
        }
        // CALLED OFF: the ghost was taken away, and nothing is asked.
        assert_eq!(grip.released(None), Act::Nothing);
        // A PICTURE DRAGGED takes its sound with it.
        let picture = lane_line(&deck, 1).clone();
        let pressed = f64::from(picture.band.0) + 1.0;
        let (grip, _) = deck.press(1, pressed);
        let mut grip = grip.expect("held");
        let (_, ghost) = grip.moved(pressed + 5.0);
        match grip.released(ghost) {
            Act::Tell(Ask::Lane {
                lane,
                change: LaneChange::Start { voice, .. },
            }) => {
                assert_eq!(lane, LaneId(1));
                assert!(voice, "a picture dragged left its sound behind");
            }
            other => panic!("the drag asked {other:?}"),
        }
    }

    /// FAILS IF: a press on any control of the deck asks anything but what
    /// the control says — the glyph turns the lane, the lane's mute its mute,
    /// the name opens its page, the track seeks there, the meters set their
    /// volume — or a stream alone, whose length nobody knows, seeks.
    #[test]
    fn every_press_asks_what_its_control_says() {
        use super::Act;
        use ronoctua_player::wire::{Ask, LaneChange};
        let (lanes, lengths) = three();
        let deck = lay(&lanes, &lengths, Some(60.0), 20.0, Room::wide(80), 0, None);
        let r = ruler(&deck).clone();
        let picture = lane_line(&deck, 1).clone();
        let sound = lane_line(&deck, 2).clone();
        let tell = |lane: u32, change| {
            Act::Tell(Ask::Lane {
                lane: LaneId(lane),
                change,
            })
        };
        assert_eq!(
            deck.press(1, f64::from(picture.toggle) + 0.5).1,
            tell(1, LaneChange::Enabled { on: false })
        );
        assert_eq!(
            deck.press(2, f64::from(sound.mute) + 0.5).1,
            tell(2, LaneChange::Mute { muted: false })
        );
        let name = picture.name.expect("a name").0;
        assert_eq!(
            deck.press(1, f64::from(name) + 1.0),
            (None, Act::Choose(LaneId(1)))
        );
        let c = f64::from(r.track.0) + 3.0;
        let (grip, act) = deck.press(0, c);
        assert!(grip.is_some(), "the track is held for a drag");
        assert_eq!(
            act,
            Act::Tell(Ask::Seek {
                seconds: deck.seconds_at(c),
                relative: false
            })
        );
        // THE PLAYHEAD is grabbed where it is: nothing asked until it moves.
        let now = f64::from(deck.playhead.expect("a playhead"));
        let (grip, act) = deck.press(1, now);
        assert_eq!(act, Act::Nothing);
        let mut grip = grip.expect("held");
        assert!(matches!(
            grip.moved(now + 4.0).0,
            Act::Tell(Ask::Seek { .. })
        ));
        // THE METERS, three quarters along.
        let (m0, m1) = sound.meter.expect("a meter");
        let c = f64::from(m0) + f64::from(m1 - m0) * 0.75;
        assert_eq!(
            deck.press(2, c).1,
            tell(2, LaneChange::Volume { percent: 98 })
        );
        let (v0, v1) = r.volume.expect("the player's volume");
        let c = f64::from(v0) + f64::from(v1 - v0) * 0.5;
        assert_eq!(deck.press(0, c).1, Act::Tell(Ask::Volume { percent: 65 }));
        assert_eq!(
            deck.press(0, f64::from(r.mute) + 0.5).1,
            Act::Tell(Ask::Mute)
        );
        // A STREAM ALONE: the bar of one lane with no length seeks nowhere.
        let live = lay(&[], |_| None, None, 40.0, Room::wide(80), 0, None);
        assert!(!live.seekable());
        assert_eq!(live.press(0, 3.0), (None, Act::Nothing));
        assert!(deck.seekable());
    }

    /// FAILS IF: the wheel over a lane that sounds stops turning that lane's
    /// own volume, or over the ruler stops turning the player's, the step
    /// the transport's wheel takes — or over a lane that plays no sound
    /// turns any volume at all, where it should scroll the lanes.
    #[test]
    fn the_wheel_turns_the_volume_under_it() {
        use ronoctua_player::wire::{Ask, LaneChange};
        let (lanes, lengths) = three();
        let deck = lay(&lanes, &lengths, Some(60.0), 0.0, Room::wide(80), 0, None);
        let step = crate::player::VOLUME_STEP;
        assert_eq!(
            deck.wheel(2, -1, 50),
            Some(Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Volume { percent: 60 + step }
            })
        );
        assert_eq!(
            deck.wheel(0, 2, 50),
            Some(Ask::Volume {
                percent: 50 - 2 * step
            })
        );
        for line in [1, 3] {
            assert_eq!(deck.wheel(line, 2, 50), None, "line {line} plays no sound");
        }
        assert_eq!(deck.wheel(0, 100, 50), Some(Ask::Volume { percent: 0 }));
    }

    /// FAILS IF: what a line of the deck WRITES is not where its press is
    /// taken — the glyph that switches a lane, its name, a mute — or a lane
    /// that plays no sound draws a mute it does not answer.
    #[test]
    fn the_words_are_written_where_they_are_pressed() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let s = playing_three();
        let deck = super::of_state(&s.lanes, Some(&s), Room::wide(80), 0);
        assert_eq!(deck.lines.len(), 4);
        for line in 0..deck.lines.len() {
            let words = deck.words(line, &s, &t, None, false);
            for w in &words {
                let middle = f64::from(w.at) + 0.5;
                let hit = deck.hit(line, middle);
                let glyph = w.text.chars().count() == 1;
                match (line, glyph) {
                    (0, true) => assert_eq!(hit, Some(Hit::Mute), "{w:?}"),
                    (0, false) => {}
                    (_, true) => assert!(
                        matches!(hit, Some(Hit::Toggle(_) | Hit::LaneMute(_))),
                        "line {line}: {w:?} answers {hit:?}"
                    ),
                    (_, false) => assert!(matches!(hit, Some(Hit::Name(_))), "{w:?}: {hit:?}"),
                }
            }
            if let Some(l) = deck.lane_line(line) {
                let mutes = words.iter().filter(|w| w.at == l.mute).count();
                assert_eq!(mutes, usize::from(l.sounds), "line {line}");
            }
        }
        // A LIT GLYPH is the lit key's tone, on its line alone — and never
        // the cursor's: in a room with no names the cursor is the glyph's
        // accent, and a lane under the pointer read as the one the keys are
        // on.
        let lit = deck.words(1, &s, &t, Some(super::Part::Toggle), false);
        assert_eq!(lit[0].color, tokens::color::TEXT_SUBTLE_LIT_KEY);
        assert_ne!(
            deck.words(2, &s, &t, None, false)[0].color,
            tokens::color::TEXT_SUBTLE_LIT_KEY
        );
        let narrow = super::of_state(&s.lanes, Some(&s), Room::narrow(40), 0);
        let cursor = narrow.words(1, &s, &t, None, true)[0].color;
        let pointed = narrow.words(1, &s, &t, Some(super::Part::Toggle), false)[0].color;
        assert_ne!(
            cursor, pointed,
            "the pointer's light is the cursor's colour"
        );
    }

    /// FAILS IF: a set's deck stops being its lanes and a file alone's the
    /// bar it always had — or where every lane is shown, one lane stops
    /// being a lane with its own line.
    #[test]
    fn a_set_is_a_deck_and_a_file_alone_is_its_bar() {
        use ronoctua_player::queue::{EntryId, Name, Node, NodeKind, Outline};
        let s = playing_three();
        let outline = |kind| Outline {
            revision: 1,
            entries: vec![Node {
                id: EntryId(1),
                name: Name::Titled("x".into()),
                kind,
                children: Vec::new(),
                lanes: 3,
            }],
        };
        assert_eq!(super::set_lanes(&s, Some(&outline(NodeKind::Set))).len(), 3);
        assert!(super::set_lanes(&s, Some(&outline(NodeKind::Single))).is_empty());
        assert!(super::set_lanes(&s, None).is_empty());
        let alone = super::of_state(super::set_lanes(&s, None), Some(&s), Room::wide(80), 0);
        assert_eq!(alone.lines.len(), 1, "a file alone is the bar");
        assert!((ruler(&alone).played.expect("played") - 1.0 / 3.0).abs() < 1e-6);
        // ONE LANE WHERE EVERY LANE IS SHOWN is a ruler and its line.
        let one = super::of_state(&s.lanes[..1], Some(&s), Room::wide(80).every_lane(), 0);
        assert_eq!(one.lines.len(), 2);
        assert!(one.playhead.is_some());
    }
}
