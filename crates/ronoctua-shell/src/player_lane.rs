//! A LANE'S OWN PAGE (`player_page::PageId::Lane`): one lane of what plays —
//! a subtitle over the picture, a sound, a picture — opened by `RET` (and the
//! second click) on its row, wherever the row is: the timeline, the subtitles'
//! page, the page of playing together, a set's lanes in the queue — and by a
//! click on its band or its name on a deck (`crate::player_timeline`).
//!
//! What it holds: the lane by its kind and name, and what it is doing
//! (refused, with the sentence under it); the file it plays; ITS OWN NAME —
//! `RET` asks it typed, holding the one it has, and emptied it takes its
//! file's back; whether it plays at all — a switch; WHEN IT BEGINS on the
//! set's clock — the arrows move it half a second, `RET` asks it typed
//! (`m:ss`, `h:mm:ss`, or `+1.5` to move it), and a time that is none is
//! refused in a sentence; WHERE IN ITS FILE IT STOPS — its end, or a time
//! typed or stepped the same way, `d` giving it its end back; a sound's own
//! volume — a bar — and its mute; whether a picture — a film's, or a sound's
//! cover — is shown among the pictures; where a subtitle's lines sit, the
//! foot of the picture or its top, and WHICH PICTURE it sits over, a ring of
//! the pictures that play; and taking it out, which closes the page back onto
//! the row that opened it. When the lane is gone the page says so
//! (`Subject::Gone`), never blank.
//!
//! The TIMELINE's page is here too ([`timeline`]): every lane on one clock, a
//! row each, the keyboard's door onto what a pointer drags on a deck.
//!
//! # Where it goes
//!
//! A picture — a film's, or a sound's cover — goes to EVERY SCREEN, or to the
//! ones a person names: a switch for every screen and one for each screen
//! the door knows by name (`View::screens`), the built-in one said so, and a
//! screen it goes to that is not connected said so too, never dropped: it
//! comes back with its cable. Turning "every screen" off leaves it on the
//! screen this door is on; the last screen it goes to is not taken away — a
//! picture shown nowhere is a hidden one, which its own switch is for.
//!
//! A SOUND goes to the machine's default output or to one the person names,
//! a row each with what kind it is. The list is the daemon's
//! (`State::sinks`), read only while a door shows these rows: the door holds
//! the outputs (`player::SinksHeld`, `Ask::Sinks`) while one of them is on
//! screen ([`sinks_in_sight`]), and lets go when none is. An output chosen
//! that is gone is said so; the sound plays on the default meanwhile.

use crate::player::{Errand, PageId, Purpose, Target, trouble_sentence};
use crate::player_rows::{Builder, Row, Slid, on_off, share};
use ronoctua_i18n::key;
use ronoctua_player::own::{Lane, LaneId, LaneKind, Screens, VOLUME_MOST_PERCENT};
use ronoctua_player::wire::{Ask, LaneChange, LaneStatus, SinkKind, State};
use ronoctua_tokens as tokens;

/// A screen a door knows BY NAME: its connector and what the compositor calls
/// it for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// Its connector (`eDP-1`).
    pub connector: String,
    /// What the compositor calls it (the make, the model).
    pub description: Option<String>,
}

/// A screen's row's label: the built-in one said so, another by what it is
/// called and its connector — two monitors of one model are two rows — and a
/// screen with no description by its connector alone.
#[must_use]
pub fn screen_label(
    connector: &str,
    description: Option<&str>,
    texts: &ronoctua_i18n::Texts,
) -> String {
    if ronoctua_display::connector::is_internal(connector) {
        return texts.t_with(key::PLAYER_SCREEN_BUILT_IN, &[("connector", connector)]);
    }
    match description.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => texts.t_with(
            key::PLAYER_SCREEN_NAMED,
            &[("description", d), ("connector", connector)],
        ),
        None => connector.to_owned(),
    }
}

/// Whether lane `lane`'s picture can be routed: a film, or a sound whose
/// cover the daemon says it has.
fn has_a_picture(s: &State, lane: &Lane) -> bool {
    match lane.kind {
        LaneKind::Video => true,
        LaneKind::Audio => s.lane_states.iter().any(|l| l.lane == lane.id && l.cover),
        LaneKind::Subtitle => false,
    }
}

/// What turning "every screen" asks: on, the picture goes to every screen;
/// off, to the screen this door is on (`here`) — or the first it knows — and
/// with no screen known by name it says so.
#[must_use]
pub fn every_screen_turned(
    s: &State,
    id: LaneId,
    known: &[Known],
    here: Option<&str>,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    let Some(lane) = s.lanes.iter().find(|l| l.id == id) else {
        return Errand::Nothing;
    };
    let screens = match &lane.screens {
        Screens::Only(_) => Screens::Every,
        Screens::Every => {
            let Some(one) = here.or_else(|| known.first().map(|k| k.connector.as_str())) else {
                return Errand::Said(texts.t(key::PLAYER_SCREENS_UNKNOWN));
            };
            Screens::Only(vec![one.to_owned()])
        }
    };
    Errand::Tell(Ask::Lane {
        lane: id,
        change: LaneChange::Screens { screens },
    })
}

