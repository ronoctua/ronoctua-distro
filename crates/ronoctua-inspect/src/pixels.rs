//! How a file becomes a picture — the TABLE, and the six ways.
//!
//! # The table is the feature
//!
//! "A preview of every kind of content" is not a list of special cases; it is
//! one table with one rule (first row that matches wins) and one thing to do to
//! extend it, which is to add a row. Everything that is not in the table gets
//! [`Refusal::Nothing`] — and then the TEXT path is asked, because most files
//! that have no picture in them have letters in them, and letters are content
//! too. Only when neither answers does the panel say there is nothing.
//!
//! # Every external path hands back a PNG on stdout
//!
//! ffmpeg is told `-f image2 -c:v png -`; pdftoppm is told `-png` and `-`. What
//! comes back is decoded by the `image` crate like any other file, which means
//! there is exactly ONE place [`Rgba`] is born and therefore exactly one place
//! the pixel ceiling, the never-enlarge rule and the straight-alpha promise
//! live. A tool that produced pixels its own way would be a second place for
//! those three to be wrong, and they would be wrong quietly.
//!
//! # Nothing here starts a thread
//!
//! Every function is synchronous. The shell runs them on the long-lived named
//! thread `ronoctua_shell::apps::painter` already has, and caches the answer
//! under a [`crate::Stamp`] so a file is decoded once and not once a frame.

use std::ffi::OsStr;
use std::path::Path;
use std::time::Instant;

use image::ImageDecoder as _;

use crate::tools::{self, Tool};
use crate::{Made, Refusal, Rgba};

/// How many pixels this crate agrees to decode.
///
/// Twenty megapixels, the same number and the same reason as
/// `ronoctua_clipboard::image::PIXEL_CEILING`: a file's HEADER declares the
/// dimensions, and a malformed — or malicious — one can declare 60000×60000,
/// which is 14 GB of RGBA allocated before any check. Reading the header first
/// is what makes refusing possible without paying for it.
///
/// It is a resource limit and not a visual value, which is why it is a constant
/// here while the preview's SIZE is a parameter (invariant 9: a number of
/// pixels on screen comes from a token).
pub const PIXEL_CEILING: u64 = 20_000_000;

/// How large an SVG may be.
///
/// Eight mebibytes, and a smaller number than the raster ceiling on purpose:
/// the header trick does not apply. `resvg` parses the WHOLE document to know
/// how big it is, so there is no cheap moment at which to refuse — the only
/// guard left is the file's size before it is opened.
pub const VECTOR_CEILING: u64 = 8 * 1024 * 1024;

/// How the bytes become pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// The `image` crate, in this process. Sub-millisecond to a few
    /// milliseconds, no child, no store path.
    Raster,
    /// `resvg`, in this process. A vector is rasterised AT the size, never
    /// rasterised large and shrunk — shrinking blurs thin lines that nothing
    /// can reconstruct.
    Vector,
    /// One frame out of something that moves, by ffmpeg with an input seek.
    /// Measured at 0.17 s on a 355 MB 1080p mp4, because `-ss` before `-i`
    /// seeks rather than decodes its way there.
    Frame,
    /// The picture INSIDE a file that is not one: a camera's embedded preview
    /// in a RAW, the still in a HEIC, the cover art on an album. Also ffmpeg —
    /// 0.21 s on a 17 MB Pixel DNG, 0.09 s on a HEIC.
    ///
    /// This is where `magick` would have gone, and it is not here because it
    /// **dumped core** on that same DNG. Decoding a real RAW is not what a
    /// thumbnail is for anyway: the camera already wrote a full-size JPEG
    /// inside the file, and that is the picture the photographer composed.
    Inside,
    /// The picture the CAMERA already rendered, out of a RAW, by exiftool.
    ///
    /// Not the same thing as [`How::Inside`], and the difference is visible:
    /// ffmpeg decodes a DNG's RAW planes, which are linear and carry no white
    /// balance or tone curve, and the preview comes out nearly black. It was
    /// measured on the owner's own Pixel photographs — 0.2 s for a picture
    /// nobody could recognise. The JPEG the camera wrote INTO the file is the
    /// picture the photographer composed, and a thumbnail wants that one.
    Embedded,
    /// The first page of a document, by pdftoppm.
    Page,
    /// An album's cover, and the sound's own shape when there is none.
    ///
    /// The same ffmpeg call [`How::Inside`] makes, with a step under it: a
    /// great many sound files carry no artwork at all, and the panel used to
    /// answer them with "there is no preview of this" — true, and useless
    /// about a file whose whole content is a waveform we can draw.
    ///
    /// So the fallback draws the WHOLE file as one still waveform, measured at
    /// 261 ms on a five-megabyte mp3 and 305 ms on a hundred-megabyte FLAC.
    ///
    /// It is a variant of its own and not a step bolted onto [`How::Inside`],
    /// because that row also serves HEICs — and a waveform drawn for a
    /// photograph would be a picture of nothing.
    Cover,
    /// Nothing, ever — the panel shows something that is not a picture.
    ///
    /// A playlist is a list of NAMES. There is no image of it, and the two
    /// pictures ffmpeg could be talked into making are both wrong: the cover
    /// of the first track it names, which is a picture of another file, or —
    /// measured, and this is what happens today — the `hls` demuxer OPENING
    /// the first segment and the panel reporting that file's duration and
    /// codec as if they were the playlist's.
    Nothing,
    /// A shape, drawn by [`crate::solid`] in this process.
    ///
    /// It is the only row that produces no picture HERE. A mesh is turned, so
    /// what the panel shows is a frame at whatever angle the person has it at,
    /// and the manager's own thread makes those the way the film's player
    /// makes its own — see [`at`], which answers `Made::unasked` for this.
    Mesh,
}

impl How {
    /// The program this needs, when it needs one.
    #[must_use]
    pub const fn tool(self) -> Option<Tool> {
        match self {
            Self::Raster | Self::Vector | Self::Mesh | Self::Nothing => None,
            Self::Frame | Self::Inside | Self::Cover => Some(Tool::Ffmpeg),
            Self::Embedded => Some(Tool::Exiftool),
            Self::Page => Some(Tool::Pdftoppm),
        }
    }

