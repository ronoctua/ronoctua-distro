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
    /// What the row is called.
    pub label: Label,
    /// What the row says.
    pub value: Value,
}

/// What a row is CALLED — a phrase of ours, or a name out of the file.
///
/// # Why a label is ever anything but a key
///
/// Nearly every row here is a question this system decided to ask, so its
/// label is a sentence in both languages and a key is the only honest way to
/// carry it. Invariant 8 is about exactly that.
///
/// A file's own tags are the other kind. A container may carry `BPM (beats per
/// minute)`, `MUSICBRAINZ_TRACKID` or anything at all a program chose to write,
/// and the owner asked for them to be shown. There is no key for a name nobody
/// has seen — and inventing `files-fact-other = { $name }` would be worse, not
/// better: the name would then arrive as a Fluent argument, which is precisely
/// how an untranslated key reaches the screen.
///
/// So the name is DATA, and it travels as data. It is the rule the manager
/// already keeps in the other direction — a camera's model is `Pixel 8 Pro` in
/// every language — and the same rule a `Choice`'s label follows.
///
/// The mechanism is narrow on purpose: [`Label::Named`] is built at ONE site,
/// out of a tag name read from a file, and `inspect-labels-are-keys` is what
/// keeps it that way. A literal there would be invariant 8's defect wearing
/// this type as a disguise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Label {
    /// A phrase of ours, by text key.
    Key(&'static str),
    /// A name the FILE chose, shown as it was written.
    Named(String),
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
    // AND NEVER OF A PLAYLIST, whose type is `audio/*` and whose content is
    // the names of other files.
    //
    // Measured against the pinned binary: ffprobe on an `.m3u8` OPENS the
    // first segment it names and reports that file's codec, sample rate and
    // duration — so the panel would say a list of nine tracks is two minutes
    // twelve long and encoded as MP3. (`http` is not on ffmpeg's default
    // protocol whitelist, so the segment it opens is a local one; the wrong
    // answer is the defect here, not a fetch.)
    if matches!(media, "image" | "audio" | "video") && !crate::playlist::is_a_playlist(mime) {
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

/// Everything the tools said about one file, and the document one of them said
/// it in.
///
/// # The probe is HANDED BACK and not thrown away
///
/// Because two questions are asked of the same ffprobe document — what the file
/// IS, which is the facts, and how it PLAYS, which is `shape_of` — and running
/// ffprobe twice for them costs 29 ms per file walked past, measured on a
/// 339-mebibyte film. One document, two readers.
pub struct Told {
    /// The sentences the panel shows.
    pub facts: Vec<Fact>,
    /// ffprobe's own JSON, empty when it was not run or said nothing.
    pub probe: String,
}

/// Everything the tools can say about one file, in order.
///
/// Synchronous, and it spawns at most two children. The caller runs it off the
/// loop that draws — the same long-lived thread the preview uses.
#[must_use]
pub fn facts(path: &Path, mime: &str, place: Place) -> Told {
    let mut out = Vec::new();
    let mut probe = String::new();
    for tool in needs(mime) {
        let args = match tool {
            Tool::Ffprobe => probe_args(path),
            Tool::Exiftool => exif_args(path),
            Tool::Pdfinfo => vec![OsString::from("--"), path.as_os_str().to_owned()],
            // AND THE THREE OLE READERS ARE NOT FACT PROGRAMS. They answer
            // what a document SAYS, which is the preview's question and not
            // this one; `needs` never names them, and this arm is what keeps
            // that true rather than assumed.
            Tool::Ffmpeg | Tool::Pdftoppm | Tool::Xls2csv => {
                continue;
            }
        };
        let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        match tools::run(tool, &borrowed) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                out.extend(match tool {
                    Tool::Ffprobe => {
                        probe = text.clone().into_owned();
                        from_probe(&text)
                    }
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
    Told { facts: out, probe }
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

/// What ties a Live Photo's two files together.
///
/// # It is the identifier or it is nothing
///
/// An iPhone writes `IMG_1234.HEIC` and `IMG_1234.MOV` and stamps the same
/// `ContentIdentifier` into both — into the Apple maker note on the picture's
/// side and the QuickTime keys on the film's. The NAME is not the tie: an
/// unrelated `photo.jpg` and `photo.mov` are perfectly ordinary, and pairing on
/// the name would play a stranger's video under somebody's photograph.
///
/// `None` when there is none, which is the answer for every picture that is not
/// half of a Live Photo — and also when exiftool is not installed, which is the
/// same answer for the caller: do not pair.
#[must_use]
pub fn content_identifier(path: &Path) -> Option<String> {
    let args = [
        OsStr::new("-json"),
        OsStr::new("-n"),
        // BOTH SPELLINGS, because the two halves keep it in different places:
        // the picture in Apple's maker note, the film in the QuickTime keys.
        // exiftool answers with whichever it finds and says nothing about the
        // other.
        OsStr::new("-ContentIdentifier"),
        OsStr::new("-MediaGroupUUID"),
        OsStr::new("--"),
        path.as_os_str(),
    ];
    let out = tools::run(Tool::Exiftool, &args).ok()?;
    let doc: serde_json::Value = serde_json::from_slice(&out).ok()?;
    let first = doc.as_array()?.first()?;
    ["ContentIdentifier", "MediaGroupUUID"]
        .iter()
        .find_map(|k| first.get(*k).and_then(serde_json::Value::as_str))
        .map(ToOwned::to_owned)
        .filter(|id| !id.is_empty())
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
/// What a moving file needs to be PLAYED, out of the probe that was already run.
///
/// # Why there is no width and no height here
///
/// Because the frame's size is not the file's business. The player asks ffmpeg
/// for an exact `W×H` and lets it letterbox whatever shape it has into that —
/// so the size is decided by the panel, and a file that lies about its
/// dimensions cannot make the byte count wrong.
///
/// That is not the obvious design and it is worth saying why it beat the
/// obvious one. Asking ffprobe for `width`/`height` and computing the scale
/// from them looks like "ask instead of guess", and it is wrong: measured, a
/// phone video reports 640×360 CODED with a matrix saying "rotate 90", and
/// ffmpeg renders 256×456. Scaling to the reported shape would squash the
/// picture AND desynchronise the frame length — a new defect introduced by the
/// fix for the old one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// Frames a second, already clamped to something a player can pace by.
    pub fps: f64,
    /// How long it runs; `None` is legal and means no timeline and no seek.
    pub seconds: Option<f64>,
    /// Whether there is anything to HEAR.
    ///
    /// # It is not a nicety, it is what keeps the picture alive
    ///
    /// Measured against the real binary: asking ffmpeg for a pulse output on a
    /// file with no audio stream makes it refuse the WHOLE command — *"output
    /// file does not contain any stream"*, exit 234, zero frames. So somebody
    /// pressing the sound key on a silent film would stop the picture, and the
    /// only thing that says the key is pointless is this.
    ///
    /// The player's respawn-in-silence still catches it if this is ever wrong;
    /// this is what stops it being reached.
    pub sound: bool,
}

/// The fastest a video is followed, in frames a second.
///
/// A panel is some 256 pixels across, and at that size sixty frames a second
/// and thirty are the same picture — while sixty is twice the wake-ups on a
/// laptop. A GIF is not capped: its cadence is what it declares, and a GIF at
/// another rate reads as broken.
pub const FASTEST_VIDEO: f64 = 30.0;

/// The slowest cadence a player will pace by.
///
/// Under this, a container is not describing a film — `1/1000` and `90000/1`
/// are what a stream with no rate reports.
const SLOWEST_FPS: f64 = 1.0;

/// What a moving file needs, read from the SAME probe the facts come from.
///
/// One document, two readers: [`from_probe`] takes the sentences and this
/// takes the numbers. No second spawn.
///
/// `None` means "this does not play": no video stream, or one that is an
/// album's cover, or a document ffprobe could not make sense of.
#[must_use]
pub fn shape_of(text: &str) -> Option<Shape> {
    let doc: serde_json::Value = serde_json::from_str(text).ok()?;
    let streams = doc.get("streams")?.as_array()?;
    let video = streams.iter().find(|s| {
        s.get("codec_type").and_then(serde_json::Value::as_str) == Some("video")
            // AN ALBUM'S COVER IS A VIDEO STREAM, and `from_probe` refuses it
            // for the same reason: playing an mp3's artwork as a film is a
            // player started for a still picture.
            && s.pointer("/disposition/attached_pic")
                .and_then(serde_json::Value::as_i64)
                != Some(1)
    })?;

    let container = doc
        .get("format")
        .and_then(|f| f.get("format_name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    // A PNG IS NOT A ONE-FRAME FILM. `pretend_film` is the same guard the
    // facts use, and it is what stops a photograph being handed to a player.
    if pretend_film(container) {
        return None;
    }

    let seconds = doc
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(text_or_number)
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.05 && *d < 86_400.0);

    // A CONTAINER WITH NO USABLE RATE IS PACED AT THE CEILING rather than
    // refused: `0/0` and `90000/1` are what a stream without one reports, and
    // neither says the file cannot be played.
    let fps = video
        .get("avg_frame_rate")
        .or_else(|| video.get("r_frame_rate"))
        .and_then(serde_json::Value::as_str)
        .and_then(ratio)
        .filter(|f| f.is_finite() && (SLOWEST_FPS..=240.0).contains(f))
        .unwrap_or(FASTEST_VIDEO)
        .min(FASTEST_VIDEO);

    // AND WHETHER THERE IS ANYTHING TO HEAR — asked of the same document, and
    // an ordinary stream and not an attached picture, for the reason above.
    let sound = streams
        .iter()
        .any(|s| s.get("codec_type").and_then(serde_json::Value::as_str) == Some("audio"));

    Some(Shape {
        fps,
        seconds,
        sound,
    })
}

/// How fast a drawn waveform runs, in frames a second.
///
/// It is not a picture being reproduced, it is one being drawn, so there is no
/// "the file's own rate" to honour — only what reads as movement against what
/// it costs. Twelve, measured: ten seconds of waveform render in 22 to 32 ms,
/// and at a panel's size the frames above that are the same picture twice.
///
/// A cadence is not a colour, a font, a size, a spacing or a codepoint, so it
/// is a constant and not a token — the same standing that [`FASTEST_VIDEO`]
/// has, three lines up.
pub const WAVES_FPS: f64 = 12.0;

/// Whether a sound file carries artwork.
///
/// ANY video stream in an audio container is the artwork — the question is not
/// asked of `disposition.attached_pic`, because a demuxer that fails to mark it
/// would silently pick the wrong filter graph, and the wrong graph is the one
/// that hangs. See [`crate::motion::Show`].
#[must_use]
pub fn has_cover(text: &str) -> bool {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    doc.get("streams")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|streams| {
            streams
                .iter()
                .any(|s| s.get("codec_type").and_then(serde_json::Value::as_str) == Some("video"))
        })
}

/// What a SOUND needs to be watched, out of the same probe.
///
/// The twin of [`shape_of`], and separate from it on purpose: `shape_of`'s
/// requirement of a video stream that is not an album's cover is the one thing
/// standing between a player and a still picture, and widening it to let sound
/// through would dissolve it.
///
/// `None` means there is no sound in here — which for an `audio/*` file means
/// the container lied or the demuxer failed, and either way there is nothing
/// to play.
#[must_use]
pub fn sound_shape_of(text: &str) -> Option<Shape> {
    let doc: serde_json::Value = serde_json::from_str(text).ok()?;
    let streams = doc.get("streams")?.as_array()?;
    streams
        .iter()
        .find(|s| s.get("codec_type").and_then(serde_json::Value::as_str) == Some("audio"))?;

    let seconds = doc
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(text_or_number)
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.05 && *d < 86_400.0);

    Some(Shape {
        fps: WAVES_FPS,
        seconds,
        // AND THERE IS ALWAYS SOMETHING TO HEAR, because that is what this
        // file IS. The field exists to stop a pulse output being asked for on
        // a silent film — a request ffmpeg answers by refusing the whole
        // command — and here the audio stream was just found.
        sound: true,
    })
}

/// The sentences the panel shows, read out of ffprobe's JSON.
///
/// # A picture opened as a film
///
/// ffprobe reports `codec_name: png` and a frame rate of `25/1` for a still
/// photograph, invented by the `png_pipe` demuxer; the container comes back as
/// "piped png sequence" for the same reason. Neither is a fact about the file —
/// both are facts about how ffmpeg opened it.
///
/// A `duration` in the FORMAT is what actually separates the two: a PNG, a HEIC
/// and a DNG have none, an animated GIF has 3.96 s. So dimensions are always
/// reported and everything that implies time is reported only when there is
/// time.
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
            label: Label::Key(key::FACT_DIMENSIONS),
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
        label: Label::Key(key::FACT_DURATION),
        value: Value::Data(clock(seconds)),
    });
    if let Some(name) = format
        .and_then(|f| f.get("format_long_name"))
        .and_then(serde_json::Value::as_str)
    {
        out.push(Fact {
            label: Label::Key(key::FACT_CONTAINER),
            value: Value::Data(name.to_owned()),
        });
    }
    if let Some(v) = video {
        if let Some(codec) = codec_name(v) {
            out.push(Fact {
                label: Label::Key(key::FACT_VIDEO),
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
                label: Label::Key(key::FACT_RATE),
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
                label: Label::Key(key::FACT_AUDIO),
                value: Value::Data(codec),
            });
        }
        if let Some(hz) = a
            .get("sample_rate")
            .and_then(text_or_number)
            .and_then(|s| s.parse::<f64>().ok())
        {
            out.push(Fact {
                label: Label::Key(key::FACT_SAMPLING),
                value: Value::Phrase {
                    key: key::VALUE_HERTZ,
                    args: vec![("khz", trimmed(hz / 1000.0))],
                },
            });
        }
        if let Some(n) = a.get("channels").and_then(serde_json::Value::as_u64) {
            out.push(Fact {
                label: Label::Key(key::FACT_CHANNELS),
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
            label: Label::Key(key::FACT_BITRATE),
            value: Value::Phrase {
                key: key::VALUE_KBPS,
                args: vec![("kbps", ((bits / 1000.0).round() as u64).to_string())],
            },
        });
    }
    out.extend(written_inside(format, audio, streams));
    out
}

/// The names a container might use for one thing.
///
/// # Why a table and not a name each
///
/// Because the containers disagree twice over. They disagree about CASE —
/// Matroska and Ogg write `TITLE`, MP4 writes `title` — and about the name
/// itself: the same fact is `album_artist` in one file, `ALBUMARTIST` in
/// another and `TPE2` where a raw ID3 frame survived. A panel that showed one
/// spelling and not the other looks like a bug in the file.
///
/// The order within a row is the order they are TRIED, so the modern spelling
/// wins where a file carries both.
///
/// It is also what the unknown tags are subtracted from: every name here is
/// already on screen under a phrase of ours, and showing it twice — once
/// translated and once raw — is one fact wearing two labels.
const SOUND_TAGS: [(&str, &[&str]); 12] = [
    (key::FACT_TITLE, &["title", "TIT2"]),
    (key::FACT_ARTIST, &["artist", "TPE1"]),
    (key::FACT_ALBUM, &["album", "TALB"]),
    (
        key::FACT_ALBUM_ARTIST,
        &["album_artist", "albumartist", "album artist", "TPE2"],
    ),
    (key::FACT_YEAR, &["date", "year", "TDRC", "TYER"]),
    (key::FACT_GENRE, &["genre", "TCON"]),
    (key::FACT_COMPOSER, &["composer", "TCOM"]),
    (
        key::FACT_PUBLISHER,
        &["publisher", "label", "organization", "TPUB"],
    ),
    (key::FACT_ISRC, &["ISRC", "TSRC"]),
    (
        key::FACT_ENCODER,
        &["encoder", "encoded_by", "encoded by", "TENC", "TSSE"],
    ),
    // THE TWO THAT ARE COUNTED rather than shown verbatim — they are here so
    // their names are subtracted from the unknown tags, and read below.
    (key::FACT_TRACK, &["track", "tracknumber", "TRCK"]),
    (key::FACT_DISC, &["disc", "discnumber", "TPOS"]),
];

/// The names that carry a TOTAL for the two counted tags.
const SOUND_TOTALS: [&str; 4] = ["TRACKTOTAL", "totaltracks", "DISCTOTAL", "totaldiscs"];

/// The codecs that throw nothing away.
///
/// A named set and not a field, because ffprobe has none: there is no
/// "lossless" in the document, and it cannot be inferred from a bit rate. The
/// short `codec_name` and never `codec_long_name`, which is prose and changes
/// between releases.
///
/// Every `pcm_*` is lossless by construction — there are some thirty of them,
/// and a list of thirty names is a list that goes stale — so they are matched
/// by prefix.
const LOSSLESS: [&str; 8] = [
    "flac", "alac", "wavpack", "tta", "truehd", "mlp", "ape", "shorten",
];

/// What the file itself says about the recording.
///
/// The curated rows first, in the order of [`SOUND_TAGS`], and then every
/// other tag the file carries under its own name — which is what the owner
/// asked for, and why [`Label`] exists.
fn written_inside(
    format: Option<&serde_json::Value>,
    audio: Option<&serde_json::Value>,
    streams: &[serde_json::Value],
) -> Vec<Fact> {
    let mut out = Vec::new();
    let tags = tags_of(format, audio);

    for (label, names) in SOUND_TAGS {
        // THE TWO COUNTED ONES ARE READ BELOW, out of the same table so their
        // names are subtracted from the unknown tags exactly once.
        if label == key::FACT_TRACK || label == key::FACT_DISC {
            continue;
        }
        if let Some(v) = tag_named(&tags, names) {
            out.push(Fact {
                label: Label::Key(label),
                value: Value::Data(v),
            });
        }
    }

    // A TRACK IS `6` WITH A TOTAL BESIDE IT, OR `6/9` IN ONE FIELD, and which
    // of the two a file uses is the container's business rather than ours.
    for (label, names, totals) in [
        (
            key::FACT_TRACK,
            &["track", "tracknumber", "TRCK"][..],
            &["TRACKTOTAL", "totaltracks"][..],
        ),
        (
            key::FACT_DISC,
            &["disc", "discnumber", "TPOS"][..],
            &["DISCTOTAL", "totaldiscs"][..],
        ),
    ] {
        let Some(raw) = tag_named(&tags, names) else {
            continue;
        };
        let (at, within) = raw
            .split_once('/')
            .map_or((raw.as_str(), None), |(a, b)| (a, Some(b)));
        let total = within
            .map(str::to_owned)
            .or_else(|| tag_named(&tags, totals));
        let at = at.trim().to_owned();
        out.push(Fact {
            label: Label::Key(label),
            value: match total.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()) {
                Some(total) => Value::Phrase {
                    key: key::VALUE_OF_TOTAL,
                    args: vec![("at", at), ("total", total)],
                },
                None => Value::Data(at),
            },
        });
    }

    if let Some(a) = audio {
        // BITS PER SAMPLE, AND NEVER `sample_fmt`.
        //
        // `sample_fmt` is the DECODER's output format: a 24-bit FLAC says
        // `s32` and every lossy codec says `fltp`, so reporting it would put
        // "32 bits" under an mp3. Measured on the owner's five files.
        //
        // `bits_per_raw_sample` arrives as a STRING in some containers and a
        // number in others — `"24"` in his FLAC — which is what `text_or_number`
        // is for; and `bits_per_sample` is 0 for "not applicable" rather than
        // absent, which is why it is filtered rather than merely parsed.
        if let Some(bits) = a
            .get("bits_per_raw_sample")
            .and_then(text_or_number)
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| {
                a.get("bits_per_sample")
                    .and_then(text_or_number)
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .filter(|b| *b > 0)
        {
            out.push(Fact {
                label: Label::Key(key::FACT_DEPTH),
                value: Value::Phrase {
                    key: key::VALUE_BITS,
                    args: vec![("n", bits.to_string())],
                },
            });
        }
        // HOW THE CHANNELS SIT, and only when it says something the channel
        // COUNT did not: `files-fact-channels` already reads "stereo", and two
        // rows saying one thing is one row too many.
        if let Some(layout) = a
            .get("channel_layout")
            .and_then(serde_json::Value::as_str)
            .filter(|l| !matches!(*l, "mono" | "stereo" | "unknown"))
        {
            out.push(Fact {
                label: Label::Key(key::FACT_LAYOUT),
                value: Value::Data(layout.to_owned()),
            });
        }
        if let Some(short) = a
            .get("codec_name")
            .and_then(serde_json::Value::as_str)
            .filter(|c| !c.is_empty() && *c != "unknown")
        {
            let kept = short.starts_with("pcm_") || LOSSLESS.contains(&short);
            out.push(Fact {
                label: Label::Key(key::FACT_COMPRESSION),
                value: Value::phrase(if kept {
                    key::VALUE_LOSSLESS
                } else {
                    key::VALUE_LOSSY
                }),
            });
        }
    }

    // THE ARTWORK, which is the ONE stream the reader above deliberately skips
    // — the opposite question, asked on purpose. Measured: 600×600 in the
    // owner's FLAC and 1280×720 in his mp3.
    if let Some(art) = streams.iter().find(|s| {
        s.pointer("/disposition/attached_pic")
            .and_then(serde_json::Value::as_i64)
            == Some(1)
    }) && let (Some(w), Some(h)) = (
        art.get("width").and_then(serde_json::Value::as_u64),
        art.get("height").and_then(serde_json::Value::as_u64),
    ) {
        out.push(Fact {
            label: Label::Key(key::FACT_COVER),
            value: Value::Phrase {
                key: key::VALUE_PIXELS,
                args: vec![("width", w.to_string()), ("height", h.to_string())],
            },
        });
    }

    // AND EVERY OTHER TAG THE FILE CARRIES, under the name the file wrote.
    //
    // Sorted by name, and that is not tidiness: ffprobe's object order is a
    // hash order, and rows that move between two readings of the same file
    // read as a bug in the panel.
    let mut named: Vec<&(&str, String)> = tags
        .iter()
        .filter(|(k, _)| {
            !SOUND_TAGS
                .iter()
                .flat_map(|(_, names)| names.iter())
                .chain(SOUND_TOTALS.iter())
                .any(|known| known.eq_ignore_ascii_case(k))
        })
        .collect();
    named.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, value) in named.into_iter().take(TAGS_CEILING) {
        out.push(Fact {
            label: Label::Named((*name).to_owned()),
            value: Value::Data(value.clone()),
        });
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
            label: Label::Key(key::FACT_CAMERA),
            value: Value::Data(camera),
        });
    }
    if let Some(lens) = string("LensModel") {
        out.push(Fact {
            label: Label::Key(key::FACT_LENS),
            value: Value::Data(lens),
        });
    }
    if let Some(when) = string("DateTimeOriginal").as_deref().and_then(taken) {
        out.push(Fact {
            label: Label::Key(key::FACT_TAKEN),
            value: Value::Data(when),
        });
    }
    if let Some(iso) = number("ISO") {
        out.push(Fact {
            label: Label::Key(key::FACT_ISO),
            value: Value::Data(trimmed(iso)),
        });
    }
    if let Some(t) = number("ExposureTime").filter(|t| *t > 0.0) {
        out.push(Fact {
            label: Label::Key(key::FACT_SHUTTER),
            value: shutter(t),
        });
    }
    if let Some(f) = number("FNumber").filter(|f| *f > 0.0) {
        out.push(Fact {
            label: Label::Key(key::FACT_APERTURE),
            value: Value::Data(format!("f/{}", trimmed(f))),
        });
    }
    if let Some(mm) = number("FocalLength").filter(|m| *m > 0.0) {
        out.push(Fact {
            label: Label::Key(key::FACT_FOCAL),
            value: Value::Phrase {
                key: key::VALUE_MILLIMETRES,
                args: vec![("mm", trimmed(mm))],
            },
        });
    }
    if let (Some(lat), Some(lon)) = (number("GPSLatitude"), number("GPSLongitude")) {
        out.push(Fact {
            label: Label::Key(key::FACT_PLACE),
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
                label: Label::Key(key::FACT_PAGES),
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

/// The most of one tag's value that is kept.
///
/// `text::LINE_BYTES`' number and its argument, one register along: a value is
/// cloned into a `Field`'s `explain` once per visible row per FRAME, and the
/// owner's own FLAC carries a 2 545-character review in `comment`. Measured on
/// his files: 548 characters in an mp3's `description`, and the same 548 again
/// in its `synopsis`.
///
/// What it costs is honest and worth writing down: the end of that review is
/// not on the message line either, and nothing on screen says it was cut.
const TAG_BYTES: usize = 512;

/// How many tags of the file's own the panel will show.
///
/// ID3 permits arbitrary `TXXX` frames, so "every tag" has no upper bound in
/// the format. The curated rows are never cut — only the ones nobody named.
const TAGS_CEILING: usize = 64;

/// A tag's value, made fit to be one row of a panel.
///
/// Control characters become spaces and runs of whitespace become one. Not a
/// nicety: `elide_end` counts CLUSTERS, and a newline is one cluster that
/// draws nothing — so a value with three of them looks short on the row and
/// arrives at the one-line message register as three lines.
fn tidy(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len().min(TAG_BYTES));
    let mut space = false;
    for c in raw.chars() {
        if c.is_control() || c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        // ON A CHARACTER BOUNDARY, because a byte ceiling applied to bytes
        // makes invalid UTF-8 out of any tag that is not ASCII.
        if out.len() + c.len_utf8() > TAG_BYTES {
            break;
        }
        out.push(c);
    }
    (!out.is_empty()).then_some(out)
}

/// Every tag the file carries, from BOTH places it may have put them.
///
/// # Ogg writes them on the stream, and everything else on the container
///
/// Measured on the owner's own files: `sample-aud.ogg` has no `tags` object on
/// its format at all, and `streams[0].tags` holds all three of them — Vorbis
/// comments belong to the stream by construction. The same three tags sit on
/// `format.tags` in his `.mp3` and his `.wav`. Reading one place found half of
/// what he has.
///
/// The format comes FIRST where both carry a name: a container's tag is about
/// the file and a stream's is about the stream, and where they disagree it is
/// the file that was asked about. His mp3 says `Lavf61.7.100` on one and
/// `Lavc61.19` on the other.
///
/// `stream` is the ORDINARY audio stream, never the artwork's — whose own tags
/// are `title: "Album cover"`, which would otherwise become the file's title.
fn tags_of<'a>(
    format: Option<&'a serde_json::Value>,
    stream: Option<&'a serde_json::Value>,
) -> Vec<(&'a str, String)> {
    let mut out: Vec<(&str, String)> = Vec::new();
    for place in [format, stream].into_iter().flatten() {
        let Some(object) = place.get("tags").and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (name, value) in object {
            if out.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)) {
                continue;
            }
            if let Some(text) = value.as_str().and_then(tidy) {
                out.push((name.as_str(), text));
            }
        }
    }
    out
}

