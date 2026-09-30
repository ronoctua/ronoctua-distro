//! Subtitle files: every text format read into cues, and up to six lanes
//! over one picture written as one ASS script.
//!
//! # The route of a file
//!
//! [`open`] decides the format from the CONTENT ([`sniff`]; the extension
//! only breaks a tie), reads the bytes in the right charset ([`charset`]) and
//! gives one of three answers:
//!
//! - [`Opened::Ours`]: a [`Track`] of [`Cue`]s (and, for ASS, [`Sign`]s) from
//!   one of our readers — SRT, WebVTT, ASS/SSA, SAMI, MicroDVD, MPL2,
//!   SubViewer, TMPlayer, SBV, TTML/DFXP/IMSC, EBU STL, YouTube's timed text
//!   and LRC. The last five are formats ffmpeg 8.1.2 does not read at all.
//! - [`Opened::Ffmpeg`]: a format ffmpeg reads and we do not ([`Tail`]:
//!   JACOSub, PJS, AQTitle, RealText, VPlayer, MPsub, SCC, Spruce STL, MCC).
//!   The engine converts it to ASS and gives the result to [`read`] as
//!   [`Format::Ass`].
//! - [`Opened::Image`]: pictures (PGS, VobSub), which mpv shows natively,
//!   one at a time and in their own colours.
//!
//! [`merge`] then writes the lanes of one picture as one script, and
//! [`show`] decides which lanes that script holds — and which lane mpv shows
//! by itself, or refuses.
//!
//! Pure, like the rest of the crate: no file is opened here, no program run.

pub mod ass;
pub mod charset;
pub mod cue;
pub mod ebu_stl;
pub mod lrc;
pub mod merge;
pub mod microdvd;
pub mod mpl2;
pub mod sami;
pub mod sbv;
pub mod show;
pub mod sniff;
pub mod srt;
pub mod subviewer;
pub mod tmplayer;
pub mod ttml;
pub mod vtt;
pub mod youtube;

use serde::{Deserialize, Serialize};

pub use ass::Sign;
pub use charset::Charset;
pub use cue::{Band, Cue, Rgb, Span};
pub use merge::{Look, MergeLane, PLAY_RES_Y, merge};
pub use sniff::{Format, Kind, Picture, Tail, sniff};

/// The largest subtitle file read: past any film's dialogue, and past the
/// heaviest typeset ASS; beyond it is not a subtitle.
pub const BYTES_MOST: usize = 16 * 1024 * 1024;

/// The most cues and signs one file gives.
pub const CUES_MOST: usize = 200_000;

/// A frame rate, as a fraction: `24000/1001` is the 23.976 of film on NTSC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameRate {
    /// Frames…
    pub numerator: u32,
    /// …per this many seconds.
    pub denominator: u32,
}

impl FrameRate {
    /// Film transferred to NTSC, 23.976: what most MicroDVD files were made
    /// for, and what one is read at when neither it nor the picture says.
    pub const FILM: Self = Self {
        numerator: 24_000,
        denominator: 1001,
    };

    /// A rate written as a decimal (`25`, `23.976`, `29.97`): the NTSC rates
    /// become their exact fractions.
    #[must_use]
    pub fn decimal(text: &str) -> Option<Self> {
        let value: f64 = text.trim().parse().ok()?;
        if !(value > 0.0 && value <= 1000.0) {
            return None;
        }
        for whole in [24u32, 30, 48, 60, 120] {
            if (value - f64::from(whole) * 1000.0 / 1001.0).abs() < 0.005 {
                return Some(Self {
                    numerator: whole * 1000,
                    denominator: 1001,
                });
            }
        }
        Some(Self {
            numerator: (value * 1000.0).round() as u32,
            denominator: 1000,
        })
    }
}

/// A file that says it is a format and does not follow it (XML that is not
/// well formed, JSON of another shape, an EBU STL cut short).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damaged;

