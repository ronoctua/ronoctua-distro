//! What is INSIDE a file — pixels for the preview, facts for the info panel.
//!
//! # Why a border of its own
//!
//! The same reason `ronoctua-files` is the only crate that touches the
//! filesystem and `ronoctua-niri` the only one that speaks `niri-ipc`: one
//! border, one crate. This one is the border with a file's CONTENT, and it is
//! separate from `ronoctua-files` for a reason that is not tidiness —
//! `ronoctua-files` builds `ronoctua-files-root`, `nix/rust.nix` extracts that
//! binary so what runs as root does not carry the desktop, and `files-vm`
//! asserts the closure. An image decoder in that crate would be an image
//! decoder in a root binary.
//!
//! # It does not draw, does not translate, does not run a thread
//!
//! Everything here is synchronous and answers one question about one file.
//! The caller owns concurrency — the shell runs this on the long-lived named
//! thread `ronoctua_shell::apps::painter` already uses, and for the same
//! reason: `systemd-run --scope` taught this project once that waiting inside
//! the loop that draws is how the footer stops responding.
//!
//! A label comes out as a text KEY plus arguments, and whoever holds the
//! bundle turns it into a sentence (invariant 8).
//!
//! # There is exactly one place [`Rgba`] is born
//!
//! Every external tool hands back a **PNG on stdout**, and that PNG is then
//! decoded by the `image` crate like any other. ffmpeg never decides the pixel
//! layout, pdftoppm never decides the pixel layout — they hand over a picture
//! and the ceilings, the demultiply and the never-enlarge rule all apply in
//! one place. A second path that produced pixels its own way would be a second
//! place for those three rules to be wrong.

#![forbid(unsafe_code)]

pub mod facts;
pub mod motion;
pub mod pixels;
pub mod text;
pub mod tools;

/// Pixels ready for the GPU, in **straight** (not premultiplied) RGBA8.
///
/// # Why it is declared here and not imported
///
/// So this crate does not depend on `ronoctua-render`, which drags in the whole
/// of wgpu. It is the same boundary — and the same crossing —
/// `ronoctua_clipboard::image` and `ronoctua_notify::icon` already have, and
/// the shell splices it in `ronoctua_shell::apps::for_renderer`, which is
/// field for field the same struct.
///
/// Straight and not premultiplied is not a detail: `tiny-skia` delivers
/// premultiplied, the pipeline expects straight, and the symptom of getting it
/// wrong is "the images look dirty", which nobody connects to alpha. See
/// [`pixels::from_svg`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes, RGBA8, straight alpha.
    pub pixels: Vec<u8>,
}

/// Why there is no picture.
///
/// # Why a refusal is a VALUE, and never an absence
///
/// Because a blank panel and a refused panel look identical on screen, and
/// only one of them has a sentence to say. `None` on its own is "who knows";
/// this is "no, and here is why", which is the difference between a file
/// manager that looks broken and one that is telling you the truth.
///
/// This is a safety net, not a profile difference: ffmpeg, exiftool and
/// poppler are installed on every machine this distro builds. What is left for
/// these variants is a stripped container, a broken environment, a test that
/// deliberately unsets a variable — and a file that really has no picture in
/// it, which is most files. Nobody should read this as dead code and delete
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing in the table serves this type AND there is no text in it
    /// either — so there is nothing to show, and it says so.
    ///
    /// # It says exactly what a DIRECTORY says, and that is the point
    ///
    /// It used to say "there is no picture inside this kind of file", and the
    /// owner reported it from the screen against a markdown file. The sentence
    /// was wrong from inside: this panel is not an image panel, it is the
    /// panel that shows what is in a file, and a preview that answers about
    /// pictures is answering a question nobody asked. From where somebody is
    /// looking, a folder and a `.zip` are the same fact — there is nothing to
    /// show — and they now say the same thing.
    Nothing,
    /// It is typed as text and the bytes are not: a NUL byte, or a run that
    /// is not UTF-8 and was not merely cut by the ceiling.
    ///
    /// The alternative was `from_utf8_lossy`, and it is worse: a panel of
    /// replacement characters is a lie about the file, while this is the
    /// truth about it.
    NotText,
    /// It is text and there is none of it.
    Empty,
    /// It could not be opened for reading — permission, most often.
    Unreadable,
    /// The program this content needs is not named by the environment. Carries
    /// the VARIABLE, the way `ronoctua_files::act::Trouble::Missing` does —
    /// the name of the program comes out of it by [`tools::program_of`].
    Missing(&'static str),
    /// Past a ceiling: [`pixels::PIXEL_CEILING`] for a raster,
    /// [`pixels::VECTOR_CEILING`] for an SVG, [`tools::STDOUT_CEILING`] for
    /// what a child wrote. The number is what was measured.
    TooBig(u64),
    /// It ran, it finished, and what came back was not a picture.
    Failed,
    /// It ran past [`tools::PATIENCE`] and was killed. The worst measured case
    /// on the owner's machine was 0.21 s, so this one means something is
    /// wrong, not that the file is large.
    TimedOut,
}