    /// Whether it costs a child process.
    #[must_use]
    pub const fn spawns(self) -> bool {
        self.tool().is_some()
    }
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    /// The row's name, `kebab-case`. It is what a log line says and what a
    /// test names; it is not shown to anybody.
    pub id: &'static str,
    /// The types this row serves EXACTLY, after aliases are folded.
    pub types: &'static [&'static str],
    /// The types this row serves through the subclass DAG — a row that claims
    /// `image/tiff` here also claims the thirty camera formats the database
    /// says ARE a TIFF underneath, without any of them being written down.
    pub descends: &'static [&'static str],
    /// The top-level media types this row serves when nothing above it
    /// matched — `video`, `audio`, `image`. This is what makes the table
    /// complete instead of long: a format nobody listed still gets a picture.
    pub media: &'static [&'static str],
    /// How it decodes.
    pub how: How,
}

/// Content kind → how it becomes pixels, in order. The FIRST match wins.
///
/// # Why the exact rows come before the catch-alls
///
/// Because a decoder in this process beats a child process by three orders of
/// magnitude, and because the catch-alls are deliberately greedy: `image` at
/// the bottom is what serves HEIC, AVIF, every camera's RAW, ICO, PSD and
/// anything else ffmpeg has a decoder for, without any of them being written
/// down here. What is written down is only what needs something OTHER than the
/// catch-all.
///
/// The one trap worth naming: a Pixel's `.dng` is `image/x-adobe-dng`, which
/// the database says descends from `image/tiff`. It must NOT reach the `tiff`
/// decoder — that would read the RAW planes — so the raster row matches exact
/// types only and never walks the subclass DAG.
pub const TABLE: [Row; 9] = [
    Row {
        // ELEVEN TYPES, AND SIX OF THEM ARE ALREADY IN THE DATABASE: the
        // pinned shared-mime-info has `model/stl` by its `*.stl` glob,
        // `model/3mf` by `*.3mf` plus a subclass edge to `application/zip`,
        // and `model/obj`, `model/gltf+json`, `model/gltf-binary` and
        // `model/iges` by their globs. That zip edge is what makes
        // `crate::settled` keep the name for a 3MF, which sniffs as a zip
        // because it IS one — the same shape the `.dng` note further down
        // describes.
        //
        // The other five are invented in `assets/icons/mime-glyphs.nix`,
        // which is the ONLY place a type is invented and which refuses to
        // invent one the database already has.
        //
        // A binary STL has no magic at all (freedesktop's only rule is the
        // word `solid`, which is the ASCII form), so its type comes from the
        // name alone and the sniff correctly says nothing. The same is true of
        // a `.step`, a `.ply` and a `.dae`.
        //
        // TWO OF THEM HOLD NO TRIANGLE — `model/step` and `model/iges` are
        // boundary representations and are MESHED here — see `mesh::brep`.
        // They are in
        // this row anyway, because what the row decides is "does the manager
        // turn this rather than show a picture of it", and the answer is yes.
        id: "mesh",
        types: &[
            "model/stl",
            "model/3mf",
            "model/obj",
            "model/x-ply",
            "model/x-off",
            "model/gltf+json",
            "model/gltf-binary",
            "model/vnd.collada+xml",
            "model/x-amf",
            "model/step",
            "model/iges",
        ],
        descends: &[],
        media: &[],
        how: How::Mesh,
    },
    Row {
        // The six the `image` crate reads with no C library behind it, so they
        // cost binary and never a store path. See the workspace's `image` line.
        id: "raster",
        types: &[
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/bmp",
            "image/tiff",
        ],
        descends: &[],
        media: &[],
        how: How::Raster,
    },
    Row {
        // `svgz` too: `usvg` un-gzips a document before parsing it, so the
        // compressed type needs no separate handling.
        id: "vector",
        types: &["image/svg+xml", "image/svg+xml-compressed"],
        descends: &[],
        media: &[],
        how: How::Vector,
    },
    Row {
        // EVERY CAMERA RAW THE DATABASE KNOWS, in one row: DNG, CR2, NEF, ARW,
        // PEF, SR2 and the rest are all subclasses of `image/tiff`. It sits
        // BELOW the raster row, which claims a plain `image/tiff` exactly, and
        // ABOVE the catch-all, which would send it to ffmpeg and get a black
        // rectangle.
        id: "camera-raw",
        types: &[],
        descends: &["image/tiff"],
        media: &[],
        how: How::Embedded,
    },
    Row {
        id: "moving",
        types: &[],
        descends: &[],
        media: &["video"],
        how: How::Frame,
    },
    Row {
        // A LIST OF THINGS TO PLAY, and it must come BEFORE the sound row
        // below or `media: audio` would claim it — which is what happens
        // today, and what makes an `.m3u8` report the first segment's codec
        // and duration as its own. See `crate::playlist`.
        id: "playlist",
        types: &crate::playlist::TYPES,
        descends: &[],
        media: &[],
        how: How::Nothing,
    },
    Row {
        // The cover art, which is a video stream inside an audio file — which
        // is why it is `-map 0:v:0` and the same program — and the file's own
        // waveform when it carries none.
        id: "sound",
        types: &[],
        descends: &[],
        media: &["audio"],
        how: How::Cover,
    },
    Row {
        id: "page",
        types: &["application/pdf"],
        descends: &[],
        media: &[],
        how: How::Page,
    },
    Row {
        // HEIC, AVIF, ICO, PSD, and whatever arrives next.
        id: "still",
        types: &[],
        descends: &[],
        media: &["image"],
        how: How::Inside,
    },
];

/// The sound types nothing on this machine can turn into sound.
///
/// # A score is not a recording
///
/// MIDI, SID, PSF and XMF do not contain sound at all — they contain
/// instructions for making it, and playing them needs a synthesiser with a
/// bank of instruments. ffmpeg has no MIDI decoder and this build carries no
/// soundfont; putting one in would be a package on the ISO for a format
/// nobody here has a file of.
///
/// Measured against the pinned binary: `ffmpeg -decoders` lists no audio
/// decoder for any of them, and `libgme` — which IS built in, and does play
/// the chiptunes — does not cover SID.
///
/// They are refused BY NAME rather than left to fail, because a child that
/// exits with nothing on stdout reaches the panel as "there is no preview of
/// this", which is true of every broken file and says nothing about this one.
/// Tracker modules are NOT here: `libopenmpt` plays them, proved against a
/// module built by hand.
const NO_SYNTHESISER: [&str; 6] = [
    "audio/midi",
    "audio/prs.sid",
    "audio/x-psf",
    "audio/x-psflib",
    "audio/mobile-xmf",
    "audio/x-xmf",
];

