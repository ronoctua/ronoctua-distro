//! What one picture shows of the subtitle lanes over it.
//!
//! # Two tracks, six lanes
//!
//! mpv draws two subtitle tracks over a picture: its primary (`sid`) and its
//! secondary (`secondary-sid`). The lanes over one picture share them:
//!
//! - every lane of WORDS is merged into one ASS script ([`super::merge`]) on
//!   the primary track, each in its own colour and stacked in lane order —
//!   the merge places every line itself, because libass does not keep the
//!   lanes' order;
//! - ONE lane of PICTURES (PGS, VobSub) goes on the secondary track, where a
//!   bitmap is drawn where the bitmap says (mpv 0.41's manual) and in its own
//!   colours. While it is there the words move to the top band, out of its
//!   way. A second is refused ([`IMAGES_PER_PICTURE`]);
//! - a lane mpv can show whole by itself is not merged when it is ALONE: an
//!   ASS file (or an ASS track of the picture's own file) keeps all of its
//!   typesetting, and subtitles on the network — which only mpv has open —
//!   are shown as they are. Among other lanes an ASS is merged (its signs
//!   kept where it put them, its dialogue in the lane's colour), and
//!   subtitles on the network are refused with the reason.
//!
//! Text never goes to the secondary track, so mpv's stripping of its styles
//! (`secondary-sub-ass-override`) never meets a lane of ours.
//!
//! Pure, like the rest of the crate: [`wanted`] reads the lanes, the engine
//! reads the files and runs ffmpeg ([`Read`]), and [`compose`] decides.

use std::sync::Arc;

use crate::limit::{Limit, TooMany};
use crate::own::{Lane, LaneId, LaneKind, Stack};
use crate::session;
use crate::subtitles::ass::Sign;
use crate::subtitles::cue::{Cue, Rgb};
use crate::subtitles::merge::{Look, MergeLane, merge};
use crate::subtitles::{Refusal, Track as Words};
use crate::wire::{Track, TrackKind, Trouble};

/// The most subtitles drawn as pictures over one picture: mpv shows them on
/// its secondary track, which is one track.
pub const IMAGES_PER_PICTURE: usize = 1;

/// Where one lane's words are.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// A file of its own, on this disk.
    File(String),
    /// A track inside a file on this disk: the `n`th subtitle stream
    /// (ffmpeg's `-map 0:s:<n>`), and — when the file is the picture's own —
    /// the id mpv gave it.
    Inside {
        /// The file.
        file: String,
        /// Its place among the file's own subtitle streams, from zero.
        n: usize,
        /// mpv's id for it, when mpv has the file open.
        own: Option<i64>,
    },
    /// A subtitle file on the network: mpv adds it by itself.
    Address(String),
    /// A track of the picture's own source, which plays from the network.
    Streamed {
        /// The address the picture plays from.
        address: String,
        /// mpv's id for the track.
        track: i64,
    },
    /// A track inside ANOTHER file on the network: nothing here can open it.
    Nowhere(String),
    /// The picture's own file, which has no subtitles in it.
    Empty(String),
}

/// One subtitle lane over a picture, as its picture needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Wanted {
    /// The lane.
    pub lane: LaneId,
    /// Its place among the lanes over this picture — its colour,
    /// `subtitle.lane-<slot + 1>` ([`session::subtitle_slot`]).
    pub slot: usize,
    /// Where its words are.
    pub source: Source,
    /// mpv's name for the track's codec, when the track is inside a file mpv
    /// has open (`subrip`, `ass`, `hdmv_pgs_subtitle`, …).
    pub codec: Option<String>,
    /// Where its lines sit.
    pub stack: Stack,
    /// How far its file's clock is from the picture's, in milliseconds: a
    /// line at `c` in its file is shown at `c + offset_ms` of the picture's.
    pub offset_ms: i64,
    /// Where in its file it begins, in milliseconds.
    pub from_ms: u64,
    /// Where in its file it stops, in milliseconds.
    pub until_ms: Option<u64>,
    /// The language it keeps ([`Lane::lang`]): which of a file of several
    /// languages it shows (a SAMI's class, [`super::sami::chosen`]).
    pub lang: Option<String>,
}

