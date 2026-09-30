//! Playing on two machines at once: the words between two daemons, and the
//! rules they follow. The sockets are the engine's (`ronoctua-player-engine`'s
//! `remote.rs`); everything here is pure and tested.
//!
//! # Only the tailnet
//!
//! The daemon listens on its TAILNET addresses and on nothing else, and it
//! refuses a connection from outside the tailnet's ranges even so. The tailnet
//! is already encrypted and already authenticated machine to machine; what is
//! added here is that a machine of the SAME tailnet still has to be paired.
//!
//! # Pairing is a code a person reads aloud
//!
//! One machine shows six digits for two minutes; the other is told the first
//! one's name and the digits. What comes back is a key — 32 random bytes —
//! that both keep in `player-peers.json` (0600), and every later request
//! carries it. A code is used once.
//!
//! # One clock, the one that plays the sound
//!
//! The machine that is joined is the master. A joining machine measures the
//! offset between the two clocks once (eight round trips, the median —
//! `sync::offset`), then asks where the master is four times a second and
//! corrects itself with the same rule a video in step on one machine follows
//! (`sync::correct`).
//!
//! # The media travels as HTTP, lane by lane
//!
//! What the master plays is lanes ([`Moment::lanes`]), and a file of one is
//! served on the same port, `GET /media/<leaf>/<lane>` with the key as a
//! bearer token and `Range` honoured — mpv seeks by asking for a range. Two
//! lanes of one file share one path, so the joining machine opens the file
//! once, as the master does. An address (YouTube, a stream) is handed over as
//! the address: the joining machine fetches it itself.
//!
//! # The role picks the lanes
//!
//! A screen takes the pictures and their subtitles, a speaker the sounds,
//! and both take everything ([`Role::plays`]). The joining machine's own
//! outputs and screens are its own: the master's are not handed over.

use ronoctua_playlist::own::{Lane, LaneId, LaneKind, Stack};
use serde::{Deserialize, Serialize};

/// The port both daemons use, unless the unit says another
/// (`RONOCTUA_PLAYER_PORT`, from `ronoctua.player.port`).
pub const PORT: u16 = 45_517;

/// The file the peers are kept in, in `$XDG_STATE_HOME/ronoctua`.
pub const PEERS_FILE: &str = "player-peers.json";

/// How long a pairing code is good for.
pub const CODE_LIFE: std::time::Duration = std::time::Duration::from_secs(120);

/// How many round trips the clock offset is measured from.
pub const CLOCK_SAMPLES: usize = 8;

/// The port, from the environment when the unit set one.
#[must_use]
pub fn port() -> u16 {
    std::env::var("RONOCTUA_PLAYER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(PORT)
}

/// What a machine can be when it joins another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Picture and sound.
    #[default]
    Both,
    /// Only the picture: the sound comes out of the other machine.
    Screen,
    /// Only the sound: a speaker in another room.
    Sound,
}

impl Role {
    /// The next role, for a ring.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Both => Self::Screen,
            Self::Screen => Self::Sound,
            Self::Sound => Self::Both,
        }
    }

    /// Whether a machine joining as this plays a lane of `kind`.
    #[must_use]
    pub const fn plays(self, kind: LaneKind) -> bool {
        match self {
            Self::Both => true,
            Self::Screen => matches!(kind, LaneKind::Video | LaneKind::Subtitle),
            Self::Sound => matches!(kind, LaneKind::Audio),
        }
    }
}

/// A knock on another daemon's door.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "knock")]
pub enum Knock {
    /// "The person read me your code; here is my name."
    Pair {
        /// The six digits.
        code: String,
        /// The knocking machine's name, to show on the other side.
        name: String,
    },
    /// "What time is it on your clock?"
    Time {
        /// The key.
        key: String,
    },
    /// "Where are you?"
    Now {
        /// The key.
        key: String,
    },
}

/// Where the master is, at one instant of its own clock.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Moment {
    /// Its clock, in milliseconds since the epoch, when this was read.
    pub clock_ms: f64,
    /// The master's current leaf, by its entry id — which is also its name in
    /// the media paths, and stays the same when the master's queue is
    /// reordered under it; `None` with nothing loaded.
    pub item: Option<u64>,
    /// Where in it, in seconds of its own clock (a set's).
    pub position: f64,
    /// Whether it plays.
    pub playing: bool,
    /// At what speed.
    pub speed: f64,
    /// What it is called, to show.
    pub title: Option<String>,
    /// What it plays: its lanes, each with where to open it.
    #[serde(default)]
    pub lanes: Vec<RemoteLane>,
}

