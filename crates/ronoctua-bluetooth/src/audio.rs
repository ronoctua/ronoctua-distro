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
