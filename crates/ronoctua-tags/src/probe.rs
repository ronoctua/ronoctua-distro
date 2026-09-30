//! ffprobe and exiftool, as JSON, into the sheet's rows.
//!
//! Both parsers take the program's text and give rows back, so a recorded
//! answer in a test is worth as much as the program — which is what lets
//! these tests run on a machine with neither installed.

use std::ffi::OsString;
use std::path::Path;

use serde::de::{Deserializer, MapAccess, Visitor};

use crate::sheet::{
    Can, Chapter, Fact, Kind, Origin, Stream, StreamKind, Tag, TagValue, TechKey, Why,
};

/// ffprobe's argument list: everything, as JSON.
///
/// `-show_programs` is for a transport stream's count; `--` so a file called
/// `-i.mkv` is a file.
pub(crate) fn probe_args(path: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        "-show_chapters",
        "-show_programs",
        "--",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(path.as_os_str().to_owned());
    args
}

/// What ffprobe said.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Probed {
    /// `format_name` (`mov,mp4,m4a,3gp,3g2,mj2`).
    pub format_name: Option<String>,
    /// `format_long_name`.
    pub format_long: Option<String>,
    /// How long, in milliseconds.
    pub duration_ms: Option<u64>,
    /// The whole bit rate.
    pub bit_rate: Option<u64>,
    /// The container's tags, by name (the workspace's `serde_json` keeps
    /// no order).
    pub tags: Vec<(String, String)>,
    /// The streams.
    pub streams: Vec<ProbedStream>,
    /// The chapters.
    pub chapters: Vec<Chapter>,
    /// How many programs.
    pub programs: usize,
}

/// One stream, as ffprobe described it.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ProbedStream {
    pub index: u32,
    pub kind: Option<StreamKind>,
    pub codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub layout: Option<String>,
    pub bits: Option<u32>,
    pub default: bool,
    pub forced: bool,
    pub hearing: bool,
    pub attached: bool,
    pub tags: Vec<(String, String)>,
}

impl ProbedStream {
    /// A tag by name, any case.
    pub(crate) fn tag(&self, name: &str) -> Option<&str> {
        tag_in(&self.tags, name)
    }
}

/// A tag by name, any case.
pub(crate) fn tag_in<'a>(tags: &'a [(String, String)], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.trim().is_empty())
}

/// ffprobe's JSON, read.
///
/// # Errors
///
/// [`Why::Unreadable`] when it is not the JSON ffprobe writes.
pub(crate) fn parse(text: &[u8]) -> Result<Probed, Why> {
    let doc: serde_json::Value = serde_json::from_slice(text).map_err(|_| Why::Unreadable)?;
    let doc = doc.as_object().ok_or(Why::Unreadable)?;
    let mut out = Probed::default();
    if let Some(format) = doc.get("format") {
        out.format_name = string(format.get("format_name"));
        out.format_long = string(format.get("format_long_name"));
        out.duration_ms = seconds(format.get("duration")).map(ms);
        out.bit_rate = number(format.get("bit_rate")).map(|n| n as u64);
        out.tags = tags(format.get("tags"));
    }
    for s in doc
        .get("streams")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let disposition = |name: &str| {
            s.get("disposition")
                .and_then(|d| d.get(name))
                .and_then(serde_json::Value::as_i64)
                == Some(1)
        };
        let kind = match s.get("codec_type").and_then(serde_json::Value::as_str) {
            Some("video") => Some(StreamKind::Video),
            Some("audio") => Some(StreamKind::Audio),
            Some("subtitle") => Some(StreamKind::Subtitle),
            Some("data") => Some(StreamKind::Data),
            Some("attachment") => Some(StreamKind::Attachment),
            _ => None,
        };
        let fps = [s.get("avg_frame_rate"), s.get("r_frame_rate")]
            .into_iter()
            .find_map(|v| v.and_then(serde_json::Value::as_str).and_then(ratio));
        out.streams.push(ProbedStream {
            index: number(s.get("index")).map_or(0, |n| n as u32),
            kind,
            codec: string(s.get("codec_name")),
            width: number(s.get("width")).map(|n| n as u32).filter(|n| *n > 0),
            height: number(s.get("height")).map(|n| n as u32).filter(|n| *n > 0),
            fps,
            sample_rate: number(s.get("sample_rate"))
                .map(|n| n as u32)
                .filter(|n| *n > 0),
            channels: number(s.get("channels"))
                .map(|n| n as u32)
                .filter(|n| *n > 0),
            layout: string(s.get("channel_layout")),
            bits: number(s.get("bits_per_raw_sample"))
                .or_else(|| number(s.get("bits_per_sample")))
                .map(|n| n as u32)
                .filter(|n| *n > 0),
            default: disposition("default"),
            forced: disposition("forced"),
            hearing: disposition("hearing_impaired"),
            attached: disposition("attached_pic"),
            tags: tags(s.get("tags")),
        });
    }
    for c in doc
        .get("chapters")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(start) = seconds(c.get("start_time")) else {
            continue;
        };
        let chapter_tags = tags(c.get("tags"));
        out.chapters.push(Chapter {
            start_ms: ms(start),
            end_ms: seconds(c.get("end_time")).map(ms),
            title: tag_in(&chapter_tags, "title").map(ToOwned::to_owned),
        });
    }
    out.programs = doc
        .get("programs")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    Ok(out)
}