impl Refusal {
    /// The text key for this refusal.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Nothing => key::NO_PREVIEW,
            Self::NotText => key::PREVIEW_NOT_TEXT,
            Self::Empty => key::PREVIEW_EMPTY,
            Self::Unreadable => key::PREVIEW_UNREADABLE,
            Self::Missing(_) => key::PREVIEW_MISSING,
            Self::TooBig(_) => key::PREVIEW_TOO_BIG,
            Self::Failed => key::PREVIEW_FAILED,
            Self::TimedOut => key::PREVIEW_SLOW,
        }
    }

    /// The key's arguments, by name.
    #[must_use]
    pub fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Nothing | Self::Failed | Self::NotText | Self::Empty | Self::Unreadable => {
                Vec::new()
            }
            Self::Missing(variable) => {
                vec![("program", tools::program_of(variable))]
            }
            Self::TooBig(amount) => vec![("amount", amount.to_string())],
            Self::TimedOut => vec![("seconds", tools::PATIENCE.as_secs().to_string())],
        }
    }
}

/// The answer to "show me this file".
///
/// It carries BOTH the pixels and the reason there are none, because the two
/// are not exclusive in the way one first assumes: a success has no reason and
/// a refusal has no pixels, but the host caches this whole value either way.
/// `rgba: None` is an ANSWER — "this file has no picture" — and not the
/// absence of one, which is the rule `ronoctua_shell::surface`'s `icons` map
/// already learned: without telling the two apart, a file with no preview is
/// asked for again on every frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// The pixels, when there are any.
    pub rgba: Option<Rgba>,
    /// Why there are none, when there are none.
    pub why: Option<Refusal>,
}

impl Made {
    /// It worked.
    #[must_use]
    pub const fn of(rgba: Rgba) -> Self {
        Self {
            rgba: Some(rgba),
            why: None,
        }
    }

    /// It did not, and here is the sentence.
    #[must_use]
    pub const fn refused(why: Refusal) -> Self {
        Self {
            rgba: None,
            why: Some(why),
        }
    }

    /// NOTHING WAS ASKED, which is neither a picture nor a refusal.
    ///
    /// For a file the caller decided not to decode at all — a film, whose
    /// still picture would be a whole ffmpeg run for a frame the player
    /// replaces 84 milliseconds later. A `Refusal` here would put a sentence in
    /// the panel about a file that has plenty to show.
    #[must_use]
    pub const fn unasked() -> Self {
        Self {
            rgba: None,
            why: None,
        }
    }

    /// Whether there is something to upload.
    #[must_use]
    pub const fn drawable(&self) -> bool {
        self.rgba.is_some()
    }
}

/// The type to act on, given what the NAME says and what the CONTENT says.
///
/// # Why this is not simply "the content wins"
///
/// Because a name can be more specific than a sniff and still be right. A
/// Pixel's `.dng` sniffs as `image/tiff` — it IS a TIFF, that is how the format
/// is built — and taking the sniff would send it to a TIFF decoder, which
/// reads the RAW planes and fails. The name says `image/x-adobe-dng`, the
/// database says that descends from `image/tiff`, and the two therefore AGREE:
/// the name is just the finer answer.
///
/// It cost a real refusal to find, on the owner's own photographs, and the
/// symptom was a `.dng` showing "this file could not be read as a picture"
/// while `ffmpeg` read it in 0.2 s.
///
/// The sniff wins only when the two genuinely disagree — a PNG called `.txt`,
/// which is the case `ronoctua_files::mime::examined` exists to catch.
#[must_use]
pub fn settled<'a>(named: &'a str, sniffed: Option<&'a str>) -> &'a str {
    let Some(found) = sniffed else {
        return named;
    };
    if ronoctua_files::mime::is_a(named, found) {
        named
    } else {
        found
    }
}

