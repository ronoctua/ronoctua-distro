//! What a file SAYS about itself — the facts only a tool can read.
//!
//! # What is deliberately NOT here
//!
//! Name, size, mode, owner, links, symlink target and the three timestamps.
//! `ronoctua_shell::files::View::detail_rows` already shows all of them, out of
//! `ronoctua_files::Entry`, `who`, `when` and `mode`, and it does it with no
//! process and no I/O beyond one `lstat`. A second list of the same facts would
//! be the "two implementations" this arrangement exists to prevent — the day
//! they disagreed about a permission bit, nothing on screen would say which is
//! right.
//!
//! What is here is the half that needs a program: dimensions, duration, codecs,
//! camera, exposure, where the photograph was taken, how many pages.
//!
//! # One call per tool, never one per fact
//!
//! `ffprobe` answers dimensions, duration, both codecs, the frame rate, the bit
//! rate and the embedded tags in a single JSON document; `exiftool` answers the
//! whole camera in another. Two spawns for a photograph, two for a film, one
//! for a PDF. Asking five programs five questions would be five process
//! creations for a panel that has to appear while somebody is holding a key
//! down.
//!
//! # Why exiftool and not an Exif crate in this process
//!
//! `kamadak-exif` would work everywhere, is small, and needs no child. It is
//! rejected because exiftool already covers every container the owner actually
//! has — DNG, HEIC, MOV — at no closure cost, and a second Exif reader is
//! exactly the duplication above: the day the two disagreed about the SIGN of a
//! longitude, the panel would show a place on the wrong side of Greenwich and
//! nothing would say which reader was wrong. If that is ever overruled it is a
//! one-file change, and this is the paragraph to argue with.
//!
//! # The parsers take a `&str`
//!
//! [`from_probe`], [`from_exif`] and [`from_pdfinfo`] are pure: text in, facts
//! out. That is what lets the whole of this module be tested on a machine with
//! none of the programs installed — the same reason `ronoctua_files` makes
//! every root a parameter.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::key;
use crate::tools::{self, Tool};

/// A row of the info panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    /// The row's label, as a text key.
    pub label: &'static str,
    /// What the row says.
    pub value: Value,
}

/// What a fact's value is.
///
/// # Why two shapes and not one
///
/// Because half of these values are DATA and half are sentences. A camera's
/// model is `Pixel 8 Pro` in every language and translating it would be wrong —
/// the same rule the file manager already follows for a `Choice`'s label, which
/// is a filename and therefore text and never a key. A frame rate is not:
/// where the unit goes, whether there is a space before it and what two
/// channels are called are all the LANGUAGE's business, and that belongs in the
/// `.ftl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Read out of the file and shown verbatim.
    Data(String),
    /// A text key and its arguments (invariant 8).
    Phrase {
        /// The key.
        key: &'static str,
        /// The arguments, by name.
        args: Vec<(&'static str, String)>,
    },
}

impl Value {
    /// A phrase with no arguments.
    #[must_use]
    pub const fn phrase(key: &'static str) -> Self {
        Self::Phrase {
            key,
            args: Vec::new(),
        }
    }
}

/// Whether the coordinates of a photograph are shown.
///
/// # Why this is a choice and not a fact like any other
///
/// Because it is the one thing in the panel that is about the PERSON and not
/// about the file. The info panel is what somebody opens just before sending a
/// photograph to a stranger, and a screen share, a projector or somebody
/// standing behind them turns "where this was taken" into a disclosure nobody
/// asked for — most often the house it was taken in.
///
/// So the default withholds the numbers and still says the file CARRIES them,
/// which is the fact that actually helps: you want to know before you send it.
/// [`Place::Shown`] is a deliberate act, and the drawing wave decides which key
/// asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// Say that the file knows where it was made; do not say where.
    #[default]
    Withheld,
    /// Show the coordinates.
    Shown,
}

/// The tools a file of this type would be asked.
///
/// For a screen that wants to say "exiftool is not installed" rather than
/// simply showing fewer rows. The facts themselves never carry an "unknown"
/// row — a fact with no answer is absent, because a panel of "unknown" is a
/// panel nobody reads twice.
#[must_use]
pub fn needs(mime: &str) -> Vec<Tool> {
    let mime = ronoctua_files::mime::canonical(mime);
    let media = mime.split_once('/').map_or(mime, |(m, _)| m);
    let mut out = Vec::new();
    if matches!(media, "image" | "audio" | "video") {
        out.push(Tool::Ffprobe);
    }
    if matches!(media, "image" | "video") {
        out.push(Tool::Exiftool);
    }
    if mime == "application/pdf" {
        out.push(Tool::Pdfinfo);
    }
    out
}