/// A JSON string or number as text.
fn string(v: Option<&serde_json::Value>) -> Option<String> {
    match v? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A number ffprobe writes as a number or as a string of one.
fn number(v: Option<&serde_json::Value>) -> Option<f64> {
    match v? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .filter(|n: &f64| n.is_finite() && *n >= 0.0)
}

/// Seconds, as ffprobe writes them.
fn seconds(v: Option<&serde_json::Value>) -> Option<f64> {
    number(v)
}

/// Seconds to whole milliseconds.
fn ms(seconds: f64) -> u64 {
    (seconds * 1000.0).round() as u64
}

/// `"24000/1001"` to 23.976…; `"0/0"` to nothing.
fn ratio(text: &str) -> Option<f64> {
    let (num, den) = text.split_once('/')?;
    let (num, den): (f64, f64) = (num.trim().parse().ok()?, den.trim().parse().ok()?);
    (den > 0.0 && num > 0.0).then(|| num / den)
}

/// A `tags` object, as pairs sorted by name.
fn tags(v: Option<&serde_json::Value>) -> Vec<(String, String)> {
    v.and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

/// A number with at most three decimals and no trailing zeros.
fn trimmed(n: f64) -> String {
    let mut s = format!("{n:.3}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    s
}

/// Whether the file is a film: a picture stream that is not a cover.
pub(crate) fn kind_of(probed: &Probed) -> Option<Kind> {
    if probed.streams.is_empty() {
        return None;
    }
    let film = probed
        .streams
        .iter()
        .any(|s| s.kind == Some(StreamKind::Video) && !s.attached);
    Some(if film { Kind::Video } else { Kind::Music })
}

/// The technical rows.
pub(crate) fn technical(probed: &Probed) -> Vec<Fact> {
    let mut out = Vec::new();
    let mut put = |key: TechKey, value: Option<String>| {
        if let Some(value) = value {
            out.push(Fact { key, value });
        }
    };
    put(
        TechKey::Container,
        probed
            .format_long
            .clone()
            .or_else(|| probed.format_name.clone()),
    );
    put(
        TechKey::DurationMs,
        probed.duration_ms.map(|n| n.to_string()),
    );
    put(
        TechKey::BitRateBitsPerSecond,
        probed.bit_rate.map(|n| n.to_string()),
    );
    if let Some(v) = probed
        .streams
        .iter()
        .find(|s| s.kind == Some(StreamKind::Video) && !s.attached)
    {
        put(TechKey::VideoCodec, v.codec.clone());
        put(TechKey::WidthPx, v.width.map(|n| n.to_string()));
        put(TechKey::HeightPx, v.height.map(|n| n.to_string()));
        put(TechKey::FramesPerSecond, v.fps.map(trimmed));
    }
    if let Some(a) = probed
        .streams
        .iter()
        .find(|s| s.kind == Some(StreamKind::Audio))
    {
        put(TechKey::AudioCodec, a.codec.clone());
        put(TechKey::SampleRateHz, a.sample_rate.map(|n| n.to_string()));
        put(TechKey::Channels, a.channels.map(|n| n.to_string()));
        put(TechKey::ChannelLayout, a.layout.clone());
        put(TechKey::BitsPerSample, a.bits.map(|n| n.to_string()));
    }
    if probed.programs > 0 {
        put(TechKey::Programs, Some(probed.programs.to_string()));
    }
    out
}

/// The streams, for the Streams page; `can` says, per stream, whether its
/// marks can be changed.
pub(crate) fn streams(probed: &Probed, can: impl Fn(&ProbedStream) -> Can) -> Vec<Stream> {
    probed
        .streams
        .iter()
        .filter_map(|s| {
            Some(Stream {
                index: s.index,
                kind: s.kind?,
                codec: s.codec.clone(),
                lang: s
                    .tag("language")
                    .filter(|l| *l != "und")
                    .map(ToOwned::to_owned),
                title: s.tag("title").map(ToOwned::to_owned),
                default: s.default,
                forced: s.forced,
                hearing: s.hearing,
                attached: s.attached,
                can: can(s),
            })
        })
        .collect()
}

/// Every tag ffprobe saw, the container's first.
pub(crate) fn listing(probed: &Probed) -> Vec<Tag> {
    let row = |stream: Option<u32>, (k, v): &(String, String)| Tag {
        origin: Origin::Ffprobe { stream },
        name: k.clone(),
        value: crate::native::text_value(v),
        can: Can::No(Why::ReadOnlyView),
    };
    let mut out: Vec<Tag> = probed.tags.iter().map(|t| row(None, t)).collect();
    for s in &probed.streams {
        out.extend(s.tags.iter().map(|t| row(Some(s.index), t)));
    }
    out
}

/// exiftool's argument list.
///
/// `-G1` names each tag's group (`ID3v2_4`, `Vorbis`, `ItemList`), `-a`
/// keeps duplicates (the same name in two groups is two tags), `-s` gives
/// the short names a file writes rather than exiftool's descriptions, and
/// `-n` gives numbers as numbers — the rule `ronoctua_inspect::facts`
/// explains.
pub(crate) fn exif_args(path: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-json", "-G1", "-a", "-s", "-n", "--"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(path.as_os_str().to_owned());
    args
}

/// The groups that are not the file's tags: exiftool's own, the
/// filesystem's, and what exiftool derived from the rest.
const NOT_TAGS: [&str; 4] = ["ExifTool", "System", "File", "Composite"];

/// The most rows exiftool may add to a sheet.
///
/// A camera's MakerNotes are hundreds of rows, and a page of them is what
/// the owner asked to see; past two thousand it is a sheet nobody scrolls
/// and a JSON line the socket carries for nothing.
const EXIF_MOST: usize = 2048;

/// exiftool's JSON, read — duplicates kept, in its order.
///
/// # Errors
///
/// [`Why::Unreadable`] when it is not the array of one object exiftool
/// writes.
pub(crate) fn parse_exif(text: &[u8]) -> Result<Vec<Tag>, Why> {
    let mut de = serde_json::Deserializer::from_slice(text);
    let files = de
        .deserialize_seq(FilesVisitor)
        .map_err(|_| Why::Unreadable)?;
    let pairs = files.into_iter().next().unwrap_or_default();
    let mut out = Vec::new();
    for (key, value) in pairs {
        if out.len() >= EXIF_MOST {
            break;
        }
        let Some((group, name)) = key.split_once(':') else {
            continue;
        };
        if NOT_TAGS.contains(&group) {
            continue;
        }
        let value = match value {
            serde_json::Value::String(s) => {
                binary(&s).unwrap_or_else(|| crate::native::text_value(&s))
            }
            serde_json::Value::Number(n) => TagValue::Text(n.to_string()),
            serde_json::Value::Bool(b) => TagValue::Text(u8::from(b).to_string()),
            serde_json::Value::Array(items) => {
                let joined: Vec<String> = items
                    .iter()
                    .map(|i| i.as_str().map_or_else(|| i.to_string(), ToOwned::to_owned))
                    .collect();
                crate::native::text_value(&joined.join("; "))
            }
            serde_json::Value::Null | serde_json::Value::Object(_) => continue,
        };
        out.push(Tag {
            origin: Origin::Exiftool {
                group: group.to_owned(),
            },
            name: name.to_owned(),
            value,
            can: Can::No(Why::ReadOnlyView),
        });
    }
    Ok(out)
}

/// exiftool's placeholder for bytes it did not print.
fn binary(text: &str) -> Option<TagValue> {
    let rest = text.strip_prefix("(Binary data ")?;
    let bytes: u64 = rest.split_whitespace().next()?.parse().ok()?;
    Some(TagValue::Binary { bytes })
}

/// One object's members, in order and with duplicates — `-a` writes the same
/// key twice when two groups hold it, and a map would keep one.
struct Pairs(Vec<(String, serde_json::Value)>);

impl<'de> serde::Deserialize<'de> for Pairs {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Pairs;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    out.push((k, v));
                }
                Ok(Pairs(out))
            }
        }
        d.deserialize_map(V)
    }
}