/// Why a subtitle file gives no cues. Each is a sentence on the screen; the
/// shell turns it into its text key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "why")]
pub enum Refusal {
    /// Larger than [`BYTES_MOST`].
    TooLarge {
        /// The ceiling, in bytes.
        most_bytes: usize,
    },
    /// No subtitle format we know.
    Unknown,
    /// A format we know, not followed.
    Damaged,
    /// Read, and nothing in it to show.
    Empty,
    /// Lyrics with no times: words, not subtitles.
    Untimed,
    /// More than [`CUES_MOST`] cues.
    TooManyCues {
        /// The ceiling.
        most: usize,
    },
    /// Converting it took longer than the ceiling (a track inside a film is
    /// found by reading the whole film, and a film on a slow disk may not be
    /// read in time).
    TooSlow {
        /// The ceiling, in seconds.
        most_seconds: u64,
    },
    /// Subtitles on the network — an address, or a track inside a film that
    /// plays from one: mpv, which has them open, shows them by themselves,
    /// and they are not read here to be stacked with others.
    Streamed,
    /// A VobSub `.sub` whose `.idx` is not beside it: pictures with nothing
    /// saying when, or in which colours.
    NoIndex,
}

impl From<Damaged> for Refusal {
    fn from(_: Damaged) -> Self {
        Self::Damaged
    }
}

/// A subtitle file, read.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Its format.
    pub format: Format,
    /// The charset it was read as; `None` for a binary format.
    pub charset: Option<Charset>,
    /// A title the file carries (EBU STL's programme title), if any.
    pub title: Option<String>,
    /// Its dialogue, sorted by start.
    pub cues: Vec<Cue>,
    /// Its typesetting, kept in place (ASS only).
    pub signs: Vec<Sign>,
}

/// What a subtitle file turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub enum Opened {
    /// Read by us.
    Ours(Track),
    /// For ffmpeg to convert to ASS; then [`read`] as [`Format::Ass`].
    Ffmpeg(Tail),
    /// Pictures, for mpv.
    Image(Picture),
}

/// Decides what `bytes` are and reads them when they are ours.
///
/// `extension` (no dot) only breaks a tie between formats; `rate` is the
/// picture's frame rate, for a MicroDVD file that does not say its own.
///
/// # Errors
///
/// A [`Refusal`] with the reason.
pub fn open(
    bytes: &[u8],
    extension: Option<&str>,
    rate: Option<FrameRate>,
) -> Result<Opened, Refusal> {
    if bytes.len() > BYTES_MOST {
        return Err(Refusal::TooLarge {
            most_bytes: BYTES_MOST,
        });
    }
    let (kind, decoded) = match sniff::binary(bytes) {
        Some(kind) => (kind, None),
        None => {
            let decoded = charset::decode(bytes);
            let kind = sniff::text(&decoded.text, extension).ok_or(Refusal::Unknown)?;
            (kind, Some(decoded))
        }
    };
    match (kind, decoded) {
        (Kind::Ours(Format::EbuStl), _) => read(Format::EbuStl, bytes, rate).map(Opened::Ours),
        (Kind::Ours(format), Some(decoded)) => {
            read_decoded(format, decoded, rate).map(Opened::Ours)
        }
        (Kind::Ours(format), None) => read(format, bytes, rate).map(Opened::Ours),
        (Kind::Ffmpeg(tail), _) => Ok(Opened::Ffmpeg(tail)),
        (Kind::Image(picture), _) => Ok(Opened::Image(picture)),
    }
}

/// Reads `bytes` as `format`, whatever they look like: for a format already
/// decided (ffmpeg's ASS output, a lane whose format is remembered).
///
/// # Errors
///
/// A [`Refusal`] with the reason.
pub fn read(format: Format, bytes: &[u8], rate: Option<FrameRate>) -> Result<Track, Refusal> {
    if bytes.len() > BYTES_MOST {
        return Err(Refusal::TooLarge {
            most_bytes: BYTES_MOST,
        });
    }
    if format == Format::EbuStl {
        let (header, cues) = ebu_stl::read(bytes)?;
        let title = Some(header.title).filter(|t| !t.is_empty());
        return finish(format, None, title, cues, Vec::new());
    }
    read_decoded(format, charset::decode(bytes), rate)
}

