//! Ambient sounds: real recordings — rain, thunder, water — that go round
//! with no seam and sound ALONGSIDE the queue, several at once, each at its
//! own volume.
//!
//! # Where they come from
//!
//! `assets/ambience.nix` declares them and `nix/ambience.nix` makes each
//! loop from its approved original, measures its seam through the pinned
//! mpv, and writes `index.json` beside the loops. The player's unit names
//! that directory ([`VARIABLE`]); a machine without it has no ambient sounds,
//! and the state says so ([`Trouble::NoAmbience`]).
//!
//! # Apart from the queue
//!
//! One mpv per sound that is on, and none for one that is off. What the
//! queue is told — stop, next, the media keys, the player's own volume —
//! never reaches them; a sound stops when it is turned off, when every sound
//! is silenced, or when the player closes. Choosing one plays it at once, and
//! it goes round until then. Nothing is on when the player starts: a volume
//! is remembered, never that a sound was on.
//!
//! This module is the mixer's RULES — which sound is on, at what volume,
//! whether they are held — as effects the engine carries out on its mpvs
//! ([`Effect`]); nothing here opens anything.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    limit::{Limit, TooMany},
    own::VOLUME_MOST_PERCENT,
    wire::{Ambience, Credit, Licence, Sounding, Trouble},
};

/// The variable the player's unit names the loops' directory in.
pub const VARIABLE: &str = "RONOCTUA_AMBIENCE";

/// What `index.json` says it is.
pub const FORMAT: &str = "ronoctua-ambience";

/// The version of `index.json` this player reads.
pub const VERSION: u32 = 1;

/// The one way round this player plays a loop: mpv looping the file itself.
/// `nix/ambience.nix` measured it exact, sample for sample, across two seams
/// with the pinned mpv — and an index that names another is one this player
/// cannot keep its promise with, so it is refused.
pub const ROUTE: &str = "loop-file";

/// The volume a sound nobody set plays at: its loop is already made to the
/// loudness `assets/ambience.nix` gives it.
pub const VOLUME_FIRST_PERCENT: u8 = 100;

/// The index `nix/ambience.nix` writes, as far as the player reads it.
#[derive(Debug, Deserialize)]
struct Index {
    #[serde(default)]
    version: u32,
    at_once: u8,
    route: String,
    sounds: Vec<Entry>,
}

/// One sound of the index.
#[derive(Debug, Deserialize)]
struct Entry {
    id: String,
    group: String,
    file: String,
    author: String,
    page: String,
    licence: Licence,
}

/// One sound this machine has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    /// Its id: the text key `player-ambience-<id>` names it.
    pub id: String,
    /// Its group: `player-ambience-group-<group>`.
    pub group: String,
    /// Its loop.
    pub path: PathBuf,
    /// Who recorded it, where, and under what licence.
    pub credit: Credit,
}

/// The sounds this machine has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogue {
    /// How many may sound at once.
    pub at_once: u8,
    /// Every sound, in the declared order.
    pub sounds: Vec<Sound>,
}

/// The catalogue the unit names ([`VARIABLE`]); a machine that names none
/// has none, which is said and not guessed at.
pub fn from_env() -> Result<Catalogue, Trouble> {
    match std::env::var_os(VARIABLE) {
        Some(dir) if !dir.is_empty() => read(Path::new(&dir)),
        _ => Err(Trouble::NoAmbience),
    }
}

/// The catalogue in `dir`, from its `index.json`.
pub fn read(dir: &Path) -> Result<Catalogue, Trouble> {
    let index = dir.join("index.json");
    let path = index.to_string_lossy().into_owned();
    let text = std::fs::read_to_string(&index).map_err(|_| Trouble::Unreadable { path })?;
    parse(&text, dir)
}

