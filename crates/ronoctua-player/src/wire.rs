//! The player's words: what a client asks and what the daemon answers.
//!
//! The shape of the studio's and the queue's: one JSON object a line, and the
//! WHOLE state on every change — a client that missed one never has to catch
//! up on deltas.

use std::io::{BufRead, Write};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The socket's name under the runtime directory.
pub const SOCKET: &str = "player.sock";

/// Where the daemon listens — one per instance (`.#dev` has its own).
pub fn socket_path() -> Result<std::path::PathBuf> {
    ronoctua_xdg::runtime_file(SOCKET).context("XDG_RUNTIME_DIR is not set")
}

/// What repeats when the queue ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Repeat {
    /// Stop at the end of the queue.
    #[default]
    Nothing,
    /// Play the queue again.
    Queue,
    /// Play the same item again.
    One,
}

impl Repeat {
    /// The next one of the ring.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Nothing => Self::Queue,
            Self::Queue => Self::One,
            Self::One => Self::Nothing,
        }
    }
}

/// A request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "ask")]
pub enum Ask {
    /// Keep this connection and send the whole state on every change.
    Watch,
    /// The state, once.
    Look,
    /// Frames of `width × height` on this connection (a memfd ring follows
    /// the answer). The width is rounded down to a multiple of 16.
    Preview {
        /// Pixels across.
        width: u32,
        /// Pixels down.
        height: u32,
        /// WHOSE picture: `None` is the item playing, `Some(i)` the i-th
        /// participant of the session.
        #[serde(default)]
        of: Option<usize>,
    },
    /// Put these on the queue — paths or URLs. `replace` starts over with
    /// them and plays the first; otherwise they go at the end.
    Open {
        /// What to play.
        items: Vec<String>,
        /// Start over with them.
        replace: bool,
    },
    /// Play or pause.
    Toggle,
    /// Play.
    Play,
    /// Pause.
    Pause,
    /// Stop and forget the position.
    Stop,
    /// The next item.
    Next,
    /// The previous item.
    Previous,
    /// Play the item at `index`.
    Jump {
        /// Its place in the queue.
        index: usize,
    },
    /// Take the item at `index` off the queue.
    Remove {
        /// Its place.
        index: usize,
    },
    /// Move an item.
    Move {
        /// From here.
        from: usize,
        /// To here.
        to: usize,
    },
    /// Empty the queue.
    Clear,
    /// Go to `seconds`, or move by them when `relative`.
    Seek {
        /// Where, or by how much.
        seconds: f64,
        /// By how much, rather than where.
        relative: bool,
    },
    /// The volume, in percent (up to 130).
    Volume {
        /// Percent.
        percent: u8,
    },
    /// Mute or unmute.
    Mute,
    /// The speed, 0.25 to 4.
    Speed {
        /// A factor.
        factor: f64,
    },
    /// The next repeat mode.
    Repeat,
    /// Shuffle on or off.
    Shuffle,
    /// The audio track, by its id; `None` turns sound off.
    AudioTrack {
        /// mpv's id.
        id: Option<i64>,
    },
    /// The subtitle track; `None` hides subtitles.
    SubtitleTrack {
        /// mpv's id.
        id: Option<i64>,
    },
    /// Read the settings again.
    Reload,
    /// A participant joins the session: another file, video only, kept in
    /// step with the audio of the one playing.
    Follow {
        /// What it plays.
        item: String,
    },
    /// A participant leaves.
    Unfollow {
        /// Its place among the participants.
        index: usize,
    },
    /// Show a pairing code for two minutes, so another machine can pair.
    Invite,
    /// Pair with another machine: its tailnet name (or address) and the code
    /// it shows.
    PairWith {
        /// A name on the tailnet, or an address.
        to: String,
        /// The six digits.
        code: String,
    },
    /// Play in step with a paired machine, as `role`.
    Join {
        /// The paired machine, by name.
        host: String,
        /// Picture, sound or both.
        role: crate::remote::Role,
    },
    /// Stop playing in step with another machine.
    Leave,
    /// Forget a paired machine, in both directions.
    Forget {
        /// Its name.
        name: String,
    },
}

/// Why something does not work — each a sentence on screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "trouble")]
pub enum Trouble {
    /// The file could not be played.
    Unplayable {
        /// What mpv said.
        why: String,
    },
    /// A decoder this format needs is not installed.
    NoDecoder {
        /// Its program.
        program: String,
    },
    /// This format is known and nothing here can play it.
    Unsupported {
        /// The format's name.
        format: String,
    },
    /// The engine could not start.
    NoEngine {
        /// Why.
        why: String,
    },
    /// Another machine could not be reached, or said no.
    Remote {
        /// What happened, in the transport's words.
        why: String,
    },
}