fn read_decoded(
    format: Format,
    decoded: charset::Decoded,
    rate: Option<FrameRate>,
) -> Result<Track, Refusal> {
    let text = decoded.text.as_str();
    let none = Vec::new;
    let (cues, signs) = match format {
        Format::SubRip => (srt::read(text), none()),
        Format::WebVtt => (vtt::read(text), none()),
        Format::Ass => ass::read(text),
        Format::Sami => (sami::read(text), none()),
        Format::MicroDvd => (microdvd::read(text, rate), none()),
        Format::Mpl2 => (mpl2::read(text), none()),
        Format::SubViewer => (subviewer::read(text), none()),
        Format::TmPlayer => (tmplayer::read(text), none()),
        Format::Sbv => (sbv::read(text), none()),
        Format::Ttml => (ttml::read(text)?, none()),
        Format::EbuStl => return Err(Refusal::Damaged),
        Format::YouTubeXml => (youtube::read_xml(text)?, none()),
        Format::YouTubeJson => (youtube::read_json(text)?, none()),
        Format::Lrc => (lrc::read(text).ok_or(Refusal::Untimed)?, none()),
    };
    finish(format, Some(decoded.charset), None, cues, signs)
}

fn finish(
    format: Format,
    charset: Option<Charset>,
    title: Option<String>,
    mut cues: Vec<Cue>,
    mut signs: Vec<Sign>,
) -> Result<Track, Refusal> {
    cue::tidy(&mut cues);
    signs.retain(|s| s.end_ms > s.start_ms);
    if cues.len() + signs.len() > CUES_MOST {
        return Err(Refusal::TooManyCues { most: CUES_MOST });
    }
    if cues.is_empty() && signs.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(Track {
        format,
        charset,
        title,
        cues,
        signs,
    })
}

/// Extensions that name other things far more often than subtitles: a file
/// with one of them is only a subtitle when the person ADDS it as one, and
/// the content says so.
pub const GENERIC_EXTENSIONS: [&str; 4] = ["txt", "xml", "json", "js"];

/// Whether a file named with `extension` (no dot, any case) is taken for a
/// subtitle when it is opened with the player: every extension of every
/// format read here, sent to ffmpeg or shown as pictures, but the generic
/// ones.
#[must_use]
pub fn claims(extension: &str) -> bool {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    if GENERIC_EXTENSIONS.contains(&extension.as_str()) {
        return false;
    }
    let named = |list: &[&str]| list.contains(&extension.as_str());
    Format::ALL.iter().any(|f| named(f.extensions()))
        || Tail::ALL.iter().any(|t| named(t.extensions()))
        || named(&["sup", "idx", "sub"])
}