/// Everything the tools can say about one file, in order.
///
/// Synchronous, and it spawns at most two children. The caller runs it off the
/// loop that draws — the same long-lived thread the preview uses.
#[must_use]
pub fn facts(path: &Path, mime: &str, place: Place) -> Vec<Fact> {
    let mut out = Vec::new();
    for tool in needs(mime) {
        let args = match tool {
            Tool::Ffprobe => probe_args(path),
            Tool::Exiftool => exif_args(path),
            Tool::Pdfinfo => vec![OsString::from("--"), path.as_os_str().to_owned()],
            Tool::Ffmpeg | Tool::Pdftoppm => continue,
        };
        let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        match tools::run(tool, &borrowed) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                out.extend(match tool {
                    Tool::Ffprobe => from_probe(&text),
                    Tool::Exiftool => from_exif(&text, place),
                    _ => from_pdfinfo(&text),
                });
            }
            Err(why) => {
                // NO ROW SAYING "UNKNOWN". The absence is the answer, and a
                // caller that wants to name the missing program asks `needs`
                // and `Tool::there`.
                tracing::debug!(tool = ?tool, why = ?why, "no facts from this program");
            }
        }
    }
    out
}

/// How long a moving picture is, for the preview's seek.
///
/// A narrow probe rather than the full one: it answers in a few milliseconds
/// and the preview needs exactly this one number. A caller that has already run
/// [`facts`] should pass what it learned to `pixels::at` instead of paying for
/// this again.
#[must_use]
pub fn duration_of(path: &Path) -> Option<f64> {
    let args = [
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-show_entries"),
        OsStr::new("format=duration"),
        OsStr::new("-of"),
        OsStr::new("default=nw=1:nk=1"),
        OsStr::new("--"),
        path.as_os_str(),
    ];
    let out = tools::run(Tool::Ffprobe, &args).ok()?;
    String::from_utf8_lossy(&out).trim().parse().ok()
}

/// ffprobe's argument list.
///
/// `--` is end-of-options, so a file called `-i.mp4` is a file. ffmpeg's own
/// option parser honours it, which is worth knowing because most of its
/// documentation does not mention it.
#[must_use]
pub fn probe_args(path: &Path) -> Vec<OsString> {
    vec![
        "-v".into(),
        "error".into(),
        "-print_format".into(),
        "json".into(),
        "-show_format".into(),
        "-show_streams".into(),
        "--".into(),
        path.as_os_str().to_owned(),
    ]
}

/// exiftool's argument list.
///
/// # Why `-n` is load-bearing
///
/// Without it exiftool prints `51 deg 29' 47.19" N` — a formatted string, in
/// English, from another program's idea of how a coordinate looks. With it the
/// same tag is `51.4964416666667`, a signed decimal, and the sentence stays
/// ours in the `.ftl` (invariant 8). The same flag is what turns an exposure
/// into `0.010392` instead of `1/96`, which is the number this module can do
/// arithmetic on.
///
/// The tags are NAMED rather than asking for everything: a Pixel's DNG carries
/// several hundred, and the panel wants eleven.
#[must_use]
pub fn exif_args(path: &Path) -> Vec<OsString> {
    vec![
        "-json".into(),
        "-n".into(),
        "-Make".into(),
        "-Model".into(),
        "-LensModel".into(),
        "-ISO".into(),
        "-FNumber".into(),
        "-ExposureTime".into(),
        "-FocalLength".into(),
        "-DateTimeOriginal".into(),
        "-GPSLatitude".into(),
        "-GPSLongitude".into(),
        "--".into(),
        path.as_os_str().to_owned(),
    ]
}