struct FilesVisitor;

impl<'de> Visitor<'de> for FilesVisitor {
    type Value = Vec<Vec<(String, serde_json::Value)>>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an array of objects")
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = Vec::new();
        while let Some(Pairs(p)) = seq.next_element::<Pairs>()? {
            out.push(p);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{kind_of, parse, parse_exif, streams, technical};
    use crate::sheet::{Can, Kind, Origin, StreamKind, TagValue, TechKey, Why};

    /// ffprobe on a Matroska file with a film, two sounds, a subtitle, a
    /// cover attachment and two chapters — trimmed from the real answer.
    const MKV: &str = r#"{
      "programs": [],
      "streams": [
        {"index":0,"codec_name":"h264","codec_type":"video","width":1920,"height":1080,
         "r_frame_rate":"24000/1001","avg_frame_rate":"24000/1001",
         "disposition":{"default":1,"forced":0,"hearing_impaired":0,"attached_pic":0},
         "tags":{"language":"und","title":"Main"}},
        {"index":1,"codec_name":"opus","codec_type":"audio","sample_rate":"48000","channels":6,
         "channel_layout":"5.1","disposition":{"default":1,"attached_pic":0},
         "tags":{"language":"por"}},
        {"index":2,"codec_name":"aac","codec_type":"audio","sample_rate":"44100","channels":2,
         "disposition":{"default":0},"tags":{"language":"eng","title":"Commentary"}},
        {"index":3,"codec_name":"subrip","codec_type":"subtitle",
         "disposition":{"forced":1,"hearing_impaired":1},"tags":{"language":"por"}},
        {"index":4,"codec_name":"mjpeg","codec_type":"video","width":600,"height":600,
         "r_frame_rate":"90000/1","disposition":{"attached_pic":1},
         "tags":{"filename":"cover.jpg","mimetype":"image/jpeg"}}
      ],
      "chapters": [
        {"id":1,"start_time":"0.000000","end_time":"61.500000","tags":{"title":"Opening"}},
        {"id":2,"start_time":"61.500000","end_time":"3600.000000"}
      ],
      "format": {"format_name":"matroska,webm","format_long_name":"Matroska / WebM",
        "duration":"3600.041000","bit_rate":"8000123",
        "tags":{"title":"A Film","ENCODER":"Lavf61"}}
    }"#;