/// The catalogue an index says, its loops in `dir`. Refused whole when any
/// of it is not what `nix/ambience.nix` writes — never read in part.
pub fn parse(text: &str, dir: &Path) -> Result<Catalogue, Trouble> {
    let path = dir.join("index.json").to_string_lossy().into_owned();
    let damaged = || Trouble::Damaged { path: path.clone() };
    let raw: serde_json::Value = serde_json::from_str(text).map_err(|_| damaged())?;
    if raw.get("format").and_then(serde_json::Value::as_str) != Some(FORMAT) {
        return Err(Trouble::NotOurs { path });
    }
    let index: Index = serde_json::from_value(raw).map_err(|_| damaged())?;
    if index.version > VERSION {
        return Err(Trouble::Newer {
            path,
            found: index.version,
        });
    }
    if index.route != ROUTE || index.at_once == 0 {
        return Err(damaged());
    }
    let mut sounds: Vec<Sound> = Vec::with_capacity(index.sounds.len());
    for e in index.sounds {
        // A LOOP IS A FILE BESIDE THE INDEX, by its bare name: nothing the
        // index says reaches outside its own directory.
        let bare =
            !e.file.is_empty() && !e.file.starts_with('.') && !e.file.contains(['/', '\\', '\0']);
        if !bare || e.id.is_empty() || sounds.iter().any(|s| s.id == e.id) {
            return Err(damaged());
        }
        sounds.push(Sound {
            path: dir.join(&e.file),
            id: e.id,
            group: e.group,
            credit: Credit {
                author: e.author,
                page: e.page,
                licence: e.licence,
            },
        });
    }
    Ok(Catalogue {
        at_once: index.at_once,
        sounds,
    })
}

/// What the engine does to its mpvs for a change of the mixer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Open a sound's mpv and play its loop, round and round.
    Start {
        /// The sound.
        id: String,
        /// Its loop.
        path: PathBuf,
        /// At this volume.
        volume_percent: u8,
    },
    /// Close a sound's mpv.
    Stop {
        /// The sound.
        id: String,
    },
    /// A sound that plays goes to this volume.
    Volume {
        /// The sound.
        id: String,
        /// Percent.
        percent: u8,
    },
    /// Every sound that is on is held, or goes on.
    Hold(bool),
}

/// One sound of the mixer.
#[derive(Debug, Clone)]
struct Channel {
    sound: Sound,
    on: bool,
    volume_percent: u8,
    rounds: u64,
}

/// The mixer: which sounds are on, how loud each is, whether they are held.
#[derive(Debug, Clone)]
pub struct Mixer {
    channels: Vec<Channel>,
    at_once: u8,
    held: bool,
    /// Why this machine has no sounds, when it has none.
    trouble: Option<Trouble>,
}

impl Mixer {
    /// A mixer over what the machine has, every sound off, each at the
    /// volume it was left at (`volumes`, by id).
    #[must_use]
    pub fn new(catalogue: Result<Catalogue, Trouble>, volumes: &BTreeMap<String, u8>) -> Self {
        match catalogue {
            Ok(c) => Self {
                channels: c
                    .sounds
                    .into_iter()
                    .map(|sound| Channel {
                        volume_percent: volumes
                            .get(&sound.id)
                            .copied()
                            .unwrap_or(VOLUME_FIRST_PERCENT)
                            .min(VOLUME_MOST_PERCENT),
                        sound,
                        on: false,
                        rounds: 0,
                    })
                    .collect(),
                at_once: c.at_once,
                held: false,
                trouble: None,
            },
            Err(t) => Self {
                channels: Vec::new(),
                at_once: 0,
                held: false,
                trouble: Some(t),
            },
        }
    }

    /// The channel of `id`, or why there is none.
    fn channel(&mut self, id: &str) -> Result<&mut Channel, Trouble> {
        if let Some(t) = &self.trouble {
            return Err(t.clone());
        }
        self.channels
            .iter_mut()
            .find(|c| c.sound.id == id)
            .ok_or_else(|| Trouble::NoSuchSound { id: id.to_owned() })
    }

    /// How many sounds are on.
    fn sounding(&self) -> usize {
        self.channels.iter().filter(|c| c.on).count()
    }

