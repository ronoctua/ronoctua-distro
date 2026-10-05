//! The socket's vocabulary: one JSON object per line, both ways.
//!
//! The shape of `ronoctua-transfers::wire`: asks go in, and a `watch` gets the
//! WHOLE state on every change and never a delta — a client that missed one
//! line is right again at the next.
//!
//! The one thing on this socket that is not a line is the preview: after a
//! `preview` ask, the answer line is followed by a file descriptor (a memfd,
//! `SCM_RIGHTS`) and then ONE BYTE per frame, naming the slot that holds it.
//! See `preview.rs`.

use std::io::{BufRead, Write};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::scene::{Rect, Running, Script};

/// The socket's leaf name under `$XDG_RUNTIME_DIR` — per instance, through
/// `ronoctua_xdg::runtime_file`, so the nested loop has its own.
pub const SOCKET: &str = "studio.sock";

/// The socket's path.
pub fn socket_path() -> Result<std::path::PathBuf> {
    ronoctua_xdg::runtime_file(SOCKET).context("XDG_RUNTIME_DIR is not set")
}

/// Which sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sound {
    /// The microphone.
    Microphone,
    /// What the machine plays.
    Desktop,
}

/// What a client asks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "ask")]
pub enum Ask {
    /// The state now, and again on every change, until the client leaves.
    Watch,
    /// The state once.
    Look,
    /// Frames of the program; see the module header.
    Preview,
    /// A photo of the program (after the countdown, when there is one).
    Photo,
    /// Start or stop recording; `None` turns it over.
    Record {
        /// The wanted state.
        on: Option<bool>,
    },
    /// Start or stop streaming.
    Stream {
        /// The wanted state.
        on: Option<bool>,
    },
    /// The virtual camera on or off.
    Virtual {
        /// The wanted state.
        on: Option<bool>,
    },
    /// Put a scene on air.
    Scene {
        /// Its name.
        name: String,
    },
    /// The scene after (`1`) or before (`-1`) the one on air.
    SceneTurn {
        /// How far.
        by: i32,
    },
    /// Run a script, or stop the one running (`None`).
    Script {
        /// Its name.
        name: Option<String>,
    },
    /// The running script's next cue.
    Next,
    /// The camera layer's effect, around its ring.
    EffectTurn {
        /// How far.
        by: i32,
    },
    /// One parameter of the camera layer's effect, by its declared step.
    EffectParam {
        /// The parameter.
        name: String,
        /// Steps.
        by: i32,
        /// Of the BACKGROUND step rather than the effect — `radius` is a
        /// parameter of both `blur` and `background-blur`.
        #[serde(default)]
        background: bool,
    },
    /// The camera layer's background mode, around its ring.
    BackgroundTurn {
        /// How far.
        by: i32,
    },
    /// The camera layer shows the next (or previous) camera.
    CameraTurn {
        /// How far.
        by: i32,
    },
    /// The camera layer's effect, CHOSEN rather than turned to — a menu's
    /// answer. `None` takes it off; an id that is not a declared effect of
    /// that family is refused.
    Effect {
        /// The effect.
        name: Option<String>,
    },
    /// The camera layer's background, chosen.
    Background {
        /// The background effect.
        name: Option<String>,
    },
    /// The camera layer's camera, chosen by identity.
    Camera {
        /// Its identity's text.
        identity: String,
    },
    /// The camera shown as a mirror, or not.
    Mirror,
    /// A layer of the scene on air moved or resized — LIVE, during a
    /// recording too.
    Place {
        /// Which layer, bottom first.
        layer: usize,
        /// Where it goes.
        rect: Rect,
    },
    /// A layer shown or hidden.
    Show {
        /// Which layer.
        layer: usize,
        /// Shown?
        on: bool,
    },
    /// A sound's gain or mute.
    Sound {
        /// Which.
        which: Sound,
        /// Gain in percent, when changed.
        gain: Option<u8>,
        /// Muted, when changed.
        muted: Option<bool>,
    },
    /// A new scene, a copy of the one on air, put on air.
    SceneAdd {
        /// Its name.
        name: String,
    },
    /// A scene renamed.
    SceneRename {
        /// The name it has.
        from: String,
        /// The name it gets.
        to: String,
    },
    /// A scene removed — never the last one.
    SceneRemove {
        /// Its name.
        name: String,
    },
    /// A script added, or replaced when the name exists.
    ScriptAdd {
        /// The script.
        script: Script,
    },
    /// A script removed.
    ScriptRemove {
        /// Its name.
        name: String,
    },
    /// Read the settings and the scene book again.
    Reload,
    /// The hands in the camera, one [`Say::Hands`] per frame read, until the
    /// client leaves. The connection is what keeps the camera open: a client
    /// that dies closes it by construction.
    Hands,
}