/// The facts in ffprobe's JSON.
///
/// # Why `format.duration` decides what is reported
///
/// Because ffprobe answers a PNG with `r_frame_rate: 25/1` — a frame rate for
/// a still photograph, invented by the `png_pipe` demuxer. The container is
/// reported as "piped png sequence" for the same reason. Neither is a fact
/// about the file; both are facts about how ffmpeg opened it.
///
/// A `duration` in the FORMAT is what actually separates the two: a PNG, a
/// HEIC and a DNG have none, an animated GIF has 3.96 s. So dimensions are
/// always reported and everything that implies time is reported only when
/// there is time.
#[must_use]
pub fn from_probe(text: &str) -> Vec<Fact> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let streams = doc.get("streams").and_then(|s| s.as_array());
    let empty = Vec::new();
    let streams = streams.unwrap_or(&empty);
    let of_type = |kind: &str| {
        streams.iter().find(|s| {
            s.get("codec_type").and_then(serde_json::Value::as_str) == Some(kind)
                // AN ALBUM'S COVER IS A VIDEO STREAM. Reporting it as "the
                // video codec" of an mp3 would be true and useless.
                && s.pointer("/disposition/attached_pic")
                    .and_then(serde_json::Value::as_i64)
                    != Some(1)
        })
    };
    let video = of_type("video");
    let audio = of_type("audio");
    let format = doc.get("format");
    let container = format
        .and_then(|f| f.get("format_name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let plays = format
        .and_then(|f| f.get("duration"))
        .and_then(text_or_number)
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| *d > 0.0)
        .filter(|_| !pretend_film(container));

    let mut out = Vec::new();
    if let Some(v) = video
        && let (Some(w), Some(h)) = (
            v.get("width").and_then(serde_json::Value::as_u64),
            v.get("height").and_then(serde_json::Value::as_u64),
        )
    {
        out.push(Fact {
            label: key::FACT_DIMENSIONS,
            value: Value::Phrase {
                key: key::VALUE_PIXELS,
                args: vec![("width", w.to_string()), ("height", h.to_string())],
            },
        });
    }
    let Some(seconds) = plays else {
        return out;
    };
    out.push(Fact {
        label: key::FACT_DURATION,
        value: Value::Data(clock(seconds)),
    });
    if let Some(name) = format
        .and_then(|f| f.get("format_long_name"))
        .and_then(serde_json::Value::as_str)
    {
        out.push(Fact {
            label: key::FACT_CONTAINER,
            value: Value::Data(name.to_owned()),
        });
    }
    if let Some(v) = video {
        if let Some(codec) = codec_name(v) {
            out.push(Fact {
                label: key::FACT_VIDEO,
                value: Value::Data(codec),
            });
        }
        if let Some(fps) = v
            .get("avg_frame_rate")
            .or_else(|| v.get("r_frame_rate"))
            .and_then(serde_json::Value::as_str)
            .and_then(ratio)
        {
            out.push(Fact {
                label: key::FACT_RATE,
                value: Value::Phrase {
                    key: key::VALUE_FPS,
                    args: vec![("fps", trimmed(fps))],
                },
            });
        }
    }
    if let Some(a) = audio {
        if let Some(codec) = codec_name(a) {
            out.push(Fact {
                label: key::FACT_AUDIO,
                value: Value::Data(codec),
            });
        }
        if let Some(hz) = a
            .get("sample_rate")
            .and_then(text_or_number)
            .and_then(|s| s.parse::<f64>().ok())
        {
            out.push(Fact {
                label: key::FACT_SAMPLING,
                value: Value::Phrase {
                    key: key::VALUE_HERTZ,
                    args: vec![("khz", trimmed(hz / 1000.0))],
                },
            });
        }
        if let Some(n) = a.get("channels").and_then(serde_json::Value::as_u64) {
            out.push(Fact {
                label: key::FACT_CHANNELS,
                value: Value::Phrase {
                    key: key::VALUE_CHANNELS,
                    args: vec![("n", n.to_string())],
                },
            });
        }
    }
    if let Some(bits) = format
        .and_then(|f| f.get("bit_rate"))
        .and_then(text_or_number)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|b| *b > 0.0)
    {
        out.push(Fact {
            label: key::FACT_BITRATE,
            value: Value::Phrase {
                key: key::VALUE_KBPS,
                args: vec![("kbps", ((bits / 1000.0).round() as u64).to_string())],
            },
        });
    }
    // THE TAGS ARE CASE-INSENSITIVE because the containers disagree: Matroska
    // and Ogg write `TITLE`, MP4 writes `title`, and a panel that showed one
    // and not the other would look like a bug in the file.
    for (label, wanted) in [
        (key::FACT_TITLE, "title"),
        (key::FACT_ARTIST, "artist"),
        (key::FACT_ALBUM, "album"),
    ] {
        if let Some(v) = tag(format, wanted) {
            out.push(Fact {
                label,
                value: Value::Data(v),
            });
        }
    }
    out
}

