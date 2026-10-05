//! What a person chose about the studio: `studio.json`, and the scene book.
//!
//! Two files, because they change for different reasons and at different
//! rates: the settings (`studio.json`) are turned in the settings view, the
//! scene book (`studio/scenes.json`) is edited with the pointer in the camera
//! app, sometimes thirty times a second while a layer is dragged. One file for
//! both would make every drag rewrite the encoder's settings.
//!
//! The stream KEY is in neither: it is a secret, and it lives in a file of its
//! own at `0600` ([`stream_key_path`]), read by ffmpeg through `-/rtmp_playpath`
//! so it never appears in a process list.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::scene::Book;

/// The settings file's name.
pub const FILE: &str = "studio.json";
/// The scene book's name, under [`book_home`].
pub const BOOK: &str = "scenes.json";
/// The stream key's file name.
pub const KEY: &str = "studio-stream-key";

/// The container a recording is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Container {
    /// Matroska, and nothing else: survives a crash mid-recording.
    Mkv,
    /// Written as Matroska and REMUXED at the end — a crash still leaves the
    /// Matroska, and a finished recording is what every phone plays.
    #[default]
    Mp4,
}

/// Which video encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encoder {
    /// The GPU's (VA-API) when it answers, the CPU's when it does not.
    #[default]
    Auto,
    /// The GPU's; a machine without one refuses and says so.
    Hardware,
    /// The CPU's (x264): the same everywhere, and heavier.
    Software,
}

/// When the modeline shows the camera mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    /// Whenever any program has a camera open.
    #[default]
    WhenInUse,
    /// Never — the light on the camera is the only sign.
    Never,
}

/// What the virtual camera sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sends {
    /// The program: the scene on air, effects and all.
    #[default]
    Program,
    /// The camera layer alone, with its effects — what a video call wants
    /// even while the scene on air is the screen.
    Camera,
}

/// What the virtual camera's picture goes through when it sends the camera
/// alone ([`Sends::Camera`]).
///
/// A CHAIN OF ITS OWN, so a call can have the background blurred while the
/// preview shows the plain picture — two chains on one capture
/// (`sources::Shared::own`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VirtualEffect {
    /// The camera layer's own chain — what the preview shows.
    #[default]
    Preview,
    /// No effect at all.
    None,
    /// One declared effect, at its defaults.
    Own(String),
}

impl VirtualEffect {
    /// The chain the virtual camera runs of its own, or `None` for the
    /// preview's.
    #[must_use]
    pub fn chain(&self) -> Option<ronoctua_effects::Chain> {
        match self {
            Self::Preview => None,
            Self::None => Some(ronoctua_effects::Chain(Vec::new())),
            Self::Own(id) => Some(ronoctua_effects::Chain(
                ronoctua_effects::Step::new(id).into_iter().collect(),
            )),
        }
    }

    /// The ring the settings turn: the preview's, none, then every declared
    /// effect in the table's order.
    #[must_use]
    pub fn ring() -> Vec<Self> {
        [Self::Preview, Self::None]
            .into_iter()
            .chain(
                ronoctua_effects::DECLARED
                    .iter()
                    .map(|d| Self::Own(d.id.to_owned())),
            )
            .collect()
    }
}

/// The whole settings file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Program width in pixels.
    pub width: u32,
    /// Program height.
    pub height: u32,
    /// Program frames per second.
    pub fps: u32,
    /// The container.
    pub container: Container,
    /// The encoder.
    pub encoder: Encoder,
    /// Quality, as x264's CRF (18 is near lossless, 28 small); the hardware
    /// encoder gets the matching QP.
    pub quality: u8,
    /// Seconds of countdown before a photo; 0 takes it at once.
    pub countdown: u8,
    /// Record the microphone.
    pub microphone: bool,
    /// Its gain, percent.
    pub microphone_gain: u8,
    /// Record what the machine plays.
    pub desktop: bool,
    /// Its gain, percent.
    pub desktop_gain: u8,
    /// Draw the pointer in screen captures.
    pub cursor: bool,
    /// Turn the virtual camera on whenever the studio starts.
    pub virtual_at_start: bool,
    /// What it sends.
    pub virtual_sends: Sends,
    /// What the camera it sends goes through, when it sends the camera alone.
    pub virtual_effect: VirtualEffect,
    /// The picture a replaced background shows; `None` refuses the effect.
    pub backdrop: Option<PathBuf>,
    /// The pictures the backdrop turns among in the camera's settings
    /// (`←`/`→`), chosen there in the file picker, several at once.
    pub backdrops: Vec<PathBuf>,
    /// Where photos go; `None` is the Pictures directory.
    pub pictures: Option<PathBuf>,
    /// Where recordings go; `None` is the Videos directory.
    pub videos: Option<PathBuf>,
    /// The stream's address without its key (`rtmp://host/app`, `srt://…`).
    pub stream_url: Option<String>,
    /// When the modeline shows the camera mark.
    pub mark: Mark,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 30,
            container: Container::Mp4,
            encoder: Encoder::Auto,
            quality: 23,
            countdown: 0,
            microphone: true,
            microphone_gain: 100,
            desktop: false,
            desktop_gain: 100,
            cursor: true,
            virtual_at_start: false,
            virtual_sends: Sends::Program,
            virtual_effect: VirtualEffect::Preview,
            backdrop: None,
            backdrops: Vec::new(),
            pictures: None,
            videos: None,
            stream_url: None,
            mark: Mark::WhenInUse,
        }
    }
}

