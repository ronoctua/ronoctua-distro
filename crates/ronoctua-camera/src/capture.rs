//! The one door that opens a camera for frames.
//!
//! # Why a value with a `Drop`, and never a function
//!
//! A camera that is open has a LIGHT on. The only acceptable lifetime for that
//! is "exactly as long as somebody holds this value": [`Capture`] owns the
//! ffmpeg that owns the device, its `Drop` kills and reaps it, and there is no
//! other way in this tree to get a camera's frames — `the-camera-has-one-door`
//! refuses a second `-f v4l2` input anywhere else.
//!
//! The child runs under `setpriv --pdeathsig KILL` (the microphone meter's
//! trick): the guard covers every ordinary exit and no signal, and a studio
//! killed with `SIGKILL` must not leave a process filming a room.
//!
//! # What ffmpeg is asked
//!
//! The device's own format at the chosen mode (`-input_format`, `-video_size`,
//! `-framerate`), decoded and SCALED to the size the caller will use, as raw
//! RGBA on stdout. Asking ffmpeg for the final size keeps the one expensive
//! resample in its SIMD code and hands this process a frame it only copies.

use std::{
    io::Read,
    os::unix::process::CommandExt,
    process::{ChildStdout, Command, Stdio},
    time::Duration,
};

use crate::{
    Camera, Mode,
    frames::{self, Filled, Reaped},
    input_format,
};

/// The environment variable naming ffmpeg — the same one the file manager's
/// readers use, set by `nix/modules/media.nix` on every profile.
pub const FFMPEG: &str = "RONOCTUA_FFMPEG";

/// `setpriv`, by store path, set by `nix/rust.nix` (util-linux, already in
/// every closure).
const SETPRIV: Option<&str> = option_env!("RONOCTUA_SETPRIV");

/// How long a camera may say nothing before the capture is called dead.
///
/// Longer than a video file's: a UVC camera takes up to two seconds to give
/// its first frame after the stream starts (auto exposure settling), and a
/// USB camera that was just plugged longer still.
pub const STARVED: Duration = Duration::from_secs(6);

/// An open camera.
pub struct Capture {
    _child: Reaped,
    out: ChildStdout,
    /// Pixels across of every frame this gives.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
}

/// The ffmpeg arguments for one capture, pure so a test can read them.
pub fn arguments(
    camera: &Camera,
    mode: &Mode,
    width: u32,
    height: u32,
    mirror: bool,
) -> Option<Vec<String>> {
    let format = input_format(&mode.fourcc)?;
    let mut filter = format!("scale={width}:{height}:flags=bilinear");
    if mirror {
        filter.push_str(",hflip");
    }
    let rate = format!("{}/{}", mode.interval.den, mode.interval.num.max(1));
    Some(
        [
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            // No input buffering: a live camera is worth nothing a second late.
            "-fflags",
            "nobuffer",
            "-flags",
            "low_delay",
            "-f",
            "v4l2",
            "-input_format",
            format,
            "-video_size",
            &format!("{}x{}", mode.width, mode.height),
            "-framerate",
            &rate,
            "-i",
            &camera.node.to_string_lossy(),
            "-vf",
            &filter,
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "-",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect(),
    )
}

impl Capture {
    /// Opens `camera` in `mode`, giving frames of `width × height`.
    pub fn open(
        camera: &Camera,
        mode: &Mode,
        width: u32,
        height: u32,
        mirror: bool,
    ) -> anyhow::Result<Self> {
        let ffmpeg = std::env::var(FFMPEG).map_err(|_| anyhow::anyhow!("{FFMPEG} is not set"))?;
        let args = arguments(camera, mode, width, height, mirror)
            .ok_or_else(|| anyhow::anyhow!("{} has no ffmpeg name", mode.fourcc))?;
        let mut command = match SETPRIV {
            Some(setpriv) => {
                let mut c = Command::new(setpriv);
                c.args(["--pdeathsig", "KILL", "--"]).arg(&ffmpeg);
                c
            }
            None => {
                tracing::warn!(
                    "RONOCTUA_SETPRIV was not set at build time: a killed studio would leave the camera open"
                );
                Command::new(&ffmpeg)
            }
        };
        let mut child = command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        let out = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdout"))?;
        Ok(Self {
            _child: Reaped(child),
            out,
            width,
            height,
        })
    }

    /// Bytes in one frame.
    pub fn frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// Reads the next frame into `into`, resized to fit.
    pub fn next(&mut self, into: &mut Vec<u8>, wanted: impl Fn() -> bool) -> Filled {
        into.resize(self.frame_bytes(), 0);
        frames::fill(&mut self.out, into, STARVED, wanted)
    }

    /// The pipe, for a caller that wants to poll it with others.
    pub fn pipe(&mut self) -> &mut impl Read {
        &mut self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identity, Interval, Kind};

    #[test]
    fn the_arguments_ask_for_the_devices_own_mode_and_our_size() {
        let cam = Camera {
            identity: Identity::of("c", "b", "i"),
            node: "/dev/video4".into(),
            card: "c".into(),
            driver: "uvcvideo".into(),
            bus: "b".into(),
            kind: Kind::Colour,
            formats: Vec::new(),
            controls: Vec::new(),
            streamable: true,
        };
        let mode = Mode {
            fourcc: "MJPG".into(),
            width: 1920,
            height: 1080,
            interval: Interval { num: 1, den: 30 },
        };
        let a = arguments(&cam, &mode, 1280, 720, true).expect("named");
        let joined = a.join(" ");
        assert!(
            joined.contains(
                "-f v4l2 -input_format mjpeg -video_size 1920x1080 -framerate 30/1 -i /dev/video4"
            ),
            "{joined}"
        );
        assert!(joined.contains("scale=1280:720:flags=bilinear,hflip"));
        assert!(joined.ends_with("-pix_fmt rgba -f rawvideo -"));
        let odd = Mode {
            fourcc: "ZZZZ".into(),
            ..mode
        };
        assert!(arguments(&cam, &odd, 1, 1, false).is_none());
    }
}