/// Whether this type is a score no synthesiser here can play.
#[must_use]
pub fn needs_a_synthesiser(mime: &str) -> bool {
    let mime = ronoctua_files::mime::canonical(mime);
    NO_SYNTHESISER
        .iter()
        .any(|kind| mime == *kind || ronoctua_files::mime::is_a(mime, kind))
}

/// The row that serves a type, if any.
///
/// Aliases are folded first, so `image/x-bmp` and `image/bmp` are one thing,
/// and so are `application/x-pdf` and `application/pdf`.
/// `ronoctua_files::mime` owns that table and there is exactly one of it.
#[must_use]
pub fn how(mime: &str) -> Option<&'static Row> {
    let mime = ronoctua_files::mime::canonical(mime);
    let media = mime.split_once('/').map(|(m, _)| m).unwrap_or(mime);
    TABLE.iter().find(|r| {
        r.types.contains(&mime)
            || r.descends
                .iter()
                .any(|p| ronoctua_files::mime::is_a(mime, p))
            || r.media.contains(&media)
    })
}

/// Whether this type is a shape the manager turns rather than a picture.
///
/// The one question the host asks before deciding what to do with the file:
/// a mesh gets no still and no text, and gets a thread that draws it instead.
#[must_use]
pub fn turns(mime: &str) -> bool {
    how(mime).is_some_and(|r| matches!(r.how, How::Mesh))
}

/// Where in a moving picture the frame is taken from, in seconds.
///
/// # Why not zero
///
/// Because frame zero of a film is black. Fading in from black is the
/// convention, and a strip of black rectangles is a preview that says nothing.
///
/// Ten per cent in, capped at ten seconds: the percentage keeps a short clip
/// from seeking past its own middle, and the cap keeps a two-hour film from
/// showing the twelve-minute mark, which is somewhere in the second act.
/// Under a second, or with no duration known, it is zero — there is nowhere
/// else to go, and a seek past the end returns nothing at all.
#[must_use]
pub fn seek_at(duration: Option<f64>) -> f64 {
    match duration {
        Some(d) if d >= 1.0 && d.is_finite() => (d * 0.10).min(10.0),
        _ => 0.0,
    }
}

/// A preview of `path`, which the caller has already typed.
///
/// `mime` comes from `ronoctua_files::mime::examined`, which is the one place
/// a name that LIES is caught; passing the sniffed type is what makes a PNG
/// called `.txt` show its picture. `target` is the longest side in pixels and
/// is quantised by [`crate::step`].
///
/// It never panics and never returns nothing: a [`Made`] always has either
/// pixels or a sentence.
#[must_use]
pub fn preview(path: &Path, mime: &str, target: u32, wave: &str) -> Made {
    at(path, mime, target, None, wave)
}

/// The same, with a duration the caller already knows.
///
/// The info panel probes the file anyway, so a shell that draws both can pass
/// what it learned and save a spawn. `None` means "probe if you need it", and
/// only [`How::Frame`] ever does.
#[must_use]
pub fn at(path: &Path, mime: &str, target: u32, seconds: Option<f64>, wave: &str) -> Made {
    let target = crate::step(target);
    let Some(row) = how(mime) else {
        return Made::refused(Refusal::Nothing);
    };
    // A SHAPE HAS NO STILL, and for the reason a film has none: the manager
    // turns it, so the picture is a frame at an angle this function knows
    // nothing about. Rendering a default view here would be a whole parse and
    // a whole render for an image replaced as soon as the first real frame
    // arrives — the poster problem, again.
    if matches!(row.how, How::Mesh | How::Nothing) {
        return Made::unasked();
    }
    // AND A SCORE SAYS SO, rather than failing into "there is no preview".
    if needs_a_synthesiser(mime) {
        return Made::refused(Refusal::Unsupported(crate::key::PREVIEW_NO_SYNTH));
    }
    let began = Instant::now();
    let outcome = match row.how {
        // THE FALLBACK, and the only one in this module. `image` claims TIFF
        // and reads a fraction of the TIFFs that exist — JPEG-in-TIFF, the
        // less common LZW variants, CMYK, and a RAW that arrived with no
        // extension for `crate::settled` to work with. ffmpeg reads all of
        // them, so a raster that refuses is asked again the slow way rather
        // than becoming a blank panel.
        //
        // It costs one spawn, once, and only when the fast path already
        // failed; the refusal reported is the LAST one, because that is the
        // one that describes what actually happened.
        How::Raster => from_raster_file(path, target).or_else(|why| {
            if !matches!(why, Refusal::Failed) {
                return Err(why);
            }
            from_inside(path, target)
        }),
        How::Vector => from_svg_file(path, target),
        How::Frame => {
            let known = seconds.or_else(|| crate::facts::duration_of(path));
            child_image(Tool::Ffmpeg, &frame_args(path, target, seek_at(known)))
                .and_then(|png| from_raster(&png, target))
        }
        How::Inside => from_inside(path, target),
        // AND ITS OWN FALLBACK, for the reason `How::Embedded` has one: a
        // great many sound files carry no artwork, and the answer "there is no
        // preview of this" is true and useless about a file we can draw.
        How::Cover => from_inside(path, target).or_else(|why| {
            if !matches!(why, Refusal::Failed) {
                return Err(why);
            }
            // AND NO INK MEANS NO WAVEFORM.
            //
            // The caller says what it can draw with, and an empty string says
            // "nothing". The file manager's GRID asks that way: a waveform is
            // a picture of a sound and reads beautifully in a panel, and in a
            // cell six cells wide it is a grey smudge that says less than the
            // glyph for "this is music" — which is what the owner read off the
            // screen. The panel still passes an ink and still gets its waves.
            if wave.is_empty() {
                return Err(why);
            }
            child_image(Tool::Ffmpeg, &wave_args(path, target, wave))
                .and_then(|png| from_raster(&png, target))
        }),
        // AND ITS OWN FALLBACK: not every RAW carries a preview, and a file
        // that only claims to be one carries nothing at all. ffmpeg's dark
        // rectangle is a worse picture than the camera's JPEG and a better
        // one than no picture.
        How::Embedded => from_embedded(path, target).or_else(|why| {
            if !matches!(why, Refusal::Failed) {
                return Err(why);
            }
            from_inside(path, target)
        }),
        How::Page => child_image(Tool::Pdftoppm, &page_args(path, target))
            .and_then(|png| from_raster(&png, target)),
        // Answered above, before any of this was set up.
        How::Mesh | How::Nothing => return Made::unasked(),
    };
    match outcome {
        Ok(rgba) => {
            // THE PROOF LINE. `nix run .#png` cannot show this — it loads a
            // wasip2 component and draws its display list, and the file
            // manager is native shell code with no display list at all. So
            // the decode is proved the way `tests/clipboard-vm.nix` proves
            // its own: a log line with the real dimensions, asserted in a VM.
            tracing::debug!(
                width = rgba.width,
                height = rgba.height,
                ms = began.elapsed().as_secs_f64() * 1000.0,
                kind = row.id,
                mime,
                "file preview ready"
            );
            Made::of(rgba)
        }
        Err(why) => {
            tracing::debug!(
                kind = row.id,
                mime,
                why = ?why,
                ms = began.elapsed().as_secs_f64() * 1000.0,
                "file preview refused"
            );
            Made::refused(why)
        }
    }
}

