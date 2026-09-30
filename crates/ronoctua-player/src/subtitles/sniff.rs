//! Which format a subtitle file is, from what it CONTAINS.
//!
//! The extension lies too often to decide: `.sub` is MicroDVD, SubViewer,
//! MPsub or a VobSub picture stream; `.txt` is TMPlayer or VPlayer; `.stl` is
//! EBU's binary format, Spruce's text one or a 3D model; an SRT is saved as
//! `.vtt` by a converter that only renamed it. So the content decides, and the
//! extension only breaks a tie between formats the content fits equally well.
//!
//! Three answers: a format we read ourselves ([`Kind::Ours`]), one ffmpeg
//! reads for the engine ([`Kind::Ffmpeg`]), or a picture format mpv shows
//! natively ([`Kind::Image`]).

use serde::{Deserialize, Serialize};

use crate::subtitles::charset;
use crate::subtitles::{ebu_stl, microdvd, mpl2, sbv, srt, subviewer, tmplayer};

/// A format we read ourselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// SubRip, `.srt`.
    SubRip,
    /// WebVTT, `.vtt`.
    WebVtt,
    /// Advanced SubStation Alpha and SubStation Alpha, `.ass`/`.ssa`.
    Ass,
    /// Microsoft SAMI, `.smi`/`.sami`.
    Sami,
    /// MicroDVD, frames in braces, `.sub`/`.txt`.
    MicroDvd,
    /// MPL2, deciseconds in brackets, `.txt`/`.mpl`.
    Mpl2,
    /// SubViewer 1 and 2, `.sub`.
    SubViewer,
    /// TMPlayer, `hh:mm:ss:` lines, `.txt`.
    TmPlayer,
    /// YouTube's own SubViewer, `.sbv`.
    Sbv,
    /// TTML, DFXP and IMSC text, `.ttml`/`.dfxp`/`.xml`.
    Ttml,
    /// EBU Tech 3264 subtitling data exchange, binary, `.stl`.
    EbuStl,
    /// YouTube timed text in XML (formats 1 to 3), `.srv1`–`.srv3`.
    YouTubeXml,
    /// YouTube timed text in JSON, `.json3`.
    YouTubeJson,
    /// Timed lyrics, `.lrc`.
    Lrc,
}

impl Format {
    /// Every format we read, in the order a tie falls to.
    pub const ALL: [Self; 14] = [
        Self::SubRip,
        Self::WebVtt,
        Self::Ass,
        Self::Sami,
        Self::MicroDvd,
        Self::Mpl2,
        Self::SubViewer,
        Self::TmPlayer,
        Self::Sbv,
        Self::Ttml,
        Self::EbuStl,
        Self::YouTubeXml,
        Self::YouTubeJson,
        Self::Lrc,
    ];

    /// The extensions a file of this format is usually named with, lower
    /// case. Only a tie-breaker: the content decides.
    #[must_use]
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::SubRip => &["srt"],
            Self::WebVtt => &["vtt", "webvtt"],
            Self::Ass => &["ass", "ssa"],
            Self::Sami => &["smi", "sami"],
            Self::MicroDvd => &["sub", "txt"],
            Self::Mpl2 => &["txt", "mpl"],
            Self::SubViewer => &["sub"],
            Self::TmPlayer => &["txt"],
            Self::Sbv => &["sbv"],
            Self::Ttml => &["ttml", "dfxp", "xml"],
            Self::EbuStl => &["stl"],
            Self::YouTubeXml => &["srv1", "srv2", "srv3", "xml"],
            Self::YouTubeJson => &["json3", "json"],
            Self::Lrc => &["lrc"],
        }
    }
}

/// A text format ffmpeg reads and we do not: the engine converts it to ASS
/// (`ffmpeg -f <demuxer> -i … -f ass -`) and reads that with our ASS reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tail {
    /// JACOSub, `.jss`.
    JacoSub,
    /// Phoenix Japanimation Society, `.pjs`.
    Pjs,
    /// AQTitle, `.aqt`.
    AqTitle,
    /// RealText, `.rt`.
    RealText,
    /// VPlayer, `hh:mm:ss.cc:` lines, `.txt`.
    VPlayer,
    /// MPlayer's own, `.sub`.
    MpSub,
    /// Scenarist closed captions, `.scc`.
    Scc,
    /// Spruce's text STL, `.stl`.
    SpruceStl,
    /// MacCaption, `.mcc`.
    Mcc,
}