/// Whether the subtitle file at `subtitle` is named after the film at
/// `film`: the same name, or the film's name and then a tag
/// (`film.pt-BR.srt`, `film.en.forced.srt`), ignoring case. What opening a
/// subtitle file goes over, and what a door lists as "beside the file".
#[must_use]
pub fn named_after(subtitle: &str, film: &str) -> bool {
    let stem = |p: &str| {
        std::path::Path::new(p)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_lowercase)
    };
    let (Some(words), Some(picture)) = (stem(subtitle), stem(film)) else {
        return false;
    };
    words == picture
        || words
            .strip_prefix(&picture)
            .is_some_and(|rest| rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_written_as_a_decimal_is_exact() {
        assert_eq!(FrameRate::decimal("23.976"), Some(FrameRate::FILM));
        assert_eq!(
            FrameRate::decimal("29.97"),
            Some(FrameRate {
                numerator: 30_000,
                denominator: 1001
            })
        );
        assert_eq!(
            FrameRate::decimal("25"),
            Some(FrameRate {
                numerator: 25_000,
                denominator: 1000
            })
        );
        assert_eq!(FrameRate::decimal("0"), None);
        assert_eq!(FrameRate::decimal("x"), None);
    }

    #[test]
    fn refusals_say_why() {
        assert_eq!(
            open(b"just words\n", Some("srt"), None),
            Err(Refusal::Unknown)
        );
        assert_eq!(
            open(b"1\n00:00:01,000 --> 00:00:02,000\n\n", Some("srt"), None),
            Err(Refusal::Empty)
        );
        assert_eq!(
            open(b"<tt xmlns=\"x\"><body>", Some("ttml"), None),
            Err(Refusal::Damaged)
        );
        assert_eq!(
            read(Format::Lrc, b"words only\n", None),
            Err(Refusal::Untimed)
        );
        let huge = vec![b' '; BYTES_MOST + 1];
        assert_eq!(
            open(&huge, None, None),
            Err(Refusal::TooLarge {
                most_bytes: BYTES_MOST
            })
        );
        assert_eq!(
            open(b"Scenarist_SCC V1.0\n\n", Some("scc"), None),
            Ok(Opened::Ffmpeg(Tail::Scc))
        );
        assert_eq!(
            serde_json::to_string(&Refusal::TooManyCues { most: 3 }).unwrap_or_default(),
            r#"{"why":"too-many-cues","most":3}"#
        );
    }

    #[test]
    fn every_reader_is_reached_from_open() {
        let fixtures: [(&[u8], &str, Format); 12] = [
            (b"1\n00:00:01,000 --> 00:00:02,000\nx\n", "srt", Format::SubRip),
            (b"WEBVTT\n\n00:01.000 --> 00:02.000\nx\n", "vtt", Format::WebVtt),
            (
                b"[Script Info]\n\n[Events]\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,x\n",
                "ass",
                Format::Ass,
            ),
            (b"<SAMI><BODY><SYNC Start=1000><P>x<SYNC Start=2000><P>&nbsp;</BODY></SAMI>", "smi", Format::Sami),
            (b"{1}{1}25\n{25}{50}x\n", "sub", Format::MicroDvd),
            (b"[10][20]x\n", "txt", Format::Mpl2),
            (b"[INFORMATION]\n[SUBTITLE]\n00:00:01.00,00:00:02.00\nx\n", "sub", Format::SubViewer),
            (b"00:00:01:x\n", "txt", Format::TmPlayer),
            (b"0:00:01.000,0:00:02.000\nx\n", "sbv", Format::Sbv),
            (
                b"<tt xmlns=\"http://www.w3.org/ns/ttml\"><body><div><p begin=\"1s\" end=\"2s\">x</p></div></body></tt>",
                "ttml",
                Format::Ttml,
            ),
            (b"{\"events\":[{\"tStartMs\":1000,\"dDurationMs\":1000,\"segs\":[{\"utf8\":\"x\"}]}]}", "json3", Format::YouTubeJson),
            (b"[00:01.00]x\n", "lrc", Format::Lrc),
        ];
        for (bytes, extension, format) in fixtures {
            match open(bytes, Some(extension), None) {
                Ok(Opened::Ours(track)) => {
                    assert_eq!(track.format, format);
                    assert_eq!(
                        track.cues.first().map(Cue::text).as_deref(),
                        Some("x"),
                        "{format:?}"
                    );
                    assert_eq!(track.cues[0].start_ms, 1000, "{format:?}");
                }
                other => panic!("{format:?}: {other:?}"),
            }
        }
    }

    /// FAILS IF: a subtitle file is taken for another film's — a tag after
    /// the film's name must match, a longer film name must not.
    #[test]
    fn a_subtitle_is_named_after_its_film() {
        assert!(named_after("/m/Film.srt", "/m/film.mkv"));
        assert!(named_after("/m/film.pt-BR.srt", "/m/film.mkv"));
        assert!(named_after("/m/film.en.forced.ass", "/x/film.mp4"));
        assert!(!named_after("/m/film2.srt", "/m/film.mkv"));
        assert!(!named_after("/m/film.srt", "/m/film.pt.mkv"));
        assert!(!named_after("", "/m/film.mkv"));
    }

    #[test]
    fn the_player_claims_subtitle_names_but_not_generic_ones() {
        for ours in [
            "srt", "ASS", ".vtt", "ttml", "dfxp", "sbv", "stl", "sup", "idx", "sub", "jss",
            "json3", "srv3", "lrc",
        ] {
            assert!(claims(ours), "{ours}");
        }
        for generic in ["txt", "xml", "json", "js", "mkv"] {
            assert!(!claims(generic), "{generic}");
        }
    }

    #[test]
    fn no_cut_of_any_file_makes_a_reader_fail_loudly() {
        // Files arrive cut short (a download stopped, a disk full): every
        // prefix of every format, including cuts inside a character, is a
        // refusal or cues — never a panic.
        let files: [&[u8]; 12] = [
            "1\n00:00:01,000 --> 00:00:02,000\n<i>Olá</i> {\\an8}&amp; €\n".as_bytes(),
            "WEBVTT\n\n00:01.000 --> 00:02.000 line:0\n<c.red>é</c> &aaaaaaaaaaa€;\n".as_bytes(),
            "[Script Info]\nPlayResX: 640\n\n[Events]\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\\pos(1,2)\\p1}m 0 0 l 1 1{\\p0}\\N{\\i1}ü\n".as_bytes(),
            "<SAMI><BODY><SYNC Start=1000><P Class=A>ç<br>x<SYNC Start=2000><P Class=A>&nbsp;</BODY></SAMI>".as_bytes(),
            "{1}{1}25\n{25}{50}{y:i}ã|b\n".as_bytes(),
            "[10][20]/ñ|b\n".as_bytes(),
            "[SUBTITLE]\n[00:00:01]\nø|x\n[00:00:02]\n".as_bytes(),
            "00:00:01:ß|x\n00:00:02,1=y\n".as_bytes(),
            "<tt xmlns=\"http://www.w3.org/ns/ttml\"><body><div><p begin=\"1s\" end=\"2s\">å<br/>b</p></div></body></tt>".as_bytes(),
            "<timedtext format=\"3\"><body><p t=\"1000\" d=\"1000\">ō<s>x</s></p></body></timedtext>".as_bytes(),
            "{\"events\":[{\"tStartMs\":1000,\"dDurationMs\":1000,\"segs\":[{\"utf8\":\"ž\"}]}]}".as_bytes(),
            "[00:01.00]ĳ\n[00:02.00]x\n".as_bytes(),
        ];
        for file in files {
            for end in 0..=file.len() {
                let _ = open(&file[..end], Some("srt"), None);
                let _ = open(&file[..end], None, Some(FrameRate::FILM));
            }
        }
        for format in Format::ALL {
            for file in files {
                let _ = read(format, file, None);
            }
        }
    }

    #[test]
    fn deep_nesting_is_bounded_not_a_stack_overflow() {
        let depth = 100_000;
        let mut ttml = String::from("<tt xmlns=\"http://www.w3.org/ns/ttml\"><body>");
        ttml.push_str(&"<div>".repeat(depth));
        ttml.push_str("<p begin=\"1s\" end=\"2s\">deep</p>");
        ttml.push_str(&"</div>".repeat(depth));
        ttml.push_str("</body></tt>");
        assert_eq!(
            open(ttml.as_bytes(), Some("ttml"), None),
            Err(Refusal::Damaged)
        );
        let mut spans = String::from(
            "<tt xmlns=\"http://www.w3.org/ns/ttml\"><body><div><p begin=\"1s\" end=\"2s\">",
        );
        spans.push_str(&"<span>".repeat(depth));
        spans.push_str(&"</span>".repeat(depth));
        spans.push_str("x</p></div></body></tt>");
        assert_eq!(
            open(spans.as_bytes(), Some("ttml"), None),
            Err(Refusal::Damaged)
        );
        let mut youtube = String::from("<timedtext>");
        youtube.push_str(&"<p>".repeat(depth));
        assert_eq!(
            open(youtube.as_bytes(), Some("srv3"), None),
            Err(Refusal::Damaged)
        );
        // A document as deep as the ceiling is still read.
        let mut fine = String::from("<tt xmlns=\"http://www.w3.org/ns/ttml\"><body>");
        fine.push_str(&"<div>".repeat(ronoctua_xml::DEPTH_MOST - 4));
        fine.push_str("<p begin=\"1s\" end=\"2s\">x</p>");
        fine.push_str(&"</div>".repeat(ronoctua_xml::DEPTH_MOST - 4));
        fine.push_str("</body></tt>");
        let Ok(Opened::Ours(track)) = open(fine.as_bytes(), Some("ttml"), None) else {
            panic!("a document at the ceiling is refused");
        };
        assert_eq!(track.cues.len(), 1);
    }
}
