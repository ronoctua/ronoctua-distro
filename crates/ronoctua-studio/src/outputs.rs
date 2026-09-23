//! Where the program goes: a recording, a stream, the virtual camera, a photo.
//!
//! # One ffmpeg per output, fed on a thread of its own
//!
//! Each output is a child reading raw RGBA on stdin. Writing 3.7 MB to a pipe
//! blocks when the child is slow, and the compositor must never wait on an
//! encoder — so every output has a WRITER thread behind a two-frame channel,
//! and a frame the channel will not take is DROPPED and counted. A recording
//! that falls behind loses a frame; a live camera that waited would freeze
//! every output at once.
//!
//! # Sound travels by FIFO
//!
//! ffmpeg reads the mixed sound from a named pipe in `$XDG_RUNTIME_DIR`
//! (`-f s16le -i <fifo>`), because giving a child a second pipe on fd 3 needs
//! `pre_exec`, which is `unsafe`. The FIFO is ours, `0600`, and removed when
//! the output ends.
//!
//! # And every output is a value with a `Drop`
//!
//! Dropping it closes stdin (ffmpeg finishes the file), waits, and — for a
//! recording — remuxes to MP4 when asked. A recording is never KILLED on the
//! ordinary path: killing ffmpeg is how a Matroska loses its index.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use ronoctua_effects::Frame;

use crate::prefs::{Container, Encoder, Preferences};

/// An ffmpeg with a writer thread in front of it.
pub struct Fed {
    tx: Option<SyncSender<Arc<Frame>>>,
    writer: Option<JoinHandle<()>>,
    child: Option<Child>,
    /// Frames the channel refused.
    pub dropped: u64,
    /// ffmpeg's stderr, for the refusal sentence.
    log: PathBuf,
}

impl Fed {
    fn spawn(ffmpeg: &str, args: &[String], log: PathBuf) -> Result<Self> {
        let err = std::fs::File::create(&log)?;
        let mut child = Command::new(ffmpeg)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(err)
            .process_group(0)
            .spawn()
            .context("could not run ffmpeg")?;
        let stdin = child.stdin.take().context("no stdin")?;
        let (tx, rx) = sync_channel::<Arc<Frame>>(2);
        let writer = std::thread::Builder::new()
            .name("studio-writer".into())
            .spawn(move || feed(stdin, &rx))?;
        Ok(Self {
            tx: Some(tx),
            writer: Some(writer),
            child: Some(child),
            dropped: 0,
            log,
        })
    }

