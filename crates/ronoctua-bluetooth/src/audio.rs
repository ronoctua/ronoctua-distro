//! The audio profile of a Bluetooth device — the one thing BlueZ cannot say.
//!
//! # Why this is not on the bus this crate speaks
//!
//! `org.bluez.MediaTransport1` says what is PLAYING right now: a UUID, a
//! state, a codec, a volume. It cannot say what a device COULD be put in, and
//! that is the question behind the owner's "every option and every setting":
//! a headset does music in stereo (A2DP) or a call with its microphone (HFP),
//! and it cannot do both at once. Which of the two it is in is PipeWire's,
//! because PipeWire is what owns the card.
//!
//! # Why two programs and not a library
//!
//! Because the closure gains nothing: `nix path-info -r` on the system's own
//! toplevel finds `wpctl` and `pw-dump` in `sw/bin` already —
//! `services.pipewire` puts them there. Linking PipeWire's C API would be a
//! second FFI boundary in this repository (`nix/rust.nix` says why the first
//! one costs what it does), for a question asked when somebody opens one row.
//!
//! `pw-dump` READS and `wpctl` WRITES, which is also the safety line: nothing
//! here changes anything until a key is pressed.
//!
//! # The trap
//!
//! **PipeWire's id is not BlueZ's path.** A card is `bluez_card.AA_BB_CC_…`,
//! built from the ADDRESS with `:` turned into `_` — so the match is on the
//! address and never on the object path, which PipeWire has never heard of.

use anyhow::{Context as _, Result};

/// Where `pw-dump` lives, baked at build time.
const PW_DUMP: Option<&str> = option_env!("RONOCTUA_PW_DUMP");

/// Where `wpctl` lives, baked at build time.
const WPCTL: Option<&str> = option_env!("RONOCTUA_WPCTL");

/// One profile a card can be put in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// PipeWire's own index, which is what `wpctl set-profile` takes.
    pub index: i32,
    /// `a2dp-sink`, `headset-head-unit`, `off` — the id, not a sentence.
    pub name: String,
    /// Whether the device can be put in it NOW. PipeWire reports zero for a
    /// profile whose codec the other side does not offer, and a row that
    /// offered it would be a row that fails.
    pub available: bool,
}

/// What a device's card can do, and what it is doing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Card {
    /// PipeWire's device id — the first argument of `wpctl set-profile`.
    pub id: u32,
    /// Every profile, in PipeWire's own order.
    pub profiles: Vec<Profile>,
    /// The index of the one it is in.
    pub active: i32,
}

impl Card {
    /// The active profile's name, when it has one.
    #[must_use]
    pub fn now(&self) -> Option<&str> {
        self.profiles
            .iter()
            .find(|p| p.index == self.active)
            .map(|p| p.name.as_str())
    }

    /// The next available profile after the active one, wrapping.
    ///
    /// `None` when there is nothing else to move to, which is the honest
    /// answer for a device with one profile and `off`.
    #[must_use]
    pub fn next(&self, forward: bool) -> Option<&Profile> {
        let usable: Vec<&Profile> = self.profiles.iter().filter(|p| p.available).collect();
        if usable.len() < 2 {
            return None;
        }
        let at = usable.iter().position(|p| p.index == self.active)?;
        let step = if forward {
            (at + 1) % usable.len()
        } else {
            (at + usable.len() - 1) % usable.len()
        };
        usable.get(step).copied()
    }
}