impl Wanted {
    /// The path or address a sentence about this lane names.
    #[must_use]
    pub fn named(&self) -> &str {
        match &self.source {
            Source::File(p)
            | Source::Address(p)
            | Source::Nowhere(p)
            | Source::Empty(p)
            | Source::Streamed { address: p, .. }
            | Source::Inside { file: p, .. } => p,
        }
    }

    /// Whether the track inside a file is drawn as pictures, by its codec:
    /// PGS, VobSub, DVB and XSUB are bitmaps, which ffmpeg does not turn
    /// into words.
    #[must_use]
    pub fn is_image_track(&self) -> bool {
        self.codec.as_deref().is_some_and(|c| {
            matches!(
                c,
                "hdmv_pgs_subtitle"
                    | "pgssub"
                    | "dvd_subtitle"
                    | "dvdsub"
                    | "dvb_subtitle"
                    | "dvbsub"
                    | "xsub"
            )
        })
    }

    /// Whether the track inside a file is an ASS script mpv can show whole.
    #[must_use]
    pub fn is_ass_track(&self) -> bool {
        self.codec
            .as_deref()
            .is_some_and(|c| matches!(c, "ass" | "ssa"))
    }
}

/// The subtitle lanes over `picture`, in lane order — which is the order they
/// stack in — each with where its words are.
///
/// `tracks` are the tracks of the picture's own source, as mpv read them;
/// `None` while a lane over it names one of them and mpv has not said what
/// they are yet (the engine asks again when it has).
#[must_use]
pub fn wanted(picture: &Lane, lanes: &[Lane], tracks: &[Track]) -> Option<Vec<Wanted>> {
    let own_tracks: Vec<&Track> = {
        let mut own: Vec<&Track> = tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Sub && !t.external)
            .collect();
        own.sort_by_key(|t| t.id);
        own
    };
    let mut out = Vec::new();
    for lane in lanes.iter().filter(|l| {
        l.kind == LaneKind::Subtitle
            && l.enabled
            && session::picture_of(lanes, l) == Some(picture.id)
    }) {
        // PAST THE CEILING a set is refused whole (`session::refusal`); a
        // lane past it here is one no colour exists for.
        let Some(slot) =
            session::subtitle_slot(lanes, lane.id).filter(|s| *s < session::SUBTITLES_PER_PICTURE)
        else {
            continue;
        };
        let network = lane.source.contains("://");
        let mut codec = None;
        let source = if lane.source == picture.source {
            // THE PICTURE'S OWN FILE: which of its tracks, and what it is,
            // are mpv's to say.
            if tracks.is_empty() {
                return None;
            }
            let chosen = lane.track.or_else(|| {
                own_tracks
                    .iter()
                    .find(|t| t.default)
                    .or_else(|| own_tracks.first())
                    .map(|t| t.id)
            });
            let track = chosen.and_then(|id| {
                tracks
                    .iter()
                    .find(|t| t.kind == TrackKind::Sub && t.id == id)
            });
            codec = track.and_then(|t| t.codec.clone());
            match (chosen, own_tracks.iter().position(|t| Some(t.id) == chosen)) {
                (None, _) => Source::Empty(lane.source.clone()),
                (Some(id), Some(n)) if !network => Source::Inside {
                    file: lane.source.clone(),
                    n,
                    own: Some(id),
                },
                // ON THE NETWORK, or a track mpv added from elsewhere: only
                // mpv has it.
                (Some(id), _) => Source::Streamed {
                    address: lane.source.clone(),
                    track: id,
                },
            }
        } else if network {
            match lane.track {
                None => Source::Address(lane.source.clone()),
                Some(_) => Source::Nowhere(lane.source.clone()),
            }
        } else {
            match lane.track {
                None => Source::File(lane.source.clone()),
                // A TRACK OF ANOTHER FILE: mpv numbers each kind from one, in
                // the order the file has them.
                Some(k) => Source::Inside {
                    file: lane.source.clone(),
                    n: usize::try_from(k.saturating_sub(1)).unwrap_or(0),
                    own: None,
                },
            }
        };
        out.push(Wanted {
            lane: lane.id,
            slot,
            source,
            codec,
            stack: lane.stack,
            offset_ms: milliseconds(
                picture.from_seconds - picture.start_seconds + lane.start_seconds
                    - lane.from_seconds,
            ),
            from_ms: u64::try_from(milliseconds(lane.from_seconds)).unwrap_or(0),
            until_ms: lane
                .until_seconds
                .map(|u| u64::try_from(milliseconds(u)).unwrap_or(0)),
            lang: lane.lang.clone(),
        });
    }
    Some(out)
}