/// ffmpeg's argument list for one frame out of something that moves.
///
/// `-ss` goes BEFORE `-i`, which is what makes it an input seek — the demuxer
/// jumps and only the frames after it are decoded. After `-i` it would decode
/// its way there, which on a 355 MB file is the difference between 0.17 s and
/// a minute.
///
/// `-nostdin` because ffmpeg reads stdin for its interactive keys, and a child
/// of the footer that swallows keystrokes is a bug with no visible cause.
/// `-an -sn -dn` throw away the streams we did not ask for. The filter's inner
/// quotes are ffmpeg's OWN — the filtergraph parser needs them so the comma
/// inside `min()` is not read as the start of the next filter — and there is
/// no shell here to eat them. `-2` for the height keeps it even and the aspect
/// ratio true; `min(target,iw)` is what stops a small video being enlarged.
#[must_use]
pub fn frame_args(path: &Path, target: u32, seconds: f64) -> Vec<std::ffi::OsString> {
    vec![
        "-v".into(),
        "error".into(),
        "-nostdin".into(),
        "-ss".into(),
        format!("{seconds:.3}").into(),
        "-i".into(),
        path.as_os_str().to_owned(),
        "-frames:v".into(),
        "1".into(),
        "-an".into(),
        "-sn".into(),
        "-dn".into(),
        "-vf".into(),
        format!("scale='min({target},iw)':-2").into(),
        "-f".into(),
        "image2".into(),
        "-c:v".into(),
        "png".into(),
        "-".into(),
    ]
}

/// ffmpeg's argument list for the picture inside a file that is not one.
///
/// No `-ss`: a HEIC, a RAW and an album cover are one frame long, and seeking
/// three seconds into them lands past the end.
#[must_use]
pub fn inside_args(path: &Path, target: u32) -> Vec<std::ffi::OsString> {
    vec![
        "-v".into(),
        "error".into(),
        "-nostdin".into(),
        "-i".into(),
        path.as_os_str().to_owned(),
        "-frames:v".into(),
        "1".into(),
        // A COMPLEX FILTER, AND NOT `-vf`, FOR A PICTURE THAT IS A GRID.
        //
        // A modern HEIC is not one picture in the file: an iPhone photograph
        // is sixty tiles of 512 by 512, each a stream of its own, and ffmpeg
        // puts them back together by building a COMPLEX filtergraph of its
        // own. A `-vf` cannot stand beside one — *"simple and complex
        // filtering cannot be used together for the same stream"* — and the
        // command dies. Written this way there is one graph and it composes.
        //
        // Naming `0:v:0` instead was what this used to do, and it was worse
        // than dying: it SUCCEEDS and hands back one tile. The owner
        // photographed a brown rectangle where a photograph should have been,
        // and nothing anywhere failed.
        //
        // Measured, and it is the whole of the delay he then reported: with
        // the scale here the tiled file takes 245 ms; without it, letting the
        // full 4032-by-3024 image out of the pipe, 2 253 ms — of which the
        // decode is 187 and the rest is PNG compressing twelve megapixels
        // nobody wanted.
        "-filter_complex".into(),
        format!("scale='min({target},iw)':-2").into(),
        "-f".into(),
        "image2".into(),
        "-c:v".into(),
        "png".into(),
        "-".into(),
    ]
}

/// The picture inside a file that is not one, at the size asked for.
fn from_inside(path: &Path, target: u32) -> Result<Rgba, Refusal> {
    child_image(Tool::Ffmpeg, &inside_args(path, target)).and_then(|png| from_raster(&png, target))
}

/// The command line that draws a whole sound as one still waveform.
///
/// The picture a sound file has when it carries no artwork. It is the WHOLE
/// file at once — `showwavespic` reads it end to end — which is what makes it
/// a portrait of the piece rather than a moment of it: the quiet passages and
/// the loud ones are where they are, and the shape is recognisably that track.
///
/// Measured: 261 ms on a five-megabyte mp3, 305 ms on a hundred-megabyte
/// 24-bit FLAC. Paid once per file and kept under a [`crate::Stamp`].
///
/// `ink` arrives as a string because this crate receives no token values.
#[must_use]
pub fn wave_args(path: &Path, target: u32, ink: &str) -> Vec<std::ffi::OsString> {
    use std::ffi::OsString;
    let mut out: Vec<OsString> = Vec::new();
    for a in ["-v", "error", "-nostdin", "-i"] {
        out.push(OsString::from(a));
    }
    out.push(path.as_os_str().to_owned());
    out.push(OsString::from("-filter_complex"));
    // HALF THE WIDTH TALL, which is the shape a waveform is drawn in wherever
    // it is drawn. The panel fits whatever comes back into its own box, so
    // this decides proportion and not size.
    out.push(OsString::from(format!(
        "[0:a]showwavespic=s={target}x{tall}:colors={ink}[drawn]",
        tall = (target / 2).max(1),
    )));
    for a in [
        "-map",
        "[drawn]",
        "-frames:v",
        "1",
        "-f",
        "image2",
        "-c:v",
        "png",
        "-",
    ] {
        out.push(OsString::from(a));
    }
    out
}