/// Where a joining machine opens one of the master's lanes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "from")]
pub enum RemoteSource {
    /// An address, opened as itself.
    Address {
        /// The address.
        url: String,
    },
    /// One of the master's files, served at `path` on its port.
    Media {
        /// `/media/<leaf>/<lane>` ([`media_path`]).
        path: String,
        /// The file's own name: its extension says whether the joining
        /// machine needs one of our decoders for it.
        name: String,
    },
}

/// One of the master's lanes, as a joining machine plays it: where to open
/// it, and the stretch of the set's clock it plays — never the master's
/// outputs or screens, which are that machine's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteLane {
    /// Its name in the master's set.
    pub id: LaneId,
    /// What it contributes.
    pub kind: LaneKind,
    /// Where to open it.
    pub source: RemoteSource,
    /// Which of its source's tracks.
    #[serde(default)]
    pub track: Option<i64>,
    /// Its title.
    #[serde(default)]
    pub title: Option<String>,
    /// Its language.
    #[serde(default)]
    pub lang: Option<String>,
    /// When it begins on the set's clock, seconds.
    #[serde(default)]
    pub start_seconds: f64,
    /// Where in its source it begins, seconds.
    #[serde(default)]
    pub from_seconds: f64,
    /// Where in its source it stops, seconds.
    #[serde(default)]
    pub until_seconds: Option<f64>,
    /// It plays.
    pub enabled: bool,
    /// Its own volume, percent.
    pub volume_percent: u8,
    /// Its own mute.
    #[serde(default)]
    pub muted: bool,
    /// For subtitles: the picture they go over.
    #[serde(default)]
    pub over: Option<LaneId>,
    /// For subtitles: where the lines sit.
    #[serde(default)]
    pub stack: Stack,
}

/// Where the master serves the file of `lane` of its leaf `leaf`.
#[must_use]
pub fn media_path(leaf: u64, lane: LaneId) -> String {
    format!("/media/{leaf}/{}", lane.0)
}

/// The leaf and the lane a request's path names ([`media_path`]); `None` for
/// anything else — a path with more, or less, or not numbers.
#[must_use]
pub fn media_of(path: &str) -> Option<(u64, LaneId)> {
    let rest = path.strip_prefix("/media/")?;
    let (leaf, lane) = rest.split_once('/')?;
    Some((leaf.parse().ok()?, LaneId(lane.parse().ok()?)))
}

/// The master's lanes, as it hands them over: an address as itself, a file
/// at the path of the FIRST lane that plays it — so two lanes of one file are
/// one download, and one player, on the joining machine too.
#[must_use]
pub fn lanes_of(leaf: u64, lanes: &[Lane]) -> Vec<RemoteLane> {
    lanes
        .iter()
        .map(|lane| {
            let source = if lane.source.contains("://") {
                RemoteSource::Address {
                    url: lane.source.clone(),
                }
            } else {
                let first = lanes
                    .iter()
                    .find(|l| l.source == lane.source)
                    .map_or(lane.id, |l| l.id);
                RemoteSource::Media {
                    path: media_path(leaf, first),
                    name: crate::queue::file_name(&lane.source).to_owned(),
                }
            };
            RemoteLane {
                id: lane.id,
                kind: lane.kind,
                source,
                track: lane.track,
                title: lane.title.clone(),
                lang: lane.lang.clone(),
                start_seconds: lane.start_seconds,
                from_seconds: lane.from_seconds,
                until_seconds: lane.until_seconds,
                enabled: lane.enabled,
                volume_percent: lane.volume_percent,
                muted: lane.muted,
                over: lane.over,
                stack: lane.stack,
            }
        })
        .collect()
}

