//! THE PICTURES OF WHAT PLAYS, ON A SCREEN: which of them a door shows, where
//! each one goes, and the Layout page (`player_page::PageId::Layout`) where a
//! person decides how.
//!
//! # Every door composes its own screen
//!
//! A set can send a film to one screen and another film to the other, and a
//! cover to both (`own::Lane::screens`). So no door shows "the pictures": the
//! window, the column and each screen's footer ask which pictures are ROUTED
//! TO THE SCREEN THEY ARE ON ([`tiles_on`]) — the window by the screen its
//! surface entered, the column by the screen its workspace is on, the footer
//! by each of its surfaces' own — and place only those, with the one
//! placement every door shares (`ronoctua_player::arrange::place`). A door
//! that does not know its screen yet shows every picture: one that vanished
//! because a compositor was slow to name an output would be a defect with no
//! symptom but an empty band.
//!
//! # A picture is a lane's
//!
//! Each picture a door shows is asked of the daemon BY ITS LANE
//! (`Ask::Preview { lane }`), and its texture is keyed by the lane and the
//! screen (`player:picture:lane-<id>:<connector>`, `Picture::key`): a lane
//! moved to another place of the order keeps its frames, and one screen's
//! texture is never another's. No door asks for more than
//! `session::PICTURES_MOST`, which is also what the renderer's ceiling counts
//! (`image::PLAYER_PICTURES`, bound to it below).
//!
//! # The Layout page
//!
//! The FORM is a ring — the seven of `own::Form`, each with its glyph — that
//! the arrows and `RET` turn. Under it a SCHEMATIC draws where each picture
//! goes in that form, a tile each, with no word on it; the tile of the
//! picture whose row the cursor is on is lit. Then a row per picture that can
//! be shown — every film that is on, every sound whose cover can be — in the
//! order they are placed: `RET` shows or hides it (a sound's cover is shown or
//! not), and the arrows move it earlier or later, as the `◀` and `▶` at the
//! end of its row do for a pointer (invariant 19). Every change is the
//! arrangement WHOLE (`Ask::Arrange`): the daemon keeps one, and a door never
//! sends half of it.

use crate::key;
use crate::player::{Errand, Target};
use crate::player_rows::Builder;
use ronoctua_player::arrange::{self, Air, Placement, Rect, Tile};
use ronoctua_player::own::{Arrangement, Form, Lane, LaneId, LaneKind};
use ronoctua_player::wire::{Ask, State, Trouble};
use ronoctua_tokens as tokens;

// THE RENDERER'S CEILING COUNTS THE PLAYER'S PICTURES by a number of its own
// — it knows no player — and a door draws up to `PICTURES_MOST` of them in one
// frame. Should the two ever part, the textures past the ceiling are dropped
// with no error anywhere (`image::CEILING`): this refuses to build instead.
const _: () = assert!(
    ronoctua_render::image::PLAYER_PICTURES >= ronoctua_player::session::PICTURES_MOST,
    "the renderer's ceiling counts fewer pictures than one set shows"
);

/// The most pictures a door shows at once.
pub const PICTURES_MOST: usize = ronoctua_player::session::PICTURES_MOST;

/// How what plays shows its pictures: the state's arrangement — or, with
/// none said (another machine's lanes, played in step), the one every set
/// starts with.
#[must_use]
pub fn arrangement_of(s: &State) -> Arrangement {
    s.arrangement.clone().unwrap_or_default()
}

/// The form the pictures of what plays share a screen in.
#[must_use]
pub fn form_of(s: &State) -> Form {
    s.arrangement.as_ref().map_or(Form::Auto, |a| a.form)
}

/// Whether lane `id` has a picture to show NOW: a film that is on, or a
/// sound's cover the person chose to show — the daemon's word
/// (`LaneState::picture`).
fn showing(s: &State, id: LaneId) -> bool {
    s.lane_states.iter().any(|l| l.lane == id && l.picture)
}