/// The program sizes the settings ring through, and nothing between them —
/// every one a size every encoder and every receiving site accepts.
pub const SIZES: [(u32, u32); 4] = [(640, 360), (1280, 720), (1920, 1080), (2560, 1440)];
/// The rates the settings ring through.
pub const RATES: [u32; 4] = [15, 24, 30, 60];

impl Preferences {
    /// The program's size, kept even (every encoder wants that) and inside
    /// the ring's largest.
    pub fn size(&self) -> (u32, u32) {
        let even = |v: u32, top: u32| (v.clamp(16, top) / 2) * 2;
        (even(self.width, 3840), even(self.height, 2160))
    }

    /// The frame rate, inside `1..=60`.
    pub fn rate(&self) -> u32 {
        self.fps.clamp(1, 60)
    }

    /// Where photos land.
    pub fn pictures_dir(&self) -> PathBuf {
        self.pictures
            .clone()
            .unwrap_or_else(|| ronoctua_xdg::user_dir("XDG_PICTURES_DIR", "Pictures"))
    }

    /// Where recordings land.
    pub fn videos_dir(&self) -> PathBuf {
        self.videos
            .clone()
            .unwrap_or_else(|| ronoctua_xdg::user_dir("XDG_VIDEOS_DIR", "Videos"))
    }
}

/// The directory the settings live in.
#[must_use]
pub fn home() -> Option<PathBuf> {
    ronoctua_store::config_dir()
}

/// The directory the scene book lives in.
#[must_use]
pub fn book_home() -> Option<PathBuf> {
    home().map(|h| h.join("studio"))
}

/// The settings store.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(home(), FILE)
}

/// The scene book's store.
#[must_use]
pub fn book() -> ronoctua_store::Store<Book> {
    ronoctua_store::Store::at(book_home(), BOOK)
}

/// Writes a store back, REFUSING over a file that will not parse (a reader
/// degrades, a writer must refuse — `ronoctua_audio::prefs::keep`).
pub fn keep<T>(store: &ronoctua_store::Store<T>, value: &T) -> anyhow::Result<()>
where
    T: Serialize + serde::de::DeserializeOwned + Default,
{
    if store.parsed().is_none() {
        anyhow::bail!(
            "{} will not parse, and writing would replace it",
            store
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
    }
    store.write(value)?;
    Ok(())
}

/// Where the stream key is kept.
#[must_use]
pub fn stream_key_path() -> Option<PathBuf> {
    home().map(|h| h.join(KEY))
}

/// Whether a key has been set — the one thing the settings view may say about
/// it.
pub fn has_stream_key() -> bool {
    stream_key_path().is_some_and(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > 0))
}

/// Stores the key at `0600`, written before it is readable by anybody.
pub fn set_stream_key(key: &str) -> anyhow::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let path = stream_key_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    f.write_all(key.trim().as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_even_and_bounded() {
        let p = Preferences {
            width: 1281,
            height: 99_999,
            ..Preferences::default()
        };
        assert_eq!(p.size(), (1280, 2160));
    }

    #[test]
    fn a_hand_written_file_with_one_line_keeps_the_rest() {
        let p: Preferences = serde_json::from_str(r#"{"fps": 60}"#).expect("parses");
        assert_eq!(p.fps, 60);
        assert_eq!(p.size(), (1280, 720));
        assert!(p.microphone);
    }
}