/// The lanes a machine joining as `role` plays, each opened where `open`
/// says (the address itself, the master's path as a URL, or a copy fetched
/// whole): with this machine's own default output and every screen.
///
/// A subtitle lane over a picture this machine does not play goes too — it
/// is laid over the first picture that is left, and a speaker plays none.
#[must_use]
pub fn joined_lanes(
    role: Role,
    lanes: &[RemoteLane],
    open: impl Fn(&RemoteSource) -> String,
) -> Vec<Lane> {
    let kept: Vec<&RemoteLane> = lanes.iter().filter(|l| role.plays(l.kind)).collect();
    kept.iter()
        .map(|remote| {
            let mut lane = Lane::new(remote.id, remote.kind, open(&remote.source));
            lane.track = remote.track;
            lane.title.clone_from(&remote.title);
            lane.lang.clone_from(&remote.lang);
            lane.start_seconds = remote.start_seconds;
            lane.from_seconds = remote.from_seconds;
            lane.until_seconds = remote.until_seconds;
            lane.enabled = remote.enabled;
            lane.volume_percent = remote.volume_percent;
            lane.muted = remote.muted;
            lane.over = remote.over.filter(|over| {
                kept.iter()
                    .any(|k| k.id == *over && k.kind == LaneKind::Video)
            });
            lane.stack = remote.stack;
            lane
        })
        .collect()
}

/// The answer to a knock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "answer")]
pub enum Answer {
    /// Paired: the key to carry from now on.
    Paired {
        /// The key.
        key: String,
        /// The answering machine's name.
        name: String,
    },
    /// The master's clock.
    Time {
        /// Milliseconds since the epoch.
        ms: f64,
    },
    /// Where the master is.
    Now(Moment),
    /// No: a wrong or old code, an unknown key.
    Refused,
}

/// A machine this one is paired with.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Peer {
    /// Its name, as it said it.
    pub name: String,
    /// Where to reach it — only for the machines this one JOINS.
    pub address: Option<String>,
    /// The key.
    pub key: String,
}

/// Every pairing, both directions.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Peers {
    /// Machines allowed to join this one.
    pub guests: Vec<Peer>,
    /// Machines this one can join.
    pub hosts: Vec<Peer>,
}

impl Peers {
    /// Whether `key` is a guest's.
    #[must_use]
    pub fn admits(&self, key: &str) -> bool {
        !key.is_empty() && self.guests.iter().any(|p| p.key == key)
    }
}

/// Is `ip` on a tailnet? Tailscale gives out 100.64.0.0/10 and
/// fd7a:115c:a1e0::/48, and nothing else is let in.
#[must_use]
pub fn on_the_tailnet(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (o[1] & 0xC0) == 64
        }
        std::net::IpAddr::V6(v6) => {
            let s = v6.segments();
            s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0
        }
    }
}

/// Six digits out of four random bytes.
#[must_use]
pub fn code_of(random: [u8; 4]) -> String {
    format!("{:06}", u32::from_le_bytes(random) % 1_000_000)
}

/// A key out of 32 random bytes, as hex.
#[must_use]
pub fn key_of(random: &[u8; 32]) -> String {
    random.iter().map(|b| format!("{b:02x}")).collect()
}

/// Random bytes from the kernel.
///
/// # Errors
///
/// `/dev/urandom` could not be read — on Linux, never in practice.
pub fn random<const N: usize>() -> std::io::Result<[u8; N]> {
    use std::io::Read as _;
    let mut out = [0u8; N];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut out)?;
    Ok(out)
}

/// Codes compared in constant time: a code is a secret for two minutes.
#[must_use]
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The byte range a `Range:` header asks of a file `len` long: inclusive
/// `(first, last)`. `None` for no header or one this server does not answer
/// (several ranges), which is then the whole file.
#[must_use]
pub fn range_of(header: Option<&str>, len: u64) -> Option<(u64, u64)> {
    let spec = header?.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (first, last) = match (a.trim(), b.trim()) {
        ("", n) => {
            // THE LAST n BYTES.
            let n: u64 = n.parse().ok()?;
            (len.saturating_sub(n), len - 1)
        }
        (f, "") => (f.parse().ok()?, len - 1),
        (f, l) => (f.parse().ok()?, l.parse::<u64>().ok()?.min(len - 1)),
    };
    (first <= last && first < len).then_some((first, last))
}

/// Where the master is NOW, from a moment it reported and the offset between
/// the two clocks (`theirs + offset = ours`).
#[must_use]
pub fn master_now(m: &Moment, now_ms: f64, offset_ms: f64) -> f64 {
    if !m.playing {
        return m.position;
    }
    let their_now = now_ms - offset_ms;
    m.position + ((their_now - m.clock_ms) / 1000.0).max(0.0) * m.speed
}

/// Milliseconds since the epoch, as a float — the clock both sides compare.
#[must_use]
pub fn clock_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// The store.
#[must_use]
pub fn peers() -> ronoctua_store::Store<Peers> {
    ronoctua_store::Store::at(ronoctua_store::state_dir(), PEERS_FILE)
}

