//! THE SUBTITLES' PAGE (`player_page::PageId::Tracks`, of the subtitles):
//! every subtitle LANE over the picture, and where more come from.
//!
//! # A lane a row, its colour on a chip
//!
//! Up to six subtitles lie over one picture at once, each in its own colour
//! (`session::subtitle_slot`: its place among the subtitles over that
//! picture, on or off, so hiding one never repaints the others). Each is a
//! row: its colour CHIP, its language and its name, and what it is doing — at
//! the foot of the picture or at its top, hidden, getting ready, or refused
//! with the sentence under it.
//!
//! The lane colours are measured against a film's dark picture and the black
//! rim every subtitle wears (`subtitle.edge`), never against a page: on a
//! light theme the first lane's yellow would vanish into it. So a lane's
//! colour is drawn ONLY inside a chip whose ground is that rim
//! (`player_rows::Chip`, `chrome::chip`) — a small sample of how the lane
//! looks over the film.
//!
//! # What each gesture does
//!
//! - a click on the CHIP shows or hides the lane at once, as `v` on the row
//!   does (the lane `enabled`, `Ask::Lane`);
//! - `←`/`→` move it between the foot of the picture and the top
//!   (`LaneChange::Stack`), a ring of two;
//! - `d` takes it out;
//! - `RET` — and the second click on the row — opens the lane's own page
//!   (`crate::player_lane`).
//!
//! # Where more come from
//!
//! - **Add a file**: the files, listing every subtitle file — and a file of
//!   any name the person picks, taken for one because they said so
//!   (`player::Purpose::Subtitle`).
//! - **Find subtitles**: the page that searches the internet for this film's,
//!   with the person's key (`crate::player_find`).
//! - **Beside the file**: the subtitle files in the film's folder named after
//!   it (`subtitles::named_after`: `film.srt`, `film.pt-BR.srt`), read once a
//!   film ([`Beside`]).
//! - **Inside the file**: the film's own subtitles, as the media in the file
//!   has them (`crate::player_tracks`): `RET` has the film's subtitle lane
//!   show that one, the glyph at its end lays it over as a lane of its own.
//!
//! A seventh over one picture is refused here, in the sentence the player
//! would answer with, before anything is sent — and the player refuses it
//! too.

use crate::key;
use crate::player::{Errand, PageId, Purpose, Target, TrackKind, trouble_sentence};
use crate::player_queue::lane_name;
use crate::player_rows::Builder;
use crate::wake::Wake;
use ronoctua_player::limit::{Limit, TooMany};
use ronoctua_player::own::{Lane, LaneKind, Stack};
use ronoctua_player::session::{SUBTITLES_PER_PICTURE, picture_of, subtitle_slot};
use ronoctua_player::wire::{Ask, LaneState, LaneStatus, State, Trouble};
use ronoctua_tokens as tokens;
use std::path::{Path, PathBuf};

/// The six lane colours, by a lane's slot among the subtitles over its
/// picture (`session::subtitle_slot`) — the same the daemon paints them in.
const COLOURS: [tokens::Color; SUBTITLES_PER_PICTURE] = [
    tokens::color::SUBTITLE_LANE_ONE,
    tokens::color::SUBTITLE_LANE_TWO,
    tokens::color::SUBTITLE_LANE_THREE,
    tokens::color::SUBTITLE_LANE_FOUR,
    tokens::color::SUBTITLE_LANE_FIVE,
    tokens::color::SUBTITLE_LANE_SIX,
];

/// The colour subtitle lane `lane` wears over its picture — `None` for one
/// past the sixth, which the player refuses and paints in nothing.
#[must_use]
pub fn colour_of(s: &State, lane: ronoctua_player::own::LaneId) -> Option<tokens::Color> {
    subtitle_slot(&s.lanes, lane).and_then(|slot| COLOURS.get(slot).copied())
}

/// The film subtitles go over: the first lane of pictures — a cover is none.
#[must_use]
pub fn film(s: &State) -> Option<&Lane> {
    s.lanes.iter().find(|l| l.kind == LaneKind::Video)
}

/// What a read of the folder leaves for the door: the film it was read for,
/// and its files, once they are read.
type Slot = std::sync::Arc<std::sync::Mutex<Option<(String, Vec<PathBuf>)>>>;

