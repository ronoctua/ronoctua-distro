//! THE GLOBE of the player's directories (plan P9): the countries of the
//! world radio and TV pages, a country's stations, where each one is.
//!
//! # The daemon places, the door groups and draws
//!
//! A directory's page arrives with its marks already placed
//! (`ronoctua_player::find::Globe`): a country at the point its name goes on
//! a map, a station that says where it is at its place, the others at their
//! country's. The view hands them to the shell's globe (`crate::globe`,
//! `Owner::Player`), whose thread groups the ones too close on the picture
//! and paints them; the view says which group is LIT — the one under the
//! pointer, or else the one the cursor's row stands for.
//!
//! # Where it is drawn
//!
//! In the window and the column, the first `space.player.globeLines` lines of
//! the page ([`Builder::globe`]), and under them a line saying where the
//! pointer is. In the footer, while its media panel stands beside the rows,
//! the panel's picture box — the film gives way while a directory's page is
//! open — and the line stays among the rows. Every door draws it where
//! [`box_of`] says and takes the pointer by [`picture_px`]: ONE geometry for
//! the paint and the hand.
//!
//! # One gesture, in every door
//!
//! Hovering a group lights it, and the line says where it is and how many it
//! holds. The first click brings the cursor to the first row it stands for;
//! the second does what `RET` does there ([`View::globe_clicked`]): one
//! station plays, a country opens its page, and several are framed and the
//! rows narrowed to them. A drag turns it and the wheel brings it nearer;
//! `C-k`, `C-j` and `C-r` do the same from the keyboard.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::globe::{Globe, MarkSet, Owner};
use crate::key;
use crate::player::{PageId, Target, View};
use crate::player_rows::{Builder, Row};
use ronoctua_atlas::{Placed, Point};
use ronoctua_broadcast::Code;
use ronoctua_broadcast::picks::Kind;
use ronoctua_i18n::Arg;
use ronoctua_player::find::{self, Desk, Mark, MarkKind, Page, Query, RadioList, TvList};
use ronoctua_player::wire::{DeskKind, State};
use ronoctua_render::image::Area;
use ronoctua_tokens as tokens;

/// THE VIEW'S GLOBE: the pose and the pictures (the shell's [`Globe`]), the
/// set of marks built from the page on screen, the group under the pointer,
/// and the subject it was last framed for.
///
/// INSIDE CELLS, as the file manager's globe is: a door measures and asks
/// for the picture while it paints, from a view it only reads.
#[derive(Debug)]
pub struct Seen {
    globe: RefCell<Globe>,
    /// The set of marks, and what it was built from — the page's query, its
    /// desk's generation and whether its rows are narrowed — so a paint that
    /// finds the same page does not rebuild thousands of marks.
    set: RefCell<Option<(u64, Arc<MarkSet>)>>,
    /// The group of marks under the pointer, as the door found it in the
    /// picture last drawn — and the subject that picture was of: a page
    /// changed under a resting pointer holds other marks by the same ids, and
    /// the old group would light and name one of them.
    under: Option<(String, Placed)>,
    /// The last click on the globe only brought the cursor to a row: the
    /// door keeps its scroll, or the globe at the top of the page would scroll
    /// out from under the hand before its second click.
    selected: std::cell::Cell<bool>,
    /// The subject the globe was last framed for: a page's marks are framed
    /// ONCE when they come — never again while the hand turns them.
    framed: RefCell<String>,
}

impl Default for Seen {
    fn default() -> Self {
        Self {
            globe: RefCell::new(Globe::new(Owner::Player)),
            set: RefCell::new(None),
            under: None,
            selected: std::cell::Cell::new(false),
            framed: RefCell::new(String::new()),
        }
    }
}

/// How a key turns the globe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// Nearer (`C-k`).
    Nearer,
    /// Further (`C-j`).
    Further,
    /// Back to how the page frames it: the whole world, or the country
    /// (`C-r`).
    Whole,
}

/// Whether `page` draws a globe: the world radio's and the world TV's, and a
/// list of stations or channels — each carries its marks. None without the
/// world data, which the page says instead ([`rows`]).
#[must_use]
pub const fn on(page: PageId) -> bool {
    matches!(
        page,
        PageId::Radio | PageId::Stations | PageId::Tv | PageId::TvChannels
    )
}

/// Whether the page on top draws a globe: a page that carries marks, but
/// never the list of what has NO place on the map — a globe there would show
/// nothing of it.
fn drawn_on(v: &View) -> bool {
    let top = v.top();
    on(top.page) && top.query != Some(Query::Tv(TvList::Placeless))
}