/// Lane `id`'s picture's width over its height — its size as the daemon says
/// it, else the shape a picture is given before its size is known.
#[must_use]
pub fn aspect_of(s: &State, id: LaneId) -> f32 {
    s.lane_states
        .iter()
        .find(|l| l.lane == id)
        .and_then(|l| l.size)
        .filter(|(w, h)| *w > 0 && *h > 0)
        .map_or(arrange::DEFAULT_ASPECT, |(w, h)| {
            #[allow(clippy::cast_precision_loss)]
            let shape = w as f32 / h as f32;
            shape
        })
}

/// THE PICTURES A DOOR ON `screen` SHOWS, in the arrangement's order: each
/// lane with a picture now, not hidden, routed to that screen — at most
/// [`PICTURES_MOST`]. `screen` is the door's connector; `None`, a door that
/// does not know its screen, shows every picture routed anywhere.
#[must_use]
pub fn tiles_on(s: &State, screen: Option<&str>) -> Vec<Tile> {
    let lanes: Vec<Lane> = s
        .lanes
        .iter()
        .filter(|l| showing(s, l.id))
        .cloned()
        .collect();
    let mut tiles = arrange::tiles(&lanes, &arrangement_of(s), screen, |id| {
        Some(aspect_of(s, id))
    });
    tiles.truncate(PICTURES_MOST);
    tiles
}

/// The air a door keeps between its pictures: `space.player.tile*`.
#[must_use]
pub fn air() -> Air {
    Air {
        gap_px: f32::from(tokens::space::PLAYER_TILEGAP),
        corner_pct: u8::try_from(tokens::space::PLAYER_CORNERPCT).unwrap_or(u8::MAX),
        floor_px: f32::from(tokens::space::PLAYER_TILEFLOOR),
    }
}

/// Where each of `tiles` goes in `area` — logical pixels of the door that
/// draws them — in the form of what plays: the ONE placement every door
/// draws with and takes a click by.
#[must_use]
pub fn placed(s: &State, tiles: &[Tile], area: Rect) -> Placement {
    arrange::place(form_of(s), tiles, area, air())
}

/// The screens among `screens` (connectors) that something of what plays is
/// routed to — where "show on every screen" opens a stage.
#[must_use]
pub fn screens_with_pictures<'a>(s: &State, screens: &'a [String]) -> Vec<&'a str> {
    screens
        .iter()
        .map(String::as_str)
        .filter(|c| !tiles_on(s, Some(c)).is_empty())
        .collect()
}

/// The pictures the Layout page arranges, in the order they are placed:
/// every film that is on — shown or hidden — and every sound that is on
/// whose cover could be shown.
#[must_use]
pub fn candidates(s: &State) -> Vec<&Lane> {
    let covered = |id: LaneId| s.lane_states.iter().any(|l| l.lane == id && l.cover);
    let found: Vec<Tile> = s
        .lanes
        .iter()
        .filter(|l| {
            l.enabled
                && match l.kind {
                    LaneKind::Video => true,
                    LaneKind::Audio => covered(l.id),
                    LaneKind::Subtitle => false,
                }
        })
        .map(|l| Tile {
            lane: l.id,
            aspect: aspect_of(s, l.id),
        })
        .collect();
    arrange::ordered(&arrangement_of(s), found)
        .into_iter()
        .filter_map(|t| s.lanes.iter().find(|l| l.id == t.lane))
        .collect()
}

/// The form's glyph.
#[must_use]
pub const fn form_glyph(form: Form) -> char {
    match form {
        Form::Auto => tokens::icon::PLAYER_FORM_AUTO,
        Form::Single => tokens::icon::PLAYER_FORM_SINGLE,
        Form::SideBySide => tokens::icon::PLAYER_FORM_SIDE,
        Form::Stacked => tokens::icon::PLAYER_FORM_STACKED,
        Form::Grid => tokens::icon::PLAYER_FORM_GRID,
        Form::Strip => tokens::icon::PLAYER_FORM_STRIP,
        Form::Corner => tokens::icon::PLAYER_FORM_CORNER,
    }
}

/// The form's name.
#[must_use]
pub const fn form_key(form: Form) -> &'static str {
    match form {
        Form::Auto => key::PLAYER_FORM_AUTO,
        Form::Single => key::PLAYER_FORM_SINGLE,
        Form::SideBySide => key::PLAYER_FORM_SIDE,
        Form::Stacked => key::PLAYER_FORM_STACKED,
        Form::Grid => key::PLAYER_FORM_GRID,
        Form::Strip => key::PLAYER_FORM_STRIP,
        Form::Corner => key::PLAYER_FORM_CORNER,
    }
}