/// The subtitle files beside the film that plays, named after it — READ ONCE
/// A FILM, and again when the page opens: the rows are built several times a
/// frame, and a folder read on each was the listing that froze a door before
/// (`player_kinds::glyph_if_played`).
///
/// OFF THE LOOP: a film on a network mount is a folder that takes seconds to
/// list, and the door's loop is the desktop's. A door with a loop hands the
/// view a [`Wake`], the folder is read on a thread, and the door is woken to
/// take the list; a door with none — a photograph, a test — has nothing else
/// to do meanwhile, and reads it in place.
#[derive(Debug, Default)]
pub struct Beside {
    /// The film they were read for.
    film: Option<String>,
    /// The files, sorted.
    pub files: Vec<PathBuf>,
    /// The film `files` were read for — they stay on the page while the
    /// same film's folder is read again, and never under another film.
    files_of: Option<String>,
    /// The read under way, when one is.
    coming: Option<Slot>,
}

impl Beside {
    /// The files beside the film `state` plays — read only when that film is
    /// not the one they were read for — and a read that finished, taken.
    pub fn look(&mut self, state: Option<&State>, wake: Option<&Wake>) {
        if let Some((of, files)) = self
            .coming
            .as_ref()
            .and_then(|slot| slot.lock().ok()?.take())
        {
            self.files = files;
            self.files_of = Some(of);
            self.coming = None;
        }
        let film = state.and_then(film).map(|l| l.source.clone());
        if film == self.film {
            return;
        }
        self.film.clone_from(&film);
        self.coming = None;
        if self.files_of != film {
            self.files = Vec::new();
            self.files_of = None;
        }
        let Some(film) = film else {
            return;
        };
        let Some(wake) = wake else {
            self.files = beside(&film);
            self.files_of = Some(film);
            return;
        };
        let slot: Slot = std::sync::Arc::default();
        self.coming = Some(slot.clone());
        let wake = wake.clone();
        let born = std::thread::Builder::new()
            .name("player-beside".to_owned())
            .spawn(move || {
                let files = beside(&film);
                if let Ok(mut s) = slot.lock() {
                    *s = Some((film, files));
                }
                wake.call();
            });
        if let Err(e) = born {
            tracing::warn!(error = %e, "could not start the thread that reads the film's folder");
            self.coming = None;
        }
    }

    /// Read again at the next look: the page opened again, and the folder
    /// may hold files it did not when it was last read.
    pub fn forget(&mut self) {
        self.film = None;
        self.coming = None;
    }

    /// Whether the folder is being read now: the page says nothing of what
    /// is beside the file until it knows.
    #[must_use]
    pub const fn reading(&self) -> bool {
        self.coming.is_some()
    }
}

/// The subtitle files in the folder of `film`, named after it — none for a
/// film on the network, which has no folder here.
fn beside(film: &str) -> Vec<PathBuf> {
    if film.contains("://") {
        return Vec::new();
    }
    let Some(folder) = Path::new(film).parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let extension = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            !name.starts_with('.')
                && ronoctua_player::subtitles::claims(extension)
                && ronoctua_player::subtitles::named_after(&p.to_string_lossy(), film)
                && p.is_file()
        })
        .collect();
    out.sort();
    out
}

/// A subtitle lane's name: the film's own track by its name among the film's
/// subtitles, else the title it was given, else its file's name — with its
/// language in front when the name does not say it.
#[must_use]
pub fn name(lane: &Lane, s: &State, texts: &ronoctua_i18n::Texts, width_cells: u16) -> String {
    let kin = crate::player_tracks::of_kind(TrackKind::Subtitles, s);
    let own = film(s)
        .filter(|f| f.source == lane.source)
        .and(lane.track)
        .and_then(|id| kin.iter().find(|t| t.id == id));
    let named = own.map_or_else(
        || lane_name(lane, &s.lanes, texts, width_cells),
        |t| crate::player::track_name(t, &kin, texts),
    );
    let lang = lane
        .lang
        .as_deref()
        .or_else(|| own.and_then(|t| t.lang.as_deref()))
        .map(str::trim)
        .filter(|l| !l.is_empty());
    // THE LANGUAGE BY ITS NAME, from the table the searches bring — the code
    // as the file says it when the table has no such language: a guessed
    // name would be a word the file never said.
    let lang = lang.map(|l| crate::player_find::tongue_name(l, texts));
    match lang {
        Some(l) if !named.contains(&l) => format!("{l} · {named}"),
        _ => named,
    }
}

