//! Whether a file MOVES, and what it takes to watch it move.
//!
//! # This crate answers the question and does not own the answer
//!
//! Deciding that a file animates is the same kind of question as "what is
//! inside it", which is what this crate is for. RUNNING the animation is not:
//! it needs a thread, and a live child process, and something to stop them
//! when the cursor moves on — and this crate's own promise is that it spawns
//! no threads, because the shell owns concurrency.
//!
//! So what is here is the probe. The player is `ronoctua_shell::motion`.
//!
//! # Two sources, one shape
//!
//! A GIF is decoded in process by the `image` crate; a video is decoded by
//! ffmpeg, which is installed anyway and reads a variety no in-process decoder
//! would. Both end as RGBA frames of a known size at a known cadence, which is
//! all the player needs to know — so the player has ONE loop and not two.

use std::path::Path;

use crate::{Refusal, tools};

/// The most a frame may measure on either side.
///
/// A ceiling on OUR OWN arithmetic, not on the file's: the panel decides the
/// size, so this only ever bites if a door reports an absurd box. A thousand
/// and twenty-four square is four mebibytes of RGBA, which is four times the
/// largest still this system decodes.
pub const WIDEST: u32 = 1024;

/// The smallest frame worth producing.
///
/// Two, because `pad` wants even numbers and one pixel is not a picture.
pub const NARROWEST: u32 = 2;

/// What a pipe holds before the process writing to it has to wait, in bytes.
///
/// # The pause depends on this number, and nothing else says so
///
/// Pausing is the loop not reading: the child fills the pipe, blocks on the
/// write, and the sound stops with it because it comes out of the same
/// process. That only works while a FRAME IS BIGGER THAN THE PIPE — otherwise
/// the child writes frame after frame into the slack and goes on making sound
/// with nobody reading.
///
/// Measured, and the arithmetic is not close: Linux gives a pipe 65 536 bytes,
/// so a two-by-two frame — sixteen bytes — leaves room for four thousand of
/// them, which is **five and a half minutes** of sound at twelve frames a
/// second. The owner found it exactly that way: *"após o clique no pause, o
/// ícone volta para o ícone de play e a contagem de tempo para, como esperado,
/// porém o áudio continua tocando no background"*.
///
/// It was my own doing — a frame nobody draws, made as small as possible to
/// save the transfer, which quietly bought the saving out of the pause.
pub const PIPE_BYTES: usize = 64 * 1024;

/// How long the player waits for a frame before giving up on the child.
///
/// Five seconds, the same patience `tools::run` spends, against a measured
/// eighty-four to two hundred and forty-two milliseconds to the first frame of
/// a three-hundred-megabyte film. It is a guard against a source that has
/// stopped answering — a device that went away, a mount that hung — and not
/// against a slow file.
pub const STARVED: std::time::Duration = std::time::Duration::from_secs(5);

/// How long a paused film keeps its child alive.
///
/// Somebody pauses and walks away. Without this, an ffmpeg holds a file handle
/// and a decoder for as long as the session lasts. Past it the child is killed
/// and the POSITION is kept, so pressing play seeks back to where they were.
pub const PAUSE_CEILING: std::time::Duration = std::time::Duration::from_secs(300);

/// How quickly a child that cannot open the sound device gives itself away.
///
/// It exits almost at once, so an exit this soon after a spawn that asked for
/// audio is the machine saying it has none — and the player answers by
/// respawning silently and saying so, rather than by leaving the picture dead.
pub const AUDIO_GRACE: std::time::Duration = std::time::Duration::from_millis(400);

/// The slowest a frame is held, in milliseconds.
///
/// A GIF may declare zero, or one centisecond, and browsers have clamped that
/// for twenty years for the same reason this does: a hundred frames a second
/// in a thumbnail is a wake-up per ten milliseconds and no visible difference.
///
/// It is also the tick the player checks its orders on, so a stop is obeyed
/// within a fiftieth of a second however long the frame.
pub const SLOWEST_FRAME_MS: u32 = 20;

/// What a file needs to be watched moving.
///
/// # There is no frame COUNT and no frame SIZE here
///
/// A count was here for an hour and it lied: the answer is computed lazily —
/// two frames settle "more than one", which is the whole question — so it held
/// 2 for a file with forty-four.
///
/// A size is absent for a better reason: the frame's size is decided by the
/// PANEL and enforced on ffmpeg, so nothing the file says about its dimensions
/// can make it wrong. See [`stream_args`].
#[derive(Debug, Clone, PartialEq)]
pub struct Moving {
    /// Which decoder the player should open.
    pub how: Source,
    /// What the frames are made of — the file's own picture, or one drawn from
    /// its sound.
    pub show: Show,
    /// How fast it runs and how long it lasts.
    ///
    /// A GIF has no duration — it loops — so `seconds` is `None` there, and
    /// that is what decides it gets no timeline.
    pub shape: crate::facts::Shape,
    /// Where the moving part lives, when the file is not itself the film.
    ///
    /// `Some` IS the definition of a motion photo — there is no second flag
    /// saying the same thing — and it is what makes the player show the
    /// photograph first and the film after it, in a loop.
    pub inside: Option<Inside>,
}

/// Where the moving part LIVES, when it is not the file itself.
///
/// # A photograph with a film inside it
///
/// Phones record a second or two of video alongside the shutter. Google and
/// Samsung append it to the JPEG; Apple writes it beside, as its own file. The
/// picture is a real photograph either way — what changes is only where the
/// film is, which is why this is a field and not a second `Source`: the
/// decoder is the same ffmpeg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inside {
    /// A stretch of the SAME file, from `at` to `to`.
    ///
    /// Google's motion photos and Samsung's. Measured on the owner's own two
    /// Pixel files: the video begins at a single `ftyp` box and runs to the end.
    Range {
        /// The first byte of the video.
        at: u64,
        /// One past its last.
        to: u64,
    },
    /// Another file entirely — Apple's Live Photo.
    ///
    /// `IMG_1234.HEIC` and `IMG_1234.MOV`, which are two files on disk and one
    /// thing to a person. What ties them is a `ContentIdentifier` written into
    /// both, and NOT the name: an unrelated `photo.jpg` and `photo.mov` are
    /// ordinary, and pairing by name alone would join them.
    Beside(std::path::PathBuf),
}

/// Where the frames come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The `image` crate, in process. GIF today.
    InProcess,
    /// ffmpeg, which reads the variety.
    Ffmpeg,
}

/// What the frames are MADE of.
///
/// # Sound is a film whose picture we draw
///
/// A player needs a stream of frames for a reason that has nothing to do with
/// looking at anything: the frames ARE the clock, and not reading them is what
/// pauses the child. Take them away and each of those mechanisms needs a second
/// implementation — a timer in both doors, and a pause that kills instead of
/// blocking, which clicks.
///
/// So for a file with no picture of its own, one is drawn: the album's cover
/// with a live waveform along its foot, or the waveform alone. Measured, ten
/// seconds of waveform costs 22 to 32 ms, and not reading it leaves the child
/// on 0.04 s of CPU over three — exactly what a film does.
///
/// This is a field on [`Moving`] and NOT a third [`Source`], because `Source`
/// says which decoder, and the decoder is still ffmpeg. A third arm would be a
/// second player function beside `play_video`, and `motion-never-leaks-a-child`
/// guards that one by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    /// The file's own moving picture — a film, or a GIF.
    Film,
    /// A picture drawn FROM the sound, because the file has none.
    Sound {
        /// Whether there is artwork to draw the waves over.
        cover: bool,
    },
}

/// Does this file move, and how would one watch it?
///
/// `None` is the ordinary answer: a photograph does not move, and neither does
/// a GIF of one frame — which is most GIFs on a machine, because the format is
/// still used for stills.
///
/// `probe` is the ffprobe document the inspector ALREADY ran for the facts. No
/// second spawn: one document, read twice for two different things.
#[must_use]
pub fn moving(path: &Path, mime: &str, probe: &str, beside: Option<&Path>) -> Option<Moving> {
    let mime = ronoctua_files::mime::canonical(mime);
    // A PHOTOGRAPH WITH A FILM IN IT, asked FIRST and only of pictures.
    //
    // It has to come before the GIF branch and before the media-type gate,
    // because a motion photo IS an image by every other answer: `image/jpeg`,
    // and ffprobe reading the whole file reports `image2` — which
    // `pretend_film` correctly refuses. The film is somewhere else.
    let media = mime.split_once('/').map(|(m, _)| m).unwrap_or(mime);
    if media == "image"
        && let Some(found) = motion_photo(path, beside)
    {
        return Some(found);
    }
    if mime == "image/gif" {
        // COUNTED, AND NOT ASSUMED. A one-frame GIF is a picture, and starting
        // a player for it would be a thread and a clock for a still image.
        let (frames, delay_ms) = gif_frames(path)?;
        if frames <= 1 {
            return None;
        }
        return Some(Moving {
            how: Source::InProcess,
            // A GIF IS A PICTURE, silent by construction.
            show: Show::Film,
            shape: crate::facts::Shape {
                // ITS OWN CADENCE, uncapped: a GIF at another rate reads as
                // broken, and the owner asked for the file's own.
                fps: 1000.0 / f64::from(delay_ms.max(SLOWEST_FRAME_MS)),
                // A GIF LOOPS. No duration means no timeline and no seek, and
                // that is right: its frames are inter-dependent, so seeking
                // means decoding from the first one — 707 ms on a
                // forty-four-frame file, measured.
                seconds: None,
                // AND A GIF IS SILENT, which is a property of the format and
                // not a reading of this one.
                sound: false,
            },
            // A GIF IS THE FILM, and not a picture with one inside.
            inside: None,
        });
    }
    // AND SOUND, whose picture does not exist until we draw it.
    //
    // It is asked BEFORE the media gate below and answered from the SAME
    // probe: an audio file has no video stream to shape a film from, so
    // `shape_of` would refuse it — rightly, because what it guards is a player
    // being started for an album's cover.
    if media == "audio" {
        // A SCORE IS NOT A RECORDING, and starting a player for one would be a
        // thread and a child process to produce silence. The panel says so
        // instead — see `pixels::needs_a_synthesiser`.
        if crate::pixels::needs_a_synthesiser(mime) {
            return None;
        }
        return Some(Moving {
            how: Source::Ffmpeg,
            show: Show::Sound {
                cover: crate::facts::has_cover(probe),
            },
            shape: crate::facts::sound_shape_of(probe)?,
            inside: None,
        });
    }
    // EVERY MOVING PICTURE ffmpeg READS, which is the variety the owner asked
    // for — decided by the SHAPE the probe reports, so a file with no video
    // stream, or one whose only video stream is an album's cover, does not
    // start a player.
    if media != "video" {
        return None;
    }
    Some(Moving {
        how: Source::Ffmpeg,
        show: Show::Film,
        shape: crate::facts::shape_of(probe)?,
        inside: None,
    })
}