/// The facts in exiftool's JSON.
///
/// It answers with an ARRAY of one object per file, and a file it knows nothing
/// about still gets an object — with `SourceFile` and nothing else. That is not
/// an error and must not read like one: a PNG with no camera in it simply has
/// no camera rows.
#[must_use]
pub fn from_exif(text: &str, place: Place) -> Vec<Fact> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(one) = doc.as_array().and_then(|a| a.first()) else {
        return Vec::new();
    };
    let string = |k: &str| {
        one.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    };
    let number = |k: &str| one.get(k).and_then(serde_json::Value::as_f64);

    let mut out = Vec::new();
    // MAKE AND MODEL ARE ONE FACT. "Google" on one row and "Pixel 8 Pro" on
    // the next is two rows saying one thing, and the second one already
    // repeats the first on most cameras ("NIKON CORPORATION" / "NIKON D850").
    if let Some(camera) = camera(string("Make").as_deref(), string("Model").as_deref()) {
        out.push(Fact {
            label: key::FACT_CAMERA,
            value: Value::Data(camera),
        });
    }
    if let Some(lens) = string("LensModel") {
        out.push(Fact {
            label: key::FACT_LENS,
            value: Value::Data(lens),
        });
    }
    if let Some(when) = string("DateTimeOriginal").as_deref().and_then(taken) {
        out.push(Fact {
            label: key::FACT_TAKEN,
            value: Value::Data(when),
        });
    }
    if let Some(iso) = number("ISO") {
        out.push(Fact {
            label: key::FACT_ISO,
            value: Value::Data(trimmed(iso)),
        });
    }
    if let Some(t) = number("ExposureTime").filter(|t| *t > 0.0) {
        out.push(Fact {
            label: key::FACT_SHUTTER,
            value: shutter(t),
        });
    }
    if let Some(f) = number("FNumber").filter(|f| *f > 0.0) {
        out.push(Fact {
            label: key::FACT_APERTURE,
            value: Value::Data(format!("f/{}", trimmed(f))),
        });
    }
    if let Some(mm) = number("FocalLength").filter(|m| *m > 0.0) {
        out.push(Fact {
            label: key::FACT_FOCAL,
            value: Value::Phrase {
                key: key::VALUE_MILLIMETRES,
                args: vec![("mm", trimmed(mm))],
            },
        });
    }
    if let (Some(lat), Some(lon)) = (number("GPSLatitude"), number("GPSLongitude")) {
        out.push(Fact {
            label: key::FACT_PLACE,
            value: match place {
                Place::Withheld => Value::phrase(key::VALUE_PLACE_WITHHELD),
                // FIVE DECIMALS is about a metre, which is as much as a phone
                // knows and more than anybody reads off a screen.
                Place::Shown => Value::Phrase {
                    key: key::VALUE_COORDINATES,
                    args: vec![("lat", format!("{lat:.5}")), ("lon", format!("{lon:.5}"))],
                },
            },
        });
    }
    out
}

/// The page count in pdfinfo's output.
///
/// `Pages:          12` — a colon, then whitespace, then the number. Nothing
/// else in that report is worth a row that the mime type and the size do not
/// already give.
#[must_use]
pub fn from_pdfinfo(text: &str) -> Vec<Fact> {
    text.lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .and_then(|n| n.trim().parse::<u64>().ok())
        .map(|n| {
            vec![Fact {
                label: key::FACT_PAGES,
                value: Value::Phrase {
                    key: key::VALUE_PAGES,
                    args: vec![("n", n.to_string())],
                },
            }]
        })
        .unwrap_or_default()
}

/// Whether ffmpeg opened a still photograph by pretending it was a film.
///
/// # Why the demuxer's NAME is the only place this shows
///
/// A JPEG comes back with `duration: 0.04`, `r_frame_rate: 25/1` and a bit
/// rate of 4 439 000 kbit/s — one frame at ffmpeg's default frame rate, and a
/// bit rate computed from it. Every one of those numbers is a fact about how
/// ffmpeg opened the file and none of them is a fact about the file, and a
/// panel that showed "25 fps" under a photograph would be lying with a
/// straight face. It was caught on the owner's own `.jpg`.
///
/// The demuxers that do it are `image2` and everything ending in `_pipe`
/// (`png_pipe`, `webp_pipe`, `tiff_pipe`). An animated GIF is opened by the
/// real `gif` demuxer and keeps its 3.96 s and its 11 fps, which is right.
///
/// `format_name` can be a comma-separated list, so any member counts.
fn pretend_film(format_name: &str) -> bool {
    format_name
        .split(',')
        .any(|n| n == "image2" || n.ends_with("_pipe"))
}