/// What turning the ring of forms asks: the arrangement whole, with the next
/// form — or the one before.
#[must_use]
pub fn form_turned(s: &State, forward: bool) -> Ask {
    let mut arrangement = arrangement_of(s);
    let all = Form::ALL;
    let at = all.iter().position(|f| *f == arrangement.form).unwrap_or(0);
    let n = all.len();
    arrangement.form = all[if forward {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }];
    Ask::Arrange { arrangement }
}

/// What moving picture `id` one place later — or earlier — asks: the
/// arrangement whole, its order every picture the page arranges in their new
/// places. `None` at the end it would move past.
#[must_use]
pub fn moved(s: &State, id: LaneId, later: bool) -> Option<Ask> {
    let mut order: Vec<LaneId> = candidates(s).iter().map(|l| l.id).collect();
    let at = order.iter().position(|l| *l == id)?;
    let to = if later { at + 1 } else { at.checked_sub(1)? };
    if to >= order.len() {
        return None;
    }
    order.swap(at, to);
    let mut arrangement = arrangement_of(s);
    arrangement.order = order;
    Some(Ask::Arrange { arrangement })
}

/// What a picture's row says: shown or hidden — a sound's, its cover shown —
/// and, shown but LEFT OUT by this door (`left_out`), that there is no room
/// for it here. HIDDEN is the word a hidden subtitle lane says, one key for
/// one state on every page of the view (`labels-of-a-view-are-distinct`
/// refuses two keys of one word): `player-hidden`.
fn shown_word(
    lane: &Lane,
    arrangement: &Arrangement,
    left_out: bool,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let shown = match lane.kind {
        LaneKind::Audio => arrangement.covers.contains(&lane.id),
        LaneKind::Video => !arrangement.hidden.contains(&lane.id),
        LaneKind::Subtitle => false,
    };
    texts.t(match (lane.kind, shown) {
        (_, true) if left_out => key::PLAYER_LAYOUT_NO_ROOM,
        (LaneKind::Audio, true) => key::PLAYER_LAYOUT_COVER,
        (_, true) => key::PLAYER_LAYOUT_SHOWN,
        (_, false) => key::PLAYER_HIDDEN,
    })
}

/// The pictures a door on `screen` would show that it LEAVES OUT in a box of
/// `room` (logical pixels): the form places them smaller than a picture is
/// placed at (`space.player.tileFloor`), and `arrange::place` drops them —
/// what the page says, since nothing on the door's own pictures does.
#[must_use]
pub fn left_out(s: &State, screen: Option<&str>, room: Option<(f32, f32)>) -> Vec<LaneId> {
    let Some((width, height)) = room else {
        return Vec::new();
    };
    let tiles = tiles_on(s, screen);
    let placed = placed(
        s,
        &tiles,
        Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
    );
    tiles
        .iter()
        .map(|t| t.lane)
        .filter(|l| !placed.iter().any(|(p, _)| p == l))
        .collect()
}