    #[test]
    fn a_film_s_answer_becomes_rows() {
        let p = parse(MKV.as_bytes()).expect("it is ffprobe's JSON");
        assert_eq!(p.format_name.as_deref(), Some("matroska,webm"));
        assert_eq!(p.duration_ms, Some(3_600_041));
        assert_eq!(kind_of(&p), Some(Kind::Video));
        let facts = technical(&p);
        let get = |k: TechKey| facts.iter().find(|f| f.key == k).map(|f| f.value.as_str());
        assert_eq!(get(TechKey::Container), Some("Matroska / WebM"));
        assert_eq!(get(TechKey::VideoCodec), Some("h264"));
        assert_eq!(get(TechKey::FramesPerSecond), Some("23.976"));
        assert_eq!(get(TechKey::WidthPx), Some("1920"));
        assert_eq!(get(TechKey::SampleRateHz), Some("48000"));
        assert_eq!(get(TechKey::Channels), Some("6"));
        assert_eq!(get(TechKey::ChannelLayout), Some("5.1"));
        assert_eq!(get(TechKey::Programs), None);

        let s = streams(&p, |_| Can::No(Why::NoPlace));
        assert_eq!(s.len(), 5);
        assert_eq!(s[0].lang, None, "`und` is no language");
        assert_eq!(s[0].title.as_deref(), Some("Main"));
        assert!(s[3].forced && s[3].hearing);
        assert_eq!(s[3].kind, StreamKind::Subtitle);
        assert!(s[4].attached, "the cover is a picture stream, not a film");

        assert_eq!(p.chapters.len(), 2);
        assert_eq!(p.chapters[0].title.as_deref(), Some("Opening"));
        assert_eq!(p.chapters[0].end_ms, Some(61_500));
        assert_eq!(p.chapters[1].title, None);
    }