/// pdftoppm's argument list for the first page.
///
/// `-f 1 -l 1` is the page range and `-singlefile` is what makes it write one
/// image. `-scale-to` fits the longer side, so the aspect ratio stays true
/// without arithmetic here.
///
/// # The output goes to stdout by SAYING NOTHING, and a `-` is a filename
///
/// This ended in `-`, with a comment calling it "poppler's way of saying
/// stdout". It is not. pdftoppm's last argument is the output ROOT, and given
/// a `-` it writes a file called `-.png` in whatever directory the process
/// happens to be in — putting **nothing at all** on stdout.
///
/// So the PDF preview never worked: `child_image` read an empty pipe, answered
/// `Refusal::Failed`, and the panel said there was no preview of a file it had
/// just rendered. Every attempt left a `-.png` behind, which is how it was
/// found — one turned up in the repository's own root after a test run.
///
/// Omitting the root is what writes to stdout: measured, 36 277 bytes of PNG
/// for the same page that was going to the file.
///
/// The test that was meant to guard this asserted `last() == "-"` — the code as
/// written rather than the thing wanted, which is the shape `AGENTS.md` names.
/// Its replacement runs the real binary and counts what comes back.
#[must_use]
pub fn page_args(path: &Path, target: u32) -> Vec<std::ffi::OsString> {
    vec![
        "-f".into(),
        "1".into(),
        "-l".into(),
        "1".into(),
        "-singlefile".into(),
        "-png".into(),
        "-scale-to".into(),
        target.to_string().into(),
        path.as_os_str().to_owned(),
    ]
}

/// exiftool's argument list for the picture the camera wrote into a RAW.
///
/// `-b` is "give me the binary value and nothing else", so what arrives on
/// stdout is a JPEG and not a report about one.
#[must_use]
pub fn embedded_args(path: &Path) -> Vec<std::ffi::OsString> {
    vec![
        "-b".into(),
        "-PreviewImage".into(),
        "--".into(),
        path.as_os_str().to_owned(),
    ]
}

/// exiftool's argument list for the Exif orientation, as a bare number.
///
/// `-s3` drops the tag's name and `-n` drops the English sentence, so the
/// whole answer is `6\n`.
#[must_use]
pub fn orientation_args(path: &Path) -> Vec<std::ffi::OsString> {
    vec![
        "-n".into(),
        "-s3".into(),
        "-Orientation".into(),
        "--".into(),
        path.as_os_str().to_owned(),
    ]
}

/// The picture inside a RAW, the right way up.
///
/// # Why this costs a SECOND call to the same program
///
/// Because the preview a camera embeds usually carries no Exif of its own —
/// the owner's Pixel writes a 1280×964 JPEG with no orientation tag at all,
/// while the DNG around it says `Orientation: 6`. Decoding the JPEG alone
/// gives a photograph lying on its side, which is the single most obvious way
/// a preview can be wrong.
///
/// The second call is `-s3 -Orientation`, whose entire output is one digit; it
/// costs a process creation and no reading of the file's pixels. It happens
/// for camera RAWs only, and the answer is cached under a [`crate::Stamp`], so
/// it is paid once per file and not once per frame.
///
/// # Errors
///
/// As [`from_raster`], plus [`Refusal::Failed`] when the file has no embedded
/// preview — which is what sends the caller on to ffmpeg.
pub fn from_embedded(path: &Path, target: u32) -> Result<Rgba, Refusal> {
    let jpeg = child_image(Tool::Exiftool, &embedded_args(path))?;
    let mut picture = decoded(&jpeg)?;
    if let Some(turn) = turn_of(path) {
        picture.apply_orientation(turn);
    }
    Ok(fit(&picture, target))
}

/// The Exif orientation of a file, as the camera recorded it.
///
/// `None` when the tool is absent, the tag is absent or the value is not one
/// of the eight Exif defines — all three mean "leave it as it is", which is
/// the right answer and not an error.
fn turn_of(path: &Path) -> Option<image::metadata::Orientation> {
    let args: Vec<std::ffi::OsString> = orientation_args(path);
    let borrowed: Vec<&OsStr> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    let out = tools::run(Tool::Exiftool, &borrowed).ok()?;
    let n: u8 = String::from_utf8_lossy(&out).trim().parse().ok()?;
    image::metadata::Orientation::from_exif(n)
}

/// Runs a tool that writes a picture to stdout, and hands the bytes back.
///
/// PNG from ffmpeg and pdftoppm, JPEG from exiftool — which one it is does not
/// matter here, because the bytes go through the same decoder afterwards.
fn child_image(tool: Tool, args: &[std::ffi::OsString]) -> Result<Vec<u8>, Refusal> {
    let borrowed: Vec<&OsStr> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    let out = tools::run(tool, &borrowed)?;
    if out.is_empty() {
        // IT RAN AND SAID NOTHING. A seek past the end does this, and so does
        // a file with no video stream at all; both are "not a picture".
        return Err(Refusal::Failed);
    }
    Ok(out)
}

/// The decoder's own ceiling, derived from ours.
///
/// The output image is four bytes a pixel and the decoder is allowed the same
/// again for its working buffers. It catches what the header check cannot: a
/// format whose declared size is honest but whose content unpacks to something
/// else.
fn limits() -> image::Limits {
    // FROM NONE, and then the one we want — rather than from `default()`,
    // whose value the `image` crate is free to change under us. The output
    // image is four bytes a pixel and the decoder gets the same again for its
    // working buffers.
    let mut l = image::Limits::no_limits();
    l.max_alloc = Some(PIXEL_CEILING * 4 * 2);
    l
}

/// Refuses a picture that is larger than [`PIXEL_CEILING`].
fn within(width: u32, height: u32) -> Result<(), Refusal> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels > PIXEL_CEILING {
        return Err(Refusal::TooBig(pixels));
    }
    Ok(())
}

/// Fits a decoded image inside a box of `target` pixels.
///
/// It NEVER enlarges. A 24 px icon blown up to fill a 512 px box is the same
/// picture as a 512 px one, and that was a real defect in the icon chain
/// (`ronoctua_notify::icon::Fit`). `Lanczos3` and not `Nearest` for the other
/// half of the same lesson: the reduction is large, and a nearest-neighbour
/// resampler turns thin lines into a blur of blocks.
fn fit(image: &image::DynamicImage, target: u32) -> Rgba {
    let longest = image.width().max(image.height());
    let rgba = if target > 0 && longest > target {
        image
            .resize(target, target, image::imageops::FilterType::Lanczos3)
            .to_rgba8()
    } else {
        image.to_rgba8()
    };
    Rgba {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    }
}

/// Decodes raster bytes, header first.
///
/// # Errors
///
/// [`Refusal::TooBig`] past [`PIXEL_CEILING`], [`Refusal::Failed`] when the
/// bytes are not a picture we know how to read.
pub fn from_raster(bytes: &[u8], target: u32) -> Result<Rgba, Refusal> {
    Ok(fit(&decoded(bytes)?, target))
}