/// What turning screen `connector` asks: the picture goes there too, or no
/// longer — from every screen, to every OTHER one this door knows. The last
/// screen it goes to is not taken away, and that is said.
#[must_use]
pub fn screen_turned(
    s: &State,
    id: LaneId,
    connector: &str,
    known: &[Known],
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    let Some(lane) = s.lanes.iter().find(|l| l.id == id) else {
        return Errand::Nothing;
    };
    let mut these = match &lane.screens {
        Screens::Every => known
            .iter()
            .map(|k| k.connector.clone())
            .filter(|c| c != connector)
            .collect(),
        Screens::Only(these) if these.iter().any(|c| c == connector) => {
            these.iter().filter(|c| *c != connector).cloned().collect()
        }
        Screens::Only(these) => {
            let mut more = these.clone();
            more.push(connector.to_owned());
            more
        }
    };
    if these.is_empty() {
        return Errand::Said(texts.t(key::PLAYER_SCREEN_LAST));
    }
    these.dedup();
    Errand::Tell(Ask::Lane {
        lane: id,
        change: LaneChange::Screens {
            screens: Screens::Only(these),
        },
    })
}

/// A sound output's kind: its glyph and its word.
#[must_use]
pub const fn sink_kind(kind: SinkKind) -> (char, &'static str) {
    match kind {
        SinkKind::Headphones => (
            tokens::icon::PLAYER_SINK_HEADPHONES,
            key::PLAYER_SINK_HEADPHONES,
        ),
        SinkKind::Speakers => (
            tokens::icon::PLAYER_SINK_SPEAKERS,
            key::PLAYER_SINK_SPEAKERS,
        ),
        SinkKind::Bluetooth => (
            tokens::icon::PLAYER_SINK_BLUETOOTH,
            key::PLAYER_SINK_BLUETOOTH,
        ),
        SinkKind::Other => (tokens::icon::PLAYER_SINK_OTHER, key::PLAYER_SINK_OTHER),
    }
}

/// Whether a row of the sound outputs is among the rows ON SCREEN — the ones
/// from `offset`, `room` of them: what a door holds the outputs for.
#[must_use]
pub fn sinks_in_sight(rows: &[Row], offset: usize, room: usize) -> bool {
    rows.iter()
        .skip(offset)
        .take(room)
        .any(|r| matches!(r.target, Target::LaneSink(..)))
}

/// WHERE A PICTURE GOES: every screen, or each screen the door knows by name
/// — and the ones it goes to that are not connected.
fn screens_rows(lane: &Lane, known: &[Known], b: &mut Builder<'_>) {
    let id = lane.id;
    b.heading(key::PLAYER_HEADING_LANE_SCREENS);
    b.pair(
        b.t(key::PLAYER_LANE_EVERY_SCREEN),
        on_off(lane.screens == Screens::Every, b.texts),
        Target::LaneEveryScreen(id),
        "lane-every-screen",
    );
    for k in known {
        let glyph = if ronoctua_display::connector::is_internal(&k.connector) {
            tokens::icon::PLAYER_SCREEN_BUILT_IN
        } else {
            tokens::icon::PLAYER_SCREEN
        };
        b.pair_with_mark(
            glyph,
            screen_label(&k.connector, k.description.as_deref(), b.texts),
            on_off(lane.screens.shows_on(Some(&k.connector)), b.texts),
            Target::LaneScreen(id, k.connector.clone()),
            &format!("lane-screen-{}", k.connector),
        );
    }
    if let Screens::Only(these) = &lane.screens {
        for c in these
            .iter()
            .filter(|c| !known.iter().any(|k| &k.connector == *c))
        {
            b.pair_with_mark(
                tokens::icon::PLAYER_SCREEN_ABSENT,
                c.clone(),
                b.t(key::PLAYER_SCREEN_ABSENT),
                Target::LaneScreen(id, c.clone()),
                &format!("lane-screen-{c}"),
            );
        }
    }
    if known.is_empty() {
        b.note(key::PLAYER_SCREENS_UNKNOWN);
    }
}

/// WHERE A SOUND GOES: the machine's default output, and each one the
/// daemon reads — the one in use said so — and one chosen that is gone.
fn sink_rows(lane: &Lane, s: &State, b: &mut Builder<'_>) {
    let id = lane.id;
    b.heading(key::PLAYER_HEADING_LANE_SOUND);
    b.pair_with_mark(
        tokens::icon::PLAYER_SINK_DEFAULT,
        b.t(key::PLAYER_SINK_DEFAULT),
        crate::player_tracks::in_use(b, lane.sink.is_none()),
        Target::LaneSink(id, None),
        "lane-sink-default",
    );
    for sink in &s.sinks {
        let (glyph, word) = sink_kind(sink.kind);
        let chosen = lane.sink.as_deref() == Some(sink.device.as_str());
        b.pair_with_mark(
            glyph,
            crate::player::sink_label(&sink.device, &s.sinks, b.texts),
            if chosen {
                crate::player_tracks::in_use(b, true)
            } else {
                b.t(word)
            },
            Target::LaneSink(id, Some(sink.device.clone())),
            &format!("lane-sink-{}", sink.device),
        );
    }
    if let Some(gone) = lane
        .sink
        .as_ref()
        .filter(|d| !s.sinks.iter().any(|k| &k.device == *d))
    {
        b.pair_with_mark(
            tokens::icon::PLAYER_SINK_OTHER,
            crate::player::sink_label(gone, &s.sinks, b.texts),
            b.t(key::PLAYER_SINK_GONE),
            Target::LaneSink(id, Some(gone.clone())),
            &format!("lane-sink-{gone}"),
        );
    }
    if s.sinks.is_empty() {
        b.note(key::PLAYER_SINKS_NONE);
    }
}