/// What a subtitle lane is doing, in its row's value: hidden, waiting,
/// getting ready, refused — and, while it shows, where its lines sit.
fn doing(lane: &Lane, now: Option<&LaneState>, texts: &ronoctua_i18n::Texts) -> String {
    if !lane.enabled {
        return texts.t(key::PLAYER_HIDDEN);
    }
    texts.t(match now.map(|l| &l.status) {
        None | Some(LaneStatus::Pending) => key::PLAYER_LANE_PENDING,
        Some(LaneStatus::Preparing) => key::PLAYER_LANE_PREPARING,
        Some(LaneStatus::Ended) => key::PLAYER_LANE_ENDED,
        Some(LaneStatus::Refused(_)) => key::PLAYER_LANE_REFUSED,
        Some(LaneStatus::Playing) => stack_word(lane.stack),
    })
}

/// Where a subtitle's lines sit, in words — its row's value and its page's.
#[must_use]
pub const fn stack_word(stack: Stack) -> &'static str {
    match stack {
        Stack::Bottom => key::PLAYER_SUBTITLE_BOTTOM,
        Stack::Top => key::PLAYER_SUBTITLE_TOP,
    }
}

/// Whether lane `lane` keeps its own typography only while it is alone: an
/// ASS file (or a film's ASS track) shown with another subtitle over the same
/// picture is merged into one script, in its lane's colour, keeping its signs
/// and losing its styles — which its row says.
fn merged_plain(lane: &Lane, s: &State) -> bool {
    let styled = |l: &Lane| {
        let extension = Path::new(&l.source)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let own_codec = film(s)
            .filter(|f| f.source == l.source)
            .and(l.track)
            .and_then(|id| {
                crate::player_tracks::of_kind(TrackKind::Subtitles, s)
                    .into_iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.codec.clone())
            });
        matches!(extension.as_str(), "ass" | "ssa")
            || matches!(own_codec.as_deref(), Some("ass" | "ssa"))
    };
    let picture = picture_of(&s.lanes, lane);
    lane.enabled
        && styled(lane)
        && s.lanes.iter().any(|o| {
            o.id != lane.id
                && o.kind == LaneKind::Subtitle
                && o.enabled
                && picture_of(&s.lanes, o) == picture
        })
}

/// The page's rows, under its title.
pub fn page(s: &State, beside: &Beside, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_SUBTITLES);
    for lane in s.lanes.iter().filter(|l| l.kind == LaneKind::Subtitle) {
        let named = name(lane, s, b.texts, b.room);
        let now = s.lane_states.iter().find(|l| l.lane == lane.id);
        let value = doing(lane, now, b.texts);
        let spot = format!("subtitle-{}", lane.id.0);
        match colour_of(s, lane.id) {
            Some(colour) => b.pair_with_chip(
                (
                    if lane.enabled {
                        tokens::icon::PLAYER_SUBTITLE_SHOWN
                    } else {
                        tokens::icon::PLAYER_SUBTITLE_HIDDEN
                    },
                    colour,
                ),
                named,
                value,
                (Target::Subtitle(lane.id), Target::Shown(lane.id)),
                &spot,
            ),
            // PAST THE SIXTH there is no colour to wear, and no chip: the
            // player refuses it, and its row says so under it.
            None => b.pair_with_mark(
                tokens::icon::PLAYER_SUBTITLES,
                named,
                value,
                Target::Subtitle(lane.id),
                &spot,
            ),
        }
        if let Some(LaneStatus::Refused(t)) = now.map(|l| &l.status) {
            b.warning(&trouble_sentence(t, b.texts));
        } else if merged_plain(lane, s) {
            b.note(key::PLAYER_LANE_MERGED_PLAIN);
        }
    }
    // WHERE MORE COME FROM: any file, and the network — the page that
    // searches for this film's.
    for page in [PageId::Pick(Purpose::Subtitle), PageId::FindSubtitles] {
        b.pair(
            b.t(page.key()),
            String::new(),
            Target::Go(page),
            page.spot(),
        );
    }
    b.heading(key::PLAYER_HEADING_BESIDE);
    if beside.files.is_empty() && !beside.reading() {
        b.note(key::PLAYER_SUBTITLES_BESIDE_NONE);
    }
    for path in &beside.files {
        let location = path.to_string_lossy();
        let plays = s
            .lanes
            .iter()
            .any(|l| l.kind == LaneKind::Subtitle && l.source == location);
        b.pair(
            crate::player::name_of(
                &ronoctua_player::queue::Name::Location(location.to_string()),
                b.texts,
                b.room,
            ),
            crate::player_tracks::in_use(b, plays),
            Target::Beside(path.clone()),
            &format!("beside-{location}"),
        );
    }
    b.heading(key::PLAYER_HEADING_INSIDE);
    if crate::player_tracks::of_kind(TrackKind::Subtitles, s).is_empty() {
        b.note(key::PLAYER_SUBTITLES_INSIDE_NONE);
    }
    crate::player_tracks::rows_of(TrackKind::Subtitles, s, b);
}