/// One tag out of that list, by any of the names a container might use for it.
fn tag_named(tags: &[(&str, String)], names: &[&str]) -> Option<String> {
    names.iter().find_map(|wanted| {
        tags.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(wanted))
            .map(|(_, v)| v.clone())
    })
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
pub fn clock(seconds: f64) -> String {
    // TRUNCATED, and not rounded, because the ELAPSED time is truncated and
    // the two are read side by side: a track of 293,8 s rounded to `4:54` is a
    // total the counter beside it never reaches. The owner has both on screen
    // at once — the transport says one and the facts sheet said the other, on
    // the same file.
    //
    // It is also what every other player writes, and what `ffprobe`'s own
    // 293.802154 becomes everywhere else.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = seconds.max(0.0).floor() as u64;
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
        Fact, Label, Place, Value, camera, clock, from_exif, from_pdfinfo, from_probe, needs,
        ratio, shutter, taken, trimmed,
    };
    use crate::key;
    use crate::tools::Tool;

    fn value(facts: &[Fact], label: &'static str) -> Option<Value> {
        facts
            .iter()
            .find(|f| f.label == Label::Key(label))
            .map(|f| f.value.clone())
    }

    /// FAILS IF: half the tags a file carries stop reaching the panel.
    ///
    /// The gap this pins was measured on the owner's own files, not imagined:
    /// `sample-aud.ogg` has NO `tags` object on its format at all, and all
    /// three of its tags sit on `streams[0]` — Vorbis comments belong to the
    /// stream by construction. Reading one place showed half of what he has.
    #[test]
    fn a_sounds_tags_are_read_from_both_places_it_may_have_put_them() {
        // THE OGG SHAPE: nothing on the format, everything on the stream.
        let ogg = r#"{"streams":[{"codec_type":"audio","codec_name":"vorbis",
                                  "sample_rate":"44100","channels":2,
                                  "tags":{"TITLE":"Heaven","ARTIST":"Julia Michaels",
                                          "DATE":"2018","BPM (beats per minute)":"120"}}],
                      "format":{"format_name":"ogg","duration":"75.0"}}"#;
        let f = from_probe(ogg);
        assert_eq!(
            value(&f, key::FACT_TITLE),
            Some(Value::Data("Heaven".to_owned()))
        );
        assert_eq!(
            value(&f, key::FACT_ARTIST),
            Some(Value::Data("Julia Michaels".to_owned()))
        );
        assert_eq!(
            value(&f, key::FACT_YEAR),
            Some(Value::Data("2018".to_owned()))
        );
        // AND A TAG NOBODY NAMED KEEPS THE NAME THE FILE WROTE — which is what
        // `Label::Named` is for, and the whole of the owner's "and the unknown
        // ones too".
        assert_eq!(
            named(&f, "BPM (beats per minute)"),
            Some(Value::Data("120".to_owned()))
        );
        // AND IT IS NOT ALSO SHOWN UNDER A PHRASE OF OURS: a curated name and
        // its aliases are subtracted, so nothing appears twice.
        assert!(named(&f, "TITLE").is_none());
        assert!(named(&f, "DATE").is_none());

        // THE FORMAT WINS WHERE BOTH CARRY A NAME. Measured: his mp3 says
        // `Lavf61.7.100` on the format and `Lavc61.19` on the stream, and it
        // is the file that was asked about.
        let both = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3",
                                   "tags":{"encoder":"Lavc61.19"}}],
                       "format":{"format_name":"mp3","duration":"191.8",
                                 "tags":{"encoder":"Lavf61.7.100"}}}"#;
        assert_eq!(
            value(&from_probe(both), key::FACT_ENCODER),
            Some(Value::Data("Lavf61.7.100".to_owned()))
        );
    }

    /// FAILS IF: a number the file did not state is reported as if it had.
    ///
    /// Every rule here came out of reading the owner's five files, and the
    /// dangerous one is `sample_fmt`: it is the DECODER's output format, so a
    /// 24-bit FLAC says `s32` and every lossy codec says `fltp`. Reporting it
    /// would put "32 bits" under an mp3 — true of the decoder and a lie about
    /// the file.
    #[test]
    fn bit_depth_comes_from_the_file_and_never_from_the_decoder() {
        // A FLAC: the depth is a STRING, on `bits_per_raw_sample`.
        let flac = r#"{"streams":[{"codec_type":"audio","codec_name":"flac",
                                   "sample_fmt":"s32","bits_per_sample":0,
                                   "bits_per_raw_sample":"24","channels":2}],
                       "format":{"format_name":"flac","duration":"293.8"}}"#;
        let f = from_probe(flac);
        assert_eq!(
            value(&f, key::FACT_DEPTH),
            Some(Value::Phrase {
                key: key::VALUE_BITS,
                args: vec![("n", "24".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_COMPRESSION),
            Some(Value::phrase(key::VALUE_LOSSLESS))
        );

        // AN MP3: no depth at all, and nothing invented from `fltp`.
        let mp3 = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3",
                                  "sample_fmt":"fltp","bits_per_sample":0,
                                  "channels":2}],
                      "format":{"format_name":"mp3","duration":"132.9"}}"#;
        let f = from_probe(mp3);
        assert!(
            value(&f, key::FACT_DEPTH).is_none(),
            "a bit depth was invented for a lossy codec"
        );
        assert_eq!(
            value(&f, key::FACT_COMPRESSION),
            Some(Value::phrase(key::VALUE_LOSSY))
        );

        // A WAV: `bits_per_sample` is the one that carries it, and every
        // `pcm_*` is lossless by construction rather than by a list.
        let wav = r#"{"streams":[{"codec_type":"audio","codec_name":"pcm_s16le",
                                  "sample_fmt":"s16","bits_per_sample":16,
                                  "channels":2}],
                      "format":{"format_name":"wav","duration":"29.98"}}"#;
        let f = from_probe(wav);
        assert_eq!(
            value(&f, key::FACT_DEPTH),
            Some(Value::Phrase {
                key: key::VALUE_BITS,
                args: vec![("n", "16".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_COMPRESSION),
            Some(Value::phrase(key::VALUE_LOSSLESS))
        );
    }

    /// FAILS IF: the artwork's own tags become the file's, or a track number
    /// stops reading like one.
    #[test]
    fn a_record_says_which_track_it_is_and_what_its_sleeve_measures() {
        // `6/9` IN ONE FIELD, and a cover whose tags must not leak.
        let one = r#"{"streams":[{"codec_type":"audio","codec_name":"flac",
                                  "tags":{"track":"6/9","TITLE":"Billie Jean"}},
                                 {"codec_type":"video","codec_name":"mjpeg",
                                  "width":600,"height":600,
                                  "disposition":{"attached_pic":1},
                                  "tags":{"title":"Album cover","comment":"Cover (front)"}}],
                      "format":{"format_name":"flac","duration":"293.8"}}"#;
        let f = from_probe(one);
        assert_eq!(
            value(&f, key::FACT_TRACK),
            Some(Value::Phrase {
                key: key::VALUE_OF_TOTAL,
                args: vec![("at", "6".to_owned()), ("total", "9".to_owned())],
            })
        );
        assert_eq!(
            value(&f, key::FACT_COVER),
            Some(Value::Phrase {
                key: key::VALUE_PIXELS,
                args: vec![("width", "600".to_owned()), ("height", "600".to_owned())],
            })
        );
        // THE SLEEVE'S OWN TITLE IS NOT THE RECORD'S.
        assert_eq!(
            value(&f, key::FACT_TITLE),
            Some(Value::Data("Billie Jean".to_owned()))
        );

        // AND THE OTHER SPELLING: a number here and a total beside it.
        let two = r#"{"streams":[{"codec_type":"audio","codec_name":"flac",
                                  "tags":{"track":"06","TRACKTOTAL":"09"}}],
                      "format":{"format_name":"flac","duration":"293.8"}}"#;
        assert_eq!(
            value(&from_probe(two), key::FACT_TRACK),
            Some(Value::Phrase {
                key: key::VALUE_OF_TOTAL,
                args: vec![("at", "06".to_owned()), ("total", "09".to_owned())],
            })
        );
        // AND THE TOTAL IS NOT ALSO SHOWN AS A TAG OF ITS OWN.
        assert!(named(&from_probe(two), "TRACKTOTAL").is_none());
    }

    /// FAILS IF: one tag can fill the panel, or bring newlines into a register
    /// that holds one line.
    ///
    /// Measured on the owner's files: a 2 545-character AllMusic review in a
    /// FLAC's `comment`, with embedded newlines, and a 548-character YouTube
    /// blurb repeated in an mp3's `description` and `synopsis`.
    #[test]
    fn a_tag_is_cut_and_flattened_before_it_reaches_a_row() {
        let long = "a ".repeat(600);
        let probe = format!(
            r#"{{"streams":[{{"codec_type":"audio","codec_name":"flac",
                              "tags":{{"comment":"{long}","note":"one\ntwo\t three"}}}}],
                "format":{{"format_name":"flac","duration":"293.8"}}}}"#
        );
        let f = from_probe(&probe);
        let Some(Value::Data(cut)) = named(&f, "comment") else {
            panic!("the tag did not become a row");
        };
        assert!(
            cut.len() <= super::TAG_BYTES,
            "{} bytes survived",
            cut.len()
        );
        // AND ON A CHARACTER BOUNDARY, which is what the length being valid
        // UTF-8 at all proves.
        assert!(cut.is_char_boundary(cut.len()));

        let Some(Value::Data(flat)) = named(&f, "note") else {
            panic!("the tag did not become a row");
        };
        assert_eq!(
            flat, "one two three",
            "a newline reached a one-line register"
        );
    }

    /// What a row the FILE named says, by that name.
    fn named(facts: &[Fact], name: &str) -> Option<Value> {
        facts
            .iter()
            .find(|f| f.label == Label::Named(name.to_owned()))
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
        assert_eq!(
            f.iter()
                .filter(|x| x.label == Label::Key(key::FACT_VIDEO))
                .count(),
            1
        );
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
        assert!(!joined.contains("51.49"), "the latitude leaked: {joined}");
        assert!(!joined.contains("0.26"), "the longitude leaked: {joined}");

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
                label: Label::Key(key::FACT_PAGES),
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
        // TRUNCATED, and the same rule the transport's counter uses: a total
        // the elapsed time can never reach is a total that is wrong on screen
        // beside it. `3.96` is three seconds and a bit, and the counter beside
        // it will read `0:03` at its last frame.
        assert_eq!(clock(3.96), "0:03");
        assert_eq!(clock(293.802_154), "4:53", "the owner's own FLAC");
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