/// The Layout page's rows, under its title — for a door on `screen` that
/// lays its pictures out in `room` (logical pixels), when it shows any.
pub fn page(s: &State, (screen, room): (Option<&str>, Option<(f32, f32)>), b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_LAYOUT);
    // ANOTHER MACHINE'S PICTURES are shown as that machine arranges them:
    // the daemon would refuse every row, so the page says it once.
    if s.arrangement.is_none()
        && let Some(j) = &s.joined
    {
        b.warning(&crate::player::trouble_sentence(
            &Trouble::Following {
                host: j.host.clone(),
            },
            b.texts,
        ));
        return;
    }
    let arrangement = arrangement_of(s);
    b.inert(crate::chrome::AIR);
    b.pair_with_mark(
        form_glyph(arrangement.form),
        b.t(key::PLAYER_LAYOUT_FORM),
        b.t(form_key(arrangement.form)),
        Target::LayoutForm,
        "layout-form",
    );
    b.inert(crate::chrome::AIR);
    b.schematic(Schematic::of(s, b.room, (screen, room)));
    b.heading(key::PLAYER_HEADING_LAYOUT_PICTURES);
    let dropped = left_out(s, screen, room);
    let lanes = candidates(s);
    if lanes.is_empty() {
        b.note(key::PLAYER_LAYOUT_NOTHING);
    }
    let last = lanes.len().saturating_sub(1);
    for (i, lane) in lanes.iter().enumerate() {
        let earlier = (i > 0).then_some((
            tokens::icon::PLAYER_LAYOUT_EARLIER,
            Target::LayoutMove {
                lane: lane.id,
                later: false,
            },
        ));
        let later = (i < last).then_some((
            tokens::icon::PLAYER_LAYOUT_LATER,
            Target::LayoutMove {
                lane: lane.id,
                later: true,
            },
        ));
        let name = crate::player_queue::lane_name(lane, &s.lanes, b.texts, b.room);
        let value = shown_word(lane, &arrangement, dropped.contains(&lane.id), b.texts);
        b.marked(crate::player_queue::lane_glyph(lane.kind), |b| {
            b.pair_with_controls(
                name,
                value,
                Target::LayoutPicture(lane.id),
                &format!("layout-picture-{}", lane.id.0),
                &[earlier, later],
            );
        });
    }
    // A PICTURE LEFT OUT HERE says why, under the pictures: a choice that
    // shows nothing in this door is one the page says nothing of otherwise.
    if !dropped.is_empty() {
        b.note(key::PLAYER_LAYOUT_LEFT_OUT);
    }
    // THE TILE OF THE ROW THE CURSOR IS ON, lit — known once its row is.
    b.light_schematic();
}

/// What a key does on a row of the Layout page — `None` for a row of
/// another page.
#[must_use]
pub fn turn(target: &Target, press: crate::player::Press, s: &State) -> Option<Errand> {
    use crate::player::Press;
    let asked = |ask: Option<Ask>| ask.map_or(Errand::Nothing, Errand::Tell);
    Some(match (target, press) {
        (Target::LayoutForm, Press::Less) => Errand::Tell(form_turned(s, false)),
        (Target::LayoutForm, Press::More | Press::Enter) => Errand::Tell(form_turned(s, true)),
        (Target::LayoutPicture(id), Press::Enter) => {
            asked(crate::player_lane::shown_turned(s, *id))
        }
        (Target::LayoutPicture(id), Press::Less | Press::More) => {
            asked(moved(s, *id, press == Press::More))
        }
        (Target::LayoutMove { lane, later }, Press::Enter) => asked(moved(s, *lane, *later)),
        (Target::LayoutForm | Target::LayoutPicture(_) | Target::LayoutMove { .. }, _) => {
            Errand::Nothing
        }
        _ => return None,
    })
}

/// THE SCHEMATIC of the Layout page: where each picture goes in the form of
/// what plays, drawn as tiles in a box the shape of the door's own pictures,
/// the tile of the row under the cursor lit — placed ONCE, when the rows are
/// built, by the placement the door shows the pictures with, its floor
/// included, so the drawing is what this door shows and never a picture of
/// something else: a picture it leaves out has no tile.
#[derive(Debug, Clone, PartialEq)]
pub struct Schematic {
    /// How wide the box is, in cells.
    pub cells: u16,
    /// How tall, in lines — `space.player.schematicLines`.
    pub lines: u16,
    /// The form, which says whether the tiles overlap (the corner's do).
    pub form: Form,
    /// Each picture's place, in FRACTIONS of the box (`x`, `y`, width,
    /// height), in the order they are drawn: the first under the others.
    pub places: Vec<(LaneId, [f32; 4])>,
    /// The picture whose row the cursor is on.
    pub lit: Option<LaneId>,
}