/// When a lane begins, as a person writes it and as the typed question reads
/// it back (`player_timeline::parse_start`): `m:ss`, `h:mm:ss` from an hour
/// on, and the tenth of a second after the language's decimal mark when
/// there is one — the arrows move a lane half a second.
#[must_use]
pub fn start_said(seconds: f64, texts: &ronoctua_i18n::Texts) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    #[allow(clippy::cast_precision_loss)]
    let whole = crate::player::clock((tenths / 10) as f64);
    match tenths % 10 {
        0 => whole,
        tenth => format!("{whole}{}{tenth}", texts.decimal_mark()),
    }
}

/// How long lane `lane`'s FILE is, in seconds, once the player said it
/// (`LaneState::duration_seconds`, what mpv reads of the file).
fn file_length(s: &State, lane: &Lane) -> Option<f64> {
    s.lane_states
        .iter()
        .find(|l| l.lane == lane.id)
        .and_then(|l| l.duration_seconds)
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// Where lane `lane` stops in its own file NOW, in seconds of the file: where
/// it was cut, else its file's end once the player said how long it is.
/// `None` while neither is known.
#[must_use]
pub fn until_now(s: &State, lane: &Lane) -> Option<f64> {
    lane.until_seconds.or_else(|| file_length(s, lane))
}

/// Where a lane stops, as its row says it: the time it was cut at — as
/// [`start_said`] writes one — or its end.
#[must_use]
pub fn until_said(lane: &Lane, texts: &ronoctua_i18n::Texts) -> String {
    lane.until_seconds.map_or_else(
        || texts.t(key::PLAYER_LANE_UNTIL_END),
        |u| start_said(u, texts),
    )
}

/// What the arrows on "Plays until" ask: the end half a second earlier or
/// later — from its end, the first step back cuts it there — never at or
/// before where it begins; stepped to its file's end or past it, it plays to
/// its end again. Nothing while where it ends is not known, nor a step later
/// than its end.
#[must_use]
pub fn until_stepped(s: &State, lane: &Lane, later: bool) -> Errand {
    let step = crate::player_timeline::OFFSET_STEP_SECONDS;
    let Some(now) = until_now(s, lane) else {
        return Errand::Nothing;
    };
    if later && lane.until_seconds.is_none() {
        return Errand::Nothing;
    }
    let to = if later { now + step } else { now - step };
    let until = match file_length(s, lane) {
        Some(end) if to >= end => None,
        _ => Some(to.max(lane.from_seconds + step)),
    };
    Errand::Tell(Ask::Lane {
        lane: lane.id,
        change: LaneChange::Trim {
            from_seconds: lane.from_seconds,
            until_seconds: until,
        },
    })
}

/// The pictures a subtitle lane can sit over: every film of what plays, in
/// its order.
fn pictures_of(s: &State) -> Vec<&Lane> {
    s.lanes
        .iter()
        .filter(|l| l.kind == LaneKind::Video)
        .collect()
}

/// The picture subtitle lane `lane` sits over: the one it names, else the
/// first of what plays.
fn over_of<'a>(s: &'a State, lane: &Lane) -> Option<&'a Lane> {
    let pictures = pictures_of(s);
    lane.over
        .and_then(|id| pictures.iter().copied().find(|p| p.id == id))
        .or_else(|| pictures.first().copied())
}

/// What turning "Over the picture" asks: the next picture of what plays — or
/// the one before — for the subtitles to sit over. Nothing with one picture
/// or none: there is nothing to turn to.
#[must_use]
pub fn over_turned(s: &State, id: LaneId, forward: bool) -> Errand {
    let Some(lane) = s.lanes.iter().find(|l| l.id == id) else {
        return Errand::Nothing;
    };
    let pictures = pictures_of(s);
    if pictures.len() < 2 {
        return Errand::Nothing;
    }
    let now = over_of(s, lane).map_or(0, |p| {
        pictures.iter().position(|q| q.id == p.id).unwrap_or(0)
    });
    let n = pictures.len();
    let to = if forward {
        (now + 1) % n
    } else {
        (now + n - 1) % n
    };
    Errand::Tell(Ask::Lane {
        lane: id,
        change: LaneChange::Over {
            over: Some(pictures[to].id),
        },
    })
}

/// What turning a lane's "shown" switch asks: the arrangement WHOLE, with the
/// lane moved in or out of the pictures hidden — or, for a sound, of the
/// covers shown. `None` for subtitles, which are shown by playing, and while
/// no arrangement is said (another machine's lanes, played in step).
#[must_use]
pub fn shown_turned(s: &State, id: LaneId) -> Option<Ask> {
    let lane = s.lanes.iter().find(|l| l.id == id)?;
    let mut arrangement = s.arrangement.clone()?;
    let list = match lane.kind {
        LaneKind::Video => &mut arrangement.hidden,
        LaneKind::Audio => &mut arrangement.covers,
        LaneKind::Subtitle => return None,
    };
    if let Some(at) = list.iter().position(|l| *l == id) {
        list.remove(at);
    } else {
        list.push(id);
    }
    Some(Ask::Arrange { arrangement })
}

/// Whether lane `id`'s picture is shown, and the key its row is labelled by
/// — a film's picture, or a sound's cover — when it has one to show.
fn shown_of(s: &State, id: LaneId) -> Option<(bool, &'static str)> {
    let lane = s.lanes.iter().find(|l| l.id == id)?;
    let arrangement = s.arrangement.as_ref()?;
    match lane.kind {
        LaneKind::Video => Some((
            !arrangement.hidden.contains(&id),
            key::PLAYER_LANE_PICTURE_SHOWN,
        )),
        // A SOUND WITH A COVER — the daemon says so of a lane that plays.
        LaneKind::Audio => s
            .lane_states
            .iter()
            .any(|l| l.lane == id && l.cover)
            .then(|| {
                (
                    arrangement.covers.contains(&id),
                    key::PLAYER_LANE_COVER_SHOWN,
                )
            }),
        LaneKind::Subtitle => None,
    }
}