/// What laying the subtitle file at `location` over the picture asks: a lane
/// of its own over the film, from the set's start — refused in a sentence
/// when there is no picture, when six already lie over it, or when it lies
/// there already.
#[must_use]
pub fn lay_over(location: &str, s: &State, texts: &ronoctua_i18n::Texts) -> Errand {
    let named = || {
        crate::player::name_of(
            &ronoctua_player::queue::Name::Location(location.to_owned()),
            texts,
            crate::player::NAME_IN_A_SENTENCE,
        )
    };
    let Some(picture) = film(s) else {
        return Errand::Said(trouble_sentence(
            &Trouble::NoPicture {
                path: location.to_owned(),
            },
            texts,
        ));
    };
    let over: Vec<&Lane> = s
        .lanes
        .iter()
        .filter(|l| l.kind == LaneKind::Subtitle && picture_of(&s.lanes, l) == Some(picture.id))
        .collect();
    if over
        .iter()
        .any(|l| l.source == location && l.track.is_none())
    {
        return Errand::Said(texts.t(key::PLAYER_TRACK_PLAYS_ALREADY));
    }
    // THE SEVENTH, refused as the player would refuse it — before it is
    // sent, and in the same words.
    if over.len() >= SUBTITLES_PER_PICTURE {
        return Errand::Said(trouble_sentence(
            &Trouble::TooMany(TooMany {
                what: Limit::Subtitles,
                most: SUBTITLES_PER_PICTURE,
            }),
            texts,
        ));
    }
    Errand::TellSaying(
        Ask::AddLane {
            source: location.to_owned(),
            kind: LaneKind::Subtitle,
            track: None,
            start_seconds: 0.0,
            over: Some(picture.id),
        },
        texts.t_with(key::PLAYER_SUBTITLE_ADDED, &[("name", &named())]),
    )
}