/// The picture in these bytes, at full size and the right way up.
fn decoded(bytes: &[u8]) -> Result<image::DynamicImage, Refusal> {
    let read = || {
        image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| Refusal::Failed)
    };
    // THE DIMENSIONS BEFORE THE PIXELS: `into_dimensions` reads only the
    // header, and that is what makes it possible to refuse a giant file
    // without allocating for it.
    let (width, height) = read()?.into_dimensions().map_err(|_| Refusal::Failed)?;
    within(width, height)?;
    let mut reader = read()?;
    reader.limits(limits());
    upright(reader)
}

/// The same, straight off the disk.
///
/// Opening rather than reading is the whole difference: a 400 MB TIFF is
/// refused after its header, not after 400 MB of it is in memory.
///
/// # Errors
///
/// As [`from_raster`], plus [`Refusal::Failed`] when the file cannot be opened.
pub fn from_raster_file(path: &Path, target: u32) -> Result<Rgba, Refusal> {
    let open = || {
        image::ImageReader::open(path)
            .map_err(|_| Refusal::Failed)?
            .with_guessed_format()
            .map_err(|_| Refusal::Failed)
    };
    let (width, height) = open()?.into_dimensions().map_err(|_| Refusal::Failed)?;
    within(width, height)?;
    let mut reader = open()?;
    reader.limits(limits());
    Ok(fit(&upright(reader)?, target))
}

/// Decodes, and puts the picture the way the camera was held.
///
/// # Why the tag is read BEFORE the pixels
///
/// Because `orientation()` reads the Exif block and `from_decoder` consumes
/// the decoder; asking afterwards is not possible. A phone in portrait writes
/// a landscape JPEG plus `Orientation: 6`, and a decoder that ignores the tag
/// shows every portrait photograph lying on its side — which looks like a bug
/// in the photograph rather than in the reader.
///
/// A missing or unreadable tag is [`image::metadata::Orientation::NoTransforms`]
/// and never an error: most pictures have no tag, and refusing them would be
/// refusing almost everything.
fn upright<R: std::io::BufRead + std::io::Seek>(
    reader: image::ImageReader<R>,
) -> Result<image::DynamicImage, Refusal> {
    let mut decoder = reader.into_decoder().map_err(|_| Refusal::Failed)?;
    let turn = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut picture = image::DynamicImage::from_decoder(decoder).map_err(|_| Refusal::Failed)?;
    picture.apply_orientation(turn);
    Ok(picture)
}

/// Rasterises an SVG so its longest side is `target`.
///
/// # Why it enlarges when a raster does not
///
/// Because it is a VECTOR. A 24 px document and a 512 px one describe the same
/// drawing, and asking for 512 px of it is exactly what one wants. What must
/// not happen is rasterising large and shrinking afterwards, which blurs the
/// thin lines the shrinking cannot reconstruct.
///
/// # Errors
///
/// [`Refusal::TooBig`] past [`VECTOR_CEILING`] or [`PIXEL_CEILING`],
/// [`Refusal::Failed`] when the document does not parse or has no size.
pub fn from_svg(bytes: &[u8], target: u32) -> Result<Rgba, Refusal> {
    if bytes.len() as u64 > VECTOR_CEILING {
        return Err(Refusal::TooBig(bytes.len() as u64));
    }
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options).map_err(|_| Refusal::Failed)?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 || target == 0 {
        return Err(Refusal::Failed);
    }
    let scale = target as f32 / size.width().max(size.height());
    let round = |v: f32| (v * scale).round().max(1.0) as u32;
    let (width, height) = (round(size.width()), round(size.height()));
    within(width, height)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or(Refusal::Failed)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(Rgba {
        width,
        height,
        // `tiny-skia` delivers PREMULTIPLIED RGBA and the pipeline expects
        // straight. Without undoing it every semitransparent pixel comes out
        // too dark, and the symptom is "the pictures look dirty" — which
        // nobody connects to alpha multiplication. It cost the icon chain a
        // debugging session once (`ronoctua_notify::icon::from_svg`), and the
        // test below plants the bug on purpose so it can only cost it once.
        pixels: pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let d = p.demultiply();
                [d.red(), d.green(), d.blue(), d.alpha()]
            })
            .collect(),
    })
}

/// The same, straight off the disk.
///
/// # Errors
///
/// As [`from_svg`], plus [`Refusal::Failed`] when the file cannot be read and
/// [`Refusal::TooBig`] when it is larger than [`VECTOR_CEILING`] — which is
/// checked from the metadata, before a byte is read.
pub fn from_svg_file(path: &Path, target: u32) -> Result<Rgba, Refusal> {
    let size = std::fs::metadata(path).map_err(|_| Refusal::Failed)?.len();
    if size > VECTOR_CEILING {
        return Err(Refusal::TooBig(size));
    }
    let bytes = std::fs::read(path).map_err(|_| Refusal::Failed)?;
    from_svg(&bytes, target)
}

#[cfg(test)]
mod tests {
    use super::{
        PIXEL_CEILING, TABLE, VECTOR_CEILING, frame_args, from_raster, from_svg, how, inside_args,
        page_args, seek_at,
    };
    use crate::Refusal;

    /// A real 2×1 PNG: one opaque red pixel and one opaque blue one.
    ///
    /// Encoded here rather than decoded from a fixture on disk for the reason
    /// `ronoctua_clipboard::image`'s twin gives: what we want to prove is that
    /// we read a FILE, and a fixture the repository does not carry is a
    /// fixture that cannot go missing. Nothing from the owner's own pictures
    /// is ever copied in here.
    fn png_2x1() -> Vec<u8> {
        use image::ImageEncoder as _;
        let mut out = Vec::new();
        let pixels: [u8; 8] = [255, 0, 0, 255, 0, 0, 255, 255];
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(&pixels, 2, 1, image::ExtendedColorType::Rgba8)
            .expect("encode the fixture");
        out
    }

    #[test]
    fn a_real_png_decodes() {
        let r = from_raster(&png_2x1(), 512).expect("a valid PNG has to decode");
        assert_eq!((r.width, r.height), (2, 1));
        assert_eq!(r.pixels.len(), 8, "RGBA is four bytes per pixel");
        assert_eq!(&r.pixels[..4], &[255, 0, 0, 255], "the first pixel is red");
    }

    #[test]
    fn junk_does_not_become_a_picture() {
        assert_eq!(from_raster(b"I am not a png", 512), Err(Refusal::Failed));
        assert_eq!(from_raster(&[], 512), Err(Refusal::Failed));
        // A PNG WITH ITS TAIL CUT OFF: the header is honest, the pixels are
        // not there. It must refuse and not draw noise.
        let mut half = png_2x1();
        half.truncate(half.len() / 2);
        assert_eq!(from_raster(&half, 512), Err(Refusal::Failed));
    }