/// A number that may have arrived as a JSON string.
///
/// ffprobe writes `"bit_rate": "2899884"` in some containers and
/// `"bit_rate": 2899884` in others, and a parser that handled only one would
/// lose the bit rate on exactly half of them.
fn text_or_number(v: &serde_json::Value) -> Option<String> {
    v.as_str()
        .map(ToOwned::to_owned)
        .or_else(|| v.as_f64().map(|n| n.to_string()))
}

/// A stream's codec, long name first.
fn codec_name(stream: &serde_json::Value) -> Option<String> {
    stream
        .get("codec_long_name")
        .or_else(|| stream.get("codec_name"))
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty() && *s != "unknown")
        .map(ToOwned::to_owned)
}

/// One of the container's tags, by a name compared without case.
fn tag(format: Option<&serde_json::Value>, wanted: &str) -> Option<String> {
    format
        .and_then(|f| f.get("tags"))
        .and_then(serde_json::Value::as_object)?
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(wanted))
        .and_then(|(_, v)| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// `"24/1"` becomes 24, `"0/0"` becomes nothing.
fn ratio(text: &str) -> Option<f64> {
    let (num, den) = text.split_once('/')?;
    let (num, den): (f64, f64) = (num.trim().parse().ok()?, den.trim().parse().ok()?);
    if den == 0.0 || num <= 0.0 {
        return None;
    }
    Some(num / den)
}

/// A number with at most two decimals and no trailing zeros.
///
/// `48` and not `48.00`, `44.1` and not `44.10`, `1.68` and not `1.6800000001`.
/// Digits only, so it is [`Value::Data`] and needs no language.
fn trimmed(n: f64) -> String {
    let mut s = format!("{n:.2}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    s
}

/// Seconds as a clock: `9:56`, or `1:39:56` when there are hours.
///
/// Digits and colons, which is the same notation in every language this ships
/// in — so no key, the way a filename needs no key.
fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    let (h, m, s) = (whole / 3600, (whole % 3600) / 60, whole % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// An exposure, as a photographer writes it.
///
/// Under a second it is `1/96`, which is what the camera's dial says and what
/// `-n`'s `0.010392` means. A second or more takes a key, because "s" is a
/// unit and where a unit goes is the language's business.
fn shutter(seconds: f64) -> Value {
    if seconds < 1.0 {
        return Value::Data(format!("1/{}", (1.0 / seconds).round() as u64));
    }
    Value::Phrase {
        key: key::VALUE_SECONDS,
        args: vec![("s", trimmed(seconds))],
    }
}

/// `2024:02:25 07:08:57` becomes `2024-02-25 07:08`.
///
/// Exif's own separator is a colon in the date as well, which no other format
/// on this planet uses. The seconds go because nobody has ever wanted them, and
/// the shape matches what `ronoctua_files::when` already puts on screen.
fn taken(raw: &str) -> Option<String> {
    let (date, time) = raw.trim().split_once(' ')?;
    let mut parts = date.split(':');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if y.len() != 4 || y.starts_with('0') {
        // `0000:00:00 00:00:00` is what a camera writes when its clock was
        // never set, and it is worse than no row at all.
        return None;
    }
    let mut clock = time.split(':');
    let (hh, mm) = (clock.next()?, clock.next()?);
    Some(format!("{y}-{m}-{d} {hh}:{mm}"))
}

/// Make and model as one line, without saying the maker twice.
fn camera(make: Option<&str>, model: Option<&str>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) => {
            // "NIKON CORPORATION" / "NIKON D850" is one camera, and printing
            // both gives "NIKON CORPORATION NIKON D850".
            let first = make.split_whitespace().next().unwrap_or(make);
            if model.to_lowercase().starts_with(&first.to_lowercase()) {
                Some(model.to_owned())
            } else {
                Some(format!("{make} {model}"))
            }
        }
        (None, Some(one)) | (Some(one), None) => Some(one.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Fact, Place, Value, camera, clock, from_exif, from_pdfinfo, from_probe, needs, ratio,
        shutter, taken, trimmed,
    };
    use crate::key;
    use crate::tools::Tool;

    fn value(facts: &[Fact], label: &'static str) -> Option<Value> {
        facts
            .iter()
            .find(|f| f.label == label)
            .map(|f| f.value.clone())
    }

    /// Cut from the real `ffprobe` answer for the owner's `.mov`, with the
    /// tag soup removed. It is a TRANSCRIPT and not something we generated:
    /// a fixture written by the same code that reads it proves the code talks
    /// to itself.
    const MOV: &str = r#"{
      "streams": [
        {"index":0,"codec_name":"h264","codec_long_name":"H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10",
         "codec_type":"video","width":853,"height":480,"r_frame_rate":"24/1","avg_frame_rate":"24/1",
         "duration":"596.458333","bit_rate":"2899884","disposition":{"attached_pic":0}},
        {"index":1,"codec_type":"data","r_frame_rate":"0/0","avg_frame_rate":"2400/100",
         "disposition":{"attached_pic":0}},
        {"index":2,"codec_name":"aac","codec_long_name":"AAC (Advanced Audio Coding)",
         "codec_type":"audio","sample_rate":"48000","channels":6,"bit_rate":"437605",
         "disposition":{"attached_pic":0}}
      ],
      "format": {"nb_streams":3,"format_name":"mov,mp4,m4a,3gp,3g2,mj2",
                 "format_long_name":"QuickTime / MOV","duration":"596.461667",
                 "bit_rate":"3342778","tags":{"TITLE":"Big Buck Bunny"}}
    }"#;

    /// The same program's answer for a plain PNG, which is where the trap is.
    const PNG: &str = r#"{
      "streams": [{"codec_name":"png","codec_long_name":"PNG (Portable Network Graphics) image",
                   "codec_type":"video","width":610,"height":590,
                   "r_frame_rate":"25/1","avg_frame_rate":"25/1","disposition":{"attached_pic":0}}],
      "format": {"nb_streams":1,"format_name":"png_pipe","format_long_name":"piped png sequence"}
    }"#;

    #[test]
    fn a_film_gives_up_everything_it_has() {
        let f = from_probe(MOV);
        assert_eq!(
            value(&f, key::FACT_DIMENSIONS),
            Some(Value::Phrase {
                key: key::VALUE_PIXELS,
                args: vec![("width", "853".to_owned()), ("height", "480".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_DURATION),
            Some(Value::Data("9:56".to_owned()))
        );
        assert_eq!(
            value(&f, key::FACT_CONTAINER),
            Some(Value::Data("QuickTime / MOV".to_owned()))
        );
        assert!(
            matches!(value(&f, key::FACT_VIDEO), Some(Value::Data(v)) if v.starts_with("H.264"))
        );
        assert_eq!(
            value(&f, key::FACT_RATE),
            Some(Value::Phrase {
                key: key::VALUE_FPS,
                args: vec![("fps", "24".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_SAMPLING),
            Some(Value::Phrase {
                key: key::VALUE_HERTZ,
                args: vec![("khz", "48".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_CHANNELS),
            Some(Value::Phrase {
                key: key::VALUE_CHANNELS,
                args: vec![("n", "6".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_BITRATE),
            Some(Value::Phrase {
                key: key::VALUE_KBPS,
                args: vec![("kbps", "3343".to_owned())],
            })
        );
        // THE TAG'S NAME IS UPPERCASE HERE and lowercase in mp4.
        assert_eq!(
            value(&f, key::FACT_TITLE),
            Some(Value::Data("Big Buck Bunny".to_owned()))
        );
        // The DATA stream must not become the video codec.
        assert_eq!(f.iter().filter(|x| x.label == key::FACT_VIDEO).count(), 1);
    }

    #[test]
    fn a_still_photograph_does_not_get_an_invented_frame_rate() {
        // ffprobe answers a PNG with `r_frame_rate: 25/1` and a container
        // called "piped png sequence". Both are facts about how ffmpeg opened
        // the file, not about the file, and a panel that showed them would be
        // lying with a straight face.
        let f = from_probe(PNG);
        assert_eq!(
            value(&f, key::FACT_DIMENSIONS),
            Some(Value::Phrase {
                key: key::VALUE_PIXELS,
                args: vec![("width", "610".to_owned()), ("height", "590".to_owned())],
            })
        );
        assert_eq!(value(&f, key::FACT_RATE), None, "25 fps for a still PNG");
        assert_eq!(value(&f, key::FACT_DURATION), None);
        assert_eq!(value(&f, key::FACT_CONTAINER), None);
        assert_eq!(value(&f, key::FACT_VIDEO), None);
        assert_eq!(f.len(), 1, "the dimensions alone: {f:?}");
    }

    /// The same program's answer for a plain JPEG — the trap that the PNG
    /// fixture does NOT catch, because this one has a duration.
    const JPEG: &str = r#"{
      "streams": [{"codec_name":"mjpeg","codec_long_name":"Motion JPEG","codec_type":"video",
                   "width":3072,"height":4080,"r_frame_rate":"25/1","avg_frame_rate":"25/1",
                   "disposition":{"attached_pic":0}}],
      "format": {"nb_streams":1,"format_name":"image2","format_long_name":"image2 sequence",
                 "duration":"0.040000","bit_rate":"4439000"}
    }"#;

    #[test]
    fn a_photograph_is_not_a_one_frame_film() {
        // ffmpeg opens a JPEG through the `image2` demuxer, which invents a
        // duration of 0.04 s, a frame rate of 25 and a bit rate of four and a
        // half GIGAbits a second. All three passed the "has a duration" gate
        // and reached the panel on the owner's own photograph.
        let f = from_probe(JPEG);
        assert_eq!(
            value(&f, key::FACT_DIMENSIONS),
            Some(Value::Phrase {
                key: key::VALUE_PIXELS,
                args: vec![("width", "3072".to_owned()), ("height", "4080".to_owned())],
            })
        );
        assert_eq!(f.len(), 1, "the dimensions alone: {f:?}");
        assert!(super::pretend_film("image2"));
        assert!(super::pretend_film("png_pipe"));
        assert!(super::pretend_film("tiff_pipe"));
        // AN ANIMATED GIF IS NOT ONE: the real demuxer, and its 3.96 s and
        // its 11 fps are facts about the file.
        assert!(!super::pretend_film("gif"));
        assert!(!super::pretend_film("mov,mp4,m4a,3gp,3g2,mj2"));
    }

    #[test]
    fn junk_gives_no_facts_and_does_not_panic() {
        assert!(from_probe("").is_empty());
        assert!(from_probe("i am not json").is_empty());
        assert!(from_probe("{}").is_empty());
        assert!(from_exif("[]", Place::Withheld).is_empty());
        assert!(from_exif("i am not json", Place::Withheld).is_empty());
        assert!(from_pdfinfo("").is_empty());
    }

    /// The real answer for the owner's Pixel 8 Pro JPEG, verbatim.
    const PIXEL: &str = r#"[{
      "SourceFile": "x.jpg",
      "Make": "Google",
      "Model": "Pixel 8 Pro",
      "LensModel": "Pixel 8 Pro back camera 6.9mm f/1.68",
      "ISO": 25,
      "FNumber": 1.68,
      "ExposureTime": 0.010392,
      "FocalLength": 6.9,
      "DateTimeOriginal": "2024:02:25 07:08:57",
      "GPSLatitude": 51.4964416666667,
      "GPSLongitude": -0.264880555555556
    }]"#;

    #[test]
    fn a_photograph_says_what_took_it() {
        let f = from_exif(PIXEL, Place::Withheld);
        assert_eq!(
            value(&f, key::FACT_CAMERA),
            Some(Value::Data("Google Pixel 8 Pro".to_owned()))
        );
        assert_eq!(
            value(&f, key::FACT_TAKEN),
            Some(Value::Data("2024-02-25 07:08".to_owned())),
            "Exif separates the date with colons, and nobody else on the planet does"
        );
        assert_eq!(value(&f, key::FACT_ISO), Some(Value::Data("25".to_owned())));
        assert_eq!(
            value(&f, key::FACT_SHUTTER),
            Some(Value::Data("1/96".to_owned())),
            "0.010392 s is what the camera's dial calls 1/96"
        );
        assert_eq!(
            value(&f, key::FACT_APERTURE),
            Some(Value::Data("f/1.68".to_owned()))
        );
        assert_eq!(
            value(&f, key::FACT_FOCAL),
            Some(Value::Phrase {
                key: key::VALUE_MILLIMETRES,
                args: vec![("mm", "6.9".to_owned())],
            })
        );
    }

    #[test]
    fn the_place_is_withheld_by_default_and_still_announced() {
        // The fact that helps before sending a photograph to a stranger is
        // that it CARRIES a location — not the location.
        let hidden = from_exif(PIXEL, Place::Withheld);
        assert_eq!(
            value(&hidden, key::FACT_PLACE),
            Some(Value::phrase(key::VALUE_PLACE_WITHHELD))
        );
        let joined = format!("{hidden:?}");
        assert!(!joined.contains("51.49"), "a latitude vazou: {joined}");
        assert!(!joined.contains("0.26"), "a longitude vazou: {joined}");

        let shown = from_exif(PIXEL, Place::Shown);
        assert_eq!(
            value(&shown, key::FACT_PLACE),
            Some(Value::Phrase {
                key: key::VALUE_COORDINATES,
                args: vec![
                    ("lat", "51.49644".to_owned()),
                    // THE SIGN IS THE HEMISPHERE, which is exactly what `-n`
                    // buys: west of Greenwich stays negative instead of
                    // arriving as an English "W".
                    ("lon", "-0.26488".to_owned()),
                ],
            })
        );
    }

    #[test]
    fn a_file_with_no_camera_in_it_gets_no_camera_rows() {
        // exiftool answers a PNG with an object holding only `SourceFile`.
        // That is not an error and must not read like one.
        assert!(from_exif(r#"[{"SourceFile":"x.png"}]"#, Place::Withheld).is_empty());
    }

    #[test]
    fn a_camera_is_not_named_twice() {
        assert_eq!(
            camera(Some("Google"), Some("Pixel 8 Pro")).unwrap(),
            "Google Pixel 8 Pro"
        );
        assert_eq!(
            camera(Some("NIKON CORPORATION"), Some("NIKON D850")).unwrap(),
            "NIKON D850"
        );
        assert_eq!(camera(None, Some("X100V")).unwrap(), "X100V");
        assert_eq!(camera(None, None), None);
    }

    #[test]
    fn a_clock_that_was_never_set_is_not_a_date() {
        assert_eq!(taken("0000:00:00 00:00:00"), None);
        assert_eq!(taken(""), None);
        assert_eq!(taken("2024:02:25 07:08:57").unwrap(), "2024-02-25 07:08");
    }

    #[test]
    fn the_page_count_comes_out_of_pdfinfo() {
        let report = "Title:          Nothing\nPages:          12\nEncrypted:      no\n";
        assert_eq!(
            from_pdfinfo(report),
            vec![Fact {
                label: key::FACT_PAGES,
                value: Value::Phrase {
                    key: key::VALUE_PAGES,
                    args: vec![("n", "12".to_owned())],
                },
            }]
        );
        assert!(from_pdfinfo("Title: x\n").is_empty());
    }

    #[test]
    fn the_small_conversions_are_what_a_person_would_write() {
        assert_eq!(clock(596.46), "9:56");
        assert_eq!(clock(3956.0), "1:05:56");
        assert_eq!(clock(3.96), "0:04");
        assert_eq!(trimmed(48.0), "48");
        assert_eq!(trimmed(44.1), "44.1");
        assert_eq!(trimmed(1.68), "1.68");
        assert!((ratio("24/1").unwrap() - 24.0).abs() < 1e-9);
        assert_eq!(ratio("0/0"), None, "a still has no frame rate");
        assert_eq!(ratio("24"), None);
        assert_eq!(shutter(0.010392), Value::Data("1/96".to_owned()));
        // A SECOND OR MORE takes a key: "s" is a unit, and where a unit goes
        // is the language's business.
        assert!(matches!(shutter(2.5), Value::Phrase { key, .. } if key == key::VALUE_SECONDS));
    }

    #[test]
    fn each_kind_of_file_asks_only_the_programs_it_needs() {
        assert_eq!(needs("image/jpeg"), vec![Tool::Ffprobe, Tool::Exiftool]);
        assert_eq!(needs("video/mp4"), vec![Tool::Ffprobe, Tool::Exiftool]);
        // NO EXIFTOOL FOR SOUND: there is no camera in an mp3, and a second
        // spawn per file is a second spawn per file.
        assert_eq!(needs("audio/flac"), vec![Tool::Ffprobe]);
        assert_eq!(needs("application/pdf"), vec![Tool::Pdfinfo]);
        assert!(needs("text/plain").is_empty());
    }
}