/// The sizes a preview is ever asked for, in pixels of the longer side.
///
/// # Why THREE steps and not the panel's exact width
///
/// Because the panel's width changes while a window is being dragged, and a
/// key that carried it would decode the file again on every `configure` —
/// sixty times a second, each one a child process. Rounding up to the next
/// step means a drag re-decodes at most twice.
///
/// The ceiling is 512 and the arithmetic is the texture cache's:
/// `ronoctua_render::image` keeps 39 textures, and 512 px of RGBA is 1 MiB
/// each — 41 MiB. The same 39 at 1200 px would be 224 MiB of footer.
///
/// It is NOT a token, and that is deliberate: what the panel *asks for* is a
/// visual value and comes from `space.files.panelPixels` (invariant 9); what
/// is here is the quantisation that keeps the cache honest.
pub const STEPS: [u32; 3] = [128, 256, 512];

/// The step a request of `target` pixels is served at.
///
/// Rounds UP, so a panel never draws a preview it has to enlarge, and clamps
/// at the last step.
#[must_use]
pub fn step(target: u32) -> u32 {
    STEPS
        .into_iter()
        .find(|s| *s >= target)
        .unwrap_or(STEPS[STEPS.len() - 1])
}

/// What identifies one decoded preview in the host's cache.
///
/// # Why mtime and size, and never the dimensions
///
/// `ronoctua_shell::surface` records the defect that taught this: the
/// clipboard's preview key had to carry a VERSION rather than the dimensions,
/// because two captures of the same monitor have the same size and are
/// different images. A file has no version counter, so `mtime` and `bytes`
/// together are it — the pair changes whenever the content does, and neither
/// alone is enough (an editor that rewrites in place keeps the size; `touch`
/// keeps the content).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stamp {
    /// The file, absolute.
    pub path: std::path::PathBuf,
    /// `st_mtime`, in seconds since the epoch.
    pub mtime: i64,
    /// The size in bytes.
    pub bytes: u64,
    /// The step it was decoded at — already through [`step`].
    pub target: u32,
}

impl Stamp {
    /// The stamp for a file at a requested size, with the size quantised.
    #[must_use]
    pub fn new(path: std::path::PathBuf, mtime: i64, bytes: u64, target: u32) -> Self {
        Self {
            path,
            mtime,
            bytes,
            target: step(target),
        }
    }

    /// The cache key, as the texture map wants it.
    ///
    /// The `prev:` prefix is what keeps it out of the way of the clipboard's
    /// `clip:<version>` and the icons' bare `Icon=` name, which share the same
    /// map.
    #[must_use]
    pub fn key(&self) -> String {
        format!(
            "prev:{}:{}:{}:{}",
            self.path.display(),
            self.mtime,
            self.bytes,
            self.target
        )
    }
}

/// The text keys this crate names.
///
/// # Why they are literals here and not `ronoctua_i18n::key::…`
///
/// Because the generated constants only exist for keys that are already in
/// `assets/i18n/*.ftl`, and the wave that adds them there is not this one. The
/// moment they land, every constant in this module becomes a re-export of the
/// generated one and this block goes away.
///
/// Until then the guarantee invariant 8 usually gets from the compiler has to
/// come from a check: `inspect-keys-exist` reads this file's constants and
/// asserts every one of them is in both bundles. A key that is in one language
/// only already takes the build down; a key that is in NEITHER is what this
/// module can currently produce, and it is the one hole worth naming out loud.
pub mod key {
    /// There is nothing to show for this file.
    ///
    /// THE SAME KEY THE FILE MANAGER USES FOR A DIRECTORY, and that is
    /// deliberate: a folder and a file with neither picture nor text are the
    /// same fact from where somebody is looking. Two sentences for one fact is
    /// what the old `files-preview-none` was, and it said "picture" in a panel
    /// that shows content.
    pub const NO_PREVIEW: &str = "files-no-preview";
    /// It says it is text and the bytes are not.
    pub const PREVIEW_NOT_TEXT: &str = "files-preview-not-text";
    /// It is text and there is none of it.
    pub const PREVIEW_EMPTY: &str = "files-preview-empty";
    /// It could not be opened for reading.
    pub const PREVIEW_UNREADABLE: &str = "files-preview-unreadable";
    /// `{ $program }` is not installed.
    pub const PREVIEW_MISSING: &str = "files-preview-missing";
    /// Past a ceiling; `{ $amount }` is what was measured.
    pub const PREVIEW_TOO_BIG: &str = "files-preview-too-big";
    /// It ran and gave back something that is not a picture.
    pub const PREVIEW_FAILED: &str = "files-preview-failed";
    /// It ran past `{ $seconds }` seconds and was stopped.
    pub const PREVIEW_SLOW: &str = "files-preview-slow";