/// Why something does not work, for a sentence on screen. Each has a text key
/// `studio-trouble-<kebab>` in both languages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "trouble")]
pub enum Trouble {
    /// No camera is plugged in.
    NoCamera,
    /// A camera is held by another program.
    CameraBusy {
        /// Its name, from `/proc`.
        by: String,
    },
    /// A camera stopped giving frames.
    CameraSilent,
    /// The segmentation model is not installed; background effects refuse.
    NoModel,
    /// A text source has no face to be drawn with: the build named none.
    NoFace,
    /// This is a second copy of the system (the development loop): it
    /// rehearses, and writes no photo, recording or stream.
    Rehearsal,
    /// The hand models are not installed; gestures refuse.
    NoHandModel,
    /// A replaced background has no picture chosen.
    NoBackdrop,
    /// ffmpeg is not reachable.
    NoFfmpeg,
    /// The screen cannot be captured (no Wayland, or the compositor does not
    /// offer the protocol).
    NoScreen,
    /// A window source matches no open window.
    NoWindow,
    /// The virtual camera device does not exist (the kernel module is not
    /// loaded).
    NoVirtual,
    /// The virtual camera is being written by somebody else.
    VirtualBusy,
    /// The encoder refused.
    Encoder {
        /// ffmpeg's last line.
        why: String,
    },
    /// The stream refused or dropped.
    Stream {
        /// ffmpeg's last line.
        why: String,
    },
    /// No stream address or key is set.
    NoStreamTarget,
    /// A name that is already taken, or the last scene asked to go.
    Name,
    /// A file could not be written or read.
    Write {
        /// The error.
        why: String,
    },
}

/// One camera, as a client needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraSeen {
    /// Its identity's text.
    pub identity: String,
    /// Its name.
    pub card: String,
    /// Infrared?
    pub infrared: bool,
    /// Openable by ffmpeg?
    pub streamable: bool,
    /// Its lens shutter is closed.
    pub shuttered: bool,
}

/// A recording or a stream in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rolling {
    /// When it started, seconds since the epoch.
    pub since: u64,
    /// The file (a recording) or the address without its key (a stream).
    pub target: String,
}

/// The whole state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    /// Every camera.
    pub cameras: Vec<CameraSeen>,
    /// The camera the camera layer shows now.
    pub camera: Option<String>,
    /// Shown as a mirror.
    pub mirror: bool,
    /// The scene on air.
    pub scene: String,
    /// Every scene, in order.
    pub scenes: Vec<String>,
    /// Every script.
    pub scripts: Vec<String>,
    /// The script running, if any.
    pub running: Option<Running>,
    /// The layers of the scene on air: where each is, for the app's pointer.
    pub layers: Vec<LayerSeen>,
    /// The camera layer's effect.
    pub effect: Option<String>,
    /// Its values.
    pub effect_values: Vec<i32>,
    /// The camera layer's background mode.
    pub background: Option<String>,
    /// Its values.
    pub background_values: Vec<i32>,
    /// Recording.
    pub recording: Option<Rolling>,
    /// Streaming.
    pub streaming: Option<Rolling>,
    /// The virtual camera is on, and its node.
    pub virtual_node: Option<String>,
    /// Seconds left before a photo.
    pub countdown: Option<u8>,
    /// The last photo taken.
    pub last_photo: Option<String>,
    /// The last recording finished.
    pub last_recording: Option<String>,
    /// Microphone and desktop levels, 0–100, while sound is captured.
    pub levels: [u8; 2],
    /// Microphone and desktop gains, percent.
    pub gains: [u8; 2],
    /// Microphone and desktop muted.
    pub muted: [bool; 2],
    /// Program size and rate.
    pub width: u32,
    /// See [`State::width`].
    pub height: u32,
    /// See [`State::width`].
    pub fps: u32,
    /// Frames composed in the last second.
    pub measured_fps: u32,
    /// What is wrong.
    pub troubles: Vec<Trouble>,
}