impl Tail {
    /// Every format of the ffmpeg route: the table the engine converts by.
    pub const ALL: [Self; 9] = [
        Self::JacoSub,
        Self::Pjs,
        Self::AqTitle,
        Self::RealText,
        Self::VPlayer,
        Self::MpSub,
        Self::Scc,
        Self::SpruceStl,
        Self::Mcc,
    ];

    /// The demuxer that reads it, for `ffmpeg -f` (the pinned ffmpeg 8.1.2
    /// lists every one of them under `ffmpeg -demuxers`).
    #[must_use]
    pub const fn demuxer(self) -> &'static str {
        match self {
            Self::JacoSub => "jacosub",
            Self::Pjs => "pjs",
            Self::AqTitle => "aqtitle",
            Self::RealText => "realtext",
            Self::VPlayer => "vplayer",
            Self::MpSub => "mpsub",
            Self::Scc => "scc",
            Self::SpruceStl => "stl",
            Self::Mcc => "mcc",
        }
    }

    /// The extensions a file of it is usually named with, lower case.
    #[must_use]
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::JacoSub => &["jss", "js"],
            Self::Pjs => &["pjs"],
            Self::AqTitle => &["aqt"],
            Self::RealText => &["rt"],
            Self::VPlayer => &["txt"],
            Self::MpSub => &["sub"],
            Self::Scc => &["scc"],
            Self::SpruceStl => &["stl"],
            Self::Mcc => &["mcc"],
        }
    }
}

/// A subtitle drawn as pictures, which mpv shows natively — one at a time,
/// and in its own colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Picture {
    /// Blu-ray's Presentation Graphic Stream, `.sup`.
    Pgs,
    /// DVD's VobSub: the `.idx` index and its `.sub` stream.
    VobSub,
}

/// What a subtitle file is, and so who reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "route", content = "format")]
pub enum Kind {
    /// We read it.
    Ours(Format),
    /// ffmpeg reads it, for the engine.
    Ffmpeg(Tail),
    /// mpv shows it as pictures.
    Image(Picture),
}

impl Kind {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Ours(f) => f.extensions(),
            Self::Ffmpeg(t) => t.extensions(),
            Self::Image(Picture::Pgs) => &["sup"],
            Self::Image(Picture::VobSub) => &["idx", "sub"],
        }
    }
}

/// A format, and the recogniser of its lines: each line it knows is a vote.
type Voter = (Kind, fn(&str) -> bool);

/// How much of a text file is looked at to decide.
const HEAD_BYTES: usize = 64 * 1024;
/// How many non-blank lines of it.
const HEAD_LINES: usize = 200;

/// Decides the format of `bytes`; `extension` (without the dot) only breaks
/// a tie. `None`: no subtitle format we know.
#[must_use]
pub fn sniff(bytes: &[u8], extension: Option<&str>) -> Option<Kind> {
    if let Some(kind) = binary(bytes) {
        return Some(kind);
    }
    let head = charset::decode(&bytes[..bytes.len().min(HEAD_BYTES)]);
    text(&head.text, extension)
}

/// The binary formats, from their first bytes.
#[must_use]
pub fn binary(bytes: &[u8]) -> Option<Kind> {
    if ebu_stl::is_ebu(bytes) {
        return Some(Kind::Ours(Format::EbuStl));
    }
    // PGS: every segment starts `PG`, two timestamps, then its type.
    if bytes.len() >= 13 && bytes.starts_with(b"PG") && matches!(bytes[10], 0x14..=0x18 | 0x80) {
        return Some(Kind::Image(Picture::Pgs));
    }
    // VobSub's `.sub`: an MPEG program stream, pack header first.
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0xBA]) {
        return Some(Kind::Image(Picture::VobSub));
    }
    None
}