/// How far into a picture the vendors' metadata is looked for.
///
/// The XMP travels in an APP1 segment near the front. Measured on the owner's
/// two Pixel files: 13 KB into one and 41 KB into the other. A hundred and
/// twenty-eight kibibytes is generous against both and costs 0,02 ms.
const XMP_WINDOW: u64 = 128 * 1024;

/// How much of a picture's TAIL is searched for an appended film.
///
/// The vendors put the video last, and these videos are seconds long. Sixteen
/// mebibytes covers every one of them and bounds what a very large photograph
/// costs: measured, scanning the owner's 10,6 MB file end to end is 2,2 ms.
const TAIL_WINDOW: u64 = 16 * 1024 * 1024;

/// The picture that has a film in it, and where the film is.
///
/// # Three doors, cheapest first, and the last two need no vendor table
///
/// 1. The **XMP** the makers write, read out of the first [`XMP_WINDOW`]. It is
///    exact and it is what Google writes.
/// 2. A **scan of the tail** for an `ftyp` box. This is the general answer: it
///    covers Samsung, whose trailer format this machine has no file to test
///    against, and anything else that appends an MP4. On the owner's own files
///    it lands on exactly the byte the XMP declares, so the general route is
///    tested by the same two files the specific one is.
/// 3. A **file beside it** — Apple, whose Live Photo is two files.
///
/// Whatever is found is then CONFIRMED by ffprobe: a stretch of bytes that has
/// no video stream with a duration is not a film, whatever it looked like.
fn motion_photo(path: &Path, beside: Option<&Path>) -> Option<Moving> {
    let size = std::fs::metadata(path).ok()?.len();
    let inside = xmp_video(path, size)
        .or_else(|| tail_video(path, size))
        .or_else(|| live_photo(path, beside))?;
    let shape = crate::facts::shape_of(&probe_inside(path, &inside)?)?;
    Some(Moving {
        how: Source::Ffmpeg,
        // A MOTION PHOTO IS A FILM, and one the file really carries.
        show: Show::Film,
        shape,
        inside: Some(inside),
    })
}

/// The row the info panel shows for a photograph that carries a film.
///
/// One sentence and not four: how long the film is, which is the only thing
/// about it a person acts on. The photograph's own dimensions, camera and date
/// are already there — this says the picture MOVES, which nothing else on the
/// sheet says.
#[must_use]
pub fn fact_of(moving: &Moving) -> Option<crate::facts::Fact> {
    moving.inside.as_ref()?;
    let seconds = moving.shape.seconds?;
    Some(crate::facts::Fact {
        label: crate::facts::Label::Key(crate::key::FACT_MOTION),
        // THE SAME CLOCK THE DURATION ROW USES, so a film of three seconds
        // reads the same whether it is a file or is inside a photograph.
        value: crate::facts::Value::Data(crate::facts::clock(seconds)),
    })
}

/// The video's stretch as the makers' own metadata declares it.
///
/// Google writes an XMP `Container:Directory` whose last item is the film:
/// `Item:Semantic="MotionPhoto"` with an `Item:Length`. The items are laid end
/// to end after the primary picture, so the film's length counted back from the
/// end of the file IS its offset — verified against the owner's two files,
/// where it agrees to the byte with the `ftyp` the scan finds.
///
/// The older phones wrote `GCamera:MicroVideoOffset` instead, which is the same
/// number under another name: bytes from the end.
fn xmp_video(path: &Path, size: u64) -> Option<Inside> {
    let head = read_part(path, 0, XMP_WINDOW.min(size))?;
    let text = String::from_utf8_lossy(&head);
    let length =
        after_semantic(&text, "MotionPhoto").or_else(|| tagged(&text, "MicroVideoOffset"))?;
    (length > 0 && length < size).then(|| Inside::Range {
        at: size - length,
        to: size,
    })
}

/// The `Item:Length` of the container item with a given semantic.
///
/// A scan and not an XML parse, and that is a decision: the whole of what is
/// wanted is one number from one attribute, the document is a vendor's fixed
/// template, and a parser would be a dependency and a second thing that can
/// refuse a file this one reads.
fn after_semantic(text: &str, semantic: &str) -> Option<u64> {
    let mark = format!("Item:Semantic=\"{semantic}\"");
    let at = text.find(&mark)?;
    // THE LENGTH MAY SIT ON EITHER SIDE of the semantic inside the same item —
    // the makers order the attributes differently between versions — so the
    // item is bounded first and read whole.
    let from = text[..at].rfind("<Container:Item").unwrap_or(0);
    let to = text[at..].find("/>").map_or(text.len(), |e| at + e);
    tagged(&text[from..to], "Item:Length")
}

/// One `Name="1234"` attribute, as a number.
fn tagged(text: &str, name: &str) -> Option<u64> {
    let mark = format!("{name}=\"");
    let at = text.find(&mark)? + mark.len();
    let rest = &text[at..];
    rest[..rest.find('"')?].parse().ok()
}

/// The video found by looking for it, when nobody declared where it is.
///
/// # Why a scan beats a parser per vendor
///
/// Because there is one thing to find and every vendor puts the same thing
/// there: an MP4, and an MP4 begins with an `ftyp` box. Samsung's own trailer
/// format is documented and this machine has no Samsung file to test a parser
/// against — code written from a document and never run against a file is code
/// that says it works. The scan is exercised by the owner's own two files,
/// where it lands on the byte the XMP declares.
fn tail_video(path: &Path, size: u64) -> Option<Inside> {
    let from = size.saturating_sub(TAIL_WINDOW);
    let tail = read_part(path, from, size - from)?;
    // THE BOX'S OWN SHAPE IS THE FILTER. Four ASCII letters occur by chance in
    // a photograph; a box header does not: the size in front of it is small and
    // the brand after it is printable.
    let at = tail.windows(4).enumerate().find_map(|(i, w)| {
        if w != b"ftyp" || i < 4 {
            return None;
        }
        let len = u32::from_be_bytes([tail[i - 4], tail[i - 3], tail[i - 2], tail[i - 1]]);
        let brand = tail.get(i + 4..i + 8)?;
        let sane = (8..=64).contains(&len) && brand.iter().all(|b| b.is_ascii_graphic() || *b == 0);
        sane.then_some(i - 4)
    })?;
    #[allow(clippy::cast_possible_truncation)]
    let at = from + at as u64;
    // AND NOT THE WHOLE FILE. A plain MP4 begins at byte nought, and answering
    // `Range { at: 0 }` for one would send it round the motion-photo path for
    // no reason.
    (at > 0).then_some(Inside::Range { at, to: size })
}

/// The film that lives beside the photograph — Apple's Live Photo.
///
/// The candidate comes from the caller, because knowing what else is in the
/// FOLDER is the view's business and this crate's promise is what is inside a
/// file. What is decided here is whether the two belong together, and the
/// answer is the `ContentIdentifier` both carry — never the name. An unrelated
/// `photo.jpg` and `photo.mov` are ordinary, and a pairing by name would join
/// them and play a stranger's video under somebody's photograph.
fn live_photo(path: &Path, beside: Option<&Path>) -> Option<Inside> {
    let beside = beside?;
    let mine = crate::facts::content_identifier(path)?;
    let theirs = crate::facts::content_identifier(beside)?;
    (mine == theirs).then(|| Inside::Beside(beside.to_owned()))
}