    /// Label: how many pixels across and down.
    pub const FACT_DIMENSIONS: &str = "files-fact-dimensions";
    /// Label: how long it plays.
    pub const FACT_DURATION: &str = "files-fact-duration";
    /// Label: the wrapper the streams sit in.
    pub const FACT_CONTAINER: &str = "files-fact-container";
    /// Label: what encoded the picture.
    pub const FACT_VIDEO: &str = "files-fact-video";
    /// A photograph that carries a film inside it.
    pub const FACT_MOTION: &str = "files-fact-motion";
    /// Label: what encoded the sound.
    pub const FACT_AUDIO: &str = "files-fact-audio";
    /// Label: frames per second.
    pub const FACT_RATE: &str = "files-fact-rate";
    /// Label: samples per second.
    pub const FACT_SAMPLING: &str = "files-fact-sampling";
    /// Label: how many sound channels.
    pub const FACT_CHANNELS: &str = "files-fact-channels";
    /// Label: bits per second.
    pub const FACT_BITRATE: &str = "files-fact-bitrate";
    /// Label: the title written inside the file.
    pub const FACT_TITLE: &str = "files-fact-title";
    /// Label: who made it, as the file says.
    pub const FACT_ARTIST: &str = "files-fact-artist";
    /// Label: which album.
    pub const FACT_ALBUM: &str = "files-fact-album";
    /// Label: which camera.
    pub const FACT_CAMERA: &str = "files-fact-camera";
    /// Label: which lens.
    pub const FACT_LENS: &str = "files-fact-lens";
    /// Label: when the shutter opened.
    pub const FACT_TAKEN: &str = "files-fact-taken";
    /// Label: the film speed.
    pub const FACT_ISO: &str = "files-fact-iso";
    /// Label: how long the shutter was open.
    pub const FACT_SHUTTER: &str = "files-fact-shutter";
    /// Label: how wide the aperture was.
    pub const FACT_APERTURE: &str = "files-fact-aperture";
    /// Label: the focal length.
    pub const FACT_FOCAL: &str = "files-fact-focal";
    /// Label: where it was made.
    pub const FACT_PLACE: &str = "files-fact-place";
    /// Label: how many pages.
    pub const FACT_PAGES: &str = "files-fact-pages";

    /// Value: `{ $width } × { $height }`.
    pub const VALUE_PIXELS: &str = "files-value-pixels";
    /// Value: `{ $fps } fps`.
    pub const VALUE_FPS: &str = "files-value-fps";
    /// Value: `{ $khz } kHz`.
    pub const VALUE_HERTZ: &str = "files-value-hertz";
    /// Value: mono, stereo, or `{ $n }` channels — a Fluent selector, because
    /// which numbers get a word of their own is the LANGUAGE's business.
    pub const VALUE_CHANNELS: &str = "files-value-channels";
    /// Value: `{ $kbps } kbit/s`.
    pub const VALUE_KBPS: &str = "files-value-kbps";
    /// Value: `{ $mm } mm`.
    pub const VALUE_MILLIMETRES: &str = "files-value-millimetres";
    /// Value: `{ $s } s` — an exposure of a second or more.
    pub const VALUE_SECONDS: &str = "files-value-seconds";
    /// Value: `{ $lat }, { $lon }` in signed decimal degrees.
    pub const VALUE_COORDINATES: &str = "files-value-coordinates";
    /// Value: this file says where it was made, and the coordinates are not
    /// being shown.
    pub const VALUE_PLACE_WITHHELD: &str = "files-value-place-withheld";
    /// Value: one page, or `{ $n }` pages.
    pub const VALUE_PAGES: &str = "files-value-pages";

    /// Every key this crate can produce, for the check that keeps them honest.
    pub const ALL: [&str; 39] = [
        NO_PREVIEW,
        PREVIEW_NOT_TEXT,
        PREVIEW_EMPTY,
        PREVIEW_UNREADABLE,
        PREVIEW_MISSING,
        PREVIEW_TOO_BIG,
        PREVIEW_FAILED,
        PREVIEW_SLOW,
        FACT_DIMENSIONS,
        FACT_DURATION,
        FACT_CONTAINER,
        FACT_VIDEO,
        FACT_AUDIO,
        FACT_RATE,
        FACT_SAMPLING,
        FACT_CHANNELS,
        FACT_BITRATE,
        FACT_TITLE,
        FACT_ARTIST,
        FACT_ALBUM,
        FACT_CAMERA,
        FACT_LENS,
        FACT_TAKEN,
        FACT_ISO,
        FACT_SHUTTER,
        FACT_APERTURE,
        FACT_FOCAL,
        FACT_PLACE,
        FACT_PAGES,
        VALUE_PIXELS,
        VALUE_FPS,
        VALUE_HERTZ,
        VALUE_CHANNELS,
        VALUE_KBPS,
        VALUE_MILLIMETRES,
        VALUE_SECONDS,
        VALUE_COORDINATES,
        VALUE_PLACE_WITHHELD,
        VALUE_PAGES,
    ];
}