/// One layer, as the app's pointer needs it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSeen {
    /// Where.
    pub rect: Rect,
    /// Shown?
    pub visible: bool,
    /// A camera?
    pub camera: bool,
}

/// What the daemon says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "say")]
pub enum Say {
    /// The whole state.
    State(Box<State>),
    /// A preview stream's geometry; the memfd follows on the socket.
    Preview {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// How many slots the ring has.
        slots: u32,
    },
    /// Taken.
    Done,
    /// Not taken, and why.
    Refused(Trouble),
    /// The hands one camera frame showed.
    Hands(Seen),
}

/// The hands of one frame, as the person sees them (mirrored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Seen {
    /// The frame's width over its height: `x` and `y` are each in `0..1` of
    /// their own side, and a distance needs both in one unit.
    pub aspect: f32,
    /// Milliseconds since the daemon started, from a monotonic clock.
    pub at: u64,
    /// Up to two.
    pub hands: Vec<SeenHand>,
}

/// One hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeenHand {
    /// MediaPipe's 21 points: `[x, y, z]`, `x` and `y` in `0..1`.
    pub points: Vec<[f32; 3]>,
    /// The person's right hand.
    pub right: bool,
}

/// One JSON object and a newline, in ONE write.
///
/// Made whole first: serialised straight into a socket, every token is a
/// write of its own, and a reader that pauses for a moment has a line cut in
/// the middle by the writer's deadline — the player's daemon measured exactly
/// that (`ronoctua_player::wire::write_line`), and `a-line-is-one-write`
/// holds every daemon to it.
pub fn write_line<T: Serialize>(out: &mut impl Write, value: &T) -> Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    out.write_all(&line)?;
    out.flush()?;
    Ok(())
}

/// One line, parsed; `None` at the end of the stream.
pub fn read_line<T: DeserializeOwned>(input: &mut impl BufRead) -> Result<Option<T>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(
        serde_json::from_str(line.trim_end()).context("a line that is not an ask")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that counts the writes it was handed, as a socket would.
    struct Counted {
        writes: usize,
        bytes: Vec<u8>,
    }

    impl Write for Counted {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(b);
            Ok(b.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_line_is_one_write() {
        // THE DEFECT THIS CATCHES: serialising into the socket, a write per
        // token — a reader that pauses gets half a line.
        let mut out = Counted {
            writes: 0,
            bytes: Vec::new(),
        };
        let hands = Seen {
            aspect: 1.5,
            at: 7,
            hands: vec![SeenHand {
                points: vec![[0.5, 0.25, 0.0]; 21],
                right: true,
            }],
        };
        write_line(&mut out, &hands).expect("writes");
        assert_eq!(out.writes, 1, "one write for the whole line");
        assert_eq!(out.bytes.last(), Some(&b'\n'));
        assert_eq!(out.bytes.iter().filter(|b| **b == b'\n').count(), 1);
    }

    #[test]
    fn an_ask_reads_back_as_itself() {
        for ask in [
            Ask::Watch,
            Ask::Record { on: None },
            Ask::Place {
                layer: 1,
                rect: Rect {
                    x: 0.5,
                    y: 0.5,
                    w: 0.25,
                    h: 0.25,
                },
            },
            Ask::Sound {
                which: Sound::Desktop,
                gain: Some(80),
                muted: None,
            },
            Ask::EffectParam {
                name: "radius".into(),
                by: -1,
                background: true,
            },
        ] {
            let mut buf = Vec::new();
            write_line(&mut buf, &ask).expect("writes");
            let back: Ask = read_line(&mut buf.as_slice()).expect("reads").expect("one");
            assert_eq!(back, ask);
        }
    }

    #[test]
    fn a_state_with_missing_fields_still_reads() {
        let s: Say = serde_json::from_str(r#"{"say":"state","scene":"camera"}"#).expect("reads");
        let Say::State(s) = s else { panic!("a state") };
        assert_eq!(s.scene, "camera");
        assert!(s.cameras.is_empty());
    }
}
