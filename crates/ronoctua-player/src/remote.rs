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
//! # The media travels as HTTP
//!
//! A file on the master is served on the same port, `GET /media/<index>` with
//! the key as a bearer token and `Range` honoured — mpv seeks by asking for a
//! range. An address (YouTube, a stream) is handed over as the address: the
//! joining machine fetches it itself.

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
    /// What to open: an address, or `None` for a file served as
    /// `/media/<item>`.
    pub address: Option<String>,
    /// The item's index in the master's queue, which is also its name in the
    /// media path; `None` with nothing loaded.
    pub item: Option<usize>,
    /// Where in it, in seconds.
    pub position: f64,
    /// Whether it plays.
    pub playing: bool,
    /// At what speed.
    pub speed: f64,
    /// What it is called, to show.
    pub title: Option<String>,
    /// The file's own name, when it is a file: its extension is what says
    /// whether the following machine needs one of our decoders for it.
    #[serde(default)]
    pub name: Option<String>,
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

/// Writes the peers and keeps the file private: every key in it lets a
/// machine into this one's media.
///
/// # Errors
///
/// The file could not be written or its mode set.
pub fn keep(p: &Peers) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let store = peers();
    store.write(p)?;
    if let Some(path) = store.path() {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

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