    #[test]
    fn the_pixel_ceiling_refuses_from_the_header_alone() {
        // A PNG HEADER THAT DECLARES 60000×60000 and carries no pixels at all.
        // If the ceiling were checked after decoding, this test would try to
        // allocate 14 GB; that it returns in microseconds IS the assertion.
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let chunk = |kind: &[u8; 4], body: &[u8], into: &mut Vec<u8>| {
            let mut whole = kind.to_vec();
            whole.extend_from_slice(body);
            into.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
            into.extend_from_slice(&whole);
            into.extend_from_slice(&crc32(&whole).to_be_bytes());
        };
        let mut ihdr: Vec<u8> = Vec::new();
        ihdr.extend_from_slice(&60_000_u32.to_be_bytes());
        ihdr.extend_from_slice(&60_000_u32.to_be_bytes());
        // Eight bits a channel, colour type 6 — RGBA, four bytes a pixel.
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(b"IHDR", &ihdr, &mut png);
        // AN `IDAT` THAT IS ONE BYTE LONG. The decoder reads chunks until it
        // reaches the pixel data before it will say how big the image is, so
        // a header on its own comes back as "unexpected end of file" — which
        // is the wrong refusal and would have made this test pass for the
        // wrong reason.
        chunk(b"IDAT", &[0x78], &mut png);
        chunk(b"IEND", &[], &mut png);

        let began = std::time::Instant::now();
        let refused = from_raster(&png, 512);
        assert_eq!(refused, Err(Refusal::TooBig(3_600_000_000)));
        assert!(
            began.elapsed() < std::time::Duration::from_millis(200),
            "the refusal has to come from the header, without allocating the pixels"
        );
        const { assert!(3_600_000_000 > PIXEL_CEILING) };
    }