/// What the home's row of the subtitles says: none shown, the one shown by
/// its name, or how many.
#[must_use]
pub fn summary(s: &State, texts: &ronoctua_i18n::Texts) -> String {
    let shown: Vec<&Lane> = s
        .lanes
        .iter()
        .filter(|l| l.kind == LaneKind::Subtitle && l.enabled)
        .collect();
    match shown.as_slice() {
        [] => texts.t(key::PLAYER_SUBTITLES_OFF),
        [one] => name(one, s, texts, crate::player::NAME_IN_A_SENTENCE),
        more => texts.t_args(
            key::PLAYER_SUBTITLES_SHOWN,
            &[(
                "count",
                ronoctua_i18n::Arg::Number(i64::try_from(more.len()).unwrap_or(i64::MAX)),
            )],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::fixture::{alone, track};
    use crate::player::{Press, View};
    use ronoctua_player::own::LaneId;
    use ronoctua_player::wire::{LaneChange, TrackKind as Kind};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// The film, its English subtitles on (lane 3), and a subtitle file laid
    /// over it (lane 4) and one hidden (lane 5).
    fn film_with_lanes() -> State {
        let mut english = track(1, Kind::Sub, None, true);
        english.lang = Some("en".into());
        let mut s = alone(
            "/films/a.mkv",
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, None, true),
                english,
                track(2, Kind::Sub, Some("Signs"), false),
            ],
        );
        let mut file = Lane::new(LaneId(4), LaneKind::Subtitle, "/films/a.pt-BR.srt");
        file.lang = Some("pt-BR".into());
        file.title = Some("Commentary".into());
        let mut hidden = Lane::new(LaneId(5), LaneKind::Subtitle, "/films/a.es.srt");
        hidden.enabled = false;
        s.lanes.push(file);
        s.lanes.push(hidden);
        s
    }

    fn row_of(rows: &[crate::player_rows::Row], target: &Target) -> usize {
        rows.iter()
            .position(|r| &r.target == target)
            .unwrap_or_else(|| panic!("no row for {target:?}"))
    }

    /// FAILS IF: a subtitle lane's colour is drawn anywhere but on its chip —
    /// a label or a value in a lane colour on the page's ground, where no
    /// gate measured it — or a lane stops wearing the colour of its place
    /// among the subtitles over the picture, or a chip stops being where the
    /// click that turns it is taken.
    #[test]
    fn a_lane_s_colour_is_drawn_only_on_its_chip() {
        let t = texts();
        let s = film_with_lanes();
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        for (lane, colour) in [
            (3, tokens::color::SUBTITLE_LANE_ONE),
            (4, tokens::color::SUBTITLE_LANE_TWO),
            (5, tokens::color::SUBTITLE_LANE_THREE),
        ] {
            let r = &rows[row_of(&rows, &Target::Subtitle(LaneId(lane)))];
            let chip = r.chips.first().expect("a chip");
            let glyph = r
                .fields
                .iter()
                .find(|f| f.color == colour)
                .expect("the glyph in the lane's colour");
            // THE GLYPH IN THE CHIP'S MIDDLE CELL, the chip the control.
            assert_eq!(glyph.at, chip.at + 1, "lane {lane}");
            let control = r.cells.iter().find(|c| c.control).expect("the control");
            assert_eq!(
                (control.from, control.width),
                (chip.at, tokens::space::PLAYER_CHIPCELLS)
            );
            assert_eq!(control.target, Target::Shown(LaneId(lane)));
        }
        let lanes = COLOURS;
        for r in &rows {
            for f in &r.fields {
                if lanes.contains(&f.color) {
                    assert!(
                        r.chips.iter().any(|c| f.at == c.at + 1),
                        "a lane colour off a chip: {:?}",
                        f.text
                    );
                }
            }
        }
        // HIDDEN, it says so — and its chip draws the outlined glyph.
        let hidden = &rows[row_of(&rows, &Target::Subtitle(LaneId(5)))];
        assert!(
            hidden
                .fields
                .iter()
                .any(|f| f.text == t.t(key::PLAYER_HIDDEN))
        );
        assert!(
            hidden
                .fields
                .iter()
                .any(|f| f.text == tokens::icon::PLAYER_SUBTITLE_HIDDEN.to_string())
        );
        // THE LANGUAGE BY ITS NAME, in front of the lane's own.
        let file = &rows[row_of(&rows, &Target::Subtitle(LaneId(4)))];
        assert!(
            file.fields
                .iter()
                .any(|f| f.text == "Portuguese (Brazil) · Commentary"),
            "{:?}",
            file.fields
        );
    }

    /// FAILS IF: a gesture on a subtitle lane stops doing what its page
    /// teaches — the chip's click and `v` show or hide it, the arrows move it
    /// between the foot and the top, `d` takes it out, and `RET` (the second
    /// click) opens its own page.
    #[test]
    fn every_gesture_on_a_subtitle_lane() {
        let t = texts();
        let s = film_with_lanes();
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Subtitle(LaneId(4)));
        let chip = rows[at].chips[0];
        let lane = |change| {
            Errand::Tell(Ask::Lane {
                lane: LaneId(4),
                change,
            })
        };
        assert!(v.clicked_at(at, chip.at, &rows), "the chip acts at once");
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            lane(LaneChange::Enabled { on: false })
        );
        assert_eq!(crate::player::pressed("v"), Some(Press::Shown));
        assert_eq!(
            v.key(Press::Shown, Some(&s), 80, &t),
            lane(LaneChange::Enabled { on: false })
        );
        for press in [Press::Less, Press::More] {
            assert_eq!(
                v.key(press, Some(&s), 80, &t),
                lane(LaneChange::Stack { stack: Stack::Top })
            );
        }
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            lane(LaneChange::Remove)
        );
        // THE SECOND CLICK ON THE ROW, and `RET`: its own page.
        let label = rows[at]
            .fields
            .iter()
            .find(|f| f.at > chip.at + 1)
            .expect("a label");
        v.clicked_at(at, label.at, &rows);
        assert!(v.clicked_at(at, label.at, &rows), "the second click acts");
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Lane(LaneId(4)));
        // AND `Esc` IS BACK ON ITS ROW.
        v.key(Press::Escape, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Tracks(TrackKind::Subtitles));
        assert_eq!(v.cursor(), at);
    }

    /// FAILS IF: a file laid over the picture stops being a subtitle lane over
    /// the film — from the files beside it, the files, or typed — or the
    /// seventh over one picture is sent instead of refused in the player's
    /// own sentence, or one with no picture to go over is sent at all.
    #[test]
    fn a_subtitle_file_lies_over_the_film_and_the_seventh_is_refused() {
        let t = texts();
        let mut s = film_with_lanes();
        let ask = |source: &str| Ask::AddLane {
            source: source.into(),
            kind: LaneKind::Subtitle,
            track: None,
            start_seconds: 0.0,
            over: Some(LaneId(1)),
        };
        assert!(matches!(
            lay_over("/films/a.fr.txt", &s, &t),
            Errand::TellSaying(a, _) if a == ask("/films/a.fr.txt")
        ));
        assert_eq!(
            lay_over("/films/a.pt-BR.srt", &s, &t),
            Errand::Said(t.t(key::PLAYER_TRACK_PLAYS_ALREADY))
        );
        for id in 6..9 {
            s.lanes.push(Lane::new(
                LaneId(id),
                LaneKind::Subtitle,
                format!("/films/{id}.srt"),
            ));
        }
        assert_eq!(
            lay_over("/films/a.fr.srt", &s, &t),
            Errand::Said(trouble_sentence(
                &Trouble::TooMany(TooMany {
                    what: Limit::Subtitles,
                    most: 6
                }),
                &t
            ))
        );
        let song = alone("/m/a.flac", vec![track(1, Kind::Audio, None, true)]);
        assert!(matches!(lay_over("/m/a.srt", &song, &t), Errand::Said(_)));
        // THE FILES, for a subtitle: the pick is a lane, and the page closes.
        let dir = std::env::temp_dir().join(format!("ronoctua-subtitles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let notes = dir.join("notes.txt");
        std::fs::write(&notes, "1\n00:00:01,000 --> 00:00:02,000\nhi\n").expect("a file");
        let s = film_with_lanes();
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        v.pick_at(&dir, Purpose::Subtitle);
        let rows = v.rows(Some(&s), 80, &t);
        let (at, from) = rows
            .iter()
            .enumerate()
            .find_map(|(n, r)| {
                r.cells
                    .iter()
                    .find(|c| {
                        c.target == Target::Picker(crate::file_picker::Target::File(notes.clone()))
                    })
                    .map(|c| (n, c.from))
            })
            .expect("a text file is listed when a subtitle is asked for");
        v.clicked_at(at, from, &rows);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(v.clicked_at(at, from, &rows), "the second click acts");
        let said = v.key(Press::Enter, Some(&s), 80, &t);
        assert!(
            matches!(&said, Errand::TellSaying(a, _) if *a == ask(&notes.to_string_lossy())),
            "{said:?}"
        );
        assert_eq!(v.page(), PageId::Tracks(TrackKind::Subtitles));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a door WITH A LOOP has the folder beside the film read on
    /// that loop — a film on a network mount froze the footer — or is never
    /// woken to take what the thread read, or the page says "nothing beside
    /// the file" before it knows.
    #[test]
    fn the_folder_beside_the_film_is_read_off_the_loop() {
        let t = texts();
        let dir = std::env::temp_dir().join(format!("ronoctua-beside-off-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        for name in ["film.mkv", "film.srt"] {
            std::fs::write(dir.join(name), "").expect("a file");
        }
        let film = dir.join("film.mkv").to_string_lossy().into_owned();
        let s = alone(
            &film,
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, None, true),
            ],
        );
        let (woken, wakes) = std::sync::mpsc::channel::<()>();
        let woken = std::sync::Mutex::new(woken);
        let wake = Wake::new(move || {
            let _ = woken.lock().expect("the channel").send(());
        });
        let mut v = View::blank(None).waking(wake);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        assert!(v.beside.reading(), "the folder was read on the loop");
        let none = t.t(key::PLAYER_SUBTITLES_BESIDE_NONE);
        let said = |v: &View| {
            v.rows(Some(&s), 80, &t).iter().any(|r| {
                r.fields
                    .iter()
                    .any(|f| f.text.len() > 3 && none.starts_with(&f.text))
            })
        };
        assert!(!said(&v), "nothing beside, said before it was known");
        // WOKEN UNTIL THE FOLDER IS READ: another thread of the view's may
        // wake the door first, under a loaded machine.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            wakes
                .recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
                .expect("the door is woken when the folder is read");
            v.settle_in(Some(&s), 80, &t);
            if !v.beside.reading() {
                break;
            }
        }
        assert!(!v.beside.reading());
        assert_eq!(v.beside.files, vec![dir.join("film.srt")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the files beside the film stop being listed by the one rule
    /// of a subtitle named after its film, or a file already laid over it
    /// stops saying so, or the page stops leading to the search on the
    /// internet.
    #[test]
    fn beside_the_file_and_the_search_on_the_internet() {
        let t = texts();
        let dir = std::env::temp_dir().join(format!("ronoctua-beside-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        for name in [
            "film.mkv",
            "film.srt",
            "film.pt-BR.srt",
            "other.srt",
            "film.txt",
            ".film.srt",
        ] {
            std::fs::write(dir.join(name), "").expect("a file");
        }
        let film = dir.join("film.mkv").to_string_lossy().into_owned();
        let mut s = alone(
            &film,
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, None, true),
            ],
        );
        s.lanes.push(Lane::new(
            LaneId(4),
            LaneKind::Subtitle,
            dir.join("film.srt").to_string_lossy().into_owned(),
        ));
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            80,
            &t,
        );
        assert_eq!(
            v.beside.files,
            vec![dir.join("film.pt-BR.srt"), dir.join("film.srt")]
        );
        let rows = v.rows(Some(&s), 80, &t);
        let laid = &rows[row_of(&rows, &Target::Beside(dir.join("film.srt")))];
        assert_eq!(
            laid.fields.get(1).map(|f| f.text.clone()),
            Some(t.t(key::PLAYER_TRACK_IN_USE))
        );
        let other = row_of(&rows, &Target::Beside(dir.join("film.pt-BR.srt")));
        v.clicked(other, &rows);
        v.clicked(other, &rows);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(
                Ask::AddLane {
                    kind: LaneKind::Subtitle,
                    ..
                },
                _
            )
        ));
        let search = row_of(&rows, &Target::Go(PageId::FindSubtitles));
        v.clicked(search, &rows);
        v.clicked(search, &rows);
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::FindSubtitles);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a lane the player refused stops saying why under its row,
    /// or an ASS file shown with another stops saying it loses its own
    /// styles.
    #[test]
    fn a_refused_lane_says_why_and_a_merged_one_what_it_loses() {
        let t = texts();
        let mut s = film_with_lanes();
        s.lanes[3].source = "/films/a.pt-BR.ass".into();
        s.lane_states.push(LaneState {
            lane: LaneId(5),
            duration_seconds: None,
            status: LaneStatus::Refused(Trouble::TooMany(TooMany {
                what: Limit::SubtitlePictures,
                most: 1,
            })),
            picture: false,
            drift_ms: 0,
            master: false,
            size: None,
            cover: false,
            languages: Vec::new(),
        });
        s.lanes[4].enabled = true;
        let mut v = View::blank(None);
        v.key(
            Press::Go(PageId::Tracks(TrackKind::Subtitles)),
            Some(&s),
            200,
            &t,
        );
        let said: Vec<String> = v
            .rows(Some(&s), 200, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        let refusal = trouble_sentence(
            &Trouble::TooMany(TooMany {
                what: Limit::SubtitlePictures,
                most: 1,
            }),
            &t,
        );
        assert!(
            said.iter().any(|l| refusal.starts_with(l.as_str())),
            "{said:?}"
        );
        let plain = t.t(key::PLAYER_LANE_MERGED_PLAIN);
        assert!(
            said.iter().any(|l| plain.starts_with(l.as_str())),
            "{said:?}"
        );
    }

    /// FAILS IF: the home's row of the subtitles stops saying what is shown
    /// over the picture — none, the one by its name, or how many.
    #[test]
    fn the_home_says_what_lies_over_the_picture() {
        let t = texts();
        let mut s = film_with_lanes();
        assert_eq!(summary(&s, &t), "2 shown");
        s.lanes[3].enabled = false;
        s.lanes[2].enabled = false;
        assert_eq!(summary(&s, &t), t.t(key::PLAYER_SUBTITLES_OFF));
        s.lanes[3].enabled = true;
        assert_eq!(summary(&s, &t), "Portuguese (Brazil) · Commentary");
    }
}
