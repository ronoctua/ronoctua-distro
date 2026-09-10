//! The audio profile and volume of a Bluetooth device — the one thing BlueZ
//! cannot say.
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
//! # Why this file no longer runs a program
//!
//! It used to spawn `pw-dump` and `wpctl` and parse the graph itself. That
//! made **two** readers of one daemon in one repository — the shape
//! `ronoctua-disks` writes a whole header against — and the day they
//! disagreed about a volume, nothing would say which was right.
//!
//! The parsing lives in `ronoctua-audio` now, which is the one boundary with
//! PipeWire. What is left here is the Bluetooth question asked of it: **by
//! ADDRESS**, because PipeWire has never heard of a BlueZ object path.
//!
//! # The trap that survives the move
//!
//! `wpctl set-profile` takes a **DEVICE** and `wpctl set-volume` takes a
//! **NODE**, and on the headset this was measured against they are 84 and 83.

use anyhow::Result;

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
        let step = if forward { 1 } else { usable.len() - 1 };
        Some(usable[(at + step) % usable.len()])
    }
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

/// The card of the device at this ADDRESS.
///
/// # Errors
///
/// `pw-dump` is not installed, did not run, or answered something that is not
/// the array of objects it documents.
pub fn card(address: &str) -> Result<Option<Card>> {
    let dump = ronoctua_audio::pipewire::dump()?;
    Ok(card_of(&dump, address))
}

/// The sink of the device at this ADDRESS.
///
/// # Errors
///
/// `pw-dump` is not installed, did not run, or answered something that is not
/// the array of objects it documents.
pub fn sink(address: &str) -> Result<Option<Sink>> {
    let dump = ronoctua_audio::pipewire::dump()?;
    Ok(sink_of(&dump, address))
}

/// Puts the device's card in a profile.
///
/// # Errors
///
/// `wpctl` is not installed, or refused.
pub fn choose(card: u32, profile: i32) -> Result<()> {
    ronoctua_audio::pipewire::carry_out(&ronoctua_audio::Order::Profile {
        device: card,
        index: profile,
    })
}

/// Sets the volume of a node, as a percentage.
///
/// # Errors
///
/// `wpctl` is not installed, or refused.
pub fn set_volume(node: u32, percent: u8) -> Result<()> {
    ronoctua_audio::pipewire::carry_out(&ronoctua_audio::Order::Volume { node, percent })
}

/// Silences a node, or gives it back.
///
/// # Errors
///
/// `wpctl` is not installed, or refused.
pub fn set_mute(node: u32, muted: bool) -> Result<()> {
    ronoctua_audio::pipewire::carry_out(&ronoctua_audio::Order::Mute { node, muted })
}

/// The name PipeWire gives the card of a Bluetooth address.
///
/// `bluez_card.AA_BB_CC_…`, built from the ADDRESS with `:` turned into `_`.
/// The match is on this and never on the BlueZ object path, which PipeWire
/// has never heard of.
fn card_name(address: &str) -> String {
    format!("bluez_card.{}", address.replace(':', "_")).to_lowercase()
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
    let graph = ronoctua_audio::graph::Graph::once(dump).ok()?;
    let wanted = address.to_lowercase();
    graph
        .of(ronoctua_audio::Kind::Sink)
        .into_iter()
        .find(|n| {
            n.address
                .as_deref()
                .is_some_and(|a| a.to_lowercase() == wanted)
        })
        .map(|n| Sink {
            id: n.id,
            volume: n.volume,
            muted: n.muted,
        })
}

/// The card for an address, out of what `pw-dump` printed.
#[must_use]
pub fn card_of(dump: &str, address: &str) -> Option<Card> {
    let graph = ronoctua_audio::graph::Graph::once(dump).ok()?;
    let wanted = card_name(address);
    let device = graph
        .devices
        .iter()
        .find(|d| d.name.to_lowercase() == wanted)?;
    Some(Card {
        id: device.id,
        active: device.active_profile,
        profiles: device
            .profiles
            .iter()
            .map(|p| Profile {
                index: p.index,
                name: p.name.clone(),
                available: p.available.usable(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two nodes of one headset, trimmed to what `sink_of` reads. The address
    /// is invented (invariant 13), and the two node names disagree with each
    /// other on purpose — that is the trap the property match exists for.
    const NODES: &str = r#"[
      { "id": 83, "type": "PipeWire:Interface:Node",
        "info": { "props": { "media.class": "Audio/Sink",
                             "node.name": "bluez_output.AA_BB_CC_DD_EE_FF.1",
                             "api.bluez5.address": "AA:BB:CC:DD:EE:FF",
                             "device.id": 84 },
                  "params": { "Props": [ { "volume": 0.367886,
                                           "mute": false,
                                           "channelVolumes": [0.367886, 0.367886] } ] } } },
      { "id": 91, "type": "PipeWire:Interface:Node",
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

    /// What a real dump STARTS with, in `pw-dump`'s own order: the core, a
    /// module, and a metadata object that carries no `info` at all. None of
    /// the three has `media.class` or `device.name`, and all three come
    /// before anything that plays sound.
    const BEFORE: &str = r#"
      { "id": 0, "type": "PipeWire:Interface:Core",
        "info": { "props": { "core.name": "pipewire-0", "cpu.max-align": 32 } } },
      { "id": 1, "type": "PipeWire:Interface:Module",
        "info": { "props": { "module.name": "libpipewire-module-rt" } } },
      { "id": 30, "type": "PipeWire:Interface:Metadata" },
    "#;

    #[test]
    fn the_graph_is_walked_past_what_is_not_a_node() {
        // THE DEFECT THE OWNER SAW: the second column of a connected headset
        // had no volume row. Both readers gave up on the first object of the
        // dump instead of stepping over it, so on every real machine they
        // answered "no card" and "no sink" — and the fixtures above, trimmed
        // to the keys that are read, could never show it.
        let dump = format!("[{BEFORE}{}", &NODES[1..]);
        let sink = sink_of(&dump, "AA:BB:CC:DD:EE:FF").expect("the headset still has a sink");
        assert_eq!(sink.id, 83);
        assert_eq!(sink.volume, 72);

        let dump = format!("[{BEFORE}{}", &DUMP[1..]);
        let card = card_of(&dump, "00:00:00:00:00:02").expect("the headset still has a card");
        assert_eq!(card.id, 42);
        assert_eq!(card.now(), Some("a2dp-sink"));
    }

    /// ONE object, in the shape `pw-dump` prints, trimmed to what is read.
    /// The address is invented (invariant 13).
    const DUMP: &str = r#"[
      { "id": 7, "type": "PipeWire:Interface:Device",
        "info": { "props": { "media.class": "Audio/Device",
                             "device.name": "alsa_card.pci-0000_00_1f.3" } } },
      { "id": 42, "type": "PipeWire:Interface:Device", "info": {
          "props": { "media.class": "Audio/Device",
                     "device.name": "bluez_card.00_00_00_00_00_02" },
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

    #[test]
    fn the_volume_is_no_longer_computed_twice() {
        // The reason this file stopped parsing: one boundary, one answer. If
        // the cube root ever came back here it would be a second copy, free
        // to drift from `ronoctua_audio::perceived` without a test noticing.
        assert_eq!(ronoctua_audio::perceived(0.367886), 72);
        let sink = sink_of(NODES, "AA:BB:CC:DD:EE:FF").expect("the headset has a sink");
        assert_eq!(sink.volume, ronoctua_audio::perceived(0.367886));
    }
}