/// A window of a file, read without holding the rest of it.
fn read_part(path: &Path, at: u64, len: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek};

    let len = usize::try_from(len).ok()?;
    if len == 0 {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(std::io::SeekFrom::Start(at)).ok()?;
    let mut out = vec![0u8; len];
    let mut got = 0usize;
    while got < len {
        match file.read(&mut out[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    out.truncate(got);
    (!out.is_empty()).then_some(out)
}

/// ffprobe's answer about a stretch of bytes, or a file beside.
///
/// This is what turns "these bytes look like a film" into "these bytes ARE a
/// film". Measured at 107 ms on the owner's own file, and about 20 ms to refuse
/// something that is not one — the same speed the corrupt files already refuse
/// at.
fn probe_inside(path: &Path, inside: &Inside) -> Option<String> {
    use std::ffi::OsStr;

    let url = input_url(path, inside);
    let args = [
        OsStr::new("-v"),
        OsStr::new("error"),
        OsStr::new("-print_format"),
        OsStr::new("json"),
        OsStr::new("-show_format"),
        OsStr::new("-show_streams"),
        // NO `--` HERE, and that is not an omission: what follows is a URL and
        // not a path, and `subfile,…` cannot be mistaken for an option.
        url.as_os_str(),
    ];
    let out = tools::run(tools::Tool::Ffprobe, &args).ok()?;
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// How ffmpeg is told to read the film, wherever it lives.
///
/// One place, because the argument list and this string have to agree and
/// `stream_args` builds the first. `subfile` is ffmpeg's own protocol for a
/// stretch of a file — no copy, no temporary, no 4 MB written to disk to watch
/// three seconds of video.
#[must_use]
pub fn input_url(path: &Path, inside: &Inside) -> std::ffi::OsString {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStrExt;

    match inside {
        Inside::Range { at, to } => {
            let mut url = OsString::from(format!("subfile,,start,{at},end,{to},,:"));
            url.push(std::ffi::OsStr::from_bytes(path.as_os_str().as_bytes()));
            url
        }
        Inside::Beside(other) => other.as_os_str().to_owned(),
    }
}

/// How many frames a GIF holds, and how long the first one is held for.
///
/// # TWO is the whole question, and it is asked lazily
///
/// The question is "more than one", so the second frame settles it — and
/// `image`'s frame iterator is lazy, so asking for two decodes two. Composing
/// all forty-four of the owner's own sample costs seven hundred milliseconds,
/// and this is asked while a cursor walks a directory. Measured at 41 ms.
/// WHETHER THIS FILE MOVES AND CAN BE MOVED IN PROCESS.
///
/// The three web animation formats — GIF, animated WebP and APNG — all have a
/// LAZY frame iterator inside the `image` crate, so playing one costs a file
/// descriptor and a frame. Everything else that moves costs a CHILD PROCESS,
/// and that is the whole line the file manager's grid draws between what
/// animates by itself and what animates under the pointer: a screenful of the
/// first is twelve descriptors, a screenful of the second is sixty ffmpegs.
///
/// It reads TWO frames and stops. A one-frame GIF is a picture, and starting an
/// animator for it would be a slot and a clock for a still — the same argument
/// [`moving`] already makes.
#[must_use]
pub fn animates(path: &Path, mime: &str) -> bool {
    use image::AnimationDecoder;

    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let file = std::io::BufReader::new(file);
    let two = |frames: image::Frames<'_>| frames.take(2).filter(Result::is_ok).count() > 1;
    match mime {
        "image/gif" => {
            image::codecs::gif::GifDecoder::new(file).is_ok_and(|d| two(d.into_frames()))
        }
        "image/webp" => image::codecs::webp::WebPDecoder::new(file)
            .is_ok_and(|d| d.has_animation() && two(d.into_frames())),
        "image/png" | "image/apng" => image::codecs::png::PngDecoder::new(file)
            .and_then(image::codecs::png::PngDecoder::apng)
            .is_ok_and(|d| two(d.into_frames())),
        _ => false,
    }
}

fn gif_frames(path: &Path) -> Option<(usize, u32)> {
    use image::AnimationDecoder;

    let file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let decoder = image::codecs::gif::GifDecoder::new(file).ok()?;
    let mut delay = SLOWEST_FRAME_MS;
    let mut n = 0usize;
    for frame in decoder.into_frames().take(2) {
        let Ok(frame) = frame else { break };
        if n == 0 {
            let (num, den) = frame.delay().numer_denom_ms();
            delay = num / den.max(1);
        }
        n += 1;
    }
    Some((n, delay))
}

/// The exact frame a panel wants, in pixels.
///
/// Even on both sides, because the pixel format wants it; never larger than
/// the box; and never larger than [`WIDEST`], which bounds the buffer whatever
/// a door reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    /// Across.
    pub width: u32,
    /// Down.
    pub height: u32,
}

impl Fit {
    /// The frame for a box, quantised so a drag does not respawn per pixel.
    ///
    /// # Why the box is rounded to a step
    ///
    /// Dragging a window's edge changes the box sixty times a second, and a
    /// changed box is a changed wish, which is a kill and a respawn — 108 to
    /// 242 milliseconds each, measured. Quantising to [`Self::STEP`] means a
    /// full drag from the smallest panel to the largest crosses at most seven
    /// steps. It is [`crate::step`]'s argument, in two dimensions.
    #[must_use]
    pub fn of(box_w: f32, box_h: f32) -> Option<Self> {
        if !box_w.is_finite() || !box_h.is_finite() || box_w < 1.0 || box_h < 1.0 {
            return None;
        }
        // THE WIDTH IS QUANTISED AND THE HEIGHT FOLLOWS THE BOX'S SHAPE.
        //
        // Both quantised independently was the first arrangement and it is what
        // the owner photographed as *"muito espaço em branco desnecessário ao
        // redor"* — the picture was FRAMED TWICE. A box of 380 by 300 came out
        // as a frame of 384 by 128, so ffmpeg letterboxed a sixteen-to-nine
        // film into a three-to-one frame — seventy-eight transparent columns
        // each side — and the door then fitted THAT into the box, adding its
        // own bands above and below. Two letterboxes, one picture.
        //
        // With the frame shaped like the box, the pad IS the panel's own air:
        // the door draws the frame filling the box and there is exactly one
        // letterbox, which is the one the shape of the film requires. Measured
        // on the same box, the visible picture goes from 225 across to 380.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let asked = box_w.ceil() as u32;
        let width = asked.div_ceil(Self::STEP) * Self::STEP;
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let height = (f64::from(width) * f64::from(box_h) / f64::from(box_w)).round() as u32;
        // AND NEITHER SIDE PASSES THE CEILING, which is a bound on OUR OWN
        // arithmetic — brought down TOGETHER, or the shape this function exists
        // to keep would be lost at the one size that needed it.
        let over = (f64::from(width) / f64::from(WIDEST))
            .max(f64::from(height) / f64::from(WIDEST))
            .max(1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bring =
            |v: u32| -> u32 { (((f64::from(v) / over).round() as u32).max(NARROWEST)) & !1 };
        let fit = Self {
            width: bring(width),
            height: bring(height),
        };
        // AND A FRAME THAT CANNOT STOP ITS OWN CHILD IS GROWN UNTIL IT CAN.
        //
        // BOTH SIDES TOGETHER, by the same argument the ceiling above makes:
        // the shape is what this function exists to keep, and a floor applied
        // to one side would bring the double letterbox back at the one size
        // that needed the guard.
        //
        // A panel narrow enough to reach here is unusual — every real one is
        // already several times a pipe — but `NARROWEST` is two, and two by two
        // is sixteen bytes: four thousand of those fit in a pipe, so the child
        // would never block and PAUSING WOULD DO NOTHING. See [`PIPE_BYTES`].
        // The defect was found on a frame chosen by hand rather than by this
        // function; it is guarded here so it cannot be reached by either road.
        Some(fit.at_least_a_pipe())
    }

    /// The same frame, grown until it can block the process writing it.
    ///
    /// # The shape is kept while it can be, and given up before the pause is
    ///
    /// Both sides double together, so an ordinary frame keeps its box's shape
    /// exactly and is merely larger — which costs nothing on screen, because
    /// the door fits it into the box either way.
    ///
    /// A shape extreme enough to put one side against [`WIDEST`] before the
    /// frame reaches a pipe cannot have both. Then the other side goes on
    /// alone, and the shape is what gives way: a letterbox is a picture with
    /// air around it, and a frame that cannot be paused is a control that
    /// lies. No panel this system lays out is anywhere near that — a preview
    /// is cells wide and lines tall — so this is a guarantee holding at the
    /// edge rather than a case anybody meets.
    #[must_use]
    fn at_least_a_pipe(self) -> Self {
        let mut grown = self;
        while !grown.blocks_a_writer() && (grown.width < WIDEST || grown.height < WIDEST) {
            grown = Self {
                width: (grown.width * 2).min(WIDEST),
                height: (grown.height * 2).min(WIDEST),
            };
        }
        grown
    }

    /// How coarsely a box is measured, in pixels.
    pub const STEP: u32 = 128;

    /// How many bytes one frame occupies as RGBA.
    #[must_use]
    pub const fn bytes(self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// Whether a frame this size can still STOP the child that writes it.
    ///
    /// # It is a frame's SHARE of the pipe that decides, not a threshold
    ///
    /// What a paused child can still do is run ahead by whatever the pipe will
    /// take: `PIPE_BYTES / bytes()` frames, plus the part of one more. So the
    /// question is not "is the frame big" but "how many of it fit", and the
    /// answer wanted is at most one.
    ///
    /// At exactly a pipe the child writes one frame, blocks on the next, and
    /// the sound stops within a frame's own time — eighty-three milliseconds
    /// at twelve a second, thirty-three at thirty. At a sixteenth of a pipe it
    /// runs four thousand ahead and the sound goes on for five minutes, which
    /// is the defect this exists to name.
    #[must_use]
    pub const fn blocks_a_writer(self) -> bool {
        self.bytes() >= PIPE_BYTES
    }

    /// The smallest frame that still stops the child that writes it.
    ///
    /// For a picture nobody draws — a playlist's, whose panel is the LIST. The
    /// frames are still the clock and still the pause, so they cannot be made
    /// as small as one likes: see [`PIPE_BYTES`].
    ///
    /// A hundred and twenty-eight across, which is [`Fit::STEP`], by a hundred
    /// and ninety-two down: ninety-six kibibytes, half again the pipe. At
    /// twelve frames a second that is a megabyte and a bit each second, and it
    /// buys the one mechanism that pauses everything in this manager.
    pub const UNSEEN: Self = Self {
        width: Self::STEP,
        height: Self::STEP + Self::STEP / 2,
    };
}

/// What the player wants opened, in one value.
///
/// A record and not nine positional parameters: the last three to arrive were
/// all `&str` — the letterbox, the waveform's ink and the sink — and three
/// strings in a row is an argument order nobody can check by reading the call.
#[derive(Debug, Clone, Copy)]
pub struct Stream<'a> {
    /// The file.
    pub path: &'a Path,
    /// What its frames are made of.
    pub show: Show,
    /// Where the film is, when the file is not itself the film.
    pub inside: Option<&'a Inside>,
    /// The exact frame the panel measured.
    pub fit: Fit,
    /// Frames a second.
    pub fps: f64,
    /// Where to start, in seconds.
    pub from: f64,
    /// The sound device, or nothing for silence.
    pub sound: Option<&'a str>,
    /// The letterbox's colour, as ffmpeg reads colours.
    pub background: &'a str,
    /// The waveform's ink, as ffmpeg reads colours.
    pub wave: &'a str,
    /// How tall the waveform is over a cover, in hundredths of the frame.
    pub wave_height: u16,
    /// How much of the frame's foot the controls cover, in hundredths.
    ///
    /// # The waves have to sit ABOVE the controls, not under them
    ///
    /// A sound's controls are always drawn, on a band across the frame's last
    /// line — so a strip at the frame's own foot is exactly the strip nobody
    /// can see. The owner photographed it: with an album's cover in the file,
    /// *"o espectro/ondas animado fica por trás dos controles"*.
    ///
    /// It is a fraction rather than a number of pixels because the caller
    /// cannot give pixels: the frame is decoded at the panel's shape and then
    /// fitted into it, so what one line of the PANEL covers is one line of the
    /// FRAME only in the same proportion. A fraction survives the fit.
    pub wave_foot: u16,
}

/// The filter chain that makes a picture EXACTLY the frame we asked for.
///
/// # The frame's size is OURS, and that is the whole design
///
/// `scale=W:H:force_original_aspect_ratio=decrease` fits the picture inside
/// the box keeping its shape, and `pad=W:H` fills the rest — so what comes out
/// of the pipe is **exactly** `W × H`, whatever the file is. `frame_bytes` is
/// then arithmetic rather than a guess, and no lie a container tells about its
/// dimensions can desynchronise the stream.
///
/// The obvious alternative — read `width`/`height` from ffprobe and scale to
/// them — was measured and is WRONG. A phone video reports 640×360 coded with
/// a matrix saying "rotate 90"; ffmpeg renders 256×456. Scaling to the
/// reported shape squashes the picture and makes every frame the wrong length.
/// Three samples were checked — landscape, portrait-by-rotation and
/// portrait-by-encoding — and all three come out of this filter at the size
/// asked for.
///
/// # And it is ONE function because there are now two graphs
///
/// A film's picture and a sound's drawn one are built differently, and the
/// check that guards this rule — `motion-never-guesses-a-frame` — looks for
/// these literals ANYWHERE in the file. A second graph with its own copy of
/// them would satisfy the grep while leaving its own dimensions to ffmpeg, and
/// the check would go green on the defect it exists to catch. Written once,
/// the claim is structurally true of both.
///
/// `background` is the panel's own colour, so the letterboxing is the air the
/// panel would have left anyway rather than a black bar. It arrives as a
/// string because this crate receives no token values.
fn size_tail(fit: Fit, background: &str) -> String {
    format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,\
         pad={w}:{h}:-1:-1:color={background},setsar=1",
        w = fit.width,
        h = fit.height,
    )
}

/// How tall the waveform is drawn over a cover, in pixels.
///
/// At least one row: a fraction of a very short frame rounds to nothing, and a
/// zero-height filter is an ffmpeg error rather than an absent strip.
fn wave_rows(fit: Fit, hundredths: u16) -> u32 {
    (fit.height * u32::from(hundredths) / 100).max(1)
}

/// The arguments ffmpeg is given to stream a file as raw frames.
///
/// Raw RGBA and not a PNG per frame: a PNG is an encode in ffmpeg and a decode
/// here, per frame, for a picture that lives eighty milliseconds.
///
/// # A sound is two inputs of the same file, and that is not a flourish
///
/// An album's cover is an `attached_pic` — ONE frame, at timestamp zero. Seek
/// the input and there is no frame after the seek at all, so `overlay` waits
/// for one that never comes while the audio runs out underneath it. Measured:
/// `-ss 150` on a five-minute FLAC took **143 875 ms and produced no frame**.
///
/// So the cover is read from an input that is never seeked, and the sound from
/// a second input of the same file that is. Measured at 159 to 298 ms to the
/// first frame, anywhere in the file — the same class as a film's respawn.
#[must_use]
pub fn stream_args(stream: &Stream<'_>) -> Vec<std::ffi::OsString> {
    use std::ffi::OsString;

    let &Stream {
        path,
        show,
        inside,
        fit,
        fps,
        from,
        sound,
        background,
        wave,
        wave_height,
        wave_foot,
    } = stream;

    let mut out: Vec<OsString> = Vec::new();
    macro_rules! push {
        ($($a:expr),+ $(,)?) => { $( out.push(OsString::from($a)); )+ };
    }
    push!("-v", "error");
    // NOTHING ON STDIN. Without it ffmpeg reads the terminal and a shell that
    // shares one loses its keystrokes.
    push!("-nostdin");

    // THE COVER'S INPUT COMES FIRST AND IS NEVER SEEKED — see the note above.
    let cover = matches!(show, Show::Sound { cover: true });
    if cover {
        push!("-i");
        out.push(path.as_os_str().to_owned());
    }

    if from > 0.0 {
        // BEFORE `-i`, which is what makes the seek an INPUT seek — the fast
        // one, measured at 108 to 242 ms anywhere in a ten-minute film.
        push!("-ss");
        out.push(OsString::from(format!("{from:.3}")));
    }
    push!("-i");
    // THE URL IS BUILT HERE, beside the rest of ffmpeg's own arguments.
    //
    // A film inside a photograph is read with `subfile`, which takes a stretch
    // of a file and no copy. The alternative was to keep the URL in the
    // caller's `PathBuf` — it would have travelled unchanged, because this is
    // an option's VALUE and nothing on the way inspects it — and that is a type
    // saying "path" about something that is not one.
    out.push(inside.map_or_else(
        || path.as_os_str().to_owned(),
        |inside| input_url(path, inside),
    ));

    match show {
        Show::Film => {
            push!("-map");
            push!("0:v:0");
            push!("-vf");
            out.push(OsString::from(format!(
                "{tail},fps={fps:.4}",
                tail = size_tail(fit, background)
            )));
        }
        Show::Sound { .. } => {
            // WHICH INPUT THE SOUND IS ON: the second when a cover took the
            // first, the only one otherwise.
            let a = u8::from(cover);
            let tail = size_tail(fit, background);
            // AND THE AUDIO IS SPLIT ONLY WHEN BOTH HALVES ARE WANTED.
            //
            // `showwaves` CONSUMES the stream it draws, so with a pulse output
            // as well there is no stream left to map — ffmpeg refuses the
            // WHOLE command, exit 234, measured. And the converse is just as
            // fatal: a labelled pad created and never mapped is the same
            // refusal, so a silent sound must not split at all.
            let (into, from_pad) = if sound.is_some() {
                (
                    format!("[{a}:a]asplit=2[wave][heard];"),
                    "[wave]".to_owned(),
                )
            } else {
                (String::new(), format!("[{a}:a]"))
            };
            let graph = if cover {
                let rows = wave_rows(fit, wave_height);
                // AND IT SITS ABOVE WHAT THE CONTROLS COVER, which is the only
                // reason this is not simply the frame's foot.
                let under = wave_rows(fit, wave_foot);
                format!(
                    "{into}\
                     {from_pad}showwaves=s={w}x{rows}:mode=cline:rate={fps:.4}:colors={wave},\
                     format=rgba[drawn];\
                     [0:v]{tail},format=rgba,loop=loop=-1:size=1:start=0,fps={fps:.4}[sleeve];\
                     [sleeve][drawn]overlay=0:{top}:shortest=1[picture]",
                    w = fit.width,
                    top = fit.height.saturating_sub(rows + under),
                )
            } else {
                format!(
                    "{into}\
                     {from_pad}showwaves=s={w}x{h}:mode=cline:rate={fps:.4}:colors={wave},\
                     format=rgba[picture]",
                    w = fit.width,
                    h = fit.height,
                )
            };
            push!("-filter_complex");
            out.push(OsString::from(graph));
            push!("-map");
            push!("[picture]");
        }
    }
    push!("-pix_fmt");
    push!("rgba");
    // NOTHING BUT PICTURE IN THIS OUTPUT, and these belong HERE — an ffmpeg
    // option applies to the output that FOLLOWS it, so `-an` after the `-`
    // applies to an output that does not exist. It was written that way for an
    // hour and worked only because the rawvideo muxer takes no audio anyway; a
    // test that counted frames is what found it.
    push!("-an");
    push!("-sn");
    push!("-dn");
    push!("-f");
    push!("rawvideo");
    push!("-");

    // ONE CHILD, TWO OUTPUTS, and that is what makes the sound stay in step
    // without anything measuring it: ffmpeg has a single mux loop and blocks on
    // whichever output is slow. The video pipe is 64 KiB and a frame is more,
    // so OUR read rate is the throttle — and our read rate is the wall clock.
    // The audio can lead by at most its own buffer.
    //
    // A second process (ffplay) would demux and read the file twice and start
    // at its own seek latency, with nothing able to correct it.
    //
    // AND NOTHING AT ALL when it is silent: the video output above already
    // refuses audio, subtitles and data, so there is no second output to write.
    if let Some(sink) = sound {
        push!("-map");
        // A FILM MAPS THE STREAM; A SOUND MAPS THE HALF THE FILTER LEFT.
        match show {
            Show::Film => {
                push!("0:a:0?");
            }
            Show::Sound { .. } => {
                push!("[heard]");
            }
        }
        push!("-f");
        push!("pulse");
        push!("-buffer_duration");
        push!("80");
        out.push(OsString::from(sink));
    }
    out
}

/// Where ffmpeg is, so the player can spawn it BY PATH.
///
/// The `PATH` under a systemd unit is minimal, and a spawn by name fails far
/// from its cause — the trap `AGENTS.md` names. This is the same lookup the
/// still-frame path already does.
///
/// # Errors
///
/// [`Refusal::Missing`] when the environment does not name it.
pub fn ffmpeg() -> Result<std::path::PathBuf, Refusal> {
    tools::Tool::Ffmpeg.path()
}

#[cfg(test)]
mod tests {
    use super::{
        Fit, Inside, Moving, NARROWEST, PIPE_BYTES, Show, Source, Stream, WIDEST, ffmpeg, moving,
        stream_args,
    };

    /// A wish with everything ordinary, for a test to vary one field of.
    ///
    /// [`Stream`] is a record precisely so a call site can say what it is ABOUT
    /// and inherit the rest — `Stream { from: 61.5, ..asking(path, fit) }` is a
    /// test about seeking, and reads like one.
    fn asking<'a>(path: &'a std::path::Path, fit: Fit) -> Stream<'a> {
        Stream {
            path,
            show: Show::Film,
            inside: None,
            fit,
            fps: 30.0,
            from: 0.0,
            sound: None,
            background: "#161616",
            wave: "#BFD3E6",
            wave_height: 20,
            wave_foot: 10,
        }
    }

    /// FAILS IF: a still picture starts a player. A thread and a child process
    /// for a photograph is a cost with nothing on the other side.
    #[test]
    fn a_still_picture_does_not_move() {
        let dir = std::env::temp_dir().join(format!("ronoctua-motion-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let png = dir.join("still.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").expect("write");
        for kind in ["image/png", "text/plain", "application/pdf", "audio/mpeg"] {
            assert!(
                moving(&png, kind, "{}", None).is_none(),
                "{kind} started a player"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a score starts a player that can only produce silence.
    ///
    /// MIDI and its relatives hold instructions rather than sound, and this
    /// build has no synthesiser to follow them — measured against the pinned
    /// binary, which lists no audio decoder for any of them. A tracker module
    /// is the opposite case and must keep playing: `libopenmpt` IS built in,
    /// and it was proved against a module built by hand.
    #[test]
    fn a_score_is_refused_and_a_tracker_module_is_not() {
        let path = std::path::Path::new("/x");
        let sounds = r#"{"streams":[{"codec_type":"audio","codec_name":"pcm_f32le"}],
                         "format":{"format_name":"libopenmpt","duration":"7.68"}}"#;
        for score in [
            "audio/midi",
            "audio/prs.sid",
            "audio/x-psf",
            "audio/x-psflib",
            "audio/x-xmf",
        ] {
            assert!(
                moving(path, score, sounds, None).is_none(),
                "{score} started a player nothing can feed"
            );
            assert!(crate::pixels::needs_a_synthesiser(score), "{score}");
        }
        // AND THE TRACKERS PLAY.
        for module in ["audio/x-mod", "audio/x-it", "audio/x-s3m", "audio/x-xm"] {
            assert!(
                !crate::pixels::needs_a_synthesiser(module),
                "{module} was refused, and libopenmpt plays it"
            );
            assert!(
                moving(path, module, sounds, None).is_some(),
                "{module} did not reach the player"
            );
        }
    }

    /// FAILS IF: a frame gets small enough that pausing stops being a pause.
    ///
    /// The pause is the loop not reading: the child fills the pipe, blocks on
    /// the write, and the sound stops with it. That holds only while a frame is
    /// BIGGER than the pipe — and the arithmetic is not close, which is what
    /// makes it easy to break by accident. A two-by-two frame is sixteen bytes,
    /// four thousand of them fit in a pipe, and the owner heard the result:
    /// the glyph and the counter stopped and the sound went on for minutes.
    #[test]
    fn no_frame_is_too_small_to_stop_the_child_writing_it() {
        // THE ONE CHOSEN BY HAND, for a picture nobody draws.
        assert!(
            Fit::UNSEEN.blocks_a_writer(),
            "the unseen frame is {} bytes against a pipe of {PIPE_BYTES}",
            Fit::UNSEEN.bytes()
        );

        // AND EVERY ONE THIS FUNCTION CAN PRODUCE, across every panel a door
        // could measure — from a sliver to a whole screen.
        for w in [
            1.0_f32, 2.0, 17.0, 64.0, 128.0, 380.0, 512.0, 1024.0, 4096.0,
        ] {
            for h in [1.0_f32, 2.0, 9.0, 26.5, 64.0, 300.0, 768.0, 2160.0] {
                let Some(fit) = Fit::of(w, h) else {
                    continue;
                };
                assert!(
                    fit.blocks_a_writer(),
                    "a {w}x{h} panel gives {}x{} — {} bytes, and a pipe holds \
                     {PIPE_BYTES}: pausing it would do nothing",
                    fit.width,
                    fit.height,
                    fit.bytes()
                );
                // AND THE SHAPE SURVIVES THE GROWING for every box a panel
                // could really be — which is the reason both sides are
                // multiplied by the same number: a frame of another shape than
                // its box is letterboxed TWICE.
                //
                // A SLIVER IS NOT ONE OF THOSE. The bound is the box's
                // lopsidedness rather than its size, because that is what
                // decides whether one side reaches `WIDEST` before the frame
                // reaches a pipe: past about thirty-two to one it cannot have
                // both, and there the shape gives way rather than the pause.
                // See `Fit::at_least_a_pipe`.
                //
                // And a box smaller than a few cells across and a line tall is
                // not a panel either: the rounding to even pixels is a whole
                // percent of a frame that small, which is arithmetic older than
                // any of this.
                let lopsided = (w / h).max(h / w);
                if lopsided > 32.0 || w < 32.0 || h < 16.0 {
                    continue;
                }
                #[allow(clippy::cast_precision_loss)]
                let (want, got) = (
                    f64::from(w) / f64::from(h),
                    f64::from(fit.width) / f64::from(fit.height),
                );
                assert!(
                    (got - want).abs() / want < 0.05,
                    "a {w}x{h} panel asked for {want:.3} and got {got:.3}"
                );
            }
        }
    }

    /// FAILS IF: sound stops reaching the player, or reaches it as a film.
    ///
    /// The line either side of this is thin and both sides matter: an audio
    /// file with an audio stream MUST play, and an audio file whose probe
    /// shows no audio stream must not — a container that lied, or a demuxer
    /// that failed, is not a player's problem to discover.
    #[test]
    fn a_sound_plays_and_says_whether_it_has_a_sleeve() {
        let path = std::path::Path::new("/x.mp3");
        let bare = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"}],
                       "format":{"format_name":"mp3","duration":"132.9"}}"#;
        let m = moving(path, "audio/mpeg", bare, None).expect("a sound plays");
        assert_eq!(m.show, Show::Sound { cover: false });
        assert_eq!(m.how, Source::Ffmpeg);
        assert!(m.inside.is_none());
        // AND IT HAS A TIMELINE, which is what makes the scrub and the two
        // seek keys work at all.
        assert_eq!(m.shape.seconds, Some(132.9));
        // AND SOMETHING TO HEAR, always: that is what this file IS.
        assert!(m.shape.sound);

        // AN ALBUM'S COVER IS A VIDEO STREAM, and here it is the one thing
        // that decides which graph gets built.
        let sleeved = r#"{"streams":[{"codec_type":"audio","codec_name":"flac"},
                                     {"codec_type":"video","codec_name":"mjpeg",
                                      "disposition":{"attached_pic":1}}],
                          "format":{"format_name":"flac","duration":"293.8"}}"#;
        let m = moving(path, "audio/flac", sleeved, None).expect("a sound plays");
        assert_eq!(m.show, Show::Sound { cover: true });

        // AND NO AUDIO STREAM IS NO PLAYER, whatever the name said.
        let silent = r#"{"streams":[{"codec_type":"video","codec_name":"mjpeg"}],
                         "format":{"format_name":"mp3","duration":"10.0"}}"#;
        assert!(moving(path, "audio/mpeg", silent, None).is_none());

        // A FILM IS STILL A FILM — the branch above must not swallow it.
        let film = r#"{"streams":[{"codec_type":"video","codec_name":"h264",
                                   "avg_frame_rate":"25/1"}],
                        "format":{"format_name":"mov,mp4","duration":"60.0"}}"#;
        let m = moving(path, "video/mp4", film, None).expect("a film plays");
        assert_eq!(m.show, Show::Film);
    }

    /// FAILS IF: a video with no readable shape starts a player anyway.
    ///
    /// Every corrupt file this system was tested against comes back from
    /// ffprobe as an error rather than a document, so this is the gate they
    /// all stop at — they never reach the player.
    #[test]
    fn a_video_whose_probe_says_nothing_does_not_move() {
        let nowhere = std::path::Path::new("/nonexistent");
        for probe in [
            "",
            "not json",
            r#"{"streams":[]}"#,
            r#"{"streams":[{"codec_type":"audio"}]}"#,
            // An album's cover is a video stream, and playing it would be a
            // player started for a still picture.
            r#"{"streams":[{"codec_type":"video","disposition":{"attached_pic":1}}]}"#,
        ] {
            assert!(
                moving(nowhere, "video/mp4", probe, None).is_none(),
                "a player was started for {probe:?}"
            );
        }
    }

    /// FAILS IF: the media type stops being the question, which is what makes
    /// "a great variety of videos" true without a list to keep up to date.
    #[test]
    fn every_moving_picture_ffmpeg_reads_is_offered_to_it() {
        let nowhere = std::path::Path::new("/nonexistent");
        let probe = r#"{"streams":[{"codec_type":"video","avg_frame_rate":"24/1"}],
                        "format":{"duration":"12.5","format_name":"mov,mp4"}}"#;
        for kind in [
            "video/mp4",
            "video/quicktime",
            "video/x-msvideo",
            "video/ogg",
            "video/webm",
            "video/x-matroska",
            "video/vnd.avi",
        ] {
            let got = moving(nowhere, kind, probe, None).expect("a video moves");
            assert_eq!(got.how, Source::Ffmpeg, "{kind}");
            assert!((got.shape.fps - 24.0).abs() < 0.01, "{kind}");
            assert_eq!(got.shape.seconds, Some(12.5), "{kind}");
        }
    }

    /// FAILS IF: the cadence stops being clamped. A container with no rate
    /// reports `0/0` or `90000/1`, and a player pacing by ninety thousand
    /// frames a second is a wake-up per eleven microseconds.
    #[test]
    fn an_absurd_cadence_is_brought_back_to_something_playable() {
        let nowhere = std::path::Path::new("/nonexistent");
        let with = |rate: &str| {
            let probe = format!(
                r#"{{"streams":[{{"codec_type":"video","avg_frame_rate":"{rate}"}}],
                     "format":{{"duration":"10","format_name":"mov,mp4"}}}}"#
            );
            moving(nowhere, "video/mp4", &probe, None).map(|m| m.shape.fps)
        };
        assert_eq!(
            with("0/0"),
            Some(30.0),
            "no rate should pace at the ceiling"
        );
        assert_eq!(with("90000/1"), Some(30.0), "an absurd rate is capped");
        assert_eq!(with("60/1"), Some(30.0), "sixty is capped to thirty");
        assert_eq!(with("1/100"), Some(30.0), "a rate below one is refused");
        // AND A REAL ONE SURVIVES: a film is 24, and capping it would be a
        // player running the picture faster than the file.
        assert_eq!(with("24/1"), Some(24.0));
    }

    /// FAILS IF: the frame's size stops being ours.
    ///
    /// Never larger than the box, never larger than the ceiling, always even,
    /// and always a whole number of steps so dragging a window's edge does not
    /// respawn per pixel.
    #[test]
    fn the_frame_is_the_size_the_panel_asked_for() {
        for (w, h) in [
            (192.0, 108.0),
            (348.0, 900.0),
            (672.0, 378.0),
            (40.0, 20.0),
            (4000.0, 4000.0),
        ] {
            let fit = Fit::of(w, h).expect("a box has a fit");
            assert_eq!(fit.width % 2, 0, "{w}x{h} is odd across");
            assert_eq!(fit.height % 2, 0, "{w}x{h} is odd down");
            assert!(
                fit.width <= WIDEST && fit.height <= WIDEST,
                "{w}x{h} past the ceiling"
            );
            assert!(fit.width >= NARROWEST && fit.height >= NARROWEST);
            assert!(
                fit.bytes() <= 4 * 1024 * 1024,
                "{w}x{h} is {} bytes",
                fit.bytes()
            );
            // AND THE FRAME IS SHAPED LIKE THE BOX, which is what keeps the
            // picture from being framed twice — once by ffmpeg's `pad` and
            // again by the door. Two per cent, which is a pixel or two at these
            // sizes and invisible; quantising the two sides SEPARATELY, which
            // is what this replaces, turned a 380-by-300 box into a 384-by-128
            // frame and the owner photographed the bands it left.
            #[allow(clippy::cast_precision_loss)]
            let (want, got) = (
                f64::from(w) / f64::from(h),
                f64::from(fit.width) / f64::from(fit.height),
            );
            assert!(
                (got - want).abs() / want < 0.02,
                "{w}x{h} asked for {want:.3} and the frame is {got:.3}                  ({}x{})",
                fit.width,
                fit.height
            );
        }
        // AND A BOX THAT IS NOT A BOX HAS NO FIT.
        for (w, h) in [(0.0, 100.0), (100.0, 0.0), (f32::NAN, 10.0), (-5.0, 10.0)] {
            assert!(Fit::of(w, h).is_none(), "{w}x{h} produced a fit");
        }
        // AND THE STEP HOLDS: a drag across the whole range crosses few sizes.
        let sizes: std::collections::BTreeSet<_> = (100..1000)
            .map(|w| Fit::of(w as f32, 400.0).expect("a fit").width)
            .collect();
        assert!(
            sizes.len() <= 8,
            "a drag would respawn {} times",
            sizes.len()
        );
    }

    /// THE TEST THAT KEEPS THE DEFECT FROM COMING BACK.
    ///
    /// FAILS IF: the height goes back to being ffmpeg's opinion. The reported
    /// defect was a frame whose height was guessed from however many bytes
    /// arrived in a read; the fix is that both numbers are literal integers in
    /// the filter, so the frame's length is arithmetic.
    #[test]
    fn the_stream_arguments_never_leave_the_size_to_the_file() {
        let path = std::path::Path::new("/x.mp4");
        let fit = Fit {
            width: 256,
            height: 144,
        };
        let args = stream_args(&asking(path, fit));
        let joined: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let all = joined.join(" ");
        assert!(all.contains("scale=256:144"), "{all}");
        assert!(all.contains("pad=256:144"), "{all}");
        // AND NEITHER DIMENSION IS LEFT TO FFMPEG. `-2` is its "work this one
        // out" marker and it is what the defect was made of; the `-1:-1` in
        // `pad` is the CENTRING offset, which is a position and not a size, so
        // the question is asked of the scale filter alone.
        let scale = all
            .split("scale=")
            .nth(1)
            .and_then(|t| t.split(',').next())
            .expect("a scale filter");
        assert!(
            !scale.contains('-'),
            "a dimension was left to ffmpeg: {scale}"
        );
        // AND SILENT BY DEFAULT: no second output at all.
        assert!(!all.contains("pulse"), "{all}");
        // THE VIDEO OUTPUT REFUSES AUDIO WHETHER OR NOT THERE IS SOUND, and
        // that is not a contradiction: `-an` belongs to the output it precedes,
        // and the picture's output never carries any. Sound is a SECOND output.
        let before_dash = all.split(" - ").next().expect("an output specifier");
        assert!(before_dash.contains("-an"), "{all}");

        // AND WITH SOUND, one child and two outputs.
        let loud = stream_args(&Stream {
            sound: Some("ronoctua-preview"),
            ..asking(path, fit)
        });
        let all = loud
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all.contains("-f pulse"), "{all}");
        // THE AUDIO OUTPUT COMES AFTER THE PICTURE'S, which is what makes it a
        // second output rather than options nobody reads.
        let (picture, sound_out) = all.split_once(" - ").expect("two outputs");
        assert!(picture.contains("rawvideo"), "{all}");
        assert!(sound_out.contains("pulse"), "{all}");
        assert!(!sound_out.contains("-an"), "{all}");

        // AND A SEEK IS AN INPUT SEEK — `-ss` before `-i`, which is the fast
        // one: 108 to 242 ms anywhere in a ten-minute film, measured.
        let sought = stream_args(&Stream {
            from: 61.5,
            ..asking(path, fit)
        });
        let ss = sought.iter().position(|a| a == "-ss").expect("a seek");
        let i = sought.iter().position(|a| a == "-i").expect("an input");
        assert!(
            ss < i,
            "the seek came after the input, which is the slow one"
        );
    }

    /// FAILS IF: a sound's arguments stop being a graph ffmpeg will accept.
    ///
    /// Two refusals are being guarded, and both are total — ffmpeg rejects the
    /// WHOLE command, both outputs, exit 234:
    ///
    /// 1. `showwaves` CONSUMES the stream it draws, so asking for a pulse
    ///    output as well without splitting first leaves nothing to map.
    /// 2. A labelled pad that is created and never mapped is the same refusal,
    ///    so a SILENT sound must not split at all.
    #[test]
    fn a_sounds_graph_splits_only_what_it_maps() {
        let path = std::path::Path::new("/x.mp3");
        let fit = Fit {
            width: 256,
            height: 144,
        };
        let said = |stream: &Stream<'_>| {
            stream_args(stream)
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        };

        // SILENT: no split, and no pad left dangling.
        let quiet = said(&Stream {
            show: Show::Sound { cover: false },
            ..asking(path, fit)
        });
        assert!(!quiet.contains("asplit"), "{quiet}");
        assert!(!quiet.contains("[heard]"), "{quiet}");
        assert!(quiet.contains("-map [picture]"), "{quiet}");

        // HEARD: split, and BOTH halves mapped.
        let loud = said(&Stream {
            show: Show::Sound { cover: false },
            sound: Some("default"),
            ..asking(path, fit)
        });
        assert!(loud.contains("asplit=2[wave][heard]"), "{loud}");
        assert!(loud.contains("-map [picture]"), "{loud}");
        assert!(loud.contains("-map [heard]"), "{loud}");
        // AND NOT THE STREAM: `0:a:0?` is the film's map, and here the filter
        // has already eaten that stream.
        assert!(!loud.contains("0:a:0?"), "{loud}");

        // A COVER IS A SECOND INPUT, and the seek belongs to the second one.
        let sought = said(&Stream {
            show: Show::Sound { cover: true },
            from: 61.5,
            ..asking(path, fit)
        });
        let words: Vec<&str> = sought.split(' ').collect();
        let inputs: Vec<usize> = words
            .iter()
            .enumerate()
            .filter(|(_, w)| **w == "-i")
            .map(|(n, _)| n)
            .collect();
        assert_eq!(inputs.len(), 2, "a cover needs two inputs: {sought}");
        let ss = words.iter().position(|w| *w == "-ss").expect("a seek");
        assert!(
            inputs[0] < ss && ss < inputs[1],
            "the seek must fall between the two inputs, or the cover is sought \
             and the graph waits for a frame that is not there: {sought}"
        );
        // AND THE COVER IS THE ONE THAT IS LOOPED, so a still frame lasts as
        // long as the sound does.
        assert!(sought.contains("loop=loop=-1:size=1"), "{sought}");
        assert!(sought.contains("[0:v]"), "{sought}");
        assert!(sought.contains("[1:a]"), "{sought}");

        // WITHOUT A COVER THERE IS ONE INPUT AND NO OVERLAY.
        let bare = said(&Stream {
            show: Show::Sound { cover: false },
            from: 61.5,
            ..asking(path, fit)
        });
        assert_eq!(bare.matches(" -i ").count(), 1, "{bare}");
        assert!(!bare.contains("overlay"), "{bare}");
        // AND THE WAVES TAKE THE WHOLE FRAME.
        assert!(bare.contains("showwaves=s=256x144"), "{bare}");

        // THE SIZE IS OURS IN BOTH GRAPHS — the same claim the film makes.
        for all in [&quiet, &sought] {
            assert!(all.contains("showwaves=s=256x"), "{all}");
        }
        assert!(sought.contains("scale=256:144"), "{sought}");
        assert!(sought.contains("pad=256:144"), "{sought}");
    }

    /// THE PROOF, AGAINST REAL FFMPEG, THAT A SOUND BECOMES FRAMES.
    ///
    /// A drawn picture has three ways to be wrong and only one of them is
    /// visible in the argument list, so this counts BYTES: the graph can stall
    /// with nothing on stdout, it can produce a single frame and stop, or it
    /// can hand back a frame that is not the size the panel asked for. All
    /// three are one assertion here.
    ///
    /// FAILS IF: the cover is put on the SEEKED input. Measured before this was
    /// written — `-ss 150` with the artwork in the graph ran for **143 875 ms
    /// and produced no frame at all**, because an `attached_pic` is one frame
    /// at timestamp zero and `overlay ... shortest=1` waits for a second one
    /// that a seek has already skipped past. The seeked case below is that
    /// regression, and it is why the cover gets an input of its own.
    #[test]
    fn a_sound_is_drawn_at_the_size_we_asked_for() {
        let Ok(ffmpeg) = super::ffmpeg() else {
            // The environment does not name it — the same refusal the still
            // preview makes, and not a failure of this test.
            return;
        };
        let dir = std::env::temp_dir().join(format!("ronoctua-snd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let make = |name: &str, args: &[&str]| {
            let out = dir.join(name);
            let ok = std::process::Command::new(&ffmpeg)
                .args(["-v", "error", "-y"])
                .args(args)
                .arg(&out)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            ok.then_some(out)
        };
        // TEN SECONDS, so a seek has somewhere to land.
        let plain = make(
            "plain.mp3",
            &[
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=10",
                "-c:a",
                "libmp3lame",
            ],
        );
        // AND THE COVER IS MUXED IN A SECOND PASS, from a picture that is
        // already one frame long.
        //
        // Doing it in one — `-map 0:a -map 1:v -frames:v 1` — truncates the
        // WHOLE file: the frame limit ends the output, and the sound came out
        // thirty milliseconds long. This test found that as "two frames of
        // four" and it looked exactly like a stalling graph.
        let sleeve = make(
            "sleeve.jpg",
            &[
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=64x64:d=1",
                "-frames:v",
                "1",
            ],
        );
        let (Some(plain), Some(sleeve)) = (plain, sleeve) else {
            return;
        };
        let covered = make(
            "covered.mp3",
            &[
                "-i",
                plain.to_str().expect("a path"),
                "-i",
                sleeve.to_str().expect("a path"),
                "-map",
                "0:a",
                "-map",
                "1:v",
                "-c:a",
                "copy",
                "-c:v",
                "mjpeg",
                "-disposition:v",
                "attached_pic",
            ],
        );
        let Some(covered) = covered else {
            return;
        };

        let fit = Fit {
            width: 256,
            height: 144,
        };
        const WANTED: usize = 4;
        for (path, cover, from) in [
            (&plain, false, 0.0),
            (&covered, true, 0.0),
            // THE ONE THAT USED TO HANG.
            (&covered, true, 6.0),
            (&plain, false, 6.0),
        ] {
            let mut args = stream_args(&Stream {
                show: Show::Sound { cover },
                fps: 8.0,
                from,
                background: "#00000000",
                ..asking(path, fit)
            });
            // THE FRAME COUNT GOES BEFORE THE OUTPUT SPECIFIER, for the reason
            // written out in the film's twin: an option applies to the output
            // that FOLLOWS it.
            let dash = args
                .iter()
                .position(|a| a == "-f")
                .expect("an output format");
            args.splice(
                dash..dash,
                [
                    std::ffi::OsString::from("-frames:v"),
                    std::ffi::OsString::from(WANTED.to_string()),
                ],
            );
            let out = std::process::Command::new(&ffmpeg)
                .args(&args)
                .output()
                .expect("ffmpeg ran");
            assert!(
                out.status.success(),
                "cover={cover} from={from}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(
                out.stdout.len(),
                WANTED * fit.bytes(),
                "cover={cover} from={from}: {} frames' worth came out, not {WANTED}",
                out.stdout.len() as f64 / fit.bytes() as f64
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE PROOF, AGAINST REAL FFMPEG, THAT THE FRAME IS THE SIZE WE ASKED FOR.
    ///
    /// Three shapes built here rather than carried in: landscape,
    /// portrait-by-rotation (the phone case — landscape frames plus a matrix
    /// saying "rotate 90") and portrait-by-encoding. All three must come out
    /// of the pipe at exactly the size the panel asked for.
    ///
    /// FAILS IF: the filter goes back to letting ffmpeg choose a dimension.
    /// Measured before this was written: the phone case reports 640x360 to
    /// ffprobe and RENDERS 256x456, so scaling to the reported shape squashes
    /// the picture and makes every frame the wrong length — the defect this
    /// replaces, reintroduced by its own fix.
    #[test]
    fn a_rotated_video_is_the_size_we_asked_for() {
        let Ok(ffmpeg) = super::ffmpeg() else {
            // The environment does not name it — the same refusal the still
            // preview makes, and not a failure of this test.
            return;
        };
        let dir = std::env::temp_dir().join(format!("ronoctua-rot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let make = |name: &str, args: &[&str]| {
            let out = dir.join(name);
            let ok = std::process::Command::new(&ffmpeg)
                .args(["-v", "error", "-y"])
                .args(args)
                .arg(&out)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            ok.then_some(out)
        };
        let flat = make(
            "flat.mp4",
            &[
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=10",
                "-t",
                "1",
                "-pix_fmt",
                "yuv420p",
            ],
        );
        let Some(flat) = flat else { return };
        let phone = make(
            "phone.mp4",
            &[
                "-display_rotation",
                "90",
                "-i",
                flat.to_str().unwrap(),
                "-c",
                "copy",
            ],
        );
        let tall = make(
            "tall.mp4",
            &[
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=360x640:rate=10",
                "-t",
                "1",
                "-pix_fmt",
                "yuv420p",
            ],
        );

        let fit = Fit {
            width: 256,
            height: 144,
        };
        for made in [Some(flat), phone, tall].into_iter().flatten() {
            // ONE FRAME, and the option goes BEFORE the output specifier —
            // ffmpeg reads options for the output that follows them, so
            // appending it after the `-` is a silent no-op. That mistake is
            // how this test first read ten frames and called the size wrong.
            // ONE FRAME, and the option goes BEFORE the output specifier —
            // ffmpeg reads options for the output that follows them, so
            // putting it after the `-` is a silent no-op. That is how this
            // test first read ten frames and called the size wrong, and it is
            // the same mistake the arguments themselves carried.
            let mut args = stream_args(&Stream {
                fps: 10.0,
                background: "black",
                ..asking(&made, fit)
            });
            let dash = args
                .iter()
                .position(|a| a == "-f")
                .expect("an output format");
            args.splice(
                dash..dash,
                [
                    std::ffi::OsString::from("-frames:v"),
                    std::ffi::OsString::from("1"),
                ],
            );
            let out = std::process::Command::new(&ffmpeg)
                .args(&args)
                .stderr(std::process::Stdio::null())
                .output()
                .expect("ffmpeg runs");
            assert_eq!(
                out.stdout.len(),
                fit.bytes(),
                "{made:?} produced {} bytes and the panel asked for {}",
                out.stdout.len(),
                fit.bytes()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a real GIF of several frames does not read as moving, or one
    /// of a single frame does.
    ///
    /// Built HERE with the `image` crate rather than described, because the
    /// question `gif_frames` answers is about a real decoder reading a real
    /// file — and the lazy two-frame count is an optimisation that a
    /// hand-written fixture would not exercise.
    #[test]
    fn a_gif_of_several_frames_moves_and_one_of_a_single_frame_does_not() {
        use image::{Delay, Frame, RgbaImage, codecs::gif::GifEncoder};

        let dir = std::env::temp_dir().join(format!("ronoctua-gif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let paint =
            |rgb: [u8; 3]| RgbaImage::from_pixel(8, 6, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
        let write = |name: &str, n: usize| {
            let path = dir.join(name);
            let file = std::fs::File::create(&path).expect("create");
            let mut out = GifEncoder::new(file);
            out.set_repeat(image::codecs::gif::Repeat::Infinite)
                .expect("repeat");
            for k in 0..n {
                #[allow(clippy::cast_possible_truncation)]
                let shade = (k * 60) as u8;
                out.encode_frame(Frame::from_parts(
                    paint([shade, 40, 200 - shade]),
                    0,
                    0,
                    Delay::from_numer_denom_ms(80, 1),
                ))
                .expect("frame");
            }
            drop(out);
            path
        };

        let many = write("many.gif", 4);
        let got = moving(&many, "image/gif", "", None).expect("four frames do not move");
        assert_eq!(got.how, Source::InProcess);
        // ITS OWN CADENCE, and not a default: 80 ms is 12.5 a second.
        assert!(
            (got.shape.fps - 12.5).abs() < 0.5,
            "the file's own cadence was not read: {}",
            got.shape.fps
        );
        // AND NO TIMELINE, which is what stops anything trying to seek into
        // frames that are inter-dependent.
        assert!(got.shape.seconds.is_none());
        assert!(!got.shape.sound);

        // AND ONE FRAME IS A PHOTOGRAPH. Starting a thread and a child process
        // for it is a cost with nothing on the other side.
        let one = write("one.gif", 1);
        assert!(
            moving(&one, "image/gif", "", None).is_none(),
            "a one-frame GIF started a player"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a picture with a film in it stops being found, whichever way
    /// the maker put it there.
    ///
    /// Built HERE, out of a real JPEG and a real MP4, because the question is
    /// about BYTES and a hand-written fixture would only exercise the parser
    /// against itself. The owner's own Pixel files stay where they are — they
    /// are photographs of his, and a store path never forgets.
    ///
    /// The three doors have to agree on the same stretch: the XMP Google
    /// writes, the `ftyp` scan that covers Samsung and anything else, and a
    /// file beside for Apple.
    #[test]
    fn a_motion_photo_is_found_however_the_maker_left_it() {
        let Ok(ffmpeg) = ffmpeg() else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("ronoctua-mp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");

        // A REAL FILM, made by ffmpeg — the same way the rotation test does it.
        let film = dir.join("clip.mp4");
        let made = std::process::Command::new(&ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=160x120:rate=10",
            ])
            .args(["-t", "1", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-y"])
            .arg(&film)
            .status();
        if !made.is_ok_and(|s| s.success()) {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let film_bytes = std::fs::read(&film).expect("the clip");

        // AND A REAL PICTURE, out of the `image` crate.
        let mut photo = Vec::new();
        image::RgbImage::from_pixel(64, 48, image::Rgb([9, 90, 180]))
            .write_to(
                &mut std::io::Cursor::new(&mut photo),
                image::ImageFormat::Jpeg,
            )
            .expect("a jpeg");

        // ONE: THE XMP GOOGLE WRITES. The item's length counted back from the
        // end of the file is the offset — which is what the owner's own two
        // files do, to the byte.
        let xmp = format!(
            "<x:xmpmeta><rdf:RDF><rdf:Description GCamera:MotionPhoto=\"1\">\
             <Container:Directory><Container:Item Item:Semantic=\"Primary\" \
             Item:Mime=\"image/jpeg\"/><Container:Item Item:Mime=\"video/mp4\" \
             Item:Semantic=\"MotionPhoto\" Item:Length=\"{}\" Item:Padding=\"0\"/>\
             </Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>",
            film_bytes.len()
        );
        let with_xmp = dir.join("google.jpg");
        let mut body = photo.clone();
        // THE XMP GOES IN THE FIRST WINDOW, where the makers put it: measured
        // at 13 KB and 41 KB into the owner's two files.
        body.extend_from_slice(xmp.as_bytes());
        body.extend_from_slice(&film_bytes);
        std::fs::write(&with_xmp, &body).expect("write");

        // TWO: NOBODY DECLARED ANYTHING — the general route, which is what
        // covers Samsung on a machine with no Samsung file to test against.
        let bare = dir.join("bare.jpg");
        let mut body = photo.clone();
        body.extend_from_slice(&film_bytes);
        std::fs::write(&bare, &body).expect("write");

        let want = film_bytes.len() as u64;
        for (name, path) in [("xmp", &with_xmp), ("scan", &bare)] {
            let got = moving(path, "image/jpeg", "", None)
                .unwrap_or_else(|| panic!("{name}: a picture with a film in it was not found"));
            assert_eq!(got.how, Source::Ffmpeg, "{name}");
            let size = std::fs::metadata(path).expect("size").len();
            assert_eq!(
                got.inside,
                Some(Inside::Range {
                    at: size - want,
                    to: size
                }),
                "{name}: the stretch is not where the film was put"
            );
            // AND THE PROBE READ IT: a shape only exists if ffprobe found a
            // video stream inside those bytes.
            assert!(got.shape.fps > 0.0, "{name}");
        }

        // AND A PLAIN PHOTOGRAPH IS STILL A PHOTOGRAPH.
        let plain = dir.join("plain.jpg");
        std::fs::write(&plain, &photo).expect("write");
        assert!(
            moving(&plain, "image/jpeg", "", None).is_none(),
            "a photograph with nothing after it started a player"
        );

        // AND BYTES THAT ONLY LOOK LIKE A BOX ARE REFUSED — by ffprobe, in
        // about twenty milliseconds, which is what the corrupt files already
        // do.
        let liar = dir.join("liar.jpg");
        let mut body = photo.clone();
        body.extend_from_slice(&[0, 0, 0, 24]);
        body.extend_from_slice(b"ftypisom");
        body.extend_from_slice(&[0u8; 64]);
        std::fs::write(&liar, &body).expect("write");
        assert!(
            moving(&liar, "image/jpeg", "", None).is_none(),
            "four letters in a photograph were taken for a film"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the film inside a picture is read by copying it out.
    ///
    /// A motion photo's video is megabytes, and writing it to a temporary file
    /// to watch three seconds of it would be the cost this whole route exists
    /// to avoid. `subfile` is ffmpeg's own protocol for a stretch of a file —
    /// measured at 107 ms on the owner's own 5,9 MB photograph.
    #[test]
    fn the_film_inside_a_picture_is_read_where_it_lies() {
        let path = std::path::Path::new("/x.jpg");
        let fit = Fit {
            width: 256,
            height: 144,
        };
        let inside = Inside::Range {
            at: 1_607_058,
            to: 5_901_053,
        };
        let args = stream_args(&Stream {
            inside: Some(&inside),
            background: "#00000000",
            ..asking(path, fit)
        });
        let all = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all.contains("subfile,,start,1607058,end,5901053,,:/x.jpg"),
            "the stretch is not what ffmpeg was pointed at: {all}"
        );
        // AND THE INPUT IS STILL THE INPUT: the URL is the value of `-i`, so
        // everything after it goes on describing the OUTPUT.
        let i = args.iter().position(|a| a == "-i").expect("an input");
        assert!(args[i + 1].to_string_lossy().starts_with("subfile,"));

        // AND A FILM BESIDE IS OPENED AS ITSELF — Apple's half, which is an
        // ordinary file and needs no protocol at all.
        let beside = Inside::Beside(std::path::PathBuf::from("/IMG_1.MOV"));
        let args = stream_args(&Stream {
            inside: Some(&beside),
            background: "#00000000",
            ..asking(path, fit)
        });
        let i = args.iter().position(|a| a == "-i").expect("an input");
        assert_eq!(args[i + 1].to_string_lossy(), "/IMG_1.MOV");

        // AND AN ORDINARY FILM IS UNTOUCHED.
        let args = stream_args(&Stream {
            background: "#00000000",
            ..asking(path, fit)
        });
        let i = args.iter().position(|a| a == "-i").expect("an input");
        assert_eq!(args[i + 1].to_string_lossy(), "/x.jpg");
    }

    /// FAILS IF: a `Moving` stops carrying what the player paces by.
    #[test]
    fn a_gif_has_a_cadence_and_no_timeline() {
        // A GIF's frames are inter-dependent, so seeking means decoding from
        // the first — 707 ms on the owner's own file. No duration is what
        // draws that line, and it needs no branch anywhere else.
        let m = Moving {
            how: Source::InProcess,
            show: Show::Film,
            shape: crate::facts::Shape {
                fps: 11.1,
                seconds: None,
                sound: false,
            },
            inside: None,
        };
        assert!(m.shape.seconds.is_none());
        assert!(m.shape.fps > 0.0);
    }
}