#[cfg(test)]
mod tests {
    use super::{Made, Refusal, STEPS, Stamp, key, step};

    #[test]
    fn every_refusal_carries_a_key() {
        // A refusal with no sentence is a blank panel with extra steps. The
        // list is written out rather than derived so a NEW variant fails here
        // instead of silently arriving keyless.
        let all = [
            Refusal::Nothing,
            Refusal::NotText,
            Refusal::Empty,
            Refusal::Unreadable,
            Refusal::Missing("RONOCTUA_FFMPEG"),
            Refusal::TooBig(20_000_001),
            Refusal::Failed,
            Refusal::TimedOut,
        ];
        for r in &all {
            assert!(!r.key().is_empty(), "{r:?} came with no key");
            assert!(
                key::ALL.contains(&r.key()),
                "{r:?} uses a key outside key::ALL"
            );
        }
    }

    #[test]
    fn a_missing_tool_names_the_program_and_not_the_variable() {
        // "RONOCTUA_FFMPEG is not installed" is a sentence about our build
        // system. "ffmpeg is not installed" is a sentence about the machine,
        // and it is the one a person can act on.
        let r = Refusal::Missing("RONOCTUA_FFMPEG");
        assert_eq!(r.args(), vec![("program", "ffmpeg".to_owned())]);
    }

    #[test]
    fn the_key_registry_has_no_repeats() {
        let mut seen = key::ALL.to_vec();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "two identical keys in key::ALL");
        for k in key::ALL {
            assert!(k.starts_with("files-"), "{k} escapes the module's prefix");
        }
    }

    #[test]
    fn the_target_rounds_up_and_stops_at_the_last_step() {
        assert_eq!(step(1), 128);
        assert_eq!(step(128), 128);
        assert_eq!(step(129), 256);
        assert_eq!(step(512), 512);
        // A PANEL WIDER THAN THE LAST STEP still gets the last step: what
        // grows past it is the texture, and 39 of them is the budget.
        assert_eq!(step(1200), *STEPS.last().unwrap());
    }

    #[test]
    fn the_stamp_quantises_and_says_what_changed() {
        let a = Stamp::new("/tmp/a.png".into(), 100, 10, 200);
        assert_eq!(a.target, 256, "the request has to rise to the step");
        assert_eq!(a.key(), "prev:/tmp/a.png:100:10:256");
        // SAME SIZE, NEW CONTENT — the case the clipboard's version counter
        // exists for. The mtime is what tells them apart.
        let b = Stamp::new("/tmp/a.png".into(), 101, 10, 200);
        assert_ne!(a.key(), b.key());
        // TOUCHED BUT NOT CHANGED is a different key too, and that is the
        // cheap side of the trade: one needless decode beats a stale picture.
        let c = Stamp::new("/tmp/a.png".into(), 100, 11, 200);
        assert_ne!(a.key(), c.key());
    }

    #[test]
    fn a_name_that_is_finer_than_the_sniff_wins() {
        // A Pixel's `.dng` sniffs as `image/tiff` because it IS one. Taking
        // the sniff sent it to a TIFF decoder, which read the RAW planes and
        // refused — on the owner's own photographs.
        assert_eq!(
            super::settled("image/x-adobe-dng", Some("image/tiff")),
            "image/x-adobe-dng"
        );
        // A NAME THAT LIES loses, which is the case `mime::examined` exists
        // for: a PNG called `.txt`.
        assert_eq!(super::settled("text/plain", Some("image/png")), "image/png");
        assert_eq!(super::settled("image/png", None), "image/png");
        assert_eq!(super::settled("image/png", Some("image/png")), "image/png");
    }

    #[test]
    fn a_refusal_is_not_a_blank() {
        let m = Made::refused(Refusal::Nothing);
        assert!(!m.drawable());
        assert!(m.why.is_some(), "a refusal with no reason is a blank panel");
    }
}