    #[test]
    fn a_song_with_a_cover_is_music() {
        let song = r#"{"streams":[
          {"index":0,"codec_type":"audio","codec_name":"mp3"},
          {"index":1,"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}}
        ],"format":{"format_name":"mp3"}}"#;
        let p = parse(song.as_bytes()).expect("parsed");
        assert_eq!(kind_of(&p), Some(Kind::Music));
        assert!(parse(b"not json").is_err());
    }

    #[test]
    fn exiftool_keeps_every_group_and_every_duplicate() {
        let dump = r#"[{
          "SourceFile": "/m/a.mp3",
          "ExifTool:ExifToolVersion": 13.1,
          "System:FileName": "a.mp3",
          "File:MIMEType": "audio/mpeg",
          "ID3v2_4:Title": "Song",
          "ID3v1:Title": "Song (short)",
          "ID3v2_4:Picture": "(Binary data 51234 bytes, use -b option to extract)",
          "ID3v2_4:Track": 3,
          "ID3v2_4:Genres": ["Rock", "Pop"],
          "Composite:Duration": 12.5
        }]"#;
        let rows = parse_exif(dump.as_bytes()).expect("parsed");
        let names: Vec<(&str, &str)> = rows
            .iter()
            .map(|t| match &t.origin {
                Origin::Exiftool { group } => (group.as_str(), t.name.as_str()),
                _ => ("?", "?"),
            })
            .collect();
        assert_eq!(
            names,
            [
                ("ID3v2_4", "Title"),
                ("ID3v1", "Title"),
                ("ID3v2_4", "Picture"),
                ("ID3v2_4", "Track"),
                ("ID3v2_4", "Genres"),
            ]
        );
        assert_eq!(rows[2].value, TagValue::Binary { bytes: 51234 });
        assert_eq!(rows[3].value, TagValue::Text("3".to_owned()));
        assert_eq!(rows[4].value, TagValue::Text("Rock; Pop".to_owned()));
        assert!(rows.iter().all(|r| r.can == Can::No(Why::ReadOnlyView)));
    }

    #[test]
    fn exiftool_writing_the_same_key_twice_keeps_both() {
        let dump = r#"[{"Vorbis:Artist":"A","Vorbis:Artist":"B"}]"#;
        let rows = parse_exif(dump.as_bytes()).expect("parsed");
        assert_eq!(rows.len(), 2);
        assert!(parse_exif(b"{}").is_err());
    }
}
