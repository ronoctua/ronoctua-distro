//! A set on one clock: which media players it needs, where each should be,
//! and which one keeps the time.
//!
//! # Lanes and voices
//!
//! A [`Lane`] is what the person arranges: "this file's picture", "its
//! Portuguese dubbing on the headphones", "the commentary, two seconds late".
//! A [`Voice`] is what the engine RUNS: one mpv, playing one source from one
//! start, with at most one picture and at most one sound. A film's picture and
//! its own sound share a voice; its second language, going to another output,
//! needs a voice of its own (`vid=no`, `aid=N`, another `audio-device`),
//! because one mpv plays one audio track to one device.
//!
//! Subtitle lanes never get a voice: they are drawn by the voice of the
//! picture they are over.
//!
//! # The clock
//!
//! The first sound that is playing keeps the time ([`master`]): a picture that
//! drifts a frame is unseen, a sound that drifts is heard. When no sound plays
//! — before the first starts, in a silent gap, after the last ends — the
//! engine runs a clock of its own, [`Master::Virtual`].
//!
//! Pure, like the rest of this crate: the engine measures and applies.

use ronoctua_playlist::own::{Lane, LaneId, LaneKind, Set};

use crate::limit::{Limit, TooMany};

/// The most lanes one set has.
///
/// The owner's own case — three pictures, two sounds and six subtitles — is
/// eleven; sixteen leaves room without letting a set grow past what a door
/// can list on one page.
pub const LANES_MOST: usize = 16;

/// The most subtitle lanes over one picture.
///
/// Six, because each is drawn in its own colour and six is how many the
/// palette tells apart on any picture, and because past six lines of
/// dialogue at once there is no picture left to read them over.
pub const SUBTITLES_PER_PICTURE: usize = 6;

/// The most media players one set runs.
///
/// Each is a decoder, and a picture's is a render thread too.
pub const VOICES_MOST: usize = 8;

/// The most pictures one set shows: pictures and the covers of sounds.
///
/// Every picture is decoded and drawn in software, on every door that shows
/// it; six high-definition pictures is already a machine's worth. It is
/// below [`VOICES_MOST`] on purpose, so a set at its picture ceiling still has
/// voices for sounds of their own.
pub const PICTURES_MOST: usize = 6;

/// One media player: one source, from one start, with at most one picture
/// and one sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    /// What it plays.
    pub source: String,
    /// When it begins on the set's clock, seconds.
    pub start_seconds: f64,
    /// Where in the source it begins, seconds.
    pub from_seconds: f64,
    /// Where in the source it stops, seconds.
    pub until_seconds: Option<f64>,
    /// The lane whose picture it plays.
    pub video: Option<LaneId>,
    /// The lane whose sound it plays.
    pub audio: Option<LaneId>,
    /// Where its sound goes: its audio lane's output.
    pub sink: Option<String>,
}

/// The voices a set needs, in lane order.
///
/// Only lanes that are ON: a lane switched off costs nothing, and switching
/// it back on regroups. A picture and a sound share a voice when they play
/// the same source over the same stretch of it from the same start, and the
/// voice has no lane of that kind yet.
#[must_use]
pub fn voices(lanes: &[Lane]) -> Vec<Voice> {
    let mut out: Vec<Voice> = Vec::new();
    for lane in lanes.iter().filter(|lane| lane.enabled) {
        let video = lane.kind == LaneKind::Video;
        if lane.kind == LaneKind::Subtitle {
            continue;
        }
        let shared = out.iter_mut().find(|voice| {
            voice.source == lane.source
                && same(voice.start_seconds, lane.start_seconds)
                && same(voice.from_seconds, lane.from_seconds)
                && match (voice.until_seconds, lane.until_seconds) {
                    (None, None) => true,
                    (Some(a), Some(b)) => same(a, b),
                    _ => false,
                }
                && if video {
                    voice.video.is_none()
                } else {
                    voice.audio.is_none()
                }
        });
        let voice = match shared {
            Some(voice) => voice,
            None => {
                out.push(Voice {
                    source: lane.source.clone(),
                    start_seconds: lane.start_seconds,
                    from_seconds: lane.from_seconds,
                    until_seconds: lane.until_seconds,
                    video: None,
                    audio: None,
                    sink: None,
                });
                let last = out.len() - 1;
                &mut out[last]
            }
        };
        if video {
            voice.video = Some(lane.id);
        } else {
            voice.audio = Some(lane.id);
            voice.sink.clone_from(&lane.sink);
        }
    }
    out
}