/// THE TIMELINE'S rows, under its title: what it is, the deck — the ruler
/// and a row for every lane of what plays, a file alone's too (moving when
/// its sound begins is how a sound late over a wireless link is brought in
/// step) — and the row that picks something more to play together.
pub(crate) fn timeline(s: &State, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_TIMELINE);
    b.inert(crate::chrome::AIR);
    b.deck(&s.lanes, s);
    b.inert(crate::chrome::AIR);
    let more = PageId::Files(Purpose::Lane);
    b.pair(
        b.t(more.key()),
        String::new(),
        Target::Go(more),
        more.spot(),
    );
}

/// The page's rows, under its title — `known`, the screens the door knows by
/// name.
pub(crate) fn page(id: LaneId, s: &State, known: &[Known], b: &mut Builder<'_>) {
    let Some(lane) = s.lanes.iter().find(|l| l.id == id) else {
        return;
    };
    let now = s.lane_states.iter().find(|l| l.lane == id);
    let doing = if lane.enabled {
        b.t(match now.map(|l| &l.status) {
            None | Some(LaneStatus::Pending) => key::PLAYER_LANE_PENDING,
            Some(LaneStatus::Preparing) => key::PLAYER_LANE_PREPARING,
            Some(LaneStatus::Ended) => key::PLAYER_LANE_ENDED,
            Some(LaneStatus::Refused(_)) => key::PLAYER_LANE_REFUSED,
            Some(LaneStatus::Playing) => key::PLAYER_STATE_PLAYING,
        })
    } else {
        b.t(key::PLAYER_LANE_SWITCHED_OFF)
    };
    // WHAT IT IS, as its row said it: a subtitle wears its colour on its chip
    // here too, which turns it as it does there.
    let spot = format!("lane-page-{}", id.0);
    match (lane.kind, crate::player_subtitles::colour_of(s, id)) {
        (LaneKind::Subtitle, Some(colour)) => b.pair_with_chip(
            (
                if lane.enabled {
                    tokens::icon::PLAYER_SUBTITLE_SHOWN
                } else {
                    tokens::icon::PLAYER_SUBTITLE_HIDDEN
                },
                colour,
            ),
            crate::player_subtitles::name(lane, s, b.texts, b.room),
            doing,
            (Target::Nothing, Target::Shown(id)),
            &spot,
        ),
        (kind, _) => b.pair_with_mark(
            crate::player_queue::lane_glyph(kind),
            if kind == LaneKind::Subtitle {
                crate::player_subtitles::name(lane, s, b.texts, b.room)
            } else {
                crate::player_queue::lane_name(lane, &s.lanes, b.texts, b.room)
            },
            doing,
            Target::Nothing,
            &spot,
        ),
    }
    if let Some(LaneStatus::Refused(t)) = now.map(|l| &l.status) {
        b.warning(&trouble_sentence(t, b.texts));
    }
    b.pair(
        b.t(key::PLAYER_LANE_SOURCE),
        crate::player::name_of(
            &ronoctua_player::queue::Name::Location(lane.source.clone()),
            b.texts,
            b.room,
        ),
        Target::Nothing,
        "lane-source",
    );
    // ITS OWN NAME — the one a person gave it, else its file's — asked
    // typed on `RET`.
    b.pair(
        b.t(key::PLAYER_LANE_TITLE),
        crate::player_queue::lane_name(lane, &s.lanes, b.texts, b.room),
        Target::LaneTitle(id),
        "lane-title",
    );
    b.pair(
        b.t(key::PLAYER_LANE_ON),
        on_off(lane.enabled, b.texts),
        Target::LaneOn(id),
        "lane-on",
    );
    b.pair(
        b.t(key::PLAYER_LANE_START),
        start_said(lane.start_seconds, b.texts),
        Target::LaneStart(id),
        "lane-start",
    );
    // WHERE IN ITS FILE IT STOPS: its end, or where it was cut.
    b.pair(
        b.t(key::PLAYER_LANE_UNTIL),
        until_said(lane, b.texts),
        Target::LaneUntil(id),
        "lane-until",
    );
    // A SOUND'S OWN VOLUME AND MUTE: a picture's voice plays none, and
    // subtitles have none to turn.
    if lane.kind == LaneKind::Audio {
        b.slider_pair(
            b.t(key::PLAYER_LANE_VOLUME),
            share(
                f64::from(lane.volume_percent),
                f64::from(VOLUME_MOST_PERCENT),
            ),
            &format!("{}%", lane.volume_percent),
            Target::LaneVolume(id),
            "lane-volume",
            Slid::Lane(id),
        );
        b.pair(
            b.t(key::PLAYER_LANE_MUTE),
            on_off(lane.muted, b.texts),
            Target::LaneMute(id),
            "lane-mute",
        );
    }
    if let Some((shown, label)) = shown_of(s, id) {
        b.pair(
            b.t(label),
            on_off(shown, b.texts),
            Target::LaneShown(id),
            "lane-shown",
        );
    }
    if lane.kind == LaneKind::Subtitle {
        b.pair(
            b.t(key::PLAYER_LANE_STACK),
            b.t(crate::player_subtitles::stack_word(lane.stack)),
            Target::LaneStack(id),
            "lane-stack",
        );
        // WHICH PICTURE IT SITS OVER: a ring of the films of what plays — a
        // fact while there is only one to sit over.
        let pictures = pictures_of(s);
        b.pair(
            b.t(key::PLAYER_LANE_OVER),
            over_of(s, lane).map_or_else(
                || b.t(key::PLAYER_LANE_OVER_NONE),
                |p| crate::player_queue::lane_name(p, &s.lanes, b.texts, b.room),
            ),
            if pictures.len() > 1 {
                Target::LaneOver(id)
            } else {
                Target::Nothing
            },
            "lane-over",
        );
    }
    b.inert(crate::chrome::AIR);
    b.pair(
        b.t(key::PLAYER_LANE_REMOVE),
        String::new(),
        Target::LaneRemove(id),
        "lane-remove",
    );
    // WHERE IT GOES: a picture's screens, a sound's output.
    if has_a_picture(s, lane) {
        screens_rows(lane, known, b);
    }
    if lane.kind == LaneKind::Audio {
        sink_rows(lane, s, b);
    }
}