/// Decides a text file's format from its text; `extension` only breaks a
/// tie.
#[must_use]
pub fn text(text: &str, extension: Option<&str>) -> Option<Kind> {
    let extension = extension.map(|e| e.trim_start_matches('.').to_ascii_lowercase());
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(HEAD_LINES)
        .collect();
    let first = lines.first().copied().unwrap_or("");
    let upper_first = first.to_ascii_uppercase();

    // Signatures: a header only one format writes.
    if upper_first.starts_with("WEBVTT") {
        return Some(Kind::Ours(Format::WebVtt));
    }
    if upper_first.starts_with("SCENARIST_SCC") {
        return Some(Kind::Ffmpeg(Tail::Scc));
    }
    if upper_first.starts_with("FILE FORMAT=MACCAPTION_MCC") {
        return Some(Kind::Ffmpeg(Tail::Mcc));
    }
    if upper_first.starts_with("# VOBSUB INDEX FILE") {
        return Some(Kind::Image(Picture::VobSub));
    }
    let head_upper: Vec<String> = lines
        .iter()
        .take(40)
        .map(|l| l.to_ascii_uppercase())
        .collect();
    let any = |p: &dyn Fn(&str) -> bool| head_upper.iter().any(|l| p(l));
    if any(&|l| l == "[SCRIPT INFO]" || l == "[V4+ STYLES]" || l == "[V4 STYLES]") {
        return Some(Kind::Ours(Format::Ass));
    }
    if first.starts_with('<') {
        let mut end = text.len().min(HEAD_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let lower = text[..end].to_ascii_lowercase();
        if lower.contains("<sami") {
            return Some(Kind::Ours(Format::Sami));
        }
        if lower.contains("<timedtext") || lower.contains("<transcript") {
            return Some(Kind::Ours(Format::YouTubeXml));
        }
        if has_element(&lower, "tt") || lower.contains(":tt ") || lower.contains(":tt>") {
            return Some(Kind::Ours(Format::Ttml));
        }
        if has_element(&lower, "window") {
            return Some(Kind::Ffmpeg(Tail::RealText));
        }
        return None;
    }
    if first.starts_with('{') && text.contains("\"events\"") {
        return Some(Kind::Ours(Format::YouTubeJson));
    }
    if any(&|l| l == "[INFORMATION]" || l == "[SUBTITLE]") {
        return Some(Kind::Ours(Format::SubViewer));
    }
    if any(&|l| l.starts_with("FORMAT=")) {
        return Some(Kind::Ffmpeg(Tail::MpSub));
    }
    if any(&|l| l.starts_with("#TIMERES") || l.starts_with("#T"))
        && lines.iter().any(|l| jacosub_line(l))
    {
        return Some(Kind::Ffmpeg(Tail::JacoSub));
    }

    // Lines: each format's own line recogniser votes; the most lines win.
    let voters: [Voter; 12] = [
        (Kind::Ours(Format::SubRip), srt::is_timing),
        (Kind::Ours(Format::Sbv), sbv::is_timing),
        (Kind::Ours(Format::SubViewer), subviewer::is_timing),
        (Kind::Ours(Format::MicroDvd), microdvd::is_line),
        (Kind::Ours(Format::Mpl2), mpl2::is_line),
        (Kind::Ours(Format::Lrc), lrc_line),
        (Kind::Ours(Format::TmPlayer), tmplayer::is_line),
        (Kind::Ffmpeg(Tail::VPlayer), vplayer_line),
        (Kind::Ffmpeg(Tail::JacoSub), jacosub_line),
        (Kind::Ffmpeg(Tail::Pjs), pjs_line),
        (Kind::Ffmpeg(Tail::AqTitle), |l: &str| {
            l.starts_with("-->> ")
        }),
        (Kind::Ffmpeg(Tail::SpruceStl), spruce_line),
    ];
    let votes: Vec<(Kind, usize)> = voters
        .iter()
        .map(|(kind, is)| (*kind, lines.iter().filter(|l| is(l)).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    let most = votes.iter().map(|(_, n)| *n).max()?;
    let tied: Vec<Kind> = votes
        .iter()
        .filter(|(_, n)| *n == most)
        .map(|(k, _)| *k)
        .collect();
    let by_extension = extension
        .as_deref()
        .and_then(|e| tied.iter().copied().find(|k| k.extensions().contains(&e)));
    by_extension.or_else(|| tied.first().copied())
}

/// Whether `<name` opens an element in `lower` (followed by a space, `>` or
/// `/`), not a longer name that starts the same.
fn has_element(lower: &str, name: &str) -> bool {
    let needle = format!("<{name}");
    lower.match_indices(&needle).any(|(at, _)| {
        lower[at + needle.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace() || c == '>' || c == '/')
    })
}

/// `[01:23.45]words`, `[01:23]` or `[01:23:45]`: a timed lyric line.
fn lrc_line(line: &str) -> bool {
    let Some(inner) = line.strip_prefix('[').and_then(|r| r.split(']').next()) else {
        return false;
    };
    let Some((minutes, seconds)) = inner.split_once(':') else {
        return false;
    };
    let seconds = seconds.replacen(':', ".", 1);
    !minutes.is_empty()
        && minutes.bytes().all(|b| b.is_ascii_digit())
        && seconds
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// `00:00:10.50:words` (or `=`, or a space): VPlayer's line — TMPlayer's
/// with hundredths.
fn vplayer_line(line: &str) -> bool {
    let mut fields = line.splitn(3, ':');
    let (Some(h), Some(m), Some(rest)) = (fields.next(), fields.next(), fields.next()) else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit());
    let Some((seconds, rest)) = rest.split_once('.') else {
        return false;
    };
    let hundredths_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    digits(h)
        && digits(m)
        && digits(seconds)
        && hundredths_end > 0
        && rest[hundredths_end..].starts_with([':', ' ', '='])
}

/// `0:00:01.00 0:00:03.00 D words`: JACOSub's timed line.
fn jacosub_line(line: &str) -> bool {
    let mut words = line.split_whitespace();
    let (Some(a), Some(b)) = (words.next(), words.next()) else {
        return false;
    };
    let clock = |s: &str| {
        let parts: Vec<&str> = s.split([':', '.']).collect();
        parts.len() == 4
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    };
    clock(a) && clock(b)
}

/// `1234, 5678, "words"`: a PJS line.
fn pjs_line(line: &str) -> bool {
    let mut parts = line.splitn(3, ',');
    let (Some(a), Some(b), Some(text)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let number = |s: &str| {
        let s = s.trim();
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    };
    number(a) && number(b) && text.trim_start().starts_with('"')
}

/// `00:00:01:00 , 00:00:03:00 , words`: Spruce STL's line, timecodes with
/// frames.
fn spruce_line(line: &str) -> bool {
    let mut parts = line.splitn(3, ',');
    let (Some(a), Some(b), Some(_)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let timecode = |s: &str| {
        let fields: Vec<&str> = s.trim().split(':').collect();
        fields.len() == 4
            && fields
                .iter()
                .all(|f| f.len() == 2 && f.bytes().all(|b| b.is_ascii_digit()))
    };
    timecode(a) && timecode(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MICRODVD: &str = "{1}{1}25\n{25}{50}Hello|world\n{75}{100}Again\n";
    const SUBVIEWER: &str =
        "[INFORMATION]\n[TITLE]x\n[END INFORMATION]\n[SUBTITLE]\n00:00:01.00,00:00:02.00\nHello\n";
    const MPSUB: &str = "TITLE=x\nFORMAT=TIME\n\n1 2\nHello\n\n0 1.5\nAgain\n";

    #[test]
    fn a_dot_sub_is_decided_by_what_it_holds() {
        assert_eq!(
            sniff(MICRODVD.as_bytes(), Some("sub")),
            Some(Kind::Ours(Format::MicroDvd))
        );
        assert_eq!(
            sniff(SUBVIEWER.as_bytes(), Some("sub")),
            Some(Kind::Ours(Format::SubViewer))
        );
        assert_eq!(
            sniff(MPSUB.as_bytes(), Some("sub")),
            Some(Kind::Ffmpeg(Tail::MpSub))
        );
        let vobsub = [0x00, 0x00, 0x01, 0xBA, 0x44, 0x00, 0x04, 0x00];
        assert_eq!(
            sniff(&vobsub, Some("sub")),
            Some(Kind::Image(Picture::VobSub))
        );
    }

    #[test]
    fn subviewer_without_its_header_ties_with_sbv_and_the_extension_breaks_it() {
        let body = "00:00:01.000,00:00:02.000\nHello\n\n00:00:03.000,00:00:04.000\nAgain\n";
        assert_eq!(
            sniff(body.as_bytes(), Some("sub")),
            Some(Kind::Ours(Format::SubViewer))
        );
        assert_eq!(
            sniff(body.as_bytes(), Some("sbv")),
            Some(Kind::Ours(Format::Sbv))
        );
        // No extension: the first of the tie, in the table's order.
        assert!(sniff(body.as_bytes(), None).is_some());
        // Hundredths are SubViewer's own spelling, whatever the name.
        let hundredths = "00:00:01.00,00:00:02.00\nHello\n\n00:00:03.00,00:00:04.00\nAgain\n";
        assert_eq!(
            sniff(hundredths.as_bytes(), Some("sbv")),
            Some(Kind::Ours(Format::SubViewer))
        );
    }

    #[test]
    fn the_content_beats_the_extension() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";
        assert_eq!(
            sniff(srt.as_bytes(), Some("vtt")),
            Some(Kind::Ours(Format::SubRip))
        );
        let tmplayer = "00:00:01:Hello\n00:00:03:Again\n";
        let vplayer = "00:00:01.50:Hello\n00:00:03.00:Again\n";
        assert_eq!(
            sniff(tmplayer.as_bytes(), Some("txt")),
            Some(Kind::Ours(Format::TmPlayer))
        );
        assert_eq!(
            sniff(vplayer.as_bytes(), Some("txt")),
            Some(Kind::Ffmpeg(Tail::VPlayer))
        );
        assert_eq!(
            sniff("[00:01.00]la\n[00:02.50]la\n".as_bytes(), Some("txt")),
            Some(Kind::Ours(Format::Lrc))
        );
        assert_eq!(
            sniff("[10][25]Hello\n[30][45]Again\n".as_bytes(), Some("txt")),
            Some(Kind::Ours(Format::Mpl2))
        );
    }

    #[test]
    fn signatures_and_pictures_are_known() {
        assert_eq!(
            sniff(b"WEBVTT\n\n00:01.000 --> 00:02.000\nx\n", Some("srt")),
            Some(Kind::Ours(Format::WebVtt))
        );
        assert_eq!(
            sniff(b"[Script Info]\nScriptType: v4.00+\n", None),
            Some(Kind::Ours(Format::Ass))
        );
        assert_eq!(
            sniff(
                b"<?xml version=\"1.0\"?>\n<tt xmlns=\"http://www.w3.org/ns/ttml\"></tt>",
                Some("xml")
            ),
            Some(Kind::Ours(Format::Ttml))
        );
        assert_eq!(
            sniff(
                b"<?xml version=\"1.0\"?><timedtext format=\"3\"></timedtext>",
                Some("xml")
            ),
            Some(Kind::Ours(Format::YouTubeXml))
        );
        assert_eq!(
            sniff(b"{\"wireMagic\":\"pb3\",\"events\":[]}", Some("json3")),
            Some(Kind::Ours(Format::YouTubeJson))
        );
        assert_eq!(
            sniff(
                b"# VobSub index file, v7 (do not modify this line!)\n",
                Some("idx")
            ),
            Some(Kind::Image(Picture::VobSub))
        );
        let mut pgs = b"PG".to_vec();
        pgs.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0x16, 0, 0x13]);
        assert_eq!(sniff(&pgs, Some("sup")), Some(Kind::Image(Picture::Pgs)));
        assert_eq!(
            sniff(b"Scenarist_SCC V1.0\n", None),
            Some(Kind::Ffmpeg(Tail::Scc))
        );
        assert_eq!(
            sniff(b"00:00:01:00 , 00:00:02:00 , Hello\n", Some("stl")),
            Some(Kind::Ffmpeg(Tail::SpruceStl))
        );
        assert_eq!(sniff(b"just some words\n", Some("srt")), None);
    }

    #[test]
    fn every_ffmpeg_route_names_a_demuxer_of_its_own() {
        let mut demuxers: Vec<&str> = Tail::ALL.iter().map(|t| t.demuxer()).collect();
        demuxers.sort_unstable();
        demuxers.dedup();
        assert_eq!(demuxers.len(), Tail::ALL.len());
    }
}