    /// Turns `id` on — it plays at once, and whatever was held goes on with
    /// it — or off. More than [`Catalogue::at_once`] is refused with the
    /// number.
    pub fn turn(&mut self, id: &str, on: bool) -> Result<Vec<Effect>, Trouble> {
        let at_once = self.at_once;
        let sounding = self.sounding();
        let held = self.held;
        let channel = self.channel(id)?;
        let mut effects = Vec::new();
        let mut start = None;
        match (channel.on, on) {
            (false, true) => {
                if sounding >= usize::from(at_once) {
                    return Err(Trouble::TooMany(TooMany {
                        what: Limit::Ambience,
                        most: usize::from(at_once),
                    }));
                }
                channel.on = true;
                channel.rounds = 0;
                start = Some(Effect::Start {
                    id: id.to_owned(),
                    path: channel.sound.path.clone(),
                    volume_percent: channel.volume_percent,
                });
            }
            (true, false) => {
                channel.on = false;
                channel.rounds = 0;
                effects.push(Effect::Stop { id: id.to_owned() });
            }
            _ => {}
        }
        // CHOSEN IS PLAYED: turning a sound on lets the held ones go on —
        // before it starts, so a sound that will not start leaves none held
        // that the state says go on.
        if on && held {
            self.held = false;
            effects.push(Effect::Hold(false));
        }
        effects.extend(start);
        if self.sounding() == 0 {
            self.held = false;
        }
        Ok(effects)
    }

    /// A sound's own volume, kept whether it is on or not (up to
    /// [`VOLUME_MOST_PERCENT`], as the player's own).
    pub fn volume(&mut self, id: &str, percent: u8) -> Result<Vec<Effect>, Trouble> {
        let channel = self.channel(id)?;
        let percent = percent.min(VOLUME_MOST_PERCENT);
        channel.volume_percent = percent;
        Ok(if channel.on {
            vec![Effect::Volume {
                id: id.to_owned(),
                percent,
            }]
        } else {
            Vec::new()
        })
    }

    /// Every sound off; each keeps its volume.
    pub fn silence(&mut self) -> Vec<Effect> {
        self.held = false;
        self.channels
            .iter_mut()
            .filter(|c| c.on)
            .map(|c| {
                c.on = false;
                c.rounds = 0;
                Effect::Stop {
                    id: c.sound.id.clone(),
                }
            })
            .collect()
    }

    /// Holds the sounds that are on, or lets them go on.
    pub fn toggle(&mut self) -> Result<Vec<Effect>, Trouble> {
        if let Some(t) = &self.trouble {
            return Err(t.clone());
        }
        if self.sounding() == 0 {
            return Err(Trouble::NothingPlaying);
        }
        self.held = !self.held;
        Ok(vec![Effect::Hold(self.held)])
    }

    /// Whether the sounds that are on are held.
    #[must_use]
    pub const fn held(&self) -> bool {
        self.held
    }

    /// `id` went round once more.
    pub fn went_round(&mut self, id: &str) {
        if let Some(c) = self.channels.iter_mut().find(|c| c.sound.id == id && c.on) {
            c.rounds += 1;
        }
    }

    /// `id` could not play: it is off, and the trouble is said.
    pub fn failed(&mut self, id: &str) {
        if let Some(c) = self.channels.iter_mut().find(|c| c.sound.id == id) {
            c.on = false;
            c.rounds = 0;
        }
        if self.sounding() == 0 {
            self.held = false;
        }
    }

    /// The volume of each sound, to remember.
    #[must_use]
    pub fn volumes(&self) -> BTreeMap<String, u8> {
        self.channels
            .iter()
            .map(|c| (c.sound.id.clone(), c.volume_percent))
            .collect()
    }