impl Schematic {
    /// The schematic of what `s` plays, at most `room` cells wide: a box of
    /// `space.player.schematicLines` — in the shape of `door`'s box (its
    /// screen, and the logical pixels it lays its pictures out in) when it
    /// shows pictures, else a screen's — and in it the pictures THAT door
    /// shows, placed as it places them in its own box, and scaled down. With
    /// no box known, every picture shown anywhere, with no floor.
    #[must_use]
    pub fn of(s: &State, room: u16, door: (Option<&str>, Option<(f32, f32)>)) -> Self {
        let lines = tokens::space::PLAYER_SCHEMATICLINES;
        let (cell, line) = (
            crate::chrome::cell_width(),
            tokens::metrics::line_height(tokens::text::size()),
        );
        let tall = f32::from(lines) * line;
        let arrangement = arrangement_of(s);
        // WHAT THE DOOR PLACES, in the door's own box — and, knowing none,
        // every picture in a screen's shape, dropping nothing.
        let (tiles, real, floor_px) = match door {
            (screen, Some((width, height))) if width > 0.0 && height > 0.0 => (
                tiles_on(s, screen),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                },
                air().floor_px,
            ),
            _ => {
                let lanes: Vec<Lane> = candidates(s).into_iter().cloned().collect();
                let tiles = arrange::tiles(&lanes, &arrangement, None, |id| Some(aspect_of(s, id)));
                let screen = Rect {
                    x: 0.0,
                    y: 0.0,
                    width: tall * arrange::DEFAULT_ASPECT,
                    height: tall,
                };
                (tiles, screen, 0.0)
            }
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let wanted = (tall * real.width / real.height.max(1.0) / cell.max(1.0)).ceil() as u16;
        let cells = wanted.min(room).max(1);
        let placed = Air { floor_px, ..air() };
        let places = arrange::place(arrangement.form, &tiles, real, placed)
            .into_iter()
            .map(|(id, r)| {
                (
                    id,
                    [
                        r.x / real.width,
                        r.y / real.height,
                        r.width / real.width,
                        r.height / real.height,
                    ],
                )
            })
            .collect();
        Self {
            cells,
            lines,
            form: arrangement.form,
            places,
            lit: None,
        }
    }

    /// Its tiles as a door paints them: the box's first cell at `x` and the
    /// top of its line `first` at `y` — logical pixels, a cell `cell` wide
    /// and a line `line` tall — and only `run` lines from `first` on screen,
    /// what is past them cut away. A tile laid ON another (the corner's) gets
    /// a ring of the ground under it first, the air `place` keeps round it.
    #[must_use]
    pub fn quads(
        &self,
        (x, y): (f32, f32),
        (first, run): (u16, u16),
        (cell, line): (f32, f32),
    ) -> Vec<ronoctua_render::quad::Quad> {
        let (wide, tall) = (f32::from(self.cells) * cell, f32::from(self.lines) * line);
        let top = y - f32::from(first) * line;
        let (from, to) = (y, y + f32::from(run) * line);
        let ring = f32::from(tokens::space::PLAYER_TILEGAP) / 2.0;
        let mut out = Vec::new();
        for (i, (lane, [fx, fy, fw, fh])) in self.places.iter().enumerate() {
            let rect = (x + fx * wide, top + fy * tall, fw * wide, fh * tall);
            if self.form == Form::Corner && i > 0 {
                let (rx, ry, rw, rh) = rect;
                out.extend(clipped(
                    ronoctua_render::quad::Quad {
                        x: rx - ring,
                        y: ry - ring,
                        width: rw + 2.0 * ring,
                        height: rh + 2.0 * ring,
                        color: tokens::color::SURFACE_CHROME,
                    },
                    from,
                    to,
                ));
            }
            out.extend(clipped(
                crate::chrome::tile(rect, self.lit == Some(*lane)),
                from,
                to,
            ));
        }
        out
    }
}