/// The page's marks — what the daemon placed for the list on screen — with
/// the desk they came on, while the page draws a globe and its list is here.
#[must_use]
pub fn of_page(v: &View) -> Option<(&Query, &Desk, &find::Globe)> {
    let top = v.top();
    if !drawn_on(v) {
        return None;
    }
    let query = top.query.as_ref()?;
    // A DIRECTORY ALLOWED NO MORE has no marks: its list is of before.
    let desk = v.allowed_listing(query)?;
    let globe = match &desk.page {
        Page::RadioCountries { globe, .. }
        | Page::Stations { globe, .. }
        | Page::TvCountries { globe, .. }
        | Page::Channels { globe, .. } => globe,
        _ => return None,
    };
    Some((query, desk, globe))
}

/// The mark a row stands for — a country's on a world page, a station's or
/// a channel's on a list — in the page on screen: what the cursor on that row
/// lights, and what a click on a group brings the cursor to.
#[must_use]
pub fn mark_of(v: &View, target: &Target) -> Option<u32> {
    let (_, desk, _) = of_page(v)?;
    match (target, &desk.page) {
        (
            Target::Opens(Query::Radio(RadioList::Country { country, .. })),
            Page::RadioCountries { countries, .. },
        )
        | (
            Target::Opens(Query::Tv(TvList::Country { country })),
            Page::TvCountries { countries, .. },
        ) => countries.iter().find(|r| r.country == *country)?.mark,
        (Target::Pick(Kind::Radio, id), Page::Stations { stations, .. }) => {
            stations.iter().find(|r| r.station.uuid == *id)?.mark
        }
        (Target::Pick(Kind::Tv, id), Page::Channels { channels, .. }) => {
            channels.iter().find(|r| r.id == *id)?.mark
        }
        _ => None,
    }
}

/// Whether a row that stands for mark `mark` is drawn: every one, unless a
/// click on a group narrowed the rows to its members ([`View::globe_clicked`]).
#[must_use]
pub fn kept(only: Option<&[u32]>, mark: Option<u32>) -> bool {
    only.is_none_or(|members| mark.is_some_and(|m| members.contains(&m)))
}

/// A country by its name in the person's language — from the world data, the
/// names the globe paints with — and by its code, as data, where the
/// machine has none.
#[must_use]
pub fn country_name(code: Code, texts: &ronoctua_i18n::Texts) -> String {
    crate::globe::world()
        .and_then(|atlas| ronoctua_atlas::country_name(&atlas, code.0, crate::globe::tongue(texts)))
        .unwrap_or_else(|| code.as_str().to_owned())
}

/// How many a count of this desk's is, in words: stations, or channels.
#[must_use]
pub fn counted(kind: DeskKind, count: u32, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        if kind == DeskKind::Radio {
            key::PLAYER_STATIONS_COUNT
        } else {
            key::PLAYER_CHANNELS_COUNT
        },
        &[("count", Arg::Number(i64::from(count)))],
    )
}

/// THE GLOBE'S ROWS, at the top of a page that draws it: its lines — the
/// doors draw the picture over them — and the line that says where the
/// pointer is. In the footer, while the media panel carries the picture,
/// only that line. Without the world data, a sentence saying so.
pub fn rows(v: &View, s: &State, b: &mut Builder<'_>) {
    if !drawn_on(v) {
        return;
    }
    if !crate::globe::present() {
        b.note(key::ATLAS_ABSENT);
        return;
    }
    if !v.panel {
        b.globe(tokens::space::PLAYER_GLOBELINES);
    }
    // ONE LINE, WHATEVER IT SAYS: the rows under it do not move as the hand
    // crosses the world. HOW MANY comes first, and the place's name after
    // it is what a narrow door cuts.
    let said = v.place_said(s, b.texts);
    let line = ronoctua_render::layout::elide_end(&said, b.room);
    b.inert(vec![Builder::piece(
        b.left,
        line,
        tokens::color::TEXT_SECONDARY,
    )]);
}

/// WHERE THE GLOBE IS DRAWN among the rows on screen: its box in the
/// surface's logical pixels — its first cell at `x0`, the first of its lines
/// shown at height `y`, `run` of them — and how many pixels of its top the
/// scroll took. The picture is painted in it (`sizes::picture_in`) and the
/// pointer read back through the same box ([`picture_px`]).
#[must_use]
pub fn box_of(
    g: &crate::player_rows::GlobeRow,
    run: usize,
    (x0, y): (f32, f32),
    (cell, line): (f32, f32),
) -> (Area, u32) {
    let run = u16::try_from(run).unwrap_or(u16::MAX).min(g.lines);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let gone = (f32::from(g.line) * line) as u32;
    (
        Area {
            x: x0 + f32::from(g.left) * cell,
            y,
            width: f32::from(g.cells) * cell,
            height: f32::from(run) * line,
        },
        gone,
    )
}