/// Seconds as whole milliseconds.
#[allow(clippy::cast_possible_truncation)]
fn milliseconds(seconds: f64) -> i64 {
    if seconds.is_finite() {
        (seconds * 1000.0).round() as i64
    } else {
        0
    }
}

/// Something mpv shows by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Native {
    /// A file (or an address) it adds.
    File(String),
    /// A track of the file it has open, by its id.
    Track(i64),
}

/// One lane, read.
#[derive(Debug, Clone)]
pub enum Read {
    /// Words, to merge — and, when mpv could show them whole by itself (an
    /// ASS script), what it would show.
    Words {
        /// The words.
        words: Arc<Words>,
        /// What mpv would show of it alone.
        native: Option<Native>,
    },
    /// Pictures: only mpv shows them, on its secondary track.
    Image(Native),
    /// Words only mpv can show — they are on the network — and only alone.
    OnlyNative(Native),
    /// Nothing to show, and why.
    Refused(Trouble),
}

/// What mpv shows by itself, and how much later than the picture's clock.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// What.
    pub native: Native,
    /// How much later than the picture's own clock, in seconds.
    pub delay_seconds: f64,
}

/// The picture's primary subtitle track.
#[derive(Debug, Clone, PartialEq)]
pub enum Primary {
    /// None.
    Nothing,
    /// The lanes, merged into this ASS script.
    Merged(String),
    /// One lane, shown by mpv itself.
    Native(Shown),
}

/// What one picture shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Show {
    /// On the primary track.
    pub primary: Primary,
    /// On the secondary track: the one lane of pictures.
    pub secondary: Option<Shown>,
    /// The lanes shown nowhere, and why.
    pub refused: Vec<(LaneId, Trouble)>,
    /// The lanes whose file carries several languages and shows one, and
    /// those languages ([`super::Track::languages`]): what a door offers to
    /// choose among.
    pub languages: Vec<(LaneId, Vec<String>)>,
}

/// Decides what a picture shows of `wanted`, each read as `reads` says (the
/// same order), with each lane's colour from `colours` by its slot and the
/// script drawn as `look` says.
#[must_use]
pub fn compose(wanted: &[Wanted], reads: &[Read], colours: &[Rgb], look: &Look) -> Show {
    let languages: Vec<(LaneId, Vec<String>)> = wanted
        .iter()
        .zip(reads)
        .filter_map(|(w, r)| match r {
            Read::Words { words, .. } if !words.languages.is_empty() => {
                Some((w.lane, words.languages.clone()))
            }
            _ => None,
        })
        .collect();
    let mut refused = Vec::new();
    let mut secondary: Option<Shown> = None;
    let mut text: Vec<(&Wanted, &Read)> = Vec::new();
    for (w, r) in wanted.iter().zip(reads) {
        match r {
            Read::Image(native) => {
                if secondary.is_none() {
                    secondary = Some(Shown {
                        native: native.clone(),
                        delay_seconds: seconds(w.offset_ms),
                    });
                } else {
                    refused.push((
                        w.lane,
                        Trouble::TooMany(TooMany {
                            what: Limit::SubtitlePictures,
                            most: IMAGES_PER_PICTURE,
                        }),
                    ));
                }
            }
            Read::Refused(t) => refused.push((w.lane, t.clone())),
            Read::Words { .. } | Read::OnlyNative(_) => text.push((w, r)),
        }
    }
    // ALONE, shown whole by mpv: a network's track always, an ASS script
    // unless a lane of pictures sends the words to the top.
    if let [(w, r)] = text.as_slice() {
        let native = match r {
            Read::OnlyNative(n) => Some(n.clone()),
            Read::Words {
                native: Some(n), ..
            } if secondary.is_none() => Some(n.clone()),
            _ => None,
        };
        if let Some(native) = native {
            return Show {
                primary: Primary::Native(Shown {
                    native,
                    delay_seconds: seconds(w.offset_ms),
                }),
                secondary,
                refused,
                languages,
            };
        }
    }
    // MERGED: every lane of words, each trimmed to its stretch.
    let mut kept: Vec<(&Wanted, Vec<Cue>, Vec<Sign>)> = Vec::new();
    for (w, r) in text {
        match r {
            Read::Words { words, .. } => kept.push((
                w,
                trimmed_cues(&words.cues, w.from_ms, w.until_ms),
                trimmed_signs(&words.signs, w.from_ms, w.until_ms),
            )),
            _ => refused.push((
                w.lane,
                Trouble::Subtitle {
                    path: w.named().to_owned(),
                    why: Refusal::Streamed,
                },
            )),
        }
    }
    if kept.is_empty() {
        return Show {
            primary: Primary::Nothing,
            secondary,
            refused,
            languages,
        };
    }
    let lanes: Vec<MergeLane<'_>> = kept
        .iter()
        .map(|(w, cues, signs)| MergeLane {
            cues,
            signs,
            colour: colours
                .get(w.slot)
                .or_else(|| colours.last())
                .copied()
                .unwrap_or(look.edge_colour),
            // THE WORDS GO UP while pictures are drawn at the foot.
            stack: if secondary.is_some() {
                Stack::Top
            } else {
                w.stack
            },
            offset_ms: w.offset_ms,
        })
        .collect();
    Show {
        primary: Primary::Merged(merge(&lanes, look)),
        secondary,
        refused,
        languages,
    }
}

