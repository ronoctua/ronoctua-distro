//! THE MEDIA IN THE FILE: a page for each kind one file carries — its sounds,
//! its pictures and its subtitles (`player_page::PageId::Tracks`; the
//! subtitles' page is `crate::player_subtitles`) — every one of them a row,
//! and the one in use saying so as its value.
//!
//! # Only a kind there is a choice of
//!
//! The home offers a page only for a kind the file gives a choice of — two
//! sounds, two pictures, and the subtitles whenever there is a picture to lay
//! them over — and the transport's glyph and `k` stand for the same
//! (`TrackKind::with_a_choice`).
//! A page whose file stopped offering one says so (`Subject::Gone`), never
//! blank.
//!
//! # What choosing does
//!
//! What plays is LANES, and the file that plays alone already is: its
//! picture, its sound and its subtitles, each a lane playing one of the
//! file's tracks. `RET` on one asks the lane of its kind for it
//! (`Ask::Track`) — the file stays one file — and the rows move under it when
//! the state says the new one is in use. The subtitles are the film's: a
//! subtitle of it no lane plays yet is laid over it as a lane ([`chosen`]),
//! and a subtitle lane is shown and hidden on the subtitles' own page.
//!
//! # And playing one TOO
//!
//! A track nothing plays carries a glyph at the end of its row — and `a` on
//! it does the same: it plays TOO, a lane of its own beside the one in use
//! (`Ask::AddLane` of the same file and that track): a film's second language
//! for the other headphones, a second angle, a second subtitle. The file
//! becomes a set, keeping its place in the queue; which output a sound goes
//! to is the lane's own page's.
//!
//! # A cover is not a picture of the file
//!
//! mpv lists an embedded cover, a `cover.jpg` beside the file and a Matroska
//! cover attachment as video tracks, and the player says them as they are
//! (`Track::albumart`). Counted as pictures, every album and many films had
//! two, and offered a choice of one that is a still ([`of_kind`] leaves them
//! out).

use crate::key;
use crate::player::{Errand, Target, track_name};
use crate::player_page::TrackKind;
use crate::player_rows::Builder;
use ronoctua_player::own::{Lane, LaneId, LaneKind};
use ronoctua_player::wire::{Ask, LaneChange, State, Track};

/// The rows of the page of `kind`, under its title — the subtitles' page is
/// their own (`crate::player_subtitles`), and this is its "Inside the file".
pub fn page(kind: TrackKind, s: &State, b: &mut Builder<'_>) {
    rows_of(kind, s, b);
}

/// The lane of `kind` whose source's tracks the page of that kind offers:
/// the first one of the kind — for a file that plays alone, its own. For the
/// SUBTITLES, the film's own: the first subtitle lane playing a track of the
/// film the subtitles go over — a subtitle file laid over it is a lane of its
/// own, with tracks of none.
#[must_use]
pub fn lane_of(kind: TrackKind, s: &State) -> Option<&Lane> {
    let wanted: LaneKind = kind.lane();
    match kind {
        // WITH NO PICTURE — a sound file that carries subtitles — its own
        // subtitle lane, the first.
        TrackKind::Subtitles => match crate::player_subtitles::film(s) {
            Some(film) => s
                .lanes
                .iter()
                .find(|l| l.kind == wanted && l.source == film.source),
            None => s.lanes.iter().find(|l| l.kind == wanted),
        },
        TrackKind::Sound | TrackKind::Picture => s.lanes.iter().find(|l| l.kind == wanted),
    }
}

/// The file whose tracks of `kind` are offered: [`lane_of`]'s — and for the
/// subtitles the film's, whether a lane plays one of them yet or not.
fn source_of(kind: TrackKind, s: &State) -> Option<&str> {
    match kind {
        TrackKind::Subtitles => crate::player_subtitles::film(s)
            .or_else(|| lane_of(kind, s))
            .map(|l| l.source.as_str()),
        TrackKind::Sound | TrackKind::Picture => lane_of(kind, s).map(|l| l.source.as_str()),
    }
}