/// `q` with what is above `from` and below `to` cut away — nothing when none
/// of it is left.
fn clipped(
    mut q: ronoctua_render::quad::Quad,
    from: f32,
    to: f32,
) -> Option<ronoctua_render::quad::Quad> {
    let top = q.y.max(from);
    let bottom = (q.y + q.height).min(to);
    if bottom <= top || q.width <= 0.0 {
        return None;
    }
    q.y = top;
    q.height = bottom - top;
    Some(q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::fixture::routed;
    use crate::player::{PageId, Press, View};
    use ronoctua_player::own::Screens;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn ids(tiles: &[Tile]) -> Vec<u32> {
        tiles.iter().map(|t| t.lane.0).collect()
    }

    /// FAILS IF: a door shows a picture routed to another screen, stops
    /// showing one routed to its own or to every screen, shows a lane with no
    /// picture now (subtitles, a film not decoded yet), or a door that does
    /// not know its screen stops showing everything.
    #[test]
    fn only_lanes_routed_here_are_tiles() {
        let s = routed();
        assert_eq!(ids(&tiles_on(&s, Some("eDP-1"))), vec![1, 3]);
        assert_eq!(ids(&tiles_on(&s, Some("DP-2"))), vec![1, 2]);
        assert_eq!(ids(&tiles_on(&s, Some("HDMI-A-1"))), vec![1]);
        assert_eq!(ids(&tiles_on(&s, None)), vec![1, 2, 3]);
        // NO PICTURE YET, no tile: the band would stay black.
        let mut waiting = s.clone();
        waiting.lane_states.retain(|l| l.lane.0 != 1);
        assert_eq!(ids(&tiles_on(&waiting, Some("eDP-1"))), vec![3]);
        // THE SHAPE is the picture's own.
        let t = tiles_on(&s, Some("eDP-1"));
        assert!((t[0].aspect - 16.0 / 9.0).abs() < 1e-4);
        assert!((t[1].aspect - 1.0).abs() < 1e-4);
        // AND "EVERY SCREEN" opens only where something goes.
        let all = ["eDP-1".to_owned(), "DP-2".to_owned(), "HDMI-A-1".to_owned()];
        assert_eq!(
            screens_with_pictures(&s, &all),
            vec!["eDP-1", "DP-2", "HDMI-A-1"]
        );
        let mut only_two = s;
        only_two.lanes[0].screens = Screens::Only(vec!["DP-2".into()]);
        only_two.lanes[2].screens = Screens::Only(vec!["DP-2".into()]);
        assert_eq!(screens_with_pictures(&only_two, &all), vec!["DP-2"]);
    }

    /// FAILS IF: a door asks for more pictures than the renderer's ceiling
    /// counts.
    #[test]
    fn no_door_shows_more_than_the_ceiling() {
        let lanes: Vec<Lane> = (1..=9)
            .map(|i| Lane::new(LaneId(i), LaneKind::Video, format!("/films/{i}.mkv")))
            .collect();
        let (mut s, _) = crate::player::fixture::set(lanes, &[Some(60.0); 9], Some(60.0), 0.0);
        for st in &mut s.lane_states {
            st.picture = true;
        }
        assert_eq!(tiles_on(&s, None).len(), PICTURES_MOST);
    }

    /// Stands on the Layout page of `s`, the cursor on `target`.
    fn on_the_page(s: &State, target: &Target) -> View {
        let t = texts();
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::Layout), Some(s), 80, &t);
        assert_eq!(v.page(), PageId::Layout);
        let rows = v.rows(Some(s), 80, &t);
        let n = rows
            .iter()
            .position(|r| &r.target == target)
            .unwrap_or_else(|| panic!("no row for {target:?}"));
        v.clicked(n, &rows);
        v
    }

    fn arranged(errand: Errand) -> Arrangement {
        match errand {
            Errand::Tell(Ask::Arrange { arrangement }) => arrangement,
            other => panic!("asked {other:?}"),
        }
    }

    /// FAILS IF: the ring stops walking the seven forms both ways and round,
    /// a picture's row stops showing or hiding it — a sound's cover included
    /// — or stops moving it by the arrows AND by its two cells, or any of it
    /// sends half an arrangement.
    #[test]
    fn the_layout_page_turns_the_form_shows_and_moves_the_pictures() {
        let t = texts();
        let s = routed();
        let mut v = on_the_page(&s, &Target::LayoutForm);
        let a = arranged(v.key(Press::More, Some(&s), 80, &t));
        assert_eq!(a.form, Form::Stacked);
        assert_eq!(a.covers, vec![LaneId(3)], "the arrangement whole");
        assert_eq!(
            arranged(v.key(Press::Less, Some(&s), 80, &t)).form,
            Form::Single
        );
        let mut last = s.clone();
        last.arrangement.as_mut().expect("one").form = Form::Corner;
        assert_eq!(form_turned(&last, true), {
            let mut a = arrangement_of(&last);
            a.form = Form::Auto;
            Ask::Arrange { arrangement: a }
        });
        // A FILM: hidden, and moved later by `→`.
        let mut v = on_the_page(&s, &Target::LayoutPicture(LaneId(1)));
        assert_eq!(
            arranged(v.key(Press::Enter, Some(&s), 80, &t)).hidden,
            vec![LaneId(1)]
        );
        assert_eq!(
            arranged(v.key(Press::More, Some(&s), 80, &t)).order,
            vec![LaneId(2), LaneId(1), LaneId(3)]
        );
        // THE FIRST MOVES NO EARLIER.
        assert_eq!(v.key(Press::Less, Some(&s), 80, &t), Errand::Nothing);
        // A COVER: its row shows or hides the cover.
        let mut v = on_the_page(&s, &Target::LayoutPicture(LaneId(3)));
        assert!(
            arranged(v.key(Press::Enter, Some(&s), 80, &t))
                .covers
                .is_empty()
        );
        // AND THE POINTER'S TWO CELLS, the arrows' twins.
        let rows = v.rows(Some(&s), 80, &t);
        let row = rows
            .iter()
            .find(|r| r.target == Target::LayoutPicture(LaneId(2)))
            .expect("the second film's row");
        let cells: Vec<&Target> = row
            .cells
            .iter()
            .filter(|c| c.control)
            .map(|c| &c.target)
            .collect();
        assert_eq!(
            cells,
            vec![
                &Target::LayoutMove {
                    lane: LaneId(2),
                    later: false
                },
                &Target::LayoutMove {
                    lane: LaneId(2),
                    later: true
                }
            ]
        );
        let first = rows
            .iter()
            .find(|r| r.target == Target::LayoutPicture(LaneId(1)))
            .expect("the first film's row");
        assert_eq!(
            first.cells.iter().filter(|c| c.control).count(),
            1,
            "the first has no way earlier"
        );
        assert_eq!(
            turn(
                &Target::LayoutMove {
                    lane: LaneId(2),
                    later: false
                },
                Press::Enter,
                &s
            )
            .map(arranged)
            .map(|a| a.order),
            Some(vec![LaneId(2), LaneId(1), LaneId(3)])
        );
        // SUBTITLES are no picture to arrange.
        assert!(
            !rows
                .iter()
                .any(|r| r.target == Target::LayoutPicture(LaneId(4)))
        );
    }

    /// FAILS IF: a picture's `◀` or `▶` is drawn over its words or against
    /// them — in a door as narrow as the small column, where the value ran
    /// under the arrows — or the value is cut to make room for them.
    #[test]
    fn a_picture_s_arrows_keep_their_own_cells() {
        // NAMES AS LONG AS A FILM'S: the values' column goes as far right as
        // the rule lets it, and the arrows are there.
        let mut s = routed();
        for (lane, name) in s
            .lanes
            .iter_mut()
            .zip(["The Lighthouse Keeper", "Second angle"])
        {
            lane.title = Some(name.to_owned());
        }
        for (language, columns) in [
            ("pt-BR", 28),
            ("pt-BR", 30),
            ("pt-BR", 32),
            ("en-GB", 30),
            ("en-GB", 80),
        ] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let mut v = View::blank(None);
            v.key(Press::Go(PageId::Layout), Some(&s), columns, &t);
            for row in v.rows(Some(&s), columns, &t) {
                if !matches!(row.target, Target::LayoutPicture(_)) {
                    continue;
                }
                let Some(first) = row.cells.iter().filter(|c| c.control).map(|c| c.from).min()
                else {
                    continue;
                };
                for f in &row.fields {
                    let arrow = row.cells.iter().any(|c| c.control && c.from == f.at);
                    let ends = f.at + ronoctua_render::layout::width(&f.text);
                    assert!(
                        arrow || ends + tokens::space::PLAYER_QUEUEAIR <= first,
                        "{:?} runs under the arrows at {first}, or against them, in {columns} \
                         columns ({language})",
                        f.text
                    );
                    // AND THE VALUE IS WHOLE: the column moved for the arrows.
                    assert!(
                        !f.text.contains('…'),
                        "{:?} was cut for the arrows in {columns} columns ({language})",
                        f.text
                    );
                }
            }
        }
    }

    /// FAILS IF: the schematic stops drawing a tile for every picture shown,
    /// stops lighting the one under the cursor, or draws past the lines a
    /// door has on screen.
    #[test]
    fn the_schematic_draws_the_form_and_lights_the_cursor_s_picture() {
        let t = texts();
        let s = routed();
        let v = on_the_page(&s, &Target::LayoutPicture(LaneId(2)));
        let rows = v.rows(Some(&s), 80, &t);
        let lines: Vec<&crate::player_rows::SchematicRow> =
            rows.iter().filter_map(|r| r.schematic.as_ref()).collect();
        assert_eq!(
            lines.len(),
            usize::from(tokens::space::PLAYER_SCHEMATICLINES)
        );
        let figure = &lines[0].figure;
        assert_eq!(figure.places.len(), 3, "two films and a cover");
        assert_eq!(figure.lit, Some(LaneId(2)));
        // SIDE BY SIDE: one row of tiles, left to right in the order.
        let xs: Vec<f32> = figure.places.iter().map(|(_, p)| p[0]).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
        let (cell, line) = (10.0, 20.0);
        let quads = figure.quads((0.0, 100.0), (0, figure.lines), (cell, line));
        assert_eq!(quads.len(), 3);
        assert_eq!(
            quads
                .iter()
                .filter(|q| q.color == tokens::color::PLAYER_TILE_LIT)
                .count(),
            1
        );
        // TWO LINES OF IT ON SCREEN: nothing drawn past them.
        let cut = figure.quads((0.0, 140.0), (2, 2), (cell, line));
        assert!(
            cut.iter()
                .all(|q| q.y >= 140.0 - 1e-3 && q.y + q.height <= 180.0 + 1e-3),
            "{cut:?}"
        );
        // A PICTURE HIDDEN is no tile.
        let mut hidden = s;
        hidden.arrangement.as_mut().expect("one").hidden = vec![LaneId(1)];
        assert_eq!(Schematic::of(&hidden, 80, (None, None)).places.len(), 2);
    }

    /// FAILS IF: a picture a door LEAVES OUT — the form would place it
    /// smaller than a picture is placed at, in this door's box — is drawn on
    /// the Layout page's schematic as if it showed, or its row still says
    /// "shown" with no sentence saying why nothing shows it; or a door with
    /// room for every picture is told one is left out.
    #[test]
    fn a_picture_the_door_leaves_out_is_said_and_not_drawn() {
        let t = texts();
        let mut s = routed();
        s.arrangement.as_mut().expect("one").form = Form::Corner;
        // A SMALL COLUMN'S BOX: the corner's picture is a quarter of it,
        // under the floor.
        let small = Some((330.0, 186.0));
        assert_eq!(left_out(&s, Some("DP-2"), small), vec![LaneId(2)]);
        let wide = Some((1600.0, 900.0));
        assert!(left_out(&s, Some("DP-2"), wide).is_empty());
        let figure = Schematic::of(&s, 80, (Some("DP-2"), small));
        assert_eq!(
            figure.places.iter().map(|(l, _)| l.0).collect::<Vec<_>>(),
            vec![1],
            "a picture left out here has a tile on the drawing"
        );
        let mut v = View::blank(None);
        v.screens_are(Vec::new(), Some("DP-2".into()));
        v.pictures_room_is(small);
        v.key(Press::Go(PageId::Layout), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let said = |text: &str| rows.iter().any(|r| r.fields.iter().any(|f| f.text == text));
        assert!(
            said(&t.t(key::PLAYER_LAYOUT_NO_ROOM)),
            "its row says it has no room"
        );
        let sentence = t.t(key::PLAYER_LAYOUT_LEFT_OUT);
        assert!(
            rows.iter().any(|r| r
                .fields
                .iter()
                .any(|f| f.text.len() > 3 && sentence.starts_with(&f.text))),
            "nothing says why"
        );
        v.pictures_room_is(wide);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(!rows.iter().any(|r| {
            r.fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_LAYOUT_NO_ROOM))
        }));
    }
}