    /// Offers a frame; a full channel drops it.
    pub fn offer(&mut self, frame: &Arc<Frame>) {
        if let Some(tx) = &self.tx {
            match tx.try_send(frame.clone()) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => self.dropped += 1,
                Err(TrySendError::Disconnected(_)) => self.tx = None,
            }
        }
    }

    /// Whether ffmpeg is still running.
    pub fn alive(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)))
    }

    /// ffmpeg's last line, for a refusal.
    pub fn last_words(&self) -> String {
        std::fs::read_to_string(&self.log)
            .ok()
            .and_then(|s| {
                s.lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }

    /// Closes stdin and waits up to `patience` for ffmpeg to finish its file;
    /// past that it is killed. `true` when it finished on its own.
    pub fn finish(&mut self, patience: Duration) -> bool {
        self.tx = None;
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
        let Some(mut child) = self.child.take() else {
            return true;
        };
        let until = Instant::now() + patience;
        while Instant::now() < until {
            if let Ok(Some(status)) = child.try_wait() {
                return status.success();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        false
    }
}

impl Drop for Fed {
    fn drop(&mut self) {
        self.finish(Duration::from_secs(10));
        let _ = std::fs::remove_file(&self.log);
    }
}

fn feed(mut stdin: ChildStdin, rx: &Receiver<Arc<Frame>>) {
    let mut written = 0u64;
    while let Ok(frame) = rx.recv() {
        if let Err(e) = stdin.write_all(&frame.rgba) {
            tracing::debug!(written, error = %e, "an encoder stopped reading");
            return;
        }
        written += 1;
    }
    tracing::debug!(written, "an encoder was fed");
}

use std::os::unix::process::CommandExt as _;

/// The raw-video input half every output shares.
fn raw_input(width: u32, height: u32, fps: u32) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-thread_queue_size",
        "64",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-video_size",
        &format!("{width}x{height}"),
        "-framerate",
        &fps.to_string(),
        "-i",
        "pipe:0",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

fn sound_input(fifo: &Path) -> Vec<String> {
    [
        "-thread_queue_size",
        "512",
        "-f",
        "s16le",
        "-ar",
        &ronoctua_audio::pipewire::CAPTURE_RATE.to_string(),
        "-ac",
        &ronoctua_audio::pipewire::CAPTURE_CHANNELS.to_string(),
        "-i",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([fifo.to_string_lossy().into_owned()])
    .collect()
}

/// Whether this machine has a VA-API render node to try.
fn render_node() -> Option<PathBuf> {
    let dir = std::fs::read_dir("/dev/dri").ok()?;
    let mut nodes: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("renderD"))
        })
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}

/// The video codec arguments for an encoder choice; `None` when hardware was
/// demanded and there is none.
pub fn video_codec(encoder: Encoder, quality: u8, hardware: Option<&Path>) -> Option<Vec<String>> {
    let q = quality.clamp(10, 40).to_string();
    let software = || -> Vec<String> {
        [
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "zerolatency",
            "-crf",
            &q,
            "-pix_fmt",
            "yuv420p",
            "-g",
            "60",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    };
    let hw = |node: &Path| -> Vec<String> {
        vec![
            "-vaapi_device".into(),
            node.to_string_lossy().into_owned(),
            "-vf".into(),
            "format=nv12,hwupload".into(),
            "-c:v".into(),
            "h264_vaapi".into(),
            "-qp".into(),
            q.clone(),
            "-g".into(),
            "60".into(),
        ]
    };
    match (encoder, hardware) {
        (Encoder::Software, _) | (Encoder::Auto, None) => Some(software()),
        (Encoder::Hardware | Encoder::Auto, Some(node)) => Some(hw(node)),
        (Encoder::Hardware, None) => None,
    }
}

/// A file name for now, in local time: `ronoctua-2026-09-23-20-41-07.<ext>`.
pub fn stamped(dir: &Path, ext: &str) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let zone = ronoctua_files::when::Zone::read_from(Path::new("/etc/localtime"));
    let c = ronoctua_files::when::civil(secs, &zone);
    let base = format!(
        "ronoctua-{:04}-{:02}-{:02}-{:02}-{:02}-{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    );
    let mut path = dir.join(format!("{base}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base}-{n}.{ext}"));
        n += 1;
    }
    path
}

/// Makes the sound FIFO, private to this user.
pub fn fifo(leaf: &str) -> Result<PathBuf> {
    let path = ronoctua_xdg::runtime_file(leaf).context("XDG_RUNTIME_DIR is not set")?;
    let _ = std::fs::remove_file(&path);
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &path,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_raw_mode(0o600),
        0,
    )?;
    Ok(path)
}

/// A recording in progress.
pub struct Recording {
    fed: Fed,
    /// The Matroska being written.
    pub path: PathBuf,
    /// Where the finished file will be.
    pub finished: PathBuf,
    /// The sound FIFO, when sound is recorded.
    pub fifo: Option<PathBuf>,
    /// When it started, epoch seconds.
    pub since: u64,
    remux: bool,
    ffmpeg: String,
}

impl Recording {
    /// Starts one.
    pub fn start(prefs: &Preferences, sound: bool) -> Result<Self> {
        let ffmpeg = std::env::var(ronoctua_camera::capture::FFMPEG)
            .context("RONOCTUA_FFMPEG is not set")?;
        let dir = prefs.videos_dir();
        std::fs::create_dir_all(&dir)?;
        let path = stamped(&dir, "mkv");
        let finished = if prefs.container == Container::Mp4 {
            path.with_extension("mp4")
        } else {
            path.clone()
        };
        let fifo = if sound {
            Some(fifo("studio-record.pcm")?)
        } else {
            None
        };
        let (w, h) = prefs.size();
        let hardware = render_node();
        let mut tried = Vec::new();
        for encoder in [prefs.encoder, Encoder::Software] {
            if tried.contains(&encoder) {
                continue;
            }
            tried.push(encoder);
            let Some(codec) = video_codec(encoder, prefs.quality, hardware.as_deref()) else {
                continue;
            };
            let mut args = raw_input(w, h, prefs.rate());
            if let Some(f) = &fifo {
                args.extend(sound_input(f));
            }
            args.extend(codec);
            if fifo.is_some() {
                args.extend(["-c:a", "aac", "-b:a", "160k"].map(str::to_owned));
            }
            args.extend([
                "-y".to_owned(),
                "-f".to_owned(),
                "matroska".to_owned(),
                path.to_string_lossy().into_owned(),
            ]);
            let log = std::env::temp_dir()
                .join(format!("ronoctua-studio-record-{}.log", std::process::id()));
            let mut fed = Fed::spawn(&ffmpeg, &args, log)?;
            // A hardware encoder that is not there dies at once; give it a
            // moment and fall back to the CPU rather than recording nothing.
            std::thread::sleep(Duration::from_millis(400));
            if fed.alive() {
                let since = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                return Ok(Self {
                    fed,
                    path,
                    finished,
                    fifo,
                    since,
                    remux: prefs.container == Container::Mp4,
                    ffmpeg,
                });
            }
            tracing::warn!(encoder = ?encoder, words = %fed.last_words(), "the encoder refused; trying the next");
        }
        anyhow::bail!("no encoder would start")
    }