/// A track of the playing file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// mpv's id.
    pub id: i64,
    /// `audio`, `video` or `sub`.
    pub kind: String,
    /// Its title, when it has one.
    pub title: Option<String>,
    /// Its language, when declared.
    pub lang: Option<String>,
    /// Its codec.
    pub codec: Option<String>,
    /// It is the one in use.
    pub selected: bool,
}

/// An item of the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// The path or URL.
    pub location: String,
    /// Its title, once known.
    pub title: Option<String>,
}

/// A line of the lyrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// When it is sung, seconds; `None` for unsynchronised lyrics.
    pub at: Option<f64>,
    /// The words.
    pub text: String,
}

/// A participant of the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    /// What it plays.
    pub item: String,
    /// How far behind (negative) or ahead it is, milliseconds.
    pub drift_ms: i64,
}

/// Playing in step with another machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joined {
    /// Its name.
    pub host: String,
    /// What this machine plays of it.
    pub role: crate::remote::Role,
    /// How far this machine is from it, in milliseconds.
    pub drift_ms: i64,
}

/// The whole state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// The queue.
    pub queue: Vec<Item>,
    /// Which item is playing.
    pub current: Option<usize>,
    /// It is playing (not paused, not stopped).
    pub playing: bool,
    /// Seconds into the item.
    pub position: f64,
    /// Its length, when known.
    pub duration: Option<f64>,
    /// Volume, percent.
    pub volume: u8,
    /// Muted.
    pub muted: bool,
    /// Speed factor.
    pub speed: f64,
    /// Repeat mode.
    pub repeat: Repeat,
    /// Shuffled.
    pub shuffle: bool,
    /// The item's title.
    pub title: Option<String>,
    /// Its artist.
    pub artist: Option<String>,
    /// Its album.
    pub album: Option<String>,
    /// The picture's size, when it has a picture.
    pub video: Option<(u32, u32)>,
    /// Its tracks.
    pub tracks: Vec<Track>,
    /// Its lyrics, when found.
    pub lyrics: Vec<LyricLine>,
    /// Where the lyrics came from (`file`, `tag`, `lrclib`).
    pub lyrics_from: Option<String>,
    /// The session's other participants.
    pub participants: Vec<Participant>,
    /// What is wrong now.
    pub troubles: Vec<Trouble>,
    /// The pairing code this machine is showing, while it lasts.
    pub invite: Option<String>,
    /// This machine's name on the tailnet, when it is on one.
    pub tailnet: Option<String>,
    /// The other machines on the tailnet that are online — who can be paired.
    pub nearby: Vec<String>,
    /// Paired machines this one can join.
    pub hosts: Vec<String>,
    /// Paired machines that can join this one.
    pub guests: Vec<String>,
    /// The machine this one plays in step with, when it does.
    pub joined: Option<Joined>,
}

/// An answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "say")]
pub enum Say {
    /// The whole state.
    State(Box<State>),
    /// A preview's geometry; the memfd came with this line.
    Preview {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// Slots in the ring.
        slots: u32,
    },
    /// Taken.
    Done,
    /// Not taken, and why.
    Refused(Trouble),
}

/// One JSON object and a newline.
pub fn write_line<T: Serialize>(out: &mut impl Write, value: &T) -> Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

/// One line, parsed; `None` at the end of the stream.
pub fn read_line<T: DeserializeOwned>(input: &mut impl BufRead) -> Result<Option<T>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(line.trim_end())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ask_and_an_answer_round_trip() {
        let a = Ask::Open {
            items: vec!["/music/a.flac".into()],
            replace: true,
        };
        let s = serde_json::to_string(&a).expect("serialises");
        assert!(s.contains("\"ask\":\"open\""), "{s}");
        assert_eq!(serde_json::from_str::<Ask>(&s).expect("parses"), a);
        let state = Say::State(Box::default());
        let s = serde_json::to_string(&state).expect("serialises");
        assert_eq!(serde_json::from_str::<Say>(&s).expect("parses"), state);
    }

    #[test]
    fn the_repeat_ring_comes_back() {
        assert_eq!(Repeat::Nothing.next().next().next(), Repeat::Nothing);
    }
}