/// The point `at` of the surface in the globe's own picture — drawn at
/// `drawn` and `size` pixels large — when it is on it: what a group of marks
/// is found at ([`Globe::mark_at`]), in the numbers it was painted from.
#[must_use]
pub fn picture_px(drawn: Area, size: (u32, u32), at: (f32, f32)) -> Option<(f32, f32)> {
    let inside = (drawn.x..drawn.x + drawn.width).contains(&at.0)
        && (drawn.y..drawn.y + drawn.height).contains(&at.1);
    if !inside || drawn.width <= 0.0 || drawn.height <= 0.0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let (sx, sy) = (size.0 as f32 / drawn.width, size.1 as f32 / drawn.height);
    Some(((at.0 - drawn.x) * sx, (at.1 - drawn.y) * sy))
}

impl View {
    /// The subject of the globe on screen — the page's list and the rows it
    /// is narrowed to — which names its picture: a new subject is a picture
    /// framed anew. `None` on a page with no globe.
    fn globe_subject(&self) -> Option<String> {
        let top = self.top();
        if !drawn_on(self) {
            return None;
        }
        let query = top
            .query
            .as_ref()
            .and_then(|q| serde_json::to_string(q).ok())
            .unwrap_or_default();
        Some(match &top.only {
            Some(members) => format!("{query}:{members:?}"),
            None => query,
        })
    }