/// The tracks of `kind` the file of [`source_of`] carries — its covers left
/// out: a still is no picture to choose.
#[must_use]
pub fn of_kind(kind: TrackKind, s: &State) -> Vec<&Track> {
    let Some(location) = source_of(kind, s) else {
        return Vec::new();
    };
    s.sources
        .iter()
        .find(|source| source.location == location)
        .map(|source| {
            source
                .tracks
                .iter()
                .filter(|t| t.kind == kind.wire() && !t.albumart)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `lane` plays `t`: it is on, and plays that track — or, left to
/// the source's own choice, the one the source chose.
#[must_use]
pub fn plays(t: &Track, lane: &Lane) -> bool {
    lane.enabled && lane.track.map_or(t.selected, |id| id == t.id)
}

/// What `RET` on one of the file's media of `kind` asks: the lane of its kind
/// plays it ([`choose`]) — and a film's subtitle while none of its own is a
/// lane yet (the film had none, or all were taken out) is laid over it as a
/// lane of its own, from the set's start.
#[must_use]
pub fn chosen(kind: TrackKind, track: Option<i64>, s: &State) -> Errand {
    if let Some(lane) = lane_of(kind, s) {
        return Errand::Tell(choose(lane.id, track));
    }
    match (kind, track, source_of(kind, s)) {
        (TrackKind::Subtitles, Some(id), Some(film)) => Errand::Tell(Ask::AddLane {
            source: film.to_owned(),
            kind: LaneKind::Subtitle,
            track: Some(id),
            start_seconds: 0.0,
            over: crate::player_subtitles::film(s).map(|l| l.id),
        }),
        _ => Errand::Nothing,
    }
}

/// What choosing `track` asks of `lane`: that it plays it — on — or, with
/// none, that it is off (what "no subtitles" is).
#[must_use]
pub const fn choose(lane: LaneId, track: Option<i64>) -> Ask {
    match track {
        Some(id) => Ask::Track {
            lane,
            track: Some(id),
        },
        None => Ask::Lane {
            lane,
            change: LaneChange::Enabled { on: false },
        },
    }
}

/// What the row of a track says as its value: that it is the one in use, or
/// nothing.
pub fn in_use(b: &Builder<'_>, yes: bool) -> String {
    if yes {
        b.t(key::PLAYER_TRACK_IN_USE)
    } else {
        String::new()
    }
}

/// Whether ANOTHER lane of `kind` plays `t` of the same file — one added to
/// play it too ([`also`]).
#[must_use]
pub fn played_too(t: &Track, kind: TrackKind, s: &State) -> bool {
    let (Some(first), Some(source)) = (lane_of(kind, s), source_of(kind, s)) else {
        return false;
    };
    s.lanes.iter().any(|l| {
        l.id != first.id
            && l.kind == kind.lane()
            && l.source == source
            && l.enabled
            && l.track == Some(t.id)
    })
}

/// A row for every track of `kind` the file carries, by its name among the
/// others (`player::track_name`): the one in use says so, one another lane
/// plays says that, and one nothing plays carries the glyph that plays it
/// too.
pub fn rows_of(kind: TrackKind, s: &State, b: &mut Builder<'_>) {
    let kin = of_kind(kind, s);
    let lane = lane_of(kind, s);
    for t in &kin {
        let here = lane.is_some_and(|lane| plays(t, lane));
        let too = played_too(t, kind, s);
        let value = if too && !here {
            b.t(key::PLAYER_TRACK_TOO)
        } else {
            in_use(b, here)
        };
        let control = (!here && !too).then_some((
            ronoctua_tokens::icon::PLAYER_ALSO,
            Target::Also { kind, id: t.id },
        ));
        b.pair_with_control(
            track_name(t, &kin, b.texts),
            value,
            Target::Track {
                kind,
                id: Some(t.id),
            },
            &format!("track-{}-{}", kind.word(), t.id),
            control,
        );
    }
}

/// What playing track `id` of the file TOO asks: a lane of its own of the
/// same file and that track, from the set's start — said as it is sent, and
/// a track that plays already says so instead.
#[must_use]
pub fn also(kind: TrackKind, id: i64, s: &State, texts: &ronoctua_i18n::Texts) -> Errand {
    let kin = of_kind(kind, s);
    let (Some(source), Some(t)) = (source_of(kind, s), kin.iter().find(|t| t.id == id)) else {
        return Errand::Nothing;
    };
    if lane_of(kind, s).is_some_and(|lane| plays(t, lane)) || played_too(t, kind, s) {
        return Errand::Said(texts.t(key::PLAYER_TRACK_PLAYS_ALREADY));
    }
    // A SUBTITLE goes over the film whose track it is.
    let over = (kind == TrackKind::Subtitles)
        .then(|| crate::player_subtitles::film(s).map(|l| l.id))
        .flatten();
    Errand::TellSaying(
        Ask::AddLane {
            source: source.to_owned(),
            kind: kind.lane(),
            track: Some(id),
            start_seconds: 0.0,
            over,
        },
        texts.t_with(
            key::PLAYER_ALSO_ADDED,
            &[("name", track_name(t, &kin, texts).as_str())],
        ),
    )
}

/// The name of the one in use of `kind`, for the home's row that opens its
/// page — for subtitles switched off, the words the page's own row says.
pub fn in_use_of(kind: TrackKind, s: &State, texts: &ronoctua_i18n::Texts) -> String {
    let kin = of_kind(kind, s);
    lane_of(kind, s)
        .and_then(|lane| kin.iter().find(|t| plays(t, lane)))
        .map_or_else(
            || texts.t(key::PLAYER_SUBTITLES_OFF),
            |t| track_name(t, &kin, texts),
        )
}

/// The page `k` opens from `on`: the first kind with a choice, or — on a page
/// of the file's media already — the next one, round to the first. `None`
/// when the file offers no choice at all.
#[must_use]
pub fn next_page(on: Option<TrackKind>, s: &State) -> Option<TrackKind> {
    let kinds: Vec<TrackKind> = TrackKind::with_a_choice(s).collect();
    let at = on.and_then(|k| kinds.iter().position(|x| *x == k));
    match at {
        Some(i) => kinds.get((i + 1) % kinds.len()).copied(),
        None => kinds.first().copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Errand, PageId, Press, View};
    use ronoctua_player::wire::TrackKind as Kind;

    fn track(id: i64, kind: Kind, title: &str, selected: bool) -> Track {
        crate::player::fixture::track(id, kind, Some(title), selected)
    }

    /// The film playing alone, with these tracks.
    fn film_of(tracks: Vec<Track>) -> State {
        crate::player::fixture::alone("/films/a.mkv", tracks)
    }

    fn film() -> State {
        film_of(vec![
            track(1, Kind::Video, "Main", true),
            track(2, Kind::Video, "Other angle", false),
            track(1, Kind::Audio, "Stereo", true),
            track(2, Kind::Audio, "Commentary", false),
            track(1, Kind::Sub, "English", false),
        ])
    }

    /// `s`'s film with only the tracks `keep` keeps.
    fn keeping(s: &State, keep: impl Fn(&Track) -> bool) -> State {
        film_of(
            s.sources[0]
                .tracks
                .iter()
                .filter(|t| keep(t))
                .cloned()
                .collect(),
        )
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// FAILS IF: the home offers a page of the file's media for a kind it
    /// gives no choice of — one sound, one picture — or leaves out a kind it
    /// does, or the section is drawn with nothing to choose.
    #[test]
    fn tracks_show_only_with_a_choice() {
        let t = texts();
        let v = View::blank(None);
        let pages = |s: &State| -> Vec<PageId> {
            v.rows(Some(s), 80, &t)
                .into_iter()
                .filter_map(|r| match r.target {
                    Target::Go(p @ PageId::Tracks(_)) => Some(p),
                    _ => None,
                })
                .collect()
        };
        let s = film();
        assert_eq!(
            pages(&s),
            TrackKind::ALL.map(PageId::Tracks).to_vec(),
            "every kind with a choice"
        );
        // ONE SOUND AND NO PICTURE: nothing to choose, no section at all.
        let song = film_of(vec![track(1, Kind::Audio, "Stereo", true)]);
        assert!(pages(&song).is_empty());
        let heading = t.t(key::PLAYER_HEADING_MEDIA);
        assert!(
            !v.rows(Some(&song), 80, &t)
                .iter()
                .any(|r| r.fields.first().is_some_and(|f| f.text == heading)),
            "the section is drawn with nothing to choose"
        );
        // ONE SOUND, ONE PICTURE AND NO SUBTITLE: the subtitles still — a
        // file beside it, any file, can lie over the picture.
        let one = film_of(vec![
            track(1, Kind::Video, "Main", true),
            track(1, Kind::Audio, "Stereo", true),
        ]);
        assert_eq!(pages(&one), vec![PageId::Tracks(TrackKind::Subtitles)]);
        // AND A SOUND FILE THAT CARRIES A SUBTITLE: it can be switched on.
        let mut tracks = song.sources[0].tracks.clone();
        tracks.push(track(3, Kind::Sub, "Portuguese", false));
        assert_eq!(
            pages(&film_of(tracks)),
            vec![PageId::Tracks(TrackKind::Subtitles)]
        );
    }

    /// FAILS IF: a cover — embedded, beside the file, a Matroska attachment,
    /// which the player says as a picture that is a cover — counts as a
    /// picture of the file: every album and many films then offered a choice
    /// of one that is a still.
    #[test]
    fn a_cover_is_not_a_picture_of_the_file() {
        let mut cover = track(2, Kind::Video, "cover", false);
        cover.albumart = true;
        let s = film_of(vec![
            track(1, Kind::Video, "Main", true),
            cover,
            track(1, Kind::Audio, "Stereo", true),
        ]);
        assert_eq!(of_kind(TrackKind::Picture, &s).len(), 1);
        assert!(!TrackKind::Picture.a_choice_in(&s));
        assert!(
            !TrackKind::with_a_choice(&s).any(|k| k == TrackKind::Picture),
            "a choice of a still"
        );
    }

    /// FAILS IF: a row of the sounds or the subtitles stops asking the
    /// player for that one — "no subtitles" included — or the one in use
    /// stops saying so, or a picture's row starts acting while the player
    /// cannot play it, or the picture's page stops saying why.
    #[test]
    fn a_track_is_chosen_on_its_page_whatever_its_kind() {
        let t = texts();
        let s = film();
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Sound)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        let commentary = rows
            .iter()
            .position(|r| {
                r.target
                    == Target::Track {
                        kind: TrackKind::Sound,
                        id: Some(2),
                    }
            })
            .expect("the commentary is a row");
        let stereo = rows
            .iter()
            .find(|r| r.fields.first().is_some_and(|f| f.text == "Stereo"))
            .expect("the stereo is a row");
        assert_eq!(
            stereo.fields.get(1).map(|f| f.text.clone()),
            Some(t.t(key::PLAYER_TRACK_IN_USE)),
            "the one in use does not say so"
        );
        v.clicked(commentary, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Track {
                lane: LaneId(2),
                track: Some(2)
            }),
            "the file's sound lane plays the commentary"
        );
        // THE SUBTITLES: inside the file, the film's subtitle lane plays
        // the one chosen — and its lane, hidden while none is chosen, shows
        // and hides by its chip.
        v.key(Press::Escape, Some(&s), 80, &t);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        let english = rows
            .iter()
            .position(|r| {
                r.target
                    == Target::Track {
                        kind: TrackKind::Subtitles,
                        id: Some(1),
                    }
            })
            .expect("the English subtitles are a row");
        v.clicked(english, &rows);
        v.clicked(english, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Track {
                lane: LaneId(3),
                track: Some(1)
            })
        );
        let lane = rows
            .iter()
            .position(|r| r.target == Target::Subtitle(LaneId(3)))
            .expect("the film's subtitle lane is a row");
        let chip = rows[lane].chips.first().expect("its chip").at;
        assert!(v.clicked_at(lane, chip, &rows));
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Lane {
                lane: LaneId(3),
                change: LaneChange::Enabled { on: true }
            }),
            "the chip shows the hidden lane"
        );
        // THE PICTURE: chosen as the others are, by its lane.
        v.key(Press::Escape, Some(&s), 80, &t);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Picture)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        let angle = rows
            .iter()
            .position(|r| r.fields.first().is_some_and(|f| f.text == "Other angle"))
            .expect("the other angle is a row");
        v.clicked(angle, &rows);
        v.clicked(angle, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Track {
                lane: LaneId(1),
                track: Some(2)
            }),
            "the file's picture lane plays the other angle"
        );
    }

    /// FAILS IF: one of the file's media that nothing plays stops carrying the
    /// glyph that plays it TOO — a click acts at once — or `a` on its row
    /// stops doing the same: a lane of its own of the same file and that
    /// track; or one already playing offers it, or plays it twice.
    #[test]
    fn one_of_the_file_s_media_plays_too() {
        let t = texts();
        let mut s = film();
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Sound)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        let commentary = rows
            .iter()
            .position(|r| r.fields.first().is_some_and(|f| f.text == "Commentary"))
            .expect("the commentary is a row");
        let stereo = rows
            .iter()
            .find(|r| r.fields.first().is_some_and(|f| f.text == "Stereo"))
            .expect("the stereo is a row");
        assert!(stereo.cells.is_empty(), "the one in use offers to play too");
        let cell = rows[commentary]
            .cells
            .first()
            .expect("the commentary carries the glyph");
        assert!(cell.control);
        assert!(
            rows[commentary]
                .fields
                .iter()
                .any(|f| f.at == cell.from
                    && f.text == ronoctua_tokens::icon::PLAYER_ALSO.to_string()),
            "the glyph is drawn where it is taken"
        );
        let too = Errand::TellSaying(
            Ask::AddLane {
                source: "/films/a.mkv".into(),
                kind: ronoctua_player::own::LaneKind::Audio,
                track: Some(2),
                start_seconds: 0.0,
                over: None,
            },
            t.t_with(key::PLAYER_ALSO_ADDED, &[("name", "Commentary")]),
        );
        assert!(
            v.clicked_at(commentary, cell.from, &rows),
            "a click acts at once"
        );
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), too);
        assert_eq!(crate::player::pressed("a"), Some(Press::Add));
        assert_eq!(v.key(Press::Add, Some(&s), 80, &t), too, "`a` on its row");
        // PLAYING TOO NOW: another lane plays it, and it says so.
        let mut lane = Lane::new(LaneId(4), LaneKind::Audio, "/films/a.mkv");
        lane.track = Some(2);
        s.lanes.push(lane);
        let rows = v.rows(Some(&s), 80, &t);
        let row = &rows[commentary];
        assert!(row.cells.is_empty(), "offered while it plays");
        assert_eq!(
            row.fields.get(1).map(|f| f.text.clone()),
            Some(t.t(key::PLAYER_TRACK_TOO))
        );
        assert_eq!(
            v.key(Press::Add, Some(&s), 80, &t),
            Errand::Said(t.t(key::PLAYER_TRACK_PLAYS_ALREADY))
        );
    }

    /// FAILS IF: `k` stops opening the first kind with a choice, or stops
    /// walking to the next one from a page of them — the pictures among them
    /// — or opens anything when the file offers no choice.
    #[test]
    fn k_walks_the_kinds_with_a_choice() {
        let s = film();
        assert_eq!(next_page(None, &s), Some(TrackKind::Sound));
        assert_eq!(
            next_page(Some(TrackKind::Sound), &s),
            Some(TrackKind::Picture)
        );
        assert_eq!(
            next_page(Some(TrackKind::Picture), &s),
            Some(TrackKind::Subtitles)
        );
        // TWO PICTURES AND NOTHING ELSE: a choice, the glyph and `k`.
        let pictures = keeping(&s, |t| t.kind == Kind::Video);
        assert_eq!(next_page(None, &pictures), Some(TrackKind::Picture));
        assert!(crate::player_transport::shown(
            "player-tracks",
            Some(&pictures)
        ));
        assert!(crate::player_transport::shown("player-tracks", Some(&s)));
        assert_eq!(
            next_page(Some(TrackKind::Subtitles), &s),
            Some(TrackKind::Sound)
        );
        // ONE PICTURE, TWO SOUNDS AND NO SUBTITLE: the sounds, and the
        // subtitles a file can lay over the picture.
        let plain = keeping(&s, |t| {
            t.kind != Kind::Sub && (t.kind != Kind::Video || t.id == 1)
        });
        assert_eq!(
            next_page(Some(TrackKind::Sound), &plain),
            Some(TrackKind::Subtitles)
        );
        // ONE SOUND AND NOTHING ELSE: nothing to choose, and `k` opens
        // nothing.
        let song = keeping(&plain, |t| t.kind == Kind::Audio && t.id == 1);
        assert_eq!(next_page(None, &song), None);
        // THROUGH THE VIEW: one page at a time, never a stack of them.
        let t = texts();
        let mut v = View::blank(None);
        assert_eq!(crate::player::pressed("k"), Some(Press::Tracks));
        v.key(Press::Tracks, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Tracks(TrackKind::Sound));
        v.key(Press::Tracks, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Tracks(TrackKind::Picture));
        assert_eq!(v.levels.len(), 2, "a second page of media over the first");
        assert_eq!(v.key(Press::Escape, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Home);
    }
}