    /// Offers a frame.
    pub fn offer(&mut self, frame: &Arc<Frame>) {
        self.fed.offer(frame);
    }

    /// Whether it is still running, and ffmpeg's words when not.
    pub fn check(&mut self) -> Result<(), String> {
        if self.fed.alive() {
            Ok(())
        } else {
            Err(self.fed.last_words())
        }
    }

    /// Ends it, and gives the finished file.
    pub fn stop(mut self) -> PathBuf {
        self.fed.finish(Duration::from_secs(20));
        if let Some(f) = &self.fifo {
            let _ = std::fs::remove_file(f);
        }
        if !self.remux {
            return self.path.clone();
        }
        // THE MATROSKA IS KEPT until the MP4 exists: a remux that fails must
        // not cost the recording.
        let ok = Command::new(&self.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
            .arg(&self.path)
            .args(["-c", "copy", "-movflags", "+faststart"])
            .arg(&self.finished)
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            let _ = std::fs::remove_file(&self.path);
            self.finished.clone()
        } else {
            self.path.clone()
        }
    }
}

/// A live stream.
pub struct Streaming {
    fed: Fed,
    /// The address, without its key.
    pub url: String,
    /// The sound FIFO.
    pub fifo: Option<PathBuf>,
    /// When it started.
    pub since: u64,
}

/// The output arguments for a stream address: FLV for RTMP(S), MPEG-TS for
/// SRT, and the KEY read from a file (`-/option <file>`, ffmpeg 7+) so it is
/// never in a process list.
pub fn stream_output(url: &str, key: &Path) -> Option<Vec<String>> {
    let key = key.to_string_lossy().into_owned();
    if url.starts_with("rtmp://") || url.starts_with("rtmps://") {
        Some(vec![
            "-f".into(),
            "flv".into(),
            "-/rtmp_playpath".into(),
            key,
            url.to_owned(),
        ])
    } else if url.starts_with("srt://") {
        Some(vec![
            "-f".into(),
            "mpegts".into(),
            "-/streamid".into(),
            key,
            url.to_owned(),
        ])
    } else {
        None
    }
}

impl Streaming {
    /// Starts one.
    pub fn start(prefs: &Preferences, sound: bool) -> Result<Self> {
        let ffmpeg = std::env::var(ronoctua_camera::capture::FFMPEG)
            .context("RONOCTUA_FFMPEG is not set")?;
        let url = prefs
            .stream_url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .context("no stream address")?;
        let key = crate::prefs::stream_key_path()
            .filter(|_| crate::prefs::has_stream_key())
            .context("no stream key")?;
        let tail = stream_output(&url, &key).context("an address that is neither rtmp nor srt")?;
        let fifo = if sound {
            Some(fifo("studio-stream.pcm")?)
        } else {
            None
        };
        let (w, h) = prefs.size();
        let mut args = raw_input(w, h, prefs.rate());
        if let Some(f) = &fifo {
            args.extend(sound_input(f));
        }
        args.extend(
            video_codec(prefs.encoder, prefs.quality, render_node().as_deref()).unwrap_or_else(
                || video_codec(Encoder::Software, prefs.quality, None).unwrap_or_default(),
            ),
        );
        // A stream needs a steady bitrate the far side can plan for.
        args.extend(["-maxrate", "4500k", "-bufsize", "9000k"].map(str::to_owned));
        if fifo.is_some() {
            args.extend(["-c:a", "aac", "-b:a", "160k", "-ar", "48000"].map(str::to_owned));
        }
        args.extend(tail);
        let log =
            std::env::temp_dir().join(format!("ronoctua-studio-stream-{}.log", std::process::id()));
        let fed = Fed::spawn(&ffmpeg, &args, log)?;
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Ok(Self {
            fed,
            url,
            fifo,
            since,
        })
    }

    /// Offers a frame.
    pub fn offer(&mut self, frame: &Arc<Frame>) {
        self.fed.offer(frame);
    }