    /// The marks of the page on screen as the globe draws them, built once
    /// per page, per answer of its desk and per narrowing — an empty set while
    /// the list is not here, so the world is drawn meanwhile.
    fn globe_set(&self, subject: &str) -> Arc<MarkSet> {
        let page = of_page(self);
        let mut hasher = std::hash::DefaultHasher::new();
        subject.hash(&mut hasher);
        page.map(|(_, d, _)| d.generation).hash(&mut hasher);
        let generation = hasher.finish();
        let mut set = self.globe.set.borrow_mut();
        if let Some((g, s)) = set.as_ref()
            && *g == generation
        {
            return s.clone();
        }
        let only = self.top().only.as_deref();
        let all = page
            .map(|(_, _, globe)| {
                globe
                    .marks
                    .iter()
                    .filter(|m| kept(only, Some(m.id)))
                    .map(|m| ronoctua_atlas::Mark {
                        at: Point {
                            lat: m.at.lat,
                            lon: m.at.lon,
                        },
                        id: m.id,
                        weight: m.weight,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let fresh = Arc::new(MarkSet { generation, all });
        *set = Some((generation, fresh.clone()));
        fresh
    }

    /// The mark lit now: the first of the group under the pointer, else the
    /// one the cursor's row stands for.
    fn globe_lit(&self, rows: &[Row]) -> Option<u32> {
        if let Some(under) = self.under_now() {
            return under.members.first().copied();
        }
        let target = rows.get(self.cursor()).map(|r| &r.target)?;
        mark_of(self, target)
    }

    /// THE BOX THE GLOBE IS DRAWN IN, told by the door as it paints: the
    /// picture's size and how much of its top the scroll took, in its theme
    /// and the person's language — and with it the page's marks, the lit
    /// one, and, the first time a page's marks are here, the framing the
    /// page asks for: a country's list framed on its marks, the rows narrowed
    /// framed on theirs, the world whole.
    pub fn globe_measured(
        &self,
        rows: &[Row],
        size: (u32, u32),
        from: u32,
        theme: usize,
        tongue: ronoctua_atlas::Tongue,
    ) {
        let portuguese = tongue == ronoctua_atlas::Tongue::Portuguese;
        let Some(subject) = self.globe_subject() else {
            return;
        };
        let set = self.globe_set(&subject);
        let lit = self.globe_lit(rows);
        let mut g = self.globe.globe.borrow_mut();
        g.looking_at(None, &subject, size, from, theme, portuguese);
        g.showing(Some(set.clone()));
        g.light(lit);
        let mut framed = self.globe.framed.borrow_mut();
        if *framed != subject && self.frame_as_the_page_does(&mut g, &set) {
            *framed = subject;
        }
    }

    /// The globe framed as the page asks — a list's marks (a country's
    /// stations, a genre's, a search's, the rows' group), or the whole world
    /// on a world page — `true` once it could be: marks not here yet are
    /// framed when they come.
    fn frame_as_the_page_does(&self, g: &mut Globe, set: &MarkSet) -> bool {
        let top = self.top();
        let list = matches!(top.page, PageId::Stations | PageId::TvChannels);
        if top.only.is_some() || list {
            let members: Vec<u32> = set.all.iter().map(|m| m.id).collect();
            return g.frame_on(&members);
        }
        g.whole_world();
        true
    }

    /// What the globe wants drawn, when it is not what was last asked for —
    /// the door hands it to its thread, or draws it in place.
    pub fn wants_globe(&self) -> Option<crate::globe::Ask> {
        self.globe.globe.borrow_mut().wants()
    }

    /// A picture, or a place, coming back from the globe's thread.
    pub fn saw_globe(&self, beat: &crate::globe::Beat) -> bool {
        self.globe.globe.borrow_mut().saw(beat)
    }

    /// The globe drawn HERE, by a door with no thread — a photograph's, a
    /// test's: the same painting the thread does (`Globe::drawn_here`).
    pub fn globe_drawn_here(&self) {
        self.globe.globe.borrow_mut().drawn_here();
    }

    /// The globe's picture and the key it goes up under, when one came back
    /// for the page on screen.
    #[must_use]
    pub fn globe_picture(&self) -> Option<(String, ronoctua_render::image::Rgba)> {
        let g = self.globe.globe.borrow();
        Some((g.key()?, g.frame()?.clone()))
    }

    /// Whether the page on screen draws a globe — the doors paint one and
    /// take the pointer on it only then.
    #[must_use]
    pub fn globe_here(&self) -> bool {
        drawn_on(self) && crate::globe::present()
    }

    /// Every group of marks the last picture drew — its members and its
    /// centre in the picture's pixels.
    #[must_use]
    pub fn globe_groups(&self) -> Vec<(Vec<u32>, (f32, f32))> {
        self.globe
            .globe
            .borrow()
            .marks()
            .iter()
            .map(|p| (p.members.clone(), p.at))
            .collect()
    }

    /// The pointer at `px` of the globe's picture, or off it (`None`): the
    /// group under it lit, and the line saying where it is. `true` when that
    /// changed what is drawn.
    pub fn globe_pointed(&mut self, px: Option<(f32, f32)>) -> bool {
        let under = px.and_then(|p| self.globe.globe.borrow().mark_at(p).cloned());
        let changed = under.as_ref().map(|u| &u.members) != self.under_now().map(|u| &u.members);
        self.globe.under = under
            .zip(self.globe_subject())
            .map(|(u, subject)| (subject, u));
        changed
    }

    /// The group under the pointer, when it was found in a picture of the
    /// page on screen — none once the page changed under a pointer at rest.
    fn under_now(&self) -> Option<&Placed> {
        let (subject, under) = self.globe.under.as_ref()?;
        (Some(subject) == self.globe_subject().as_ref()).then_some(under)
    }

    /// Whether the last click on the globe only brought the cursor to a row
    /// — the door keeps its scroll then ([`View::globe_clicked`]).
    #[must_use]
    pub fn globe_keeps_the_scroll(&self) -> bool {
        self.globe.selected.get()
    }

    /// The globe dragged by `(right, down)` pixels: turned under the hand.
    pub fn globe_panned(&self, right: f32, down: f32) -> bool {
        self.globe.globe.borrow_mut().pan(right, down)
    }

    /// The wheel over the globe, `notches` towards the person: nearer, and
    /// further the other way.
    pub fn globe_wheeled(&self, notches: i64) -> bool {
        let by = crate::globe::WHEEL_ZOOM.powi(-i32::try_from(notches).unwrap_or(0));
        self.globe.globe.borrow_mut().zoom(by)
    }

    /// A key that turns the globe: nearer, further, or back to how the page
    /// frames it.
    pub fn globe_turned(&self, turn: Turn) -> bool {
        let mut g = self.globe.globe.borrow_mut();
        match turn {
            Turn::Nearer => g.zoom(crate::globe::KEY_ZOOM),
            Turn::Further => g.zoom(1.0 / crate::globe::KEY_ZOOM),
            Turn::Whole => {
                let set = self.globe.set.borrow().as_ref().map(|(_, s)| s.clone());
                set.is_some_and(|set| self.frame_as_the_page_does(&mut g, &set))
            }
        }
    }

    /// A CLICK ON THE GLOBE at `px` of its picture: the first on a group
    /// brings the cursor to the first row it stands for; the second — the
    /// cursor's row one of its own — does what `RET` does there: one station
    /// or channel plays, a country opens its page, and several are framed
    /// and the rows narrowed to them. Nothing under the click: nothing.
    pub fn globe_clicked(
        &mut self,
        px: (f32, f32),
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> crate::player::Errand {
        use crate::player::Errand;
        self.globe.selected.set(false);
        let Some(group) = self.globe.globe.borrow().mark_at(px).cloned() else {
            return Errand::Nothing;
        };
        let rows = self.rows(state, columns, texts);
        let on_it = |r: &Row| mark_of(self, &r.target).is_some_and(|m| group.members.contains(&m));
        let here = rows
            .get(self.cursor())
            .is_some_and(|r| r.stops() && on_it(r));
        if !here
            && let Some(n) = group.members.iter().find_map(|id| {
                rows.iter()
                    .position(|r| r.stops() && mark_of(self, &r.target) == Some(*id))
            })
        {
            let top = self.top_mut();
            top.cursor = n;
            top.cell = 0;
            top.moved = true;
            // THE SCROLL STAYS: the globe is at the top of the page, and a
            // country far down the list scrolled it away before the second
            // click could land.
            self.globe.selected.set(true);
            return Errand::Nothing;
        }
        self.group_acts(&group, &rows, state, columns, texts)
    }

    /// What the second click on `group` does ([`View::globe_clicked`]).
    fn group_acts(
        &mut self,
        group: &Placed,
        rows: &[Row],
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> crate::player::Errand {
        use crate::player::{Errand, Press};
        let stood: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.stops() && mark_of(self, &r.target).is_some_and(|m| group.members.contains(&m))
            })
            .map(|(i, _)| i)
            .collect();
        // ONE ROW — one country, one station — is that row's `RET`.
        if let [only] = stood.as_slice() {
            self.top_mut().cursor = *only;
            return self.key(Press::Enter, state, columns, texts);
        }
        // SEVERAL: framed, and the rows narrowed to them.
        if stood.is_empty() {
            return Errand::Nothing;
        }
        let top = self.top_mut();
        top.only = Some(group.members.clone());
        top.cursor = 0;
        top.moved = false;
        self.globe.under = None;
        self.settle_in(state, columns, texts);
        // THE CURSOR ON THE FIRST OF THEM, not on the row that gives the
        // others back.
        let rows = self.rows(state, columns, texts);
        if let Some(n) = rows.iter().position(|r| {
            r.stops() && mark_of(self, &r.target).is_some_and(|m| group.members.contains(&m))
        }) {
            let top = self.top_mut();
            top.cursor = n;
            top.moved = true;
        }
        Errand::Nothing
    }

    /// THE LINE UNDER THE GLOBE: where the group under the pointer is — else
    /// the one the cursor's row stands for — and how many it holds. Empty
    /// with neither.
    pub fn place_said(&self, s: &State, texts: &ronoctua_i18n::Texts) -> String {
        let _ = s;
        let Some((_, desk, globe)) = of_page(self) else {
            return String::new();
        };
        let g = self.globe.globe.borrow();
        // THE GROUP UNDER THE POINTER, else the lit one — the cursor's row's.
        let group: Option<Placed> = self.under_now().cloned().or_else(|| {
            let lit = g.lit()?;
            g.marks().iter().find(|p| p.members.contains(&lit)).cloned()
        });
        let Some(group) = group else {
            return String::new();
        };
        let marks: Vec<&Mark> = group
            .members
            .iter()
            .filter_map(|id| globe.marks.iter().find(|m| m.id == *id))
            .collect();
        let count = counted(desk.kind, group.weight, texts);
        let place = match marks.as_slice() {
            [one] => match one.kind {
                MarkKind::Country => one
                    .country
                    .map_or_else(|| one.label.clone(), |c| country_name(c, texts)),
                MarkKind::Station | MarkKind::Place => {
                    let country = one.country.map(|c| country_name(c, texts));
                    match country {
                        Some(c) if one.weight <= 1 => {
                            return texts.t_with(
                                key::PLAYER_GLOBE_ONE,
                                &[("name", &one.label), ("country", &c)],
                            );
                        }
                        _ => one.label.clone(),
                    }
                }
            },
            _ => g
                .lit_place()
                .map(|a| crate::globe::said(a, texts))
                .unwrap_or_default(),
        };
        if place.is_empty() {
            return count;
        }
        texts.t_with(
            key::PLAYER_GLOBE_PLACE,
            &[("place", &place), ("count", &count)],
        )
    }
}

/// The pose a page of the world is born at — for a test that asks how far
/// a key moved it.
#[cfg(test)]
pub fn pose_of(v: &View) -> ronoctua_atlas::Pose {
    v.globe.globe.borrow().pose()
}

/// The world's countries a test hands a view — for the tests of this crate and above.
#[cfg(any(test, feature = "test-support"))]
pub mod fixture {
    use super::*;
    use ronoctua_broadcast::Spot;
    use ronoctua_player::find::{CountryRow, Globe as Marks, Listed};

    /// The country `c` is, by its code.
    pub fn code(c: &str) -> Code {
        ronoctua_broadcast::country::normal(c).expect("a country")
    }

    /// A mark of the globe, of `kind`, at `lat`/`lon`, in country `c`.
    pub fn mark(
        id: u32,
        lat: f32,
        lon: f32,
        weight: u32,
        kind: MarkKind,
        label: &str,
        c: &str,
    ) -> Mark {
        Mark {
            id,
            at: Spot { lat, lon },
            weight,
            kind,
            label: label.to_owned(),
            country: Some(code(c)),
        }
    }

    /// The world radio's countries: Brazil, Portugal, Spain — the last two
    /// close enough to be one group from far away.
    pub fn countries_desk() -> Desk {
        let rows = [("BR", 40, 0), ("PT", 12, 1), ("ES", 9, 2)];
        Desk {
            kind: DeskKind::Radio,
            generation: 3,
            busy: false,
            trouble: None,
            query: Some(Query::Radio(RadioList::Countries)),
            asked: None,
            refused: Vec::new(),
            page: Page::RadioCountries {
                countries: rows
                    .iter()
                    .map(|(c, n, m)| CountryRow {
                        country: code(c),
                        count: *n,
                        mark: Some(*m),
                    })
                    .collect(),
                globe: Marks {
                    marks: vec![
                        mark(0, -10.8, -53.1, 40, MarkKind::Country, "BR", "BR"),
                        mark(1, 39.6, -8.0, 12, MarkKind::Country, "PT", "PT"),
                        mark(2, 40.2, -3.6, 9, MarkKind::Country, "ES", "ES"),
                    ],
                    placeless: 2,
                },
                listed: Listed {
                    fetched_unix_seconds: Some(1_790_000_000),
                    stale: false,
                },
            },
            taken: None,
        }
    }

    /// The world TV's countries: the radio's, on the TV's desk.
    pub fn tv_countries_desk() -> Desk {
        let radio = countries_desk();
        let Page::RadioCountries {
            countries,
            globe,
            listed,
        } = radio.page
        else {
            unreachable!("the radio's countries");
        };
        Desk {
            kind: DeskKind::Television,
            query: Some(Query::Tv(TvList::Countries)),
            page: Page::TvCountries {
                countries,
                globe,
                listed,
            },
            ..radio
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) use super::fixture::{code, countries_desk, mark};
    use super::*;
    use crate::player::fixture::idle;
    use crate::player::{Errand, Press};
    use ronoctua_broadcast::Spot;
    use ronoctua_player::find::{Globe as Marks, Listed, StationRow};
    use ronoctua_player::wire::{Ask, DeskSummary, Thing};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// Brazil's stations: two in one city, one in another, and one that says
    /// nowhere — on the country's anchor.
    pub(crate) fn stations_desk() -> Desk {
        let station = |uuid: &str, name: &str, at: Option<Spot>, m: u32| {
            let s = ronoctua_broadcast::radio::Station {
                uuid: uuid.to_owned(),
                name: name.to_owned(),
                url: format!("https://radio.example/{uuid}"),
                alternate: None,
                homepage: None,
                country: Some(code("BR")),
                tags: vec!["jazz".into()],
                languages: vec!["portuguese".into()],
                codec: Some("MP3".into()),
                bitrate_kbps: 128,
                hls: false,
                votes: 1,
                clicks: 1,
                at,
            };
            StationRow {
                stream: s.stream(),
                station: s,
                mark: Some(m),
            }
        };
        let sp = Spot {
            lat: -23.55,
            lon: -46.63,
        };
        Desk {
            kind: DeskKind::Radio,
            generation: 7,
            busy: false,
            trouble: None,
            query: Some(Query::Radio(RadioList::Country {
                country: code("BR"),
                page: 1,
            })),
            asked: None,
            refused: Vec::new(),
            page: Page::Stations {
                stations: vec![
                    station("a", "Rádio Paulista", Some(sp), 0),
                    station("b", "Jazz SP", Some(sp), 1),
                    station(
                        "c",
                        "Frevo FM",
                        Some(Spot {
                            lat: -8.05,
                            lon: -34.9,
                        }),
                        2,
                    ),
                    station("d", "Somewhere FM", None, 3),
                ],
                globe: Marks {
                    marks: vec![
                        mark(
                            0,
                            -23.55,
                            -46.63,
                            1,
                            MarkKind::Station,
                            "Rádio Paulista",
                            "BR",
                        ),
                        mark(1, -23.56, -46.64, 1, MarkKind::Station, "Jazz SP", "BR"),
                        mark(2, -8.05, -34.9, 1, MarkKind::Station, "Frevo FM", "BR"),
                        mark(3, -10.8, -53.1, 1, MarkKind::Country, "BR", "BR"),
                    ],
                    placeless: 0,
                },
                listed: Listed::default(),
            },
            taken: None,
        }
    }

    /// A player that answers, its radio desk at `generation`.
    pub(crate) fn with_radio(generation: u64) -> State {
        let mut s = idle();
        s.desks = vec![DeskSummary {
            kind: DeskKind::Radio,
            generation,
            busy: false,
            trouble: None,
        }];
        s
    }

    /// A view on world radio, its countries here, the directory allowed.
    fn on_world_radio(s: &State) -> View {
        let t = texts();
        let mut v = View::blank(None);
        v.prefs.let_directory(find::Directory::Radio);
        v.found.given(countries_desk());
        v.key(Press::Go(PageId::Radio), Some(s), 80, &t);
        v
    }

    /// The globe measured and drawn as a door with no thread draws it,
    /// `(320, 240)` large.
    fn drawn(v: &View, s: &State) {
        let t = texts();
        let rows = v.rows(Some(s), 80, &t);
        v.globe_measured(&rows, (320, 240), 0, 0, ronoctua_atlas::Tongue::English);
        v.globe_drawn_here();
    }

    /// FAILS IF: a group of marks is grabbed anywhere but where the door drew
    /// it — the row's box and the picture's pixels are one geometry — or the
    /// globe's lines are not the rows the box is made of.
    #[test]
    fn a_mark_is_grabbed_where_it_is_drawn() {
        if !crate::globe::present() {
            return;
        }
        let t = texts();
        let s = with_radio(3);
        let v = on_world_radio(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let lines: Vec<&crate::player_rows::GlobeRow> =
            rows.iter().filter_map(|r| r.globe.as_ref()).collect();
        assert_eq!(
            lines.len(),
            usize::from(tokens::space::PLAYER_GLOBELINES),
            "the globe's lines"
        );
        // THE BOX a door draws it in, all its lines on screen, from line 2.
        let (cell, line) = (9.0, 20.0);
        let (area, gone) = box_of(lines[0], lines.len(), (4.0, 40.0), (cell, line));
        assert_eq!(gone, 0);
        assert_eq!(area.height, line * lines.len() as f32);
        let size = crate::fields::globe_texture(area);
        v.globe_measured(&rows, size, gone, 0, ronoctua_atlas::Tongue::English);
        v.globe_drawn_here();
        let (_, rgba) = v.globe_picture().expect("a picture");
        let drawn = crate::sizes::picture_in(area, rgba.width, rgba.height);
        let g = v.globe.globe.borrow();
        let groups = g.marks().to_vec();
        drop(g);
        assert!(!groups.is_empty(), "no mark was placed");
        for p in &groups {
            // THE SURFACE'S POINT where the group was painted is found back
            // as that group, through the same box.
            let at = (drawn.x + p.at.0, drawn.y + p.at.1);
            let px = picture_px(drawn, (rgba.width, rgba.height), at).expect("on the globe");
            let found = v
                .globe
                .globe
                .borrow()
                .mark_at(px)
                .map(|g| g.members.clone());
            assert_eq!(found.as_ref(), Some(&p.members));
        }
        // AND OFF THE PICTURE, nothing.
        assert!(picture_px(drawn, (rgba.width, rgba.height), (0.0, 0.0)).is_none());
    }

    /// FAILS IF: the first click on a group does anything but bring the cursor
    /// to its row — or the second, on the same group, does not do what `RET`
    /// on that row does: a country opens its page, and asks for its list.
    #[test]
    fn a_first_click_selects_and_the_second_opens_the_country() {
        if !crate::globe::present() {
            return;
        }
        let t = texts();
        let s = with_radio(3);
        let mut v = on_world_radio(&s);
        drawn(&v, &s);
        let brazil = v
            .globe
            .globe
            .borrow()
            .marks()
            .iter()
            .find(|p| p.members == vec![0])
            .map(|p| p.at)
            .expect("Brazil's mark");
        assert_eq!(v.globe_clicked(brazil, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Radio, "the first click opened something");
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Target::Opens(Query::Radio(RadioList::Country {
                country: code("BR"),
                page: 1,
            })),
            "the cursor is not on Brazil's row"
        );
        assert!(
            v.globe_keeps_the_scroll(),
            "the first click scrolls the globe away from the hand"
        );
        // THE CURSOR'S ROW LIGHTS ITS MARK.
        drawn(&v, &s);
        assert_eq!(v.globe.globe.borrow().lit(), Some(0));
        // THE POINTER RESTS ON BRAZIL as the second click opens it: on the
        // country's page its group is not the one under the pointer — ids
        // start again on every page.
        assert!(v.globe_pointed(Some(brazil)));
        assert!(v.under_now().is_some());
        let second = v.globe_clicked(brazil, Some(&s), 80, &t);
        assert!(
            !v.globe_keeps_the_scroll(),
            "a page that opened keeps the scroll"
        );
        assert!(
            v.under_now().is_none(),
            "the world page's group lights on the country's page"
        );
        assert_eq!(v.page(), PageId::Stations);
        assert_eq!(
            second,
            Errand::Tell(Ask::Find {
                query: Query::Radio(RadioList::Country {
                    country: code("BR"),
                    page: 1,
                })
            }),
            "the country's page did not ask for its stations"
        );
    }

    /// FAILS IF: the cursor on a row does not light the mark that row stands
    /// for, or the pointer over a group does not light it in its place; or a
    /// group of several stations, clicked twice, plays one of them instead of
    /// narrowing the rows to its members; or a single station, clicked
    /// twice, does not play.
    #[test]
    fn the_list_and_the_marks_light_each_other_and_a_group_narrows_the_rows() {
        if !crate::globe::present() {
            return;
        }
        let t = texts();
        let s = with_radio(7);
        let mut v = View::blank(None);
        v.prefs.let_directory(find::Directory::Radio);
        v.push_listing(
            PageId::Stations,
            Query::Radio(RadioList::Country {
                country: code("BR"),
                page: 1,
            }),
            None,
        );
        // THE LIST IT ASKS AS IT OPENS, sent (a door's key would carry it).
        let _ = v.owing(Errand::Nothing);
        v.found.given(stations_desk());
        v.settle_in(Some(&s), 80, &t);
        drawn(&v, &s);
        // THE CURSOR on Frevo FM lights mark 2.
        let rows = v.rows(Some(&s), 80, &t);
        let frevo = rows
            .iter()
            .position(|r| r.target == Target::Pick(Kind::Radio, "c".into()))
            .expect("Frevo's row");
        v.top_mut().cursor = frevo;
        drawn(&v, &s);
        assert_eq!(v.globe.globe.borrow().lit(), Some(2));
        let groups = v.globe.globe.borrow().marks().to_vec();
        // THE POINTER over the other group lights it, and says where.
        let city = groups
            .iter()
            .find(|p| p.members.contains(&0))
            .expect("the city's group")
            .clone();
        assert!(v.globe_pointed(Some(city.at)));
        drawn(&v, &s);
        assert!(
            city.members
                .contains(&v.globe.globe.borrow().lit().expect("lit"))
        );
        assert!(!v.place_said(&s, &t).is_empty(), "the line says nothing");
        v.globe_pointed(None);
        // THE CITY'S TWO STATIONS, clicked twice: narrowed to them — two
        // stations a hundredth of a degree apart are one group on a picture
        // of a whole country.
        assert!(
            city.members.len() > 1,
            "the city's two stations are not one group"
        );
        {
            v.globe_clicked(city.at, Some(&s), 80, &t);
            let second = v.globe_clicked(city.at, Some(&s), 80, &t);
            assert_eq!(second, Errand::Nothing, "a group of several played one");
            let names: Vec<String> = v
                .rows(Some(&s), 80, &t)
                .iter()
                .filter(|r| r.stops() && matches!(r.target, Target::Pick(..)))
                .map(|r| r.target.clone())
                .map(|t| format!("{t:?}"))
                .collect();
            assert_eq!(names.len(), 2, "not narrowed to the city's two: {names:?}");
            // `Esc` gives every row back before it leaves the page.
            v.key(Press::Escape, Some(&s), 80, &t);
            assert_eq!(v.page(), PageId::Stations);
            assert!(v.top().only.is_none());
            drawn(&v, &s);
        }
        // FREVO FM ALONE: the second click plays it.
        let frevo_at = v
            .globe
            .globe
            .borrow()
            .marks()
            .iter()
            .find(|p| p.members == vec![2])
            .map(|p| p.at)
            .expect("Frevo's mark");
        v.globe_clicked(frevo_at, Some(&s), 80, &t);
        let played = v.globe_clicked(frevo_at, Some(&s), 80, &t);
        let Errand::TellSaying(Ask::Put { what, play, .. }, _) = played else {
            panic!("the station did not play: {played:?}");
        };
        assert!(play);
        assert!(
            matches!(what.as_slice(), [Thing::Stream { stream }] if stream.title.as_deref() == Some("Frevo FM"))
        );
    }

    /// FAILS IF: the footer's view, its media panel standing beside the
    /// rows, still keeps the globe's lines among the rows — the panel draws
    /// it in the film's place — or loses the line that says where the
    /// pointer is; or a door with no panel draws no globe lines.
    #[test]
    fn beside_the_footer_s_panel_the_globe_leaves_the_rows() {
        if !crate::globe::present() {
            return;
        }
        let t = texts();
        let s = with_radio(3);
        let mut v = on_world_radio(&s);
        let lines = |v: &View| {
            v.rows(Some(&s), 80, &t)
                .iter()
                .filter(|r| r.globe.is_some())
                .count()
        };
        assert_eq!(lines(&v), usize::from(tokens::space::PLAYER_GLOBELINES));
        v.beside_the_panel(true);
        assert_eq!(lines(&v), 0, "the rows keep the globe beside the panel");
        assert!(v.globe_here(), "the page no longer draws a globe");
        // THE LINE UNDER IT stays among the rows, right under the title.
        v.globe_pointed(None);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(rows[1].heading && rows[1].globe.is_none());
    }

    /// FAILS IF: `C-k`, `C-j` and `C-r` do not turn the globe on a page that
    /// draws one: nearer, further, and back to the whole world.
    #[test]
    fn the_keys_bring_the_globe_nearer_and_back() {
        if !crate::globe::present() {
            return;
        }
        let t = texts();
        let s = with_radio(3);
        let mut v = on_world_radio(&s);
        drawn(&v, &s);
        let first = pose_of(&v);
        v.key(
            crate::player::pressed("C-k").expect("C-k"),
            Some(&s),
            80,
            &t,
        );
        assert!(pose_of(&v).zoom > first.zoom, "C-k did not come nearer");
        v.key(
            crate::player::pressed("C-j").expect("C-j"),
            Some(&s),
            80,
            &t,
        );
        v.key(
            crate::player::pressed("C-j").expect("C-j"),
            Some(&s),
            80,
            &t,
        );
        assert!(pose_of(&v).zoom < first.zoom, "C-j did not go further");
        v.key(
            crate::player::pressed("C-r").expect("C-r"),
            Some(&s),
            80,
            &t,
        );
        assert_eq!(pose_of(&v).zoom, ronoctua_atlas::Pose::WORLD);
    }
}