/// Two times the same to the millisecond — what "the same start" means to a
/// person dragging a lane.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.000_5
}

/// Where a lane is at a moment of the set's clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum When {
    /// It has not started: kept paused at its `from`.
    Before,
    /// It plays, and should be at this many seconds into its source.
    At {
        /// Seconds into the source.
        seconds: f64,
    },
    /// It has ended: kept at its end.
    After,
}

/// Where `lane` is at `seconds` on the set's clock, when its source runs
/// `length_seconds` (`None` for a live source, or one not measured yet).
#[must_use]
pub fn when(seconds: f64, lane: &Lane, length_seconds: Option<f64>) -> When {
    if seconds < lane.start_seconds {
        return When::Before;
    }
    let into = lane.from_seconds + (seconds - lane.start_seconds);
    match stop(lane, length_seconds) {
        Some(stop) if into >= stop => When::After,
        _ => When::At { seconds: into },
    }
}

/// Where in its source a lane stops: its trim, or the source's end, whichever
/// comes first.
fn stop(lane: &Lane, length_seconds: Option<f64>) -> Option<f64> {
    match (lane.until_seconds, length_seconds) {
        (Some(until), Some(length)) => Some(until.min(length)),
        (Some(until), None) => Some(until),
        (None, length) => length,
    }
}

/// When a lane ends on the set's clock; `None` when it does not (a live
/// source) or is not known yet.
#[must_use]
pub fn end_seconds(lane: &Lane, length_seconds: Option<f64>) -> Option<f64> {
    stop(lane, length_seconds).map(|stop| lane.start_seconds + (stop - lane.from_seconds).max(0.0))
}

/// How long the set runs: the end of the last picture or sound that is on.
///
/// Subtitles do not count — words with nothing under them are not a reason
/// to keep playing. `None` when nothing that counts is on, or when one of
/// them has no known end: then the set ends when its voices do.
#[must_use]
pub fn length(lanes: &[Lane], length_of: impl Fn(LaneId) -> Option<f64>) -> Option<f64> {
    let mut last: Option<f64> = None;
    for lane in lanes
        .iter()
        .filter(|lane| lane.enabled && lane.kind != LaneKind::Subtitle)
    {
        let end = end_seconds(lane, length_of(lane.id))?;
        last = Some(last.map_or(end, |last| last.max(end)));
    }
    last
}

/// Who keeps the set's time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Master {
    /// This lane's sound.
    Lane(LaneId),
    /// The engine's own clock: no sound is playing.
    Virtual,
}

/// Who keeps the time at `seconds`: the first sound, in lane order, that is
/// on and playing then; else the engine's own clock.
///
/// A muted sound still keeps time — muting is a volume, not a stop.
#[must_use]
pub fn master(lanes: &[Lane], length_of: impl Fn(LaneId) -> Option<f64>, seconds: f64) -> Master {
    lanes
        .iter()
        .filter(|lane| lane.enabled && lane.kind == LaneKind::Audio)
        .find(|lane| matches!(when(seconds, lane, length_of(lane.id)), When::At { .. }))
        .map_or(Master::Virtual, |lane| Master::Lane(lane.id))
}

/// The picture a subtitle lane is drawn over: the one it names, else the
/// set's first picture.
#[must_use]
pub fn picture_of(lanes: &[Lane], subtitle: &Lane) -> Option<LaneId> {
    subtitle.over.or_else(|| {
        lanes
            .iter()
            .find(|lane| lane.kind == LaneKind::Video)
            .map(|lane| lane.id)
    })
}