/// Milliseconds as seconds.
#[allow(clippy::cast_precision_loss)]
fn seconds(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

/// The cues of a lane that fall inside its stretch of the file, cut to it.
fn trimmed_cues(cues: &[Cue], from_ms: u64, until_ms: Option<u64>) -> Vec<Cue> {
    cues.iter()
        .filter_map(|c| {
            let (start_ms, end_ms) = within(c.start_ms, c.end_ms, from_ms, until_ms)?;
            Some(Cue {
                start_ms,
                end_ms,
                ..c.clone()
            })
        })
        .collect()
}

/// The signs of a lane that fall inside its stretch, cut to it.
fn trimmed_signs(signs: &[Sign], from_ms: u64, until_ms: Option<u64>) -> Vec<Sign> {
    signs
        .iter()
        .filter_map(|s| {
            let (start_ms, end_ms) = within(s.start_ms, s.end_ms, from_ms, until_ms)?;
            Some(Sign {
                start_ms,
                end_ms,
                ..s.clone()
            })
        })
        .collect()
}

/// `start..end` cut to `from..until`; `None` when nothing of it is left.
fn within(start: u64, end: u64, from: u64, until: Option<u64>) -> Option<(u64, u64)> {
    let start = start.max(from);
    let end = until.map_or(end, |u| end.min(u));
    (end > start).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::{Format, cue::Band};

    fn lane(id: u32, kind: LaneKind, source: &str) -> Lane {
        Lane::new(LaneId(id), kind, source)
    }

    fn sub(id: i64, codec: &str, default: bool, external: bool) -> Track {
        Track {
            id,
            kind: TrackKind::Sub,
            title: None,
            lang: None,
            codec: Some(codec.into()),
            selected: false,
            albumart: false,
            default,
            forced: false,
            external,
        }
    }

    fn words(text: &str, start_ms: u64, end_ms: u64) -> Arc<Words> {
        Arc::new(Words {
            format: Format::SubRip,
            charset: None,
            title: None,
            cues: vec![Cue::plain(start_ms, end_ms, text)],
            signs: Vec::new(),
            languages: Vec::new(),
        })
    }

    fn look() -> Look {
        Look {
            play_res_x: 1920,
            play_res_y: 1080,
            font: "Sans".into(),
            size_permille: 50,
            outline_permille: 3,
            margin_permille: 40,
            gap_permille: 8,
            advance_permille: 600,
            edge_colour: Rgb::new(0, 0, 0),
        }
    }

    const COLOURS: [Rgb; 6] = [
        Rgb::new(0xD4, 0xD5, 0x6B),
        Rgb::new(0x71, 0xE8, 0x6A),
        Rgb::new(0x73, 0xE3, 0xC5),
        Rgb::new(0xA9, 0xD5, 0xF1),
        Rgb::new(0xF2, 0xBF, 0xF1),
        Rgb::new(0xF2, 0xC7, 0xA8),
    ];

    fn wanted_of(lane: u32, slot: usize, source: Source) -> Wanted {
        Wanted {
            lane: LaneId(lane),
            slot,
            source,
            codec: None,
            stack: Stack::Bottom,
            offset_ms: 0,
            from_ms: 0,
            until_ms: None,
            lang: None,
        }
    }

    /// FAILS IF: a lane's words are looked for in the wrong place — its own
    /// file's track by the wrong stream number, a file beside it as a track,
    /// a network's track as something ffmpeg could read — or one is asked
    /// before mpv has said what the picture's own tracks are.
    #[test]
    fn a_lane_s_words_are_found_where_they_are() {
        let picture = lane(1, LaneKind::Video, "/m/film.mkv");
        let mut own = lane(3, LaneKind::Subtitle, "/m/film.mkv");
        own.track = Some(4);
        let beside = lane(4, LaneKind::Subtitle, "/m/film.pt.srt");
        let mut other = lane(5, LaneKind::Subtitle, "/m/other.mkv");
        other.track = Some(2);
        let mut off = lane(6, LaneKind::Subtitle, "/m/off.srt");
        off.enabled = false;
        let lanes = vec![picture.clone(), own, beside, other, off];
        assert_eq!(
            wanted(&picture, &lanes, &[]),
            None,
            "the picture's tracks are not known yet"
        );
        let tracks = [
            sub(2, "subrip", false, false),
            sub(4, "ass", false, false),
            sub(9, "ass", false, true),
        ];
        let found = wanted(&picture, &lanes, &tracks).expect("the tracks are known");
        let sources: Vec<(u32, usize, Source)> = found
            .iter()
            .map(|w| (w.lane.0, w.slot, w.source.clone()))
            .collect();
        assert_eq!(
            sources,
            vec![
                (
                    3,
                    0,
                    Source::Inside {
                        file: "/m/film.mkv".into(),
                        n: 1,
                        own: Some(4)
                    }
                ),
                (4, 1, Source::File("/m/film.pt.srt".into())),
                (
                    5,
                    2,
                    Source::Inside {
                        file: "/m/other.mkv".into(),
                        n: 1,
                        own: None
                    }
                ),
            ],
            "the lane that is off keeps its slot and is not wanted"
        );
        assert!(found[0].is_ass_track());
        assert!(!found[1].is_ass_track());
        // ON THE NETWORK: the picture's own track is mpv's alone, a file is
        // an address mpv adds, and a track of another address is nowhere.
        let far = lane(1, LaneKind::Video, "http://100.64.0.1:45517/media/7/1");
        let mut far_own = lane(2, LaneKind::Subtitle, "http://100.64.0.1:45517/media/7/1");
        far_own.track = Some(1);
        let far_file = lane(3, LaneKind::Subtitle, "https://x.example/a.srt");
        let mut far_other = lane(4, LaneKind::Subtitle, "https://x.example/b.mkv");
        far_other.track = Some(1);
        let lanes = vec![far.clone(), far_own, far_file, far_other];
        let found = wanted(&far, &lanes, &[sub(1, "subrip", true, false)]).expect("known");
        assert!(matches!(found[0].source, Source::Streamed { track: 1, .. }));
        assert_eq!(
            found[1].source,
            Source::Address("https://x.example/a.srt".into())
        );
        assert_eq!(
            found[2].source,
            Source::Nowhere("https://x.example/b.mkv".into())
        );
    }

    /// FAILS IF: the picture's own file with no choice made shows another
    /// track than the one it flags, or a file with no subtitles at all is
    /// not said to be empty.
    #[test]
    fn the_picture_s_own_choice_is_its_flagged_track() {
        let picture = lane(1, LaneKind::Video, "/m/film.mkv");
        let own = lane(3, LaneKind::Subtitle, "/m/film.mkv");
        let lanes = vec![picture.clone(), own];
        let found = wanted(
            &picture,
            &lanes,
            &[
                sub(1, "subrip", false, false),
                sub(2, "subrip", true, false),
            ],
        )
        .expect("known");
        assert_eq!(
            found[0].source,
            Source::Inside {
                file: "/m/film.mkv".into(),
                n: 1,
                own: Some(2)
            }
        );
        let video = Track {
            kind: TrackKind::Video,
            ..sub(1, "h264", false, false)
        };
        let found = wanted(&picture, &lanes, &[video]).expect("known");
        assert_eq!(found[0].source, Source::Empty("/m/film.mkv".into()));
    }

    /// FAILS IF: a lane's words are shown at the wrong moment of the
    /// picture — its start, its trim and the picture's own start and trim
    /// all move where a line of its file lands.
    #[test]
    fn a_lane_s_clock_is_moved_to_its_picture_s() {
        let mut picture = lane(1, LaneKind::Video, "/m/film.mkv");
        picture.start_seconds = 2.0;
        picture.from_seconds = 10.0;
        let mut late = lane(2, LaneKind::Subtitle, "/m/film.pt.srt");
        late.start_seconds = 3.0;
        late.from_seconds = 1.0;
        late.until_seconds = Some(61.5);
        let lanes = vec![picture.clone(), late];
        let found = wanted(&picture, &lanes, &[]).expect("no own track is named");
        // THE PICTURE shows its file's 11 at the set's 3, where the lane
        // begins with its file's 1: a line at 1 s is 10 s later there.
        assert_eq!(found[0].offset_ms, 10_000);
        assert_eq!(found[0].from_ms, 1_000);
        assert_eq!(found[0].until_ms, Some(61_500));
    }

    /// FAILS IF: two lanes of words are not merged into one script, each in
    /// the colour of its SLOT — a lane switched off must not repaint the
    /// ones after it — with the first nearest the edge.
    #[test]
    fn lanes_of_words_merge_each_in_its_slot_s_colour() {
        let w = [
            wanted_of(3, 0, Source::File("/m/a.srt".into())),
            wanted_of(5, 2, Source::File("/m/c.srt".into())),
        ];
        let reads = [
            Read::Words {
                words: words("first", 1000, 3000),
                native: None,
            },
            Read::Words {
                words: words("third", 1000, 3000),
                native: None,
            },
        ];
        let show = compose(&w, &reads, &COLOURS, &look());
        let Primary::Merged(script) = &show.primary else {
            panic!("{show:?}");
        };
        assert!(
            show.secondary.is_none() && show.refused.is_empty(),
            "{show:?}"
        );
        let style = |n: usize| {
            script
                .lines()
                .find(|l| l.starts_with(&format!("Style: Lane{n},")))
                .unwrap_or_default()
                .to_owned()
        };
        assert!(style(1).contains(&COLOURS[0].ass_style()), "{}", style(1));
        assert!(style(2).contains(&COLOURS[2].ass_style()), "{}", style(2));
        assert!(
            script.contains("first") && script.contains("third"),
            "{script}"
        );
    }

    /// FAILS IF: a lane of pictures does not go to the secondary track, a
    /// second is not refused with the ceiling, or the words stay at the foot
    /// where the pictures are drawn.
    #[test]
    fn one_lane_of_pictures_is_secondary_and_the_words_go_up() {
        let mut image = wanted_of(4, 1, Source::File("/m/a.sup".into()));
        image.offset_ms = 1500;
        let w = [
            wanted_of(3, 0, Source::File("/m/a.srt".into())),
            image,
            wanted_of(5, 2, Source::File("/m/b.sup".into())),
        ];
        let reads = [
            Read::Words {
                words: words("words", 1000, 3000),
                native: None,
            },
            Read::Image(Native::File("/m/a.sup".into())),
            Read::Image(Native::File("/m/b.sup".into())),
        ];
        let show = compose(&w, &reads, &COLOURS, &look());
        assert_eq!(
            show.secondary,
            Some(Shown {
                native: Native::File("/m/a.sup".into()),
                delay_seconds: 1.5
            })
        );
        assert_eq!(
            show.refused,
            vec![(
                LaneId(5),
                Trouble::TooMany(TooMany {
                    what: Limit::SubtitlePictures,
                    most: 1
                })
            )]
        );
        let Primary::Merged(script) = &show.primary else {
            panic!("{show:?}");
        };
        // ALIGNMENT 8: the top band.
        let lane1 = script
            .lines()
            .find(|l| l.starts_with("Style: Lane1,"))
            .unwrap_or_default();
        assert!(lane1.contains(",8,"), "{lane1}");
    }

    /// FAILS IF: an ASS script alone is not shown whole by mpv, or is shown
    /// whole while pictures need the words at the top — or subtitles on the
    /// network are merged (nothing here can read them) instead of shown
    /// alone, or refused among others with the reason.
    #[test]
    fn what_mpv_shows_whole_is_shown_whole_only_alone() {
        let mut ass = wanted_of(3, 0, Source::File("/m/a.ass".into()));
        ass.offset_ms = -250;
        let read_ass = Read::Words {
            words: words("ass", 0, 1000),
            native: Some(Native::File("/m/a.ass".into())),
        };
        let show = compose(
            std::slice::from_ref(&ass),
            std::slice::from_ref(&read_ass),
            &COLOURS,
            &look(),
        );
        assert_eq!(
            show.primary,
            Primary::Native(Shown {
                native: Native::File("/m/a.ass".into()),
                delay_seconds: -0.25
            })
        );
        let image = wanted_of(4, 1, Source::File("/m/a.sup".into()));
        let show = compose(
            &[ass.clone(), image],
            &[
                read_ass.clone(),
                Read::Image(Native::File("/m/a.sup".into())),
            ],
            &COLOURS,
            &look(),
        );
        assert!(matches!(show.primary, Primary::Merged(_)), "{show:?}");
        let far = wanted_of(
            5,
            1,
            Source::Streamed {
                address: "http://h/m".into(),
                track: 2,
            },
        );
        let alone = compose(
            std::slice::from_ref(&far),
            &[Read::OnlyNative(Native::Track(2))],
            &COLOURS,
            &look(),
        );
        assert!(matches!(
            alone.primary,
            Primary::Native(Shown {
                native: Native::Track(2),
                ..
            })
        ));
        let both = compose(
            &[ass, far],
            &[read_ass, Read::OnlyNative(Native::Track(2))],
            &COLOURS,
            &look(),
        );
        assert!(matches!(both.primary, Primary::Merged(_)), "{both:?}");
        assert_eq!(
            both.refused,
            vec![(
                LaneId(5),
                Trouble::Subtitle {
                    path: "http://h/m".into(),
                    why: Refusal::Streamed
                }
            )]
        );
    }

    /// FAILS IF: a lane shows words from outside its stretch of its file, or
    /// a line that crosses an end is not cut there.
    #[test]
    fn a_lane_shows_only_its_stretch() {
        let cues = [
            Cue::plain(0, 900, "before"),
            Cue::plain(800, 2000, "across"),
            Cue::plain(5000, 7000, "after"),
        ];
        let kept = trimmed_cues(&cues, 1000, Some(6000));
        let spans: Vec<(u64, u64, String)> = kept
            .iter()
            .map(|c| (c.start_ms, c.end_ms, c.text()))
            .collect();
        assert_eq!(
            spans,
            vec![(1000, 2000, "across".into()), (5000, 6000, "after".into())]
        );
        assert!(trimmed_cues(&cues, 0, None).len() == 3);
        let band: Option<Band> = kept[0].band;
        assert_eq!(band, None);
    }

    /// FAILS IF: a lane that could not be read is shown, or its reason is
    /// lost on the way to the door.
    #[test]
    fn a_refused_lane_says_why_and_the_rest_still_show() {
        let w = [
            wanted_of(3, 0, Source::File("/m/gone.srt".into())),
            wanted_of(4, 1, Source::File("/m/b.srt".into())),
        ];
        let gone = Trouble::Unreadable {
            path: "/m/gone.srt".into(),
        };
        let show = compose(
            &w,
            &[
                Read::Refused(gone.clone()),
                Read::Words {
                    words: words("b", 0, 1000),
                    native: None,
                },
            ],
            &COLOURS,
            &look(),
        );
        assert_eq!(show.refused, vec![(LaneId(3), gone)]);
        let Primary::Merged(script) = &show.primary else {
            panic!("{show:?}");
        };
        // THE SURVIVOR KEEPS ITS OWN COLOUR, the second slot's.
        assert!(script.contains(&COLOURS[1].ass_style()), "{script}");
        let none = compose(&[], &[], &COLOURS, &look());
        assert_eq!(none.primary, Primary::Nothing);
    }
}