    /// Whether it is still running.
    pub fn check(&mut self) -> Result<(), String> {
        if self.fed.alive() {
            Ok(())
        } else {
            Err(self.fed.last_words())
        }
    }
}

impl Drop for Streaming {
    fn drop(&mut self) {
        self.fed.finish(Duration::from_secs(5));
        if let Some(f) = &self.fifo {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// The virtual camera: the program written to our `v4l2loopback` node.
pub struct Virtual {
    fed: Fed,
    /// The node.
    pub node: PathBuf,
}

impl Virtual {
    /// Opens our loopback node, if the module is loaded.
    pub fn start(prefs: &Preferences) -> Result<Self, crate::wire::Trouble> {
        use crate::wire::Trouble;
        let node = ronoctua_camera::scan()
            .into_iter()
            .find(|c| c.kind == ronoctua_camera::Kind::Virtual)
            .map(|c| c.node)
            .or_else(find_loopback)
            .ok_or(Trouble::NoVirtual)?;
        let ffmpeg =
            std::env::var(ronoctua_camera::capture::FFMPEG).map_err(|_| Trouble::NoFfmpeg)?;
        let (w, h) = prefs.size();
        let mut args = raw_input(w, h, prefs.rate());
        // YUYV: the one format every browser and every call program accepts
        // from a loopback without asking.
        args.extend(["-pix_fmt", "yuyv422", "-f", "v4l2"].map(str::to_owned));
        args.push(node.to_string_lossy().into_owned());
        let log = std::env::temp_dir().join(format!(
            "ronoctua-studio-virtual-{}.log",
            std::process::id()
        ));
        let mut fed = Fed::spawn(&ffmpeg, &args, log).map_err(|_| Trouble::NoFfmpeg)?;
        std::thread::sleep(Duration::from_millis(300));
        if !fed.alive() {
            return Err(Trouble::VirtualBusy);
        }
        Ok(Self { fed, node })
    }

    /// Offers a frame.
    pub fn offer(&mut self, frame: &Arc<Frame>) {
        self.fed.offer(frame);
    }

    /// Whether it is still running.
    pub fn alive(&mut self) -> bool {
        self.fed.alive()
    }
}

/// A loopback node that is not listed as a capture device yet — with
/// `exclusive_caps=1` our node advertises OUTPUT until a writer opens it.
fn find_loopback() -> Option<PathBuf> {
    let dir = std::fs::read_dir(ronoctua_camera::SYS).ok()?;
    for e in dir.flatten() {
        let name = std::fs::read_to_string(e.path().join("name")).unwrap_or_default();
        if name.trim() == ronoctua_camera::OUR_LABEL {
            return Some(Path::new("/dev").join(e.file_name()));
        }
    }
    None
}

/// Writes a photo of `frame` as PNG, and gives its path.
pub fn photo(frame: &Frame, prefs: &Preferences) -> Result<PathBuf> {
    let dir = prefs.pictures_dir();
    std::fs::create_dir_all(&dir)?;
    let path = stamped(&dir, "png");
    let img =
        image::RgbaImage::from_raw(frame.width as u32, frame.height as u32, frame.rgba.clone())
            .context("a torn frame")?;
    img.save_with_format(&path, image::ImageFormat::Png)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_never_is_an_argument() {
        let args = stream_output("rtmp://a.example/live", Path::new("/k")).expect("rtmp");
        assert_eq!(
            args,
            vec![
                "-f",
                "flv",
                "-/rtmp_playpath",
                "/k",
                "rtmp://a.example/live"
            ]
        );
        let srt = stream_output("srt://b.example:9000", Path::new("/k")).expect("srt");
        assert!(srt.contains(&"-/streamid".to_owned()));
        assert!(stream_output("http://x", Path::new("/k")).is_none());
    }

    #[test]
    fn hardware_demanded_on_a_machine_without_it_refuses() {
        assert!(video_codec(Encoder::Hardware, 23, None).is_none());
        assert!(
            video_codec(Encoder::Auto, 23, None)
                .expect("software")
                .contains(&"libx264".to_owned())
        );
        let hw =
            video_codec(Encoder::Auto, 23, Some(Path::new("/dev/dri/renderD128"))).expect("hw");
        assert!(hw.contains(&"h264_vaapi".to_owned()));
    }

    #[test]
    fn a_stamped_name_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("ronoctua-stamp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let a = stamped(&dir, "png");
        std::fs::write(&a, b"x").expect("write");
        let b = stamped(&dir, "png");
        assert_ne!(a, b);
        let _ = std::fs::remove_dir_all(dir);
    }
}