/// Reads the card of the device at this ADDRESS.
///
/// # Errors
///
/// `pw-dump` is not installed, did not run, or answered something that is not
/// the array of objects it documents.
pub fn card(address: &str) -> Result<Option<Card>> {
    let program = PW_DUMP.context("this build has no pw-dump")?;
    let out = std::process::Command::new(program)
        .output()
        .context("running pw-dump")?;
    if !out.status.success() {
        anyhow::bail!("pw-dump refused: {}", out.status);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(card_of(&text, address))
}

/// Puts the device's card in a profile.
///
/// # Errors
///
/// `wpctl` is not installed, or refused.
pub fn choose(card: u32, profile: i32) -> Result<()> {
    let program = WPCTL.context("this build has no wpctl")?;
    let out = std::process::Command::new(program)
        .args(["set-profile", &card.to_string(), &profile.to_string()])
        .output()
        .context("running wpctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "wpctl refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// A node that plays sound — what a VOLUME belongs to.
///
/// # Why this is not the card
///
/// `wpctl set-profile` takes a DEVICE and `wpctl set-volume` takes a NODE, and
/// they are two different objects: on the machine this was written against the
/// headset is device 84 and sink 83. Handing `Card::id` to `set-volume` would
/// address whatever node happened to have that number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sink {
    /// PipeWire's node id — good until the profile changes, and no longer.
    /// The node is recreated with a new id, which is why a volume is
    /// remembered against the ADDRESS and never against this.
    pub id: u32,
    /// What the person reads, 0 to 100.
    pub volume: u8,
    /// Silenced, without losing the number.
    pub muted: bool,
}

/// The sink of the device at this ADDRESS.
///
/// # Errors
///
/// `pw-dump` is not installed, did not run, or answered something that is not
/// the array of objects it documents.
pub fn sink(address: &str) -> Result<Option<Sink>> {
    let program = PW_DUMP.context("this build has no pw-dump")?;
    let out = std::process::Command::new(program)
        .output()
        .context("running pw-dump")?;
    if !out.status.success() {
        anyhow::bail!("pw-dump refused: {}", out.status);
    }
    Ok(sink_of(&String::from_utf8_lossy(&out.stdout), address))
}

/// Sets the volume of a node, as a percentage.
///
/// # Errors
///
/// `wpctl` is not installed, or refused.
pub fn set_volume(node: u32, percent: u8) -> Result<()> {
    let program = WPCTL.context("this build has no wpctl")?;
    let out = std::process::Command::new(program)
        .args(["set-volume", &node.to_string(), &format!("{percent}%")])
        .output()
        .context("running wpctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "wpctl refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Silences a node, or gives it back.
///
/// # Errors
///
/// `wpctl` is not installed, or refused.
pub fn set_mute(node: u32, muted: bool) -> Result<()> {
    let program = WPCTL.context("this build has no wpctl")?;
    let out = std::process::Command::new(program)
        .args(["set-mute", &node.to_string(), if muted { "1" } else { "0" }])
        .output()
        .context("running wpctl")?;
    if !out.status.success() {
        anyhow::bail!(
            "wpctl refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// The sink for an address, out of what `pw-dump` printed.
///
/// # Why it matches on a PROPERTY and not on the node's name
///
/// Because the names disagree with themselves: the sink is
/// `bluez_output.AC_80_0A_EA_D7_EF.1` — underscores, and a trailing profile
/// index — while the source beside it is `bluez_input.AC:80:0A:EA:D7:EF`, with
/// colons. Building either string from the address gets the other one wrong.
/// `api.bluez5.address` is the plain address, on both, and `media.class` says
/// which way the sound goes.
#[must_use]
pub fn sink_of(dump: &str, address: &str) -> Option<Sink> {
    let objects: Vec<serde_json::Value> = serde_json::from_str(dump).ok()?;
    for o in objects {
        let info = o.get("info")?;
        let props = info.get("props")?;
        if props.get("media.class")?.as_str()? != "Audio/Sink" {
            continue;
        }
        let theirs = props.get("api.bluez5.address").and_then(|a| a.as_str());
        if !theirs.is_some_and(|a| a.eq_ignore_ascii_case(address)) {
            continue;
        }
        let id = u32::try_from(o.get("id")?.as_i64()?).ok()?;
        let settings = info
            .get("params")
            .and_then(|p| p.get("Props"))
            .and_then(|p| p.as_array())
            .and_then(|a| a.first());
        let muted = settings
            .and_then(|p| p.get("mute"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        // THE NUMBER PIPEWIRE KEEPS IS NOT THE NUMBER A PERSON READS.
        //
        // `channelVolumes` is linear amplitude; every mixer, `wpctl` included,
        // shows its CUBE ROOT. The headset this was measured against reported
        // `0.367886` while `wpctl status` said `0.72`, and 0.72 cubed is
        // 0.373. Showing the stored number would say 37% about a headset
        // playing at 72%, and the row would disagree with every other volume
        // control on the machine.
        let linear = settings
            .and_then(|p| p.get("channelVolumes"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .or_else(|| settings.and_then(|p| p.get("volume")))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the cube root of a number clamped to 0..=1, times 100"
        )]
        let volume = (linear.cbrt() * 100.0).round() as u8;
        return Some(Sink {
            id,
            volume: volume.min(100),
            muted,
        });
    }
    None
}

/// The card for an address, out of what `pw-dump` printed.
///
/// A free function so the parsing is testable without PipeWire: the fixture
/// beside it is one real object trimmed to the fields this reads.
#[must_use]
pub fn card_of(dump: &str, address: &str) -> Option<Card> {
    let wanted = format!("bluez_card.{}", address.replace(':', "_")).to_lowercase();
    let objects: Vec<serde_json::Value> = serde_json::from_str(dump).ok()?;
    for o in objects {
        let props = o.get("info")?.get("props")?;
        let name = props.get("device.name")?.as_str()?;
        if name.to_lowercase() != wanted {
            continue;
        }
        let id = u32::try_from(o.get("id")?.as_i64()?).ok()?;
        let params = o.get("info")?.get("params")?;
        let active = params
            .get("Profile")
            .and_then(|p| p.as_array())
            .and_then(|a| a.first())
            .and_then(|p| p.get("index"))
            .and_then(serde_json::Value::as_i64)
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(-1);
        let profiles = params
            .get("EnumProfile")
            .and_then(|p| p.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        Some(Profile {
                            index: i32::try_from(p.get("index")?.as_i64()?).ok()?,
                            name: p.get("name")?.as_str()?.to_owned(),
                            // ZERO IS "no", and anything else is yes: PipeWire
                            // spells this `available` with the enum values
                            // `unknown`, `no` and `yes`, and the numbers are
                            // 0, 1 and 2 — but some versions print the WORD.
                            // Both are read, because a profile wrongly called
                            // unavailable is a row that disappears.
                            available: match p.get("available") {
                                Some(serde_json::Value::String(s)) => s != "no",
                                Some(serde_json::Value::Number(n)) => n.as_i64() != Some(1),
                                _ => true,
                            },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Some(Card {
            id,
            profiles,
            active,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two nodes of one headset, trimmed to what `sink_of` reads. The address
    /// is invented (invariant 13), and the two node names disagree with each
    /// other on purpose — that is the trap the property match exists for.
    const NODES: &str = r#"[
      { "id": 83,
        "info": { "props": { "media.class": "Audio/Sink",
                             "node.name": "bluez_output.AA_BB_CC_DD_EE_FF.1",
                             "api.bluez5.address": "AA:BB:CC:DD:EE:FF",
                             "device.id": 84 },
                  "params": { "Props": [ { "volume": 0.367886,
                                           "mute": false,
                                           "channelVolumes": [0.367886, 0.367886] } ] } } },
      { "id": 91,
        "info": { "props": { "media.class": "Audio/Source",
                             "node.name": "bluez_input.AA:BB:CC:DD:EE:FF",
                             "api.bluez5.address": "AA:BB:CC:DD:EE:FF" },
                  "params": { "Props": [ { "channelVolumes": [1.0], "mute": true } ] } } }
    ]"#;

    #[test]
    fn the_sink_is_the_node_and_never_the_card() {
        let sink = sink_of(NODES, "AA:BB:CC:DD:EE:FF").expect("the headset has a sink");
        // The NODE, not the device it hangs off — `device.id` is 84 here.
        assert_eq!(sink.id, 83);
        // AND THE NUMBER A PERSON READS. 0.367886 is the stored amplitude; its
        // cube root is what every mixer shows, and `wpctl` said 0.72.
        assert_eq!(sink.volume, 72);
        assert!(!sink.muted);
    }

    #[test]
    fn the_source_is_not_taken_for_the_sink() {
        // The source is muted and at full volume, and it carries the SAME
        // address. Matching on the address alone would answer with it — and
        // the row would say 100%, muted, about the headphones.
        let sink = sink_of(NODES, "AA:BB:CC:DD:EE:FF").expect("the headset has a sink");
        assert_ne!(sink.id, 91);
        assert!(!sink.muted);
    }

    #[test]
    fn a_device_with_no_node_has_no_volume() {
        assert!(sink_of(NODES, "11:22:33:44:55:66").is_none());
        assert!(sink_of("not json", "AA:BB:CC:DD:EE:FF").is_none());
    }

    #[test]
    fn the_address_is_read_whatever_its_case() {
        assert!(sink_of(NODES, "aa:bb:cc:dd:ee:ff").is_some());
    }

    /// ONE object, in the shape `pw-dump` prints, trimmed to what is read.
    /// The address is invented (invariant 13).
    const DUMP: &str = r#"[
      { "id": 7, "info": { "props": { "device.name": "alsa_card.pci-0000_00_1f.3" } } },
      { "id": 42, "info": {
          "props": { "device.name": "bluez_card.00_00_00_00_00_02" },
          "params": {
            "Profile": [ { "index": 1, "name": "a2dp-sink" } ],
            "EnumProfile": [
              { "index": 0, "name": "off", "available": "yes" },
              { "index": 1, "name": "a2dp-sink", "available": "yes" },
              { "index": 2, "name": "headset-head-unit", "available": "yes" },
              { "index": 3, "name": "a2dp-sink-aptx", "available": "no" }
            ]
          }
      } }
    ]"#;

    #[test]
    fn the_card_is_found_by_address_and_not_by_path() {
        // PipeWire has never heard of `/org/bluez/hci0/dev_…`: what it knows
        // is `bluez_card.AA_BB_…`, built from the ADDRESS. Looking the card up
        // by the object path finds nothing at all, silently.
        let c = card_of(DUMP, "00:00:00:00:00:02").expect("the card");
        assert_eq!(c.id, 42);
        assert_eq!(c.active, 1);
        assert_eq!(c.now(), Some("a2dp-sink"));
        assert_eq!(c.profiles.len(), 4);
        assert!(card_of(DUMP, "00:00:00:00:00:09").is_none());
        // AND THE OTHER CARDS ARE NOT IT: the machine's own sound card is the
        // first object in every real dump.
        assert!(card_of(DUMP, "pci-0000_00_1f.3").is_none());
    }

    #[test]
    fn the_ring_skips_what_the_device_cannot_do() {
        // `a2dp-sink-aptx` is `available: no` — the other side does not offer
        // the codec — and a row that offered it would be a row that fails.
        let c = card_of(DUMP, "00:00:00:00:00:02").expect("the card");
        assert_eq!(
            c.next(true).map(|p| p.name.as_str()),
            Some("headset-head-unit")
        );
        assert_eq!(c.next(false).map(|p| p.name.as_str()), Some("off"));
    }

    #[test]
    fn one_profile_has_nowhere_to_go() {
        let one = Card {
            id: 1,
            active: 0,
            profiles: vec![Profile {
                index: 0,
                name: "off".to_owned(),
                available: true,
            }],
        };
        assert!(one.next(true).is_none());
        assert!(one.next(false).is_none());
    }

    #[test]
    fn a_dump_that_is_not_json_is_no_card_and_not_a_panic() {
        assert!(card_of("", "00:00:00:00:00:02").is_none());
        assert!(card_of("pw-dump: command not found", "00:00:00:00:00:02").is_none());
        assert!(card_of("[]", "00:00:00:00:00:02").is_none());
    }
}