#[cfg(test)]
mod tests {
    use crate::player::fixture::{alone, track};
    use crate::player::{Errand, PageId, Press, Target, TrackKind, View};
    use ronoctua_i18n::key;
    use ronoctua_player::own::{LaneId, Stack};
    use ronoctua_player::wire::{Ask, LaneChange, State, TrackKind as Kind};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn film() -> State {
        alone(
            "/films/a.mkv",
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, None, true),
                track(1, Kind::Sub, Some("English"), true),
            ],
        )
    }

    /// FAILS IF: a lane's page stops turning it on and off, stops moving a
    /// subtitle between the foot and the top, or offers that ring for a lane
    /// that is no subtitle; or taking it out stops closing the page back onto
    /// the row that opened it; or a lane that is gone leaves the page blank.
    #[test]
    fn a_lane_s_page_turns_it_moves_it_and_takes_it_out() {
        let t = texts();
        let s = film();
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Subtitle(LaneId(3)))
            .expect("the subtitle's row");
        v.clicked(at, &rows);
        v.clicked(at, &rows);
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Lane(LaneId(3)));
        let rows = v.rows(Some(&s), 80, &t);
        let press = |v: &mut View, target: Target| {
            let rows = v.rows(Some(&s), 80, &t);
            let n = rows
                .iter()
                .position(|r| r.target == target)
                .unwrap_or_else(|| panic!("no row for {target:?}"));
            v.clicked(n, &rows);
            v.clicked(n, &rows);
            v.key(Press::Enter, Some(&s), 80, &t)
        };
        let lane = |change| {
            Errand::Tell(Ask::Lane {
                lane: LaneId(3),
                change,
            })
        };
        assert_eq!(
            press(&mut v, Target::LaneOn(LaneId(3))),
            lane(LaneChange::Enabled { on: false })
        );
        assert_eq!(
            press(&mut v, Target::LaneStack(LaneId(3))),
            lane(LaneChange::Stack { stack: Stack::Top })
        );
        assert!(
            rows.iter().any(|r| !r.chips.is_empty()),
            "the chip is here too"
        );
        assert_eq!(
            press(&mut v, Target::LaneRemove(LaneId(3))),
            lane(LaneChange::Remove)
        );
        assert_eq!(v.page(), PageId::Tracks(TrackKind::Subtitles));
        // A SOUND: no ring of where its lines sit.
        v.push(PageId::Lane(LaneId(2)), None);
        assert!(
            !v.rows(Some(&s), 80, &t)
                .iter()
                .any(|r| matches!(r.target, Target::LaneStack(_)))
        );
        // GONE, it says so.
        v.push(PageId::Lane(LaneId(9)), None);
        let said: Vec<String> = v
            .rows(Some(&s), 80, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(said.contains(&t.t(key::PLAYER_PAGE_GONE)), "{said:?}");
    }

    /// The screens a door knows: the built-in one and a monitor.
    fn screens() -> Vec<super::Known> {
        vec![
            super::Known {
                connector: "eDP-1".into(),
                description: Some("BOE 0x0BCA".into()),
            },
            super::Known {
                connector: "DP-2".into(),
                description: Some("A wide monitor".into()),
            },
        ]
    }

    /// FAILS IF: a picture's page stops routing it — every screen, then the
    /// screen this door is on; a screen of its own turned in and out; the
    /// last one refused in a sentence — or stops saying the built-in screen
    /// and a screen it goes to that is not connected; or a subtitle, whose
    /// place is its picture's, gets screens of its own.
    #[test]
    fn a_picture_goes_to_every_screen_or_the_ones_named() {
        use ronoctua_player::own::Screens;
        let t = texts();
        let mut s = crate::player::fixture::routed();
        let mut v = View::blank(None);
        v.screens_are(screens(), Some("DP-2".into()));
        v.push(PageId::Lane(LaneId(1)), None);
        let rows = v.rows(Some(&s), 80, &t);
        let said: Vec<String> = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(said.contains(&t.t(key::PLAYER_HEADING_LANE_SCREENS)));
        assert!(
            said.iter().any(|w| w.contains("eDP-1")
                && w.contains(&t.t_with(key::PLAYER_SCREEN_BUILT_IN, &[("connector", "eDP-1")]))),
            "{said:?}"
        );
        assert!(
            said.iter().any(|w| w.contains("A wide monitor")),
            "{said:?}"
        );
        let press = |v: &mut View, s: &State, target: Target| {
            let rows = v.rows(Some(s), 80, &t);
            let n = rows
                .iter()
                .position(|r| r.target == target)
                .unwrap_or_else(|| panic!("no row for {target:?}"));
            v.clicked(n, &rows);
            v.key(Press::Enter, Some(s), 80, &t)
        };
        let routed = |screens| {
            Errand::Tell(Ask::Lane {
                lane: LaneId(1),
                change: LaneChange::Screens { screens },
            })
        };
        // EVERY SCREEN off: the screen this door is on.
        assert_eq!(
            press(&mut v, &s, Target::LaneEveryScreen(LaneId(1))),
            routed(Screens::Only(vec!["DP-2".into()]))
        );
        // A SCREEN OFF, from every screen: every other one.
        assert_eq!(
            press(&mut v, &s, Target::LaneScreen(LaneId(1), "DP-2".into())),
            routed(Screens::Only(vec!["eDP-1".into()]))
        );
        // ON ONE SCREEN: another turned on is added, and the last refused.
        s.lanes[0].screens = Screens::Only(vec!["eDP-1".into(), "HDMI-A-1".into()]);
        assert_eq!(
            press(&mut v, &s, Target::LaneScreen(LaneId(1), "DP-2".into())),
            routed(Screens::Only(vec![
                "eDP-1".into(),
                "HDMI-A-1".into(),
                "DP-2".into()
            ]))
        );
        let rows = v.rows(Some(&s), 80, &t);
        let absent = rows
            .iter()
            .find(|r| r.target == Target::LaneScreen(LaneId(1), "HDMI-A-1".into()))
            .expect("the screen it goes to that is not here");
        assert!(
            absent
                .fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_SCREEN_ABSENT))
        );
        s.lanes[0].screens = Screens::Only(vec!["eDP-1".into()]);
        match press(&mut v, &s, Target::LaneScreen(LaneId(1), "eDP-1".into())) {
            Errand::Said(said) => assert_eq!(said, t.t(key::PLAYER_SCREEN_LAST)),
            other => panic!("the last screen was taken away: {other:?}"),
        }
        assert_eq!(
            press(&mut v, &s, Target::LaneEveryScreen(LaneId(1))),
            routed(Screens::Every)
        );
        // A SUBTITLE has no screens of its own.
        v.push(PageId::Lane(LaneId(4)), None);
        assert!(!v.rows(Some(&s), 80, &t).iter().any(|r| matches!(
            r.target,
            Target::LaneEveryScreen(_) | Target::LaneScreen(..)
        )));
        // NO SCREEN KNOWN BY NAME: said, and "every screen" cannot be left.
        let mut blind = View::blank(None);
        blind.push(PageId::Lane(LaneId(2)), None);
        s.lanes[1].screens = Screens::Every;
        match press(&mut blind, &s, Target::LaneEveryScreen(LaneId(2))) {
            Errand::Said(said) => assert_eq!(said, t.t(key::PLAYER_SCREENS_UNKNOWN)),
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: a sound's page stops listing the default output and every
    /// one the daemon reads — the one in use said so, each by its kind — or
    /// choosing one stops sending the lane there, or an output chosen that
    /// is gone is dropped instead of said; or the rows are held for while
    /// they are off screen.
    #[test]
    fn a_sound_goes_to_the_default_or_an_output_it_names() {
        use ronoctua_player::wire::{Sink, SinkKind};
        let t = texts();
        let (mut s, _) = a_set();
        s.sinks = vec![
            Sink {
                device: "alsa_output.usb-phones".into(),
                description: "USB headset".into(),
                kind: SinkKind::Headphones,
            },
            Sink {
                device: "bluez_output.speaker".into(),
                description: "Kitchen speaker".into(),
                kind: SinkKind::Bluetooth,
            },
        ];
        let mut v = View::blank(None);
        v.push(PageId::Lane(LaneId(2)), None);
        let rows = v.rows(Some(&s), 80, &t);
        let sink_rows: Vec<&crate::player_rows::Row> = rows
            .iter()
            .filter(|r| matches!(r.target, Target::LaneSink(..)))
            .collect();
        assert_eq!(sink_rows.len(), 3, "the default and two outputs");
        assert!(
            sink_rows[0]
                .fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_TRACK_IN_USE)),
            "the default is in use"
        );
        assert!(
            sink_rows[2]
                .fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_SINK_BLUETOOTH))
        );
        let n = rows
            .iter()
            .position(|r| {
                r.target == Target::LaneSink(LaneId(2), Some("alsa_output.usb-phones".into()))
            })
            .expect("the headset's row");
        v.clicked(n, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Sink {
                    device: Some("alsa_output.usb-phones".into())
                }
            })
        );
        // GONE, said.
        s.lanes[1].sink = Some("alsa_output.gone".into());
        let rows = v.rows(Some(&s), 80, &t);
        let gone = rows
            .iter()
            .find(|r| r.target == Target::LaneSink(LaneId(2), Some("alsa_output.gone".into())))
            .expect("the output chosen that is gone");
        assert!(
            gone.fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_SINK_GONE))
        );
        // HELD ONLY WHILE A ROW OF THEM IS ON SCREEN.
        let first = rows
            .iter()
            .position(|r| matches!(r.target, Target::LaneSink(..)))
            .expect("a row of the outputs");
        assert!(super::sinks_in_sight(&rows, 0, rows.len()));
        assert!(!super::sinks_in_sight(&rows, 0, first));
        assert!(super::sinks_in_sight(&rows, first, 1));
        // A PICTURE has no output.
        v.push(PageId::Lane(LaneId(1)), None);
        assert!(
            !v.rows(Some(&s), 80, &t)
                .iter()
                .any(|r| matches!(r.target, Target::LaneSink(..)))
        );
    }

    /// A set of three lanes: a picture, a dub from 10 s at 60 %, subtitles.
    fn a_set() -> (State, Box<ronoctua_player::queue::Outline>) {
        use ronoctua_player::own::{Lane, LaneKind};
        let video = Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv");
        let mut audio = Lane::new(LaneId(2), LaneKind::Audio, "/films/dub.flac");
        audio.start_seconds = 10.0;
        audio.volume_percent = 60;
        let words = Lane::new(LaneId(3), LaneKind::Subtitle, "/films/one.srt");
        crate::player::fixture::set(
            vec![video, audio, words],
            &[Some(60.0), Some(40.0), Some(45.0)],
            Some(60.0),
            0.0,
        )
    }

    /// FAILS IF: when a lane begins stops being asked on the door's line
    /// holding when it begins now — and read back as a clock, a clock with
    /// hours, or a move from where it is, a time that is none refused in a
    /// sentence that quotes it, nothing typed never mind — or the arrows
    /// stop moving it half a second; or a sound's own volume, its mute and
    /// whether a picture is shown stop asking what their rows say, or show
    /// on a lane they mean nothing for.
    #[test]
    fn a_lane_s_start_volume_mute_and_picture_are_its_page_s() {
        use crate::player_question::{LANE_START, Question};
        let t = texts();
        let (s, outline) = a_set();
        let mut v = View::blank(None);
        v.outline_arrived(Some(outline));
        let mut pairing = crate::player_machines::Pairing::default();
        let open = |v: &mut View, id: u32| {
            v.key(Press::Go(PageId::Lane(LaneId(id))), Some(&s), 80, &t);
            assert_eq!(v.page(), PageId::Lane(LaneId(id)));
        };
        let stand = |v: &mut View, target: &Target| {
            let rows = v.rows(Some(&s), 80, &t);
            let n = rows
                .iter()
                .position(|r| &r.target == target)
                .unwrap_or_else(|| panic!("no row for {target:?}"));
            v.clicked(n, &rows);
        };
        let lane = |change| {
            Errand::Tell(Ask::Lane {
                lane: LaneId(2),
                change,
            })
        };
        open(&mut v, 2);
        // WHEN IT BEGINS: asked holding what it is.
        stand(&mut v, &Target::LaneStart(LaneId(2)));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::holding(LANE_START, "0:10".to_owned()))
        );
        let mut typed = |v: &mut View, said: &str| {
            v.key(Press::Enter, Some(&s), 80, &t);
            v.answered(LANE_START, said, &mut pairing, Some(&s), 80, &t)
        };
        assert_eq!(
            typed(&mut v, "+1,5"),
            lane(LaneChange::Start { seconds: 11.5 })
        );
        assert_eq!(
            typed(&mut v, "1:02:03"),
            lane(LaneChange::Start { seconds: 3723.0 })
        );
        assert_eq!(typed(&mut v, "  "), Errand::Nothing);
        match typed(&mut v, "soon") {
            Errand::Said(said) => assert!(said.contains("soon"), "{said}"),
            other => panic!("`soon` was read as {other:?}"),
        }
        // THE ARROWS, half a second.
        assert_eq!(
            v.key(Press::More, Some(&s), 80, &t),
            lane(LaneChange::Start { seconds: 10.5 })
        );
        // ITS OWN VOLUME: a step, and `RET` its mute.
        stand(&mut v, &Target::LaneVolume(LaneId(2)));
        assert_eq!(
            v.key(Press::Less, Some(&s), 80, &t),
            lane(LaneChange::Volume { percent: 55 })
        );
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            lane(LaneChange::Mute { muted: true })
        );
        stand(&mut v, &Target::LaneMute(LaneId(2)));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            lane(LaneChange::Mute { muted: true })
        );
        let rows = v.rows(Some(&s), 80, &t);
        let bar = rows
            .iter()
            .find_map(|r| r.slider)
            .expect("the volume is a bar");
        assert_eq!(bar.of, crate::player_rows::Slid::Lane(LaneId(2)));
        // THE PICTURE: shown, and hidden by its switch — the arrangement whole.
        v.key(Press::Escape, Some(&s), 80, &t);
        open(&mut v, 1);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r.target, Target::LaneVolume(_) | Target::LaneMute(_))),
            "a picture has no sound of its own"
        );
        stand(&mut v, &Target::LaneShown(LaneId(1)));
        match v.key(Press::Enter, Some(&s), 80, &t) {
            Errand::Tell(Ask::Arrange { arrangement }) => {
                assert_eq!(arrangement.hidden, vec![LaneId(1)]);
            }
            other => panic!("the picture's switch asked {other:?}"),
        }
        // SUBTITLES are shown by playing: no second switch.
        v.key(Press::Escape, Some(&s), 80, &t);
        open(&mut v, 3);
        assert!(
            !v.rows(Some(&s), 80, &t)
                .iter()
                .any(|r| matches!(r.target, Target::LaneShown(_) | Target::LaneVolume(_)))
        );
        // AND A START IS WRITTEN IN THE LANGUAGE'S DECIMAL MARK.
        let pt = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        assert_eq!(super::start_said(2.5, &pt), "0:02,5");
        assert_eq!(super::start_said(3725.0, &t), "1:02:05");
    }

    /// FAILS IF: a lane's page stops offering where it STOPS in its file —
    /// its end said as such, the arrows cutting it half a second from its end
    /// and letting it go past its file's end, `RET` asking it typed (a time
    /// at or before where it begins, or no time, refused in a sentence), `d`
    /// giving its end back — or ITS NAME, asked holding the one it has and
    /// emptied back to its file's; or a subtitle's WHICH PICTURE ring, which
    /// walks the films of what plays and is a fact with only one.
    #[test]
    fn a_lane_stops_where_it_is_told_is_named_and_sits_over_a_picture() {
        use crate::player::fixture::set;
        use crate::player_question::{LANE_TITLE, LANE_UNTIL, Question};
        use ronoctua_player::own::{Lane, LaneKind};
        let t = texts();
        let one = Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv");
        let two = Lane::new(LaneId(2), LaneKind::Video, "/films/two.mkv");
        let mut dub = Lane::new(LaneId(3), LaneKind::Audio, "/films/dub.flac");
        dub.from_seconds = 5.0;
        let words = Lane::new(LaneId(4), LaneKind::Subtitle, "/films/one.srt");
        let (s, _) = set(
            vec![one, two, dub, words],
            &[Some(60.0), Some(60.0), Some(90.0), None],
            Some(90.0),
            0.0,
        );
        let mut v = View::blank(None);
        let mut pairing = crate::player_machines::Pairing::default();
        v.key(Press::Go(PageId::Lane(LaneId(3))), Some(&s), 80, &t);
        let stand = |v: &mut View, target: &Target| {
            let rows = v.rows(Some(&s), 80, &t);
            let n = rows
                .iter()
                .position(|r| &r.target == target)
                .unwrap_or_else(|| panic!("no row for {target:?}"));
            v.grabbed(n);
            rows[n].clone()
        };
        let row = stand(&mut v, &Target::LaneUntil(LaneId(3)));
        assert!(
            row.fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_LANE_UNTIL_END)),
            "a lane cut nowhere says it plays to its end: {:?}",
            row.fields
        );
        let trim = |until| {
            Errand::Tell(Ask::Lane {
                lane: LaneId(3),
                change: LaneChange::Trim {
                    from_seconds: 5.0,
                    until_seconds: until,
                },
            })
        };
        assert_eq!(v.key(Press::More, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.key(Press::Less, Some(&s), 80, &t), trim(Some(89.5)));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::holding(LANE_UNTIL, "1:30".to_owned()))
        );
        assert_eq!(
            v.answered(LANE_UNTIL, "1:00", &mut pairing, Some(&s), 80, &t),
            trim(Some(60.0))
        );
        for (said, refused) in [("0:03", true), ("soon", true), ("", false)] {
            v.key(Press::Enter, Some(&s), 80, &t);
            let e = v.answered(LANE_UNTIL, said, &mut pairing, Some(&s), 80, &t);
            assert_eq!(matches!(e, Errand::Said(_)), refused, "{said:?}: {e:?}");
            if !refused {
                assert_eq!(e, Errand::Nothing, "nothing typed is never mind");
            }
        }
        // CUT, it steps later and goes back to its end past its file's.
        let mut cut = s.clone();
        cut.lanes[2].until_seconds = Some(89.8);
        assert_eq!(v.key(Press::More, Some(&cut), 80, &t), trim(None));
        assert_eq!(v.key(Press::Remove, Some(&cut), 80, &t), trim(None));
        assert_eq!(v.key(Press::Remove, Some(&s), 80, &t), Errand::Nothing);
        // ITS NAME, asked holding the one it has; emptied, its file's.
        stand(&mut v, &Target::LaneTitle(LaneId(3)));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::holding(LANE_TITLE, "dub.flac".to_owned()))
        );
        match v.answered(LANE_TITLE, "Dub (pt-BR)", &mut pairing, Some(&s), 80, &t) {
            Errand::TellSaying(
                Ask::Lane {
                    lane: LaneId(3),
                    change: LaneChange::Title { title },
                },
                _,
            ) => assert_eq!(title.as_deref(), Some("Dub (pt-BR)")),
            other => panic!("a name typed asked {other:?}"),
        }
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(
            v.answered(LANE_TITLE, " dub.flac ", &mut pairing, Some(&s), 80, &t),
            Errand::Nothing,
            "the name it has changes nothing"
        );
        v.key(Press::Enter, Some(&s), 80, &t);
        assert!(matches!(
            v.answered(LANE_TITLE, "", &mut pairing, Some(&s), 80, &t),
            Errand::TellSaying(
                Ask::Lane {
                    change: LaneChange::Title { title: None },
                    ..
                },
                _
            )
        ));
        // THE SUBTITLE'S PICTURE: a ring of the two films.
        v.key(Press::Escape, Some(&s), 80, &t);
        v.key(Press::Go(PageId::Lane(LaneId(4))), Some(&s), 80, &t);
        let row = stand(&mut v, &Target::LaneOver(LaneId(4)));
        assert!(
            row.fields.iter().any(|f| f.text == "one.mkv"),
            "{:?}",
            row.fields
        );
        assert_eq!(
            v.key(Press::More, Some(&s), 80, &t),
            Errand::Tell(Ask::Lane {
                lane: LaneId(4),
                change: LaneChange::Over {
                    over: Some(LaneId(2))
                }
            })
        );
        // ONE FILM: the row says it, and turns nothing.
        let mut alone = s.clone();
        alone.lanes.remove(1);
        let rows = v.rows(Some(&alone), 80, &t);
        assert!(
            rows.iter()
                .any(|r| r.spot == "lane-over" && r.target == Target::Nothing),
            "with one picture the row is a fact"
        );
    }
}