    /// What the state says of the ambient sounds.
    #[must_use]
    pub fn state(&self) -> Ambience {
        Ambience {
            sounds: self
                .channels
                .iter()
                .map(|c| Sounding {
                    id: c.sound.id.clone(),
                    group: c.sound.group.clone(),
                    on: c.on,
                    volume_percent: c.volume_percent,
                    rounds: c.rounds,
                    credit: c.sound.credit.clone(),
                })
                .collect(),
            at_once: self.at_once,
            paused: self.held,
            trouble: self.trouble.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = r#"{
      "format": "ronoctua-ambience", "version": 1, "at_once": 2, "route": "loop-file",
      "sounds": [
        {"id": "rain", "group": "storm", "file": "rain.flac", "samples": 480000,
         "author": "someone", "page": "https://freesound.org/s/1/", "licence": "CC0-1.0",
         "seam": {"own": {"dip_db": 0.1}}},
        {"id": "stream", "group": "water", "file": "stream.flac",
         "author": "another", "page": "https://freesound.org/s/2/", "licence": "public-domain"},
        {"id": "fire", "group": "fire", "file": "fire.flac",
         "author": "a third", "page": "https://freesound.org/s/3/", "licence": "CC0-1.0"}
      ]
    }"#;

    fn mixer() -> Mixer {
        Mixer::new(parse(INDEX, Path::new("/loops")), &BTreeMap::new())
    }

    /// FAILS IF: the index `nix/ambience.nix` writes stops being read — its
    /// loops beside it, its credits, its ceiling — or what it measured of
    /// each seam, which the player has no use for, stops being let through.
    #[test]
    fn the_index_the_build_writes_is_read() {
        let c = parse(INDEX, Path::new("/loops")).expect("read");
        assert_eq!(c.at_once, 2);
        assert_eq!(
            c.sounds.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["rain", "stream", "fire"]
        );
        assert_eq!(c.sounds[0].path, Path::new("/loops/rain.flac"));
        assert_eq!(c.sounds[1].credit.licence, Licence::PublicDomain);
        assert_eq!(c.sounds[0].credit.author, "someone");
    }

    /// FAILS IF: an index this player cannot keep its word with is read in
    /// part — a newer one, another route round, a loop outside its own
    /// directory, a sound twice, a licence nobody approved — or a file that
    /// is not an index at all is taken for a damaged one.
    #[test]
    fn an_index_that_is_not_ours_to_play_is_refused_whole() {
        let at = Path::new("/loops");
        let with = |from: &str, to: &str| {
            assert!(INDEX.contains(from), "{from}");
            parse(&INDEX.replacen(from, to, 1), at)
        };
        let damaged = Trouble::Damaged {
            path: "/loops/index.json".into(),
        };
        assert_eq!(
            with(r#""version": 1"#, r#""version": 2"#),
            Err(Trouble::Newer {
                path: "/loops/index.json".into(),
                found: 2
            })
        );
        assert_eq!(with("loop-file", "amovie"), Err(damaged.clone()));
        assert_eq!(with("rain.flac", "../rain.flac"), Err(damaged.clone()));
        assert_eq!(with("rain.flac", "/etc/rain.flac"), Err(damaged.clone()));
        assert_eq!(with(r#""stream""#, r#""rain""#), Err(damaged.clone()));
        assert_eq!(with("public-domain", "CC-BY-4.0"), Err(damaged.clone()));
        assert_eq!(with(r#""at_once": 2"#, r#""at_once": 0"#), Err(damaged));
        assert_eq!(
            with("ronoctua-ambience", "something-else"),
            Err(Trouble::NotOurs {
                path: "/loops/index.json".into()
            })
        );
        assert!(matches!(
            parse("not json", at),
            Err(Trouble::Damaged { .. })
        ));
    }

    /// FAILS IF: a machine with no sounds turns one on, or the reason it has
    /// none stops being said where a door reads it.
    #[test]
    fn a_machine_without_sounds_says_why() {
        let mut m = Mixer::new(Err(Trouble::NoAmbience), &BTreeMap::new());
        assert_eq!(m.turn("rain", true), Err(Trouble::NoAmbience));
        assert_eq!(m.toggle(), Err(Trouble::NoAmbience));
        let s = m.state();
        assert!(s.sounds.is_empty());
        assert_eq!(s.trouble, Some(Trouble::NoAmbience));
    }

    /// FAILS IF: choosing a sound does not play it at once, at the volume
    /// it was left at, or an id nobody declared is taken.
    #[test]
    fn a_sound_chosen_plays_at_once_at_its_own_volume() {
        let mut volumes = BTreeMap::new();
        volumes.insert("rain".to_owned(), 40);
        let mut m = Mixer::new(parse(INDEX, Path::new("/loops")), &volumes);
        assert_eq!(
            m.turn("rain", true),
            Ok(vec![Effect::Start {
                id: "rain".into(),
                path: "/loops/rain.flac".into(),
                volume_percent: 40
            }])
        );
        assert_eq!(m.turn("rain", true), Ok(vec![]), "on twice is on once");
        assert_eq!(
            m.turn("wind", true),
            Err(Trouble::NoSuchSound { id: "wind".into() })
        );
        let s = m.state();
        assert!(s.sounds[0].on && !s.sounds[1].on);
        assert_eq!(s.sounds[0].volume_percent, 40);
        assert_eq!(s.sounds[1].volume_percent, VOLUME_FIRST_PERCENT);
    }

    /// FAILS IF: more sounds than the ceiling play together, or the refusal
    /// loses its number.
    #[test]
    fn more_than_the_ceiling_is_refused_with_the_number() {
        let mut m = mixer();
        m.turn("rain", true).expect("first");
        m.turn("stream", true).expect("second");
        assert_eq!(
            m.turn("fire", true),
            Err(Trouble::TooMany(TooMany {
                what: Limit::Ambience,
                most: 2
            }))
        );
        assert!(!m.state().sounds[2].on);
        m.turn("rain", false).expect("one off");
        assert!(m.turn("fire", true).is_ok(), "room again");
    }

    /// FAILS IF: a volume set while a sound is off is lost, one set while it
    /// plays does not reach its mpv, or silencing forgets the volumes.
    #[test]
    fn volumes_are_kept_on_and_off_and_through_silence() {
        let mut m = mixer();
        assert_eq!(m.volume("stream", 55), Ok(vec![]), "off: kept, not played");
        m.turn("rain", true).expect("on");
        assert_eq!(
            m.volume("rain", 200),
            Ok(vec![Effect::Volume {
                id: "rain".into(),
                percent: VOLUME_MOST_PERCENT
            }])
        );
        assert_eq!(m.silence(), vec![Effect::Stop { id: "rain".into() }]);
        assert!(m.state().sounds.iter().all(|s| !s.on));
        assert_eq!(m.volumes().get("stream"), Some(&55));
        assert_eq!(m.volumes().get("rain"), Some(&VOLUME_MOST_PERCENT));
        // AND A MIXER MADE AGAIN from what was remembered has them.
        let again = Mixer::new(parse(INDEX, Path::new("/loops")), &m.volumes());
        assert_eq!(again.state().sounds[1].volume_percent, 55);
        assert!(
            again.state().sounds.iter().all(|s| !s.on),
            "never on at start"
        );
    }

    /// FAILS IF: the modeline's toggle does not hold and release the sounds
    /// that are on, toggles with nothing on, or a sound chosen while they
    /// are held stays silent.
    #[test]
    fn held_sounds_go_on_when_one_is_chosen() {
        let mut m = mixer();
        assert_eq!(m.toggle(), Err(Trouble::NothingPlaying));
        m.turn("rain", true).expect("on");
        assert_eq!(m.toggle(), Ok(vec![Effect::Hold(true)]));
        assert!(m.state().paused);
        let effects = m.turn("stream", true).expect("another");
        assert!(effects.contains(&Effect::Hold(false)), "{effects:?}");
        assert!(!m.state().paused);
        m.toggle().expect("held again");
        m.turn("rain", false).expect("off");
        m.turn("stream", false).expect("off");
        assert!(!m.state().paused, "nothing on is nothing held");
    }

    /// FAILS IF: a round is counted for a sound that is off, or the count
    /// survives turning it off and on.
    #[test]
    fn rounds_count_the_times_a_sound_went_round() {
        let mut m = mixer();
        m.went_round("rain");
        assert_eq!(m.state().sounds[0].rounds, 0);
        m.turn("rain", true).expect("on");
        m.went_round("rain");
        m.went_round("rain");
        assert_eq!(m.state().sounds[0].rounds, 2);
        m.failed("rain");
        assert!(!m.state().sounds[0].on);
        m.turn("rain", true).expect("on again");
        assert_eq!(m.state().sounds[0].rounds, 0);
    }
}