/// Which of the six subtitle colours a subtitle lane wears: its place among
/// the subtitle lanes over the same picture.
///
/// Counted over every such lane, on or off, so switching one off never
/// repaints the others.
#[must_use]
pub fn subtitle_slot(lanes: &[Lane], lane: LaneId) -> Option<usize> {
    let subtitle = lanes
        .iter()
        .find(|l| l.id == lane && l.kind == LaneKind::Subtitle)?;
    let picture = picture_of(lanes, subtitle);
    lanes
        .iter()
        .filter(|l| l.kind == LaneKind::Subtitle && picture_of(lanes, l) == picture)
        .position(|l| l.id == lane)
}

/// The ceiling this set passes, if any — checked in the order a person would
/// be told: lanes, subtitles, pictures, voices.
#[must_use]
pub fn refusal(set: &Set) -> Option<TooMany> {
    let lanes = &set.lanes;
    if lanes.len() > LANES_MOST {
        return Some(TooMany {
            what: Limit::Lanes,
            most: LANES_MOST,
        });
    }
    let crowded = lanes
        .iter()
        .filter(|lane| lane.kind == LaneKind::Subtitle)
        .any(|lane| {
            let picture = picture_of(lanes, lane);
            lanes
                .iter()
                .filter(|l| l.kind == LaneKind::Subtitle && picture_of(lanes, l) == picture)
                .count()
                > SUBTITLES_PER_PICTURE
        });
    if crowded {
        return Some(TooMany {
            what: Limit::Subtitles,
            most: SUBTITLES_PER_PICTURE,
        });
    }
    let pictures = lanes
        .iter()
        .filter(|lane| {
            lane.enabled
                && (lane.kind == LaneKind::Video
                    || (lane.kind == LaneKind::Audio && set.arrangement.covers.contains(&lane.id)))
        })
        .count();
    if pictures > PICTURES_MOST {
        return Some(TooMany {
            what: Limit::Pictures,
            most: PICTURES_MOST,
        });
    }
    if voices(lanes).len() > VOICES_MOST {
        return Some(TooMany {
            what: Limit::Voices,
            most: VOICES_MOST,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use ronoctua_playlist::own::Arrangement;

    use super::*;

    /// A lane of `kind` playing `source`, named `id`.
    fn lane(id: u32, kind: LaneKind, source: &str) -> Lane {
        Lane::new(LaneId(id), kind, source)
    }

    /// A set of these lanes.
    fn set(lanes: Vec<Lane>) -> Set {
        Set {
            name: None,
            lanes,
            arrangement: Arrangement::default(),
        }
    }

    /// FAILS IF: a film's picture and its own sound stop sharing one player.
    #[test]
    fn one_file_is_one_voice() {
        let lanes = vec![
            lane(1, LaneKind::Video, "/film.mkv"),
            lane(2, LaneKind::Audio, "/film.mkv"),
            lane(3, LaneKind::Subtitle, "/film.srt"),
        ];
        let voices = voices(&lanes);
        assert_eq!(voices.len(), 1, "{voices:#?}");
        assert_eq!(voices[0].video, Some(LaneId(1)));
        assert_eq!(voices[0].audio, Some(LaneId(2)));
    }

    /// FAILS IF: a second language of the same file shares the first one's
    /// player — one mpv sends one track to one device.
    #[test]
    fn a_second_sound_of_the_same_file_is_another_voice_with_its_output() {
        let mut dub = lane(3, LaneKind::Audio, "/film.mkv");
        dub.track = Some(2);
        dub.sink = Some("bluez_output.headphones".to_owned());
        let lanes = vec![
            lane(1, LaneKind::Video, "/film.mkv"),
            lane(2, LaneKind::Audio, "/film.mkv"),
            dub,
        ];
        let voices = voices(&lanes);
        assert_eq!(voices.len(), 2, "{voices:#?}");
        assert_eq!(voices[1].video, None);
        assert_eq!(voices[1].audio, Some(LaneId(3)));
        assert_eq!(voices[1].sink.as_deref(), Some("bluez_output.headphones"));
        assert_eq!(voices[0].sink, None);
    }

    /// FAILS IF: lanes that start apart, or play different stretches, share
    /// a player — it would play one of them at the wrong moment.
    #[test]
    fn lanes_apart_in_time_never_share_a_voice() {
        let mut late = lane(2, LaneKind::Audio, "/film.mkv");
        late.start_seconds = 2.0;
        let mut trimmed = lane(4, LaneKind::Audio, "/other.mkv");
        trimmed.until_seconds = Some(30.0);
        let lanes = vec![
            lane(1, LaneKind::Video, "/film.mkv"),
            late,
            lane(3, LaneKind::Video, "/other.mkv"),
            trimmed,
        ];
        assert_eq!(voices(&lanes).len(), 4);
        // AND A LANE THAT IS OFF runs nothing at all.
        let mut off = lanes;
        for lane in &mut off {
            lane.enabled = lane.id == LaneId(1);
        }
        assert_eq!(voices(&off).len(), 1);
    }

    /// FAILS IF: a lane is told to play before its start, past its trim, or
    /// past its source's end — or a live one is ever told it ended.
    #[test]
    fn a_lane_is_before_at_or_after() {
        let mut commentary = lane(1, LaneKind::Audio, "/c.opus");
        commentary.start_seconds = 2.0;
        commentary.from_seconds = 10.0;
        commentary.until_seconds = Some(20.0);
        assert_eq!(when(1.0, &commentary, Some(60.0)), When::Before);
        assert_eq!(
            when(2.0, &commentary, Some(60.0)),
            When::At { seconds: 10.0 }
        );
        assert_eq!(
            when(5.5, &commentary, Some(60.0)),
            When::At { seconds: 13.5 }
        );
        assert_eq!(when(12.0, &commentary, Some(60.0)), When::After);
        // THE SOURCE ENDS FIRST.
        assert_eq!(when(9.0, &commentary, Some(15.0)), When::After);
        let live = lane(2, LaneKind::Audio, "https://radio.example/live");
        assert_eq!(when(1e9, &live, None), When::At { seconds: 1e9 });
        assert_eq!(end_seconds(&commentary, Some(60.0)), Some(12.0));
        assert_eq!(end_seconds(&live, None), None);
    }

    /// FAILS IF: the clock is kept by a picture while a sound plays, by a
    /// sound that has not started, or by nothing when no sound plays.
    #[test]
    fn the_first_sound_playing_keeps_the_time() {
        let mut late = lane(2, LaneKind::Audio, "/b.opus");
        late.start_seconds = 5.0;
        let mut short = lane(3, LaneKind::Audio, "/c.opus");
        short.muted = true;
        let lanes = vec![lane(1, LaneKind::Video, "/film.mkv"), late, short];
        let length_of = |id: LaneId| match id.0 {
            1 => Some(100.0),
            2 => Some(10.0),
            _ => Some(3.0),
        };
        // A MUTED SOUND still keeps time.
        assert_eq!(master(&lanes, length_of, 1.0), Master::Lane(LaneId(3)));
        // A SILENT GAP: the short one ended, the late one has not begun.
        assert_eq!(master(&lanes, length_of, 4.0), Master::Virtual);
        assert_eq!(master(&lanes, length_of, 6.0), Master::Lane(LaneId(2)));
        // AFTER EVERY SOUND, only the picture is left.
        assert_eq!(master(&lanes, length_of, 50.0), Master::Virtual);
    }

    /// FAILS IF: the set's length stops being the end of its last picture or
    /// sound that is on.
    #[test]
    fn a_set_runs_until_its_last_lane_that_is_on() {
        let mut late = lane(2, LaneKind::Audio, "/b.opus");
        late.start_seconds = 95.0;
        let mut words = lane(3, LaneKind::Subtitle, "/w.srt");
        words.start_seconds = 500.0;
        words.until_seconds = Some(10.0);
        let mut off = lane(4, LaneKind::Video, "/long.mkv");
        off.enabled = false;
        let lanes = vec![lane(1, LaneKind::Video, "/film.mkv"), late, words, off];
        let known = |id: LaneId| match id.0 {
            1 => Some(100.0),
            2 => Some(10.0),
            _ => Some(9_999.0),
        };
        assert_eq!(length(&lanes, known), Some(105.0));
        // ONE LANE NOT MEASURED YET: nobody knows.
        assert_eq!(length(&lanes, |id| (id.0 != 2).then_some(100.0)), None);
        assert_eq!(length(&[], known), None);
    }

    /// FAILS IF: a ceiling is passed without a refusal, or the owner's own
    /// set — three pictures, two sounds, six subtitles — is refused.
    #[test]
    fn the_ceilings_refuse_and_the_owner_s_set_fits() {
        let mut lanes: Vec<Lane> = (1..=3)
            .map(|n| lane(n, LaneKind::Video, &format!("/v{n}.mkv")))
            .collect();
        lanes.push(lane(4, LaneKind::Audio, "/a.opus"));
        lanes.push(lane(5, LaneKind::Audio, "/b.opus"));
        lanes.extend((6..=11).map(|n| lane(n, LaneKind::Subtitle, &format!("/s{n}.srt"))));
        assert_eq!(refusal(&set(lanes.clone())), None);

        // A SEVENTH SUBTITLE over the same picture.
        let mut seventh = lanes.clone();
        seventh.push(lane(12, LaneKind::Subtitle, "/s12.srt"));
        assert_eq!(
            refusal(&set(seventh.clone())),
            Some(TooMany {
                what: Limit::Subtitles,
                most: SUBTITLES_PER_PICTURE
            })
        );
        // ...and the same seventh over ANOTHER picture is fine.
        if let Some(last) = seventh.last_mut() {
            last.over = Some(LaneId(2));
        }
        assert_eq!(refusal(&set(seventh)), None);

        let many: Vec<Lane> = (1..=17)
            .map(|n| lane(n, LaneKind::Audio, "/a.opus"))
            .collect();
        assert_eq!(refusal(&set(many)).map(|t| t.what), Some(Limit::Lanes));

        let pictures: Vec<Lane> = (1..=7)
            .map(|n| lane(n, LaneKind::Video, &format!("/v{n}.mkv")))
            .collect();
        assert_eq!(
            refusal(&set(pictures)).map(|t| t.what),
            Some(Limit::Pictures)
        );

        let sounds: Vec<Lane> = (1..=9)
            .map(|n| lane(n, LaneKind::Audio, &format!("/a{n}.opus")))
            .collect();
        assert_eq!(refusal(&set(sounds)).map(|t| t.what), Some(Limit::Voices));

        // A COVER is a picture too.
        let mut covered = set((1..=6)
            .map(|n| lane(n, LaneKind::Video, &format!("/v{n}.mkv")))
            .chain(std::iter::once(lane(7, LaneKind::Audio, "/a.flac")))
            .collect());
        assert_eq!(refusal(&covered), None);
        covered.arrangement.covers.push(LaneId(7));
        assert_eq!(refusal(&covered).map(|t| t.what), Some(Limit::Pictures));
    }

    /// FAILS IF: a subtitle's colour depends on which others are on, or two
    /// subtitles over one picture wear the same one.
    #[test]
    fn a_subtitle_wears_its_place_over_its_picture() {
        let mut second_picture = lane(5, LaneKind::Subtitle, "/c.srt");
        second_picture.over = Some(LaneId(2));
        let mut off = lane(3, LaneKind::Subtitle, "/a.srt");
        off.enabled = false;
        let lanes = vec![
            lane(1, LaneKind::Video, "/v1.mkv"),
            lane(2, LaneKind::Video, "/v2.mkv"),
            off,
            lane(4, LaneKind::Subtitle, "/b.srt"),
            second_picture,
        ];
        assert_eq!(subtitle_slot(&lanes, LaneId(3)), Some(0));
        assert_eq!(subtitle_slot(&lanes, LaneId(4)), Some(1));
        assert_eq!(subtitle_slot(&lanes, LaneId(5)), Some(0));
        assert_eq!(subtitle_slot(&lanes, LaneId(1)), None);
    }
}