    /// The PNG chunk CRC, so the header above is one a decoder accepts.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for b in bytes {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn an_svg_comes_back_with_straight_alpha() {
        // THE PLANTED BUG. `tiny-skia` stores premultiplied: a 50 % red is
        // (128, 0, 0, 128) in its buffer. Dropping the `.demultiply()` in
        // `from_svg` makes this assertion read `red=128`, which is the exact
        // symptom the icon chain paid for once — "the pictures look dirty",
        // and nobody connects that to alpha multiplication.
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
              <rect width="8" height="8" fill="#ff0000" fill-opacity="0.5"/>
            </svg>"##;
        let r = from_svg(svg, 8).expect("a four-line SVG has to rasterise");
        assert_eq!((r.width, r.height), (8, 8));
        let (red, green, blue, alpha) = (r.pixels[0], r.pixels[1], r.pixels[2], r.pixels[3]);
        assert_eq!((green, blue), (0, 0), "pure red has neither green nor blue");
        assert!(
            (127..=128).contains(&alpha),
            "fill-opacity 0.5 has to come out near 128, it came out {alpha}"
        );
        assert_eq!(
            red, 255,
            "the RED CHANNEL came out {red}: that is PREMULTIPLIED alpha reaching \
             the pipeline, which expects direct alpha"
        );
    }

    #[test]
    fn a_vector_is_rasterised_at_the_size_it_was_asked_for() {
        // A vector has no native resolution worth respecting: asking for 256
        // px of an 8 px document is the whole point of it being a vector.
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4">
              <rect width="8" height="4" fill="#0000ff"/>
            </svg>"##;
        let r = from_svg(svg, 256).expect("rasterise");
        assert_eq!(
            (r.width, r.height),
            (256, 128),
            "the aspect ratio has to survive"
        );
    }

    #[test]
    fn an_enormous_svg_is_refused_before_it_is_parsed() {
        let big = vec![b' '; (VECTOR_CEILING + 1) as usize];
        assert_eq!(
            from_svg(&big, 256),
            Err(Refusal::TooBig(VECTOR_CEILING + 1)),
            "resvg reads the whole document, so the ceiling is the file's size"
        );
    }

    #[test]
    fn no_two_rows_claim_the_same_type() {
        for (i, a) in TABLE.iter().enumerate() {
            for b in TABLE.iter().skip(i + 1) {
                for t in a.types {
                    assert!(!b.types.contains(t), "{t} is in both {} and {}", a.id, b.id);
                }
                for m in a.media {
                    assert!(
                        !b.media.contains(m),
                        "{m}/* is in both {} and {}",
                        a.id,
                        b.id
                    );
                }
            }
        }
    }

    #[test]
    fn a_catch_all_never_shadows_an_exact_row_below_it() {
        // The order is the rule, so a row moved up could silently swallow one
        // written below it — and the symptom would be a HEIC going to the
        // `tiff` decoder, which produces something, just not the picture.
        for (i, greedy) in TABLE.iter().enumerate() {
            for later in TABLE.iter().skip(i + 1) {
                for t in later.types {
                    let media = t.split_once('/').map(|(m, _)| m).unwrap_or(t);
                    assert!(
                        !greedy.media.contains(&media),
                        "{} swallows {t}, which {} claims afterwards",
                        greedy.id,
                        later.id
                    );
                    assert!(
                        !greedy
                            .descends
                            .iter()
                            .any(|p| ronoctua_files::mime::is_a(t, p)),
                        "{} swallows {t} through the DAG, and {} claims it afterwards",
                        greedy.id,
                        later.id
                    );
                }
            }
        }
    }

    #[test]
    fn the_table_answers_for_every_kind_the_owner_has() {
        // The formats in the owner's own test folder, by NAME only — nothing
        // from it is ever copied into this repository.
        let expected = [
            ("image/png", "raster"),
            ("image/jpeg", "raster"),
            ("image/webp", "raster"),
            ("image/gif", "raster"),
            ("image/tiff", "raster"),
            ("image/svg+xml", "vector"),
            ("image/heif", "still"),
            ("image/avif", "still"),
            ("image/x-adobe-dng", "camera-raw"),
            ("image/x-canon-cr2", "camera-raw"),
            ("image/x-nikon-nef", "camera-raw"),
            ("image/x-sony-arw", "camera-raw"),
            ("video/mp4", "moving"),
            ("video/quicktime", "moving"),
            ("video/x-msvideo", "moving"),
            ("video/ogg", "moving"),
            ("audio/flac", "sound"),
            ("application/pdf", "page"),
        ];
        for (mime, id) in expected {
            let row = how(mime).unwrap_or_else(|| panic!("{mime} has no row"));
            assert_eq!(row.id, id, "{mime}");
        }
        // A DNG IS A TIFF UNDERNEATH — which is what the `camera-raw` row
        // matches on, and what must NOT send it to the `image` crate's tiff
        // decoder. A plain TIFF is still decoded in process.
        assert!(ronoctua_files::mime::is_a(
            "image/x-adobe-dng",
            "image/tiff"
        ));
        assert_eq!(how("image/tiff").expect("tiff").id, "raster");
        // AND AN ALIAS IS THE SAME THING: the mime table folds it, we do not.
        // `image/x-bmp` and `application/x-pdf` are real rows of that table,
        // and neither is written down in ours.
        assert_eq!(how("image/x-bmp").expect("an alias of bmp").id, "raster");
        assert_eq!(
            how("application/x-pdf").expect("an alias of pdf").id,
            "page"
        );
    }

    #[test]
    fn a_file_with_no_picture_in_it_says_so() {
        assert!(how("text/plain").is_none());
        assert!(how("application/x-sharedlib").is_none());
        assert!(how("inode/directory").is_none());
    }

    #[test]
    fn the_seek_avoids_the_black_frame_and_the_end() {
        // Ten per cent in, so a short clip is not seeked past its middle.
        assert!((seek_at(Some(100.0)) - 10.0).abs() < 1e-9);
        // CAPPED, so a two-hour film shows something near the beginning and
        // not the middle of the second act.
        assert!((seek_at(Some(7200.0)) - 10.0).abs() < 1e-9);
        // NOWHERE TO GO: under a second, unknown, or nonsense.
        assert!((seek_at(Some(0.4)) - 0.0).abs() < 1e-9);
        assert!((seek_at(None) - 0.0).abs() < 1e-9);
        assert!((seek_at(Some(f64::NAN)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn the_command_lines_carry_the_flags_that_matter() {
        let path = std::path::Path::new("/tmp/x.mp4");
        let args = frame_args(path, 512, 3.0);
        let words: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        // `-ss` BEFORE `-i`, which is the difference between a seek and a
        // decode from the beginning.
        let ss = words.iter().position(|w| w == "-ss").expect("-ss");
        let i = words.iter().position(|w| w == "-i").expect("-i");
        assert!(ss < i, "-ss after -i decodes the whole file");
        assert!(
            words.contains(&"-nostdin".to_owned()),
            "ffmpeg come o stdin"
        );
        // THE FILTER'S OWN QUOTES: there is no shell here, and the
        // filtergraph parser needs them or the comma inside min() starts the
        // next filter.
        assert!(
            words.iter().any(|w| w == "scale='min(512,iw)':-2"),
            "{words:?}"
        );
        assert_eq!(words.last().map(String::as_str), Some("-"), "stdout");

        // The still shape has no seek at all: a HEIC is one frame long.
        let still: Vec<String> = inside_args(std::path::Path::new("/tmp/x.heic"), 256)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(!still.contains(&"-ss".to_owned()));
        // AND NOT `0:v:0`, WHICH USED TO BE ASSERTED HERE.
        //
        // A modern HEIC is a grid of tiles, each its own stream, and asking for
        // the first one hands back a tile: the owner photographed a brown
        // rectangle where a photograph should have been. ffmpeg's own stream
        // selection reassembles the grid — see `whole_args`.
        assert!(
            !still.contains(&"0:v:0".to_owned()),
            "asking for one stream of a HEIC asks for one TILE of it"
        );
        assert!(!still.contains(&"-map".to_owned()), "{still:?}");

        // AND THE FILTER IS THE COMPLEX KIND, because ffmpeg is already using
        // one to put a HEIC's tiles back together and a simple one cannot
        // stand beside it.
        assert!(
            still.contains(&"-filter_complex".to_owned()),
            "a simple filter cannot meet the one that rebuilds a grid: {still:?}"
        );
        assert!(!still.contains(&"-vf".to_owned()), "{still:?}");

        let page: Vec<String> = page_args(std::path::Path::new("/tmp/x.pdf"), 256)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(page.contains(&"-singlefile".to_owned()));
        // AND NOTHING AFTER THE FILE. pdftoppm's last argument is the output
        // ROOT, so anything there is a NAME it writes to and stdout stays
        // empty — see `page_args`. This used to assert the opposite.
        assert_eq!(page.last().map(String::as_str), Some("/tmp/x.pdf"));
    }

    /// THE PROOF, AGAINST REAL POPPLER, THAT A PAGE COMES BACK AS PIXELS.
    ///
    /// FAILS IF: the arguments go back to naming a file instead of writing to
    /// the pipe. Nothing in the argument list says which of the two it is —
    /// that is a fact about pdftoppm — so the only honest test runs it.
    ///
    /// It is also the test that was missing: the one there asserted the shape
    /// of the arguments, and the arguments were wrong.
    #[test]
    fn a_page_arrives_as_pixels_rather_than_as_a_file() {
        let Ok(program) = crate::tools::Tool::Pdftoppm.path() else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("ronoctua-page-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        // A DOCUMENT OF OUR OWN, and the smallest one that renders: a page,
        // no fonts, no images.
        let pdf = dir.join("one.pdf");
        std::fs::write(&pdf, MINIMAL_PDF).expect("write");

        let args = page_args(&pdf, 128);
        let borrowed: Vec<&std::ffi::OsStr> = args.iter().map(AsRef::as_ref).collect();
        let out = std::process::Command::new(&program)
            .args(&borrowed)
            .current_dir(&dir)
            .output()
            .expect("pdftoppm ran");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stdout.starts_with(b"\x89PNG\r\n\x1a\n"),
            "{} bytes came back and they are not a PNG",
            out.stdout.len()
        );
        // AND NOTHING WAS WRITTEN BESIDE THE DOCUMENT. A stray `-.png` in the
        // working directory is how this was found in the first place.
        let left: Vec<String> = std::fs::read_dir(&dir)
            .expect("the scratch directory")
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .filter(|n| n != "one.pdf")
            .collect();
        assert!(left.is_empty(), "it wrote {left:?} instead of using stdout");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One page, no fonts, no images — enough for poppler to render.
    const MINIMAL_PDF: &[u8] = b"%PDF-1.4\n\
        1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
        2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
        3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj\n\
        trailer<</Root 1 0 R>>\n";
}