/// Writes the peers, private from the FIRST byte: every key in it lets a
/// machine into this one's media.
///
/// It used to go through the store and be `chmod`ded after, which left the
/// keys in a file anyone on the machine could read for as long as the two
/// calls were apart — and a file somebody had left readable stayed readable
/// until then. It is written the way the person's own keys are
/// (`ronoctua_lookup::keys::write_private_file`): a draft created `0600`,
/// then renamed over the file. Same JSON the store reads.
///
/// # Errors
///
/// The file could not be written.
pub fn keep(p: &Peers) -> anyhow::Result<()> {
    keep_at(peers().path().as_deref(), p)
}

/// [`keep`], into `path` (`None`: no home, nothing kept — as the store does).
fn keep_at(path: Option<&std::path::Path>, p: &Peers) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(p)?;
    ronoctua_lookup::keys::write_private_file(path, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    /// FAILS IF: the paired machines' keys are ever in a file anyone can
    /// read — written and THEN made private (the defect this replaced), or
    /// left readable because the file before them was.
    #[test]
    fn the_peers_are_private_from_their_first_byte() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let dir = std::env::temp_dir().join(format!("ronoctua-peers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(PEERS_FILE);
        std::fs::create_dir_all(&dir).expect("a folder");
        std::fs::write(&path, "{}").expect("an old file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("readable");
        let p = Peers {
            hosts: vec![Peer {
                name: "other".into(),
                address: Some("100.64.0.2".into()),
                key: "k".repeat(64),
            }],
            guests: Vec::new(),
        };
        keep_at(Some(&path), &p).expect("kept");
        let mode = std::fs::metadata(&path).expect("there").mode() & 0o777;
        assert_eq!(mode, 0o600);
        let back: Peers =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("the store's JSON");
        assert_eq!(back.hosts.len(), 1);
        let left = std::fs::read_dir(&dir).expect("listed").count();
        assert_eq!(left, 1, "no draft is left");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_the_tailnets_ranges_are_let_in() {
        for yes in ["100.64.0.1", "100.127.255.254", "fd7a:115c:a1e0::1"] {
            assert!(
                on_the_tailnet(yes.parse::<IpAddr>().expect("an address")),
                "{yes}"
            );
        }
        for no in [
            "100.63.255.255",
            "100.128.0.1",
            "192.168.0.2",
            "127.0.0.1",
            "::1",
            "fd7a:115c:a1e1::1",
        ] {
            assert!(
                !on_the_tailnet(no.parse::<IpAddr>().expect("an address")),
                "{no}"
            );
        }
    }

    #[test]
    fn a_code_is_six_digits_and_a_key_is_sixty_four_hex() {
        assert_eq!(code_of([0, 0, 0, 0]), "000000");
        assert_eq!(code_of([0xff; 4]).len(), 6);
        assert_eq!(key_of(&[0xab; 32]).len(), 64);
        let r: [u8; 32] = random().expect("the kernel's randomness");
        assert_ne!(r, [0; 32]);
    }

    #[test]
    fn codes_compare_whole() {
        assert!(same("123456", "123456"));
        assert!(!same("123456", "123457"));
        assert!(!same("12345", "123456"));
    }

    #[test]
    fn a_range_is_what_mpv_asks_and_nothing_stranger() {
        assert_eq!(range_of(Some("bytes=0-"), 1000), Some((0, 999)));
        assert_eq!(range_of(Some("bytes=100-199"), 1000), Some((100, 199)));
        assert_eq!(range_of(Some("bytes=900-5000"), 1000), Some((900, 999)));
        assert_eq!(range_of(Some("bytes=-100"), 1000), Some((900, 999)));
        assert_eq!(range_of(Some("bytes=1000-"), 1000), None, "past the end");
        assert_eq!(range_of(Some("bytes=0-1,5-9"), 1000), None, "several");
        assert_eq!(range_of(None, 1000), None);
    }

    #[test]
    fn the_master_moves_on_while_it_plays_and_stands_while_it_does_not() {
        let m = Moment {
            clock_ms: 10_000.0,
            position: 30.0,
            playing: true,
            speed: 1.0,
            ..Moment::default()
        };
        // Our clock is 500 ms ahead of theirs; 2 s of theirs have passed.
        assert!((master_now(&m, 12_500.0, 500.0) - 32.0).abs() < 1e-9);
        let stopped = Moment {
            playing: false,
            ..m
        };
        assert!((master_now(&stopped, 99_999.0, 0.0) - 30.0).abs() < 1e-9);
    }

    /// A lane of `kind` playing `source`.
    fn lane(id: u32, kind: LaneKind, source: &str) -> Lane {
        Lane::new(LaneId(id), kind, source)
    }

    /// FAILS IF: a lane's file is served at a path that does not name it, a
    /// path is read as a lane it does not name, or two lanes of one file are
    /// two downloads on the joining machine.
    #[test]
    fn a_lane_travels_by_its_leaf_and_its_file() {
        assert_eq!(media_path(7, LaneId(3)), "/media/7/3");
        assert_eq!(media_of("/media/7/3"), Some((7, LaneId(3))));
        for no in [
            "/media/7",
            "/media/7/3/4",
            "/media/x/3",
            "/media/7/",
            "/other/7/3",
        ] {
            assert_eq!(media_of(no), None, "{no}");
        }
        let lanes = vec![
            lane(1, LaneKind::Video, "/m/film.mkv"),
            lane(2, LaneKind::Audio, "/m/film.mkv"),
            lane(4, LaneKind::Audio, "https://radio.example/live"),
            lane(5, LaneKind::Subtitle, "/m/film.srt"),
        ];
        let remote = lanes_of(9, &lanes);
        let path = |i: usize| match &remote[i].source {
            RemoteSource::Media { path, .. } => path.clone(),
            RemoteSource::Address { url } => url.clone(),
        };
        assert_eq!(path(0), "/media/9/1");
        assert_eq!(path(1), "/media/9/1", "one file, one path");
        assert_eq!(
            path(2),
            "https://radio.example/live",
            "an address is itself"
        );
        assert_eq!(path(3), "/media/9/5");
        assert_eq!(
            remote[3].source,
            RemoteSource::Media {
                path: "/media/9/5".into(),
                name: "film.srt".into()
            }
        );
    }

    /// FAILS IF: a machine joining as a screen plays a sound, a speaker a
    /// picture, or the master's outputs travel with its lanes.
    #[test]
    fn the_role_picks_the_lanes_and_the_outputs_stay_home() {
        let mut dub = lane(3, LaneKind::Audio, "/m/film.mkv");
        dub.sink = Some("bluez_output.headphones".into());
        let mut words = lane(4, LaneKind::Subtitle, "/m/film.srt");
        words.over = Some(LaneId(1));
        let lanes = vec![
            lane(1, LaneKind::Video, "/m/film.mkv"),
            lane(2, LaneKind::Audio, "/m/film.mkv"),
            dub,
            words,
        ];
        let remote = lanes_of(1, &lanes);
        let open = |s: &RemoteSource| match s {
            RemoteSource::Media { path, .. } => format!("http://host{path}"),
            RemoteSource::Address { url } => url.clone(),
        };
        let kinds = |role| {
            joined_lanes(role, &remote, open)
                .iter()
                .map(|l| (l.id.0, l.kind))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds(Role::Screen),
            vec![(1, LaneKind::Video), (4, LaneKind::Subtitle)]
        );
        assert_eq!(
            kinds(Role::Sound),
            vec![(2, LaneKind::Audio), (3, LaneKind::Audio)]
        );
        let both = joined_lanes(Role::Both, &remote, open);
        assert_eq!(both.len(), 4);
        assert_eq!(both[0].source, "http://host/media/1/1");
        assert!(
            both.iter().all(|l| l.sink.is_none()),
            "the master's outputs travelled"
        );
        assert_eq!(both[3].over, Some(LaneId(1)));
    }

    #[test]
    fn a_knock_and_an_answer_round_trip() {
        let k = Knock::Now { key: "k".into() };
        let s = serde_json::to_string(&k).expect("serialises");
        assert!(s.contains("\"knock\":\"now\""), "{s}");
        assert_eq!(serde_json::from_str::<Knock>(&s).expect("parses"), k);
        let a = Answer::Now(Moment::default());
        let s = serde_json::to_string(&a).expect("serialises");
        assert_eq!(serde_json::from_str::<Answer>(&s).expect("parses"), a);
    }

    #[test]
    fn a_guest_is_admitted_by_its_key_and_an_empty_key_never() {
        let p = Peers {
            guests: vec![Peer {
                name: "den".into(),
                address: None,
                key: "abc".into(),
            }],
            hosts: Vec::new(),
        };
        assert!(p.admits("abc"));
        assert!(!p.admits("abd"));
        assert!(!p.admits(""));
    }
}
