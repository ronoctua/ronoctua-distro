//! The running picture of PipeWire's graph, and how a delta is folded into it.
//!
//! # Why a merge and not a re-read
//!
//! `pw-dump --monitor` prints the whole graph once and then, for every change,
//! an array holding ONLY the objects that moved. Answering a change by running
//! `pw-dump` again would be a fork per keypress — and holding an arrow down is
//! a keypress per frame.
//!
//! So the state lives here, and every array is folded in. That is a merge with
//! three rules, and each of them was read out of what the program actually
//! prints:
//!
//! 1. **`"info": null` means the object is GONE.** An application that stops
//!    playing arrives this way and no other; treating it as an update would
//!    leave dead streams on the screen for ever.
//! 2. **`info.props` arrives WHOLE** — `pw-dump` prints the current dictionary,
//!    not a difference — so it replaces.
//! 3. **`info.params` arrives PARTIAL**, only the ones whose id changed. It
//!    merges key by key. Replacing it whole is the bug that would empty a
//!    card's profile list the first time somebody moved a slider: the volume
//!    change carries `Props` and nothing else, and `EnumProfile` would go.
//! 4. **`metadata` arrives PARTIAL too**, and it is not under `info`. It
//!    merges entry by entry, keyed on `(subject, key)`; a `null` value is a
//!    removal. Replacing the array whole is the same bug one level out, and it
//!    was READ OFF THE SCREEN: between two photographs of the sound view the
//!    "in use" marker vanished from the default microphone, because a delta
//!    carrying one changed default had thrown the other three away.
//!
//! Everything here is pure. The tests below fold hand-trimmed objects — the
//! same shape `ronoctua-bluetooth` already used for its fixtures — so the
//! rules are proved with no daemon anywhere.

use crate::{Available, Defaults, Device, Direction, Kind, Node, Profile, Route, Snapshot};
use serde_json::Value;
use std::collections::BTreeMap;

/// Every object PipeWire has told us about, by id.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    objects: BTreeMap<u32, Value>,
}

impl Graph {
    /// A graph that has been told nothing.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Fold one of `pw-dump`'s arrays in.
    ///
    /// Anything that is not an array of objects with an `id` is passed over —
    /// a stray value on the stream must not take the picture down.
    pub fn took(&mut self, delta: &Value) {
        let Some(objects) = delta.as_array() else {
            return;
        };
        for o in objects {
            let Some(id) = o
                .get("id")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
            else {
                continue;
            };
            // RULE 1: a null `info` is a removal, and the key has to be
            // PRESENT for it to mean that. An object that simply never
            // carried an `info` — the core does not — is not a removal.
            if o.get("info").is_some_and(Value::is_null) {
                self.objects.remove(&id);
                continue;
            }
            match self.objects.get_mut(&id) {
                None => {
                    self.objects.insert(id, o.clone());
                }
                Some(held) => merge(held, o),
            }
        }
    }

    /// Fold one of `pw-dump`'s arrays in, from its bytes.
    ///
    /// # Errors
    ///
    /// When the text is not JSON at all.
    pub fn took_text(&mut self, text: &str) -> serde_json::Result<()> {
        let value: Value = serde_json::from_str(text)?;
        self.took(&value);
        Ok(())
    }

    /// How many objects are held. The stream's own sanity check.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether nothing has arrived yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Read the whole thing out as plain structs.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let mut out = Snapshot::default();
        for o in self.objects.values() {
            match o.get("type").and_then(Value::as_str) {
                Some("PipeWire:Interface:Node") => {
                    if let Some(n) = node_of(o) {
                        out.nodes.push(n);
                    }
                }
                Some("PipeWire:Interface:Device") => {
                    if let Some(d) = device_of(o) {
                        out.devices.push(d);
                    }
                }
                Some("PipeWire:Interface:Metadata") => took_defaults(o, &mut out.defaults),
                _ => {}
            }
        }
        out
    }

    /// One whole reading out of one `pw-dump` run.
    ///
    /// # Errors
    ///
    /// When the text is not JSON.
    pub fn once(dump: &str) -> serde_json::Result<Snapshot> {
        let mut g = Self::fresh();
        g.took_text(dump)?;
        Ok(g.snapshot())
    }
}

/// Fold one object's update into the one held.
fn merge(held: &mut Value, fresh: &Value) {
    let Some(fresh) = fresh.as_object() else {
        return;
    };
    let Some(held) = held.as_object_mut() else {
        return;
    };
    for (key, value) in fresh {
        // RULE 4: the metadata entries are the other partial thing, and they
        // do not live under `info`.
        if key == "metadata" {
            merge_metadata(held, value);
            continue;
        }
        if key != "info" {
            held.insert(key.clone(), value.clone());
            continue;
        }
        let Some(fresh_info) = value.as_object() else {
            held.insert(key.clone(), value.clone());
            continue;
        };
        let held_info = held
            .entry("info")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(held_info) = held_info.as_object_mut() else {
            continue;
        };
        for (field, inner) in fresh_info {
            // RULE 3: the params are the one thing that arrives partial.
            if field == "params"
                && let (Some(fresh_params), Some(held_params)) = (
                    inner.as_object(),
                    held_info
                        .entry("params")
                        .or_insert_with(|| Value::Object(serde_json::Map::new()))
                        .as_object_mut(),
                )
            {
                for (name, param) in fresh_params {
                    held_params.insert(name.clone(), param.clone());
                }
                continue;
            }
            held_info.insert(field.clone(), inner.clone());
        }
    }
}

/// Fold a metadata delta into the entries held, one entry at a time.
///
/// Keyed on `(subject, key)`, which is what a metadata entry IS: the same key
/// may be set for several subjects, and PipeWire's own `default` object uses
/// subject 0 throughout. A `null` value is how an entry is cleared.
fn merge_metadata(held: &mut serde_json::Map<String, Value>, fresh: &Value) {
    let Some(fresh) = fresh.as_array() else {
        return;
    };
    let named = |v: &Value| {
        (
            v.get("subject").and_then(Value::as_i64).unwrap_or(0),
            v.get("key")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        )
    };
    let mut entries: Vec<Value> = held
        .get("metadata")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for one in fresh {
        let want = named(one);
        entries.retain(|e| named(e) != want);
        // A NULL VALUE IS A REMOVAL and not an entry saying nothing: left in
        // the list it would make the default read as absent rather than gone.
        if !one.get("value").is_some_and(Value::is_null) {
            entries.push(one.clone());
        }
    }
    held.insert("metadata".to_owned(), Value::Array(entries));
}

/// The props of one object, when it has any.
fn props(o: &Value) -> Option<&serde_json::Map<String, Value>> {
    o.get("info")?.get("props")?.as_object()
}

/// One named param of one object.
fn param<'a>(o: &'a Value, name: &str) -> Option<&'a Vec<Value>> {
    o.get("info")?.get("params")?.get(name)?.as_array()
}

/// A string prop.
fn word<'a>(p: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    p.get(key)?.as_str()
}

/// One node, out of one object.
fn node_of(o: &Value) -> Option<Node> {
    let id = o.get("id")?.as_u64().and_then(|n| u32::try_from(n).ok())?;
    let p = props(o)?;
    let name = word(p, "node.name")?.to_owned();
    // THE WORDS IN THE ORDER A PERSON WOULD WANT THEM. A stream carries
    // `media.name` (the track) and a device carries `node.description` (the
    // hardware); the nick is what a card gives when it has nothing better.
    let description = word(p, "node.description")
        .or_else(|| word(p, "media.name"))
        .or_else(|| word(p, "node.nick"))
        .unwrap_or(&name)
        .to_owned();
    let kind = word(p, "media.class").and_then(Kind::of);
    let (volume, muted) = loudness(o);
    Some(Node {
        id,
        kind,
        name,
        description,
        device: p
            .get("device.id")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        volume,
        muted,
        app: word(p, "application.name").map(str::to_owned),
        address: word(p, "api.bluez5.address").map(str::to_owned),
    })
}

/// The volume and the mute of one node.
///
/// `channelVolumes` first and `volume` after it: the second is the node's
/// overall gain and stays at 1.0 on hardware whose per-channel volumes are
/// what actually move. Reading it alone reports 100% for everything.
fn loudness(o: &Value) -> (u8, bool) {
    let Some(settings) = param(o, "Props").and_then(|a| a.first()) else {
        return (0, false);
    };
    let linear = settings
        .get("channelVolumes")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .or_else(|| settings.get("volume"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let muted = settings
        .get("mute")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (crate::perceived(linear), muted)
}

/// One card, out of one object.
fn device_of(o: &Value) -> Option<Device> {
    let id = o.get("id")?.as_u64().and_then(|n| u32::try_from(n).ok())?;
    let p = props(o)?;
    // ONLY AUDIO. The same `pw-dump` lists the webcams, and a camera is not a
    // row in a sound view.
    let class = word(p, "media.class").unwrap_or_default();
    if !class.starts_with("Audio/") {
        return None;
    }
    let name = word(p, "device.name")?.to_owned();
    let description = word(p, "device.description")
        .or_else(|| word(p, "device.nick"))
        .unwrap_or(&name)
        .to_owned();
    let profiles = param(o, "EnumProfile")
        .map(|a| a.iter().filter_map(profile_of).collect())
        .unwrap_or_default();
    let active_profile = param(o, "Profile")
        .and_then(|a| a.first())
        .and_then(|p| p.get("index"))
        .and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(-1);
    let routes = param(o, "EnumRoute")
        .map(|a| a.iter().filter_map(route_of).collect())
        .unwrap_or_default();
    let active_routes = param(o, "Route")
        .map(|a| {
            a.iter()
                .filter_map(|r| r.get("index")?.as_i64())
                .filter_map(|n| i32::try_from(n).ok())
                .collect()
        })
        .unwrap_or_default();
    Some(Device {
        id,
        name,
        description,
        profiles,
        active_profile,
        routes,
        active_routes,
    })
}

/// One profile, out of one `EnumProfile` entry.
fn profile_of(v: &Value) -> Option<Profile> {
    let index = i32::try_from(v.get("index")?.as_i64()?).ok()?;
    let name = v.get("name")?.as_str()?.to_owned();
    Some(Profile {
        index,
        description: v
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(&name)
            .to_owned(),
        name,
        available: v
            .get("available")
            .and_then(Value::as_str)
            .map_or(Available::Unknown, Available::of),
    })
}

/// One port, out of one `EnumRoute` entry.
fn route_of(v: &Value) -> Option<Route> {
    let index = i32::try_from(v.get("index")?.as_i64()?).ok()?;
    let name = v.get("name")?.as_str()?.to_owned();
    Some(Route {
        index,
        description: v
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(&name)
            .to_owned(),
        name,
        direction: v
            .get("direction")
            .and_then(Value::as_str)
            .map_or(Direction::Output, Direction::of),
        available: v
            .get("available")
            .and_then(Value::as_str)
            .map_or(Available::Unknown, Available::of),
        devices: v
            .get("devices")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_i64)
                    .filter_map(|n| i32::try_from(n).ok())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// The four defaults, out of the `default` metadata object.
fn took_defaults(o: &Value, out: &mut Defaults) {
    let is_the_one = o
        .get("props")
        .and_then(|p| p.get("metadata.name"))
        .and_then(Value::as_str)
        == Some("default");
    if !is_the_one {
        return;
    }
    let Some(entries) = o.get("metadata").and_then(Value::as_array) else {
        return;
    };
    for e in entries {
        let Some(key) = e.get("key").and_then(Value::as_str) else {
            continue;
        };
        // THE VALUE IS AN OBJECT WITH A `name`, not a string. Reading it as a
        // string gives `None` for every default there is, and the screen then
        // says nothing is chosen on a machine where everything is.
        let Some(named) = e
            .get("value")
            .and_then(|v| v.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let slot = match key {
            "default.audio.sink" => &mut out.sink,
            "default.audio.source" => &mut out.source,
            "default.configured.audio.sink" => &mut out.configured_sink,
            "default.configured.audio.source" => &mut out.configured_source,
            _ => continue,
        };
        *slot = Some(named.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One real sink, trimmed to the fields this reads.
    ///
    /// Hand-trimmed and not captured: a whole `pw-dump` carries the machine's
    /// host name, the user's name and every running program's command line,
    /// and none of that belongs in a public repository (invariant 13).
    const SINK: &str = r#"[{
      "id": 60,
      "type": "PipeWire:Interface:Node",
      "info": {
        "props": {
          "media.class": "Audio/Sink",
          "node.name": "alsa_output.speaker",
          "node.description": "Speaker",
          "device.id": 52
        },
        "params": {
          "Props": [{ "volume": 1.0, "mute": false, "channelVolumes": [0.056895, 0.056895] }],
          "EnumFormat": [{ "mediaType": "audio" }]
        }
      }
    }]"#;

    const CARD: &str = r#"[{
      "id": 52,
      "type": "PipeWire:Interface:Device",
      "info": {
        "props": { "media.class": "Audio/Device", "device.name": "alsa_card.pci",
                   "device.description": "Ryzen HD Audio Controller" },
        "params": {
          "EnumProfile": [
            { "index": 0, "name": "off", "description": "Off", "available": "yes" },
            { "index": 1, "name": "hp", "description": "Headphones", "available": "no" },
            { "index": 2, "name": "spk", "description": "Speaker", "available": "yes" }
          ],
          "Profile": [{ "index": 2, "name": "spk" }],
          "EnumRoute": [
            { "index": 0, "name": "[Out] Speaker", "description": "Speaker",
              "direction": "Output", "available": "unknown", "devices": [0] },
            { "index": 3, "name": "[Out] Headphones", "description": "Headphones",
              "direction": "Output", "available": "no", "devices": [3] }
          ],
          "Route": [{ "index": 0, "device": 0 }]
        }
      }
    }]"#;

    const DEFAULTS: &str = r#"[{
      "id": 40,
      "type": "PipeWire:Interface:Metadata",
      "props": { "metadata.name": "default" },
      "metadata": [
        { "subject": 0, "key": "default.audio.sink", "value": { "name": "alsa_output.speaker" } },
        { "subject": 0, "key": "default.configured.audio.sink", "value": { "name": "a headset" } }
      ]
    }]"#;

    fn graph_of(parts: &[&str]) -> Graph {
        let mut g = Graph::fresh();
        for p in parts {
            g.took_text(p).expect("the fixture is JSON");
        }
        g
    }

    #[test]
    fn a_sink_comes_out_whole() {
        let s = graph_of(&[SINK]).snapshot();
        let n = &s.nodes[0];
        assert_eq!(n.id, 60);
        assert_eq!(n.kind, Some(Kind::Sink));
        assert_eq!(n.description, "Speaker");
        assert_eq!(n.device, Some(52));
        // The cube root, not the stored amplitude.
        assert_eq!(n.volume, 38);
        assert!(!n.muted);
    }

    #[test]
    fn a_card_comes_out_whole() {
        let s = graph_of(&[CARD]).snapshot();
        let d = &s.devices[0];
        assert_eq!(d.profiles.len(), 3);
        assert_eq!(d.active_profile, 2);
        assert_eq!(d.routes.len(), 2);
        assert_eq!(d.active_routes, vec![0]);
        assert_eq!(d.routes[1].available, Available::No);
        assert_eq!(d.routes[1].direction, Direction::Output);
    }

    #[test]
    fn the_defaults_are_read_out_of_the_named_object() {
        let s = graph_of(&[SINK, DEFAULTS]).snapshot();
        assert_eq!(s.defaults.sink.as_deref(), Some("alsa_output.speaker"));
        assert_eq!(s.defaults.configured_sink.as_deref(), Some("a headset"));
        assert_eq!(s.adrift(Kind::Sink), Some("a headset"));
        assert_eq!(s.default_of(Kind::Sink).map(|n| n.id), Some(60));
    }

    #[test]
    fn a_metadata_object_that_is_not_the_defaults_is_passed_over() {
        let other = r#"[{ "id": 37, "type": "PipeWire:Interface:Metadata",
          "props": { "metadata.name": "sm-settings" },
          "metadata": [{ "subject": 0, "key": "default.audio.sink",
                         "value": { "name": "a lie" } }] }]"#;
        let s = graph_of(&[SINK, DEFAULTS, other]).snapshot();
        assert_eq!(s.defaults.sink.as_deref(), Some("alsa_output.speaker"));
    }

    #[test]
    fn a_volume_delta_does_not_empty_the_card() {
        // THE RULE THIS PINS is number 3 in the header. `wpctl set-volume`
        // makes `pw-dump` print the node with `Props` and nothing else. A
        // merge that replaced `params` whole would drop `EnumProfile` and
        // `EnumRoute` from the card the first time anybody touched a slider,
        // and the profile list would empty itself on screen with no error.
        let mut g = graph_of(&[SINK, CARD]);
        assert_eq!(g.snapshot().devices[0].profiles.len(), 3);

        let louder = r#"[{ "id": 52, "type": "PipeWire:Interface:Device",
          "info": { "change-mask": ["params"],
                    "params": { "Route": [{ "index": 3, "device": 3 }] } } }]"#;
        g.took_text(louder).expect("the fixture is JSON");

        let d = &g.snapshot().devices[0];
        assert_eq!(d.profiles.len(), 3, "the profiles survived a route change");
        assert_eq!(d.routes.len(), 2, "and so did the ports");
        assert_eq!(d.active_routes, vec![3], "and the new port took");
    }

    #[test]
    fn a_metadata_delta_does_not_throw_the_other_defaults_away() {
        // THE DEFECT THIS PINS was read off two photographs of the sound
        // view: between them the "in use" marker vanished from the default
        // microphone. PipeWire had reported ONE changed default, and a merge
        // that replaced the whole array lost the rest — so `default_of` found
        // nothing, the marker went, and the modeline's mute mark would have
        // gone with it.
        let mut g = graph_of(&[SINK, DEFAULTS]);
        assert_eq!(
            g.snapshot().defaults.sink.as_deref(),
            Some("alsa_output.speaker")
        );

        let one_changed = r#"[{ "id": 40, "type": "PipeWire:Interface:Metadata",
          "props": { "metadata.name": "default" },
          "metadata": [{ "subject": 0, "key": "default.configured.audio.sink",
                         "value": { "name": "something else" } }] }]"#;
        g.took_text(one_changed).expect("the fixture is JSON");

        let d = g.snapshot().defaults;
        assert_eq!(
            d.configured_sink.as_deref(),
            Some("something else"),
            "the new one took"
        );
        assert_eq!(
            d.sink.as_deref(),
            Some("alsa_output.speaker"),
            "and the rest survived"
        );
    }

    #[test]
    fn a_null_metadata_value_clears_that_one_entry() {
        let mut g = graph_of(&[SINK, DEFAULTS]);
        let cleared = r#"[{ "id": 40, "type": "PipeWire:Interface:Metadata",
          "props": { "metadata.name": "default" },
          "metadata": [{ "subject": 0, "key": "default.audio.sink", "value": null }] }]"#;
        g.took_text(cleared).expect("the fixture is JSON");
        let d = g.snapshot().defaults;
        assert_eq!(d.sink, None, "the entry went");
        assert_eq!(
            d.configured_sink.as_deref(),
            Some("a headset"),
            "and only that one"
        );
    }

    #[test]
    fn a_null_info_removes_the_object() {
        let stream = r#"[{ "id": 120, "type": "PipeWire:Interface:Node",
          "info": { "props": { "media.class": "Stream/Output/Audio",
                               "node.name": "a player", "application.name": "a player" },
                    "params": { "Props": [{ "channelVolumes": [1.0], "mute": false }] } } }]"#;
        let mut g = graph_of(&[SINK, stream]);
        assert_eq!(g.snapshot().of(Kind::Playing).len(), 1);

        g.took_text(r#"[{ "id": 120, "info": null }]"#)
            .expect("the fixture is JSON");
        assert_eq!(g.snapshot().of(Kind::Playing).len(), 0);
        assert_eq!(g.len(), 1, "and only that one went");
    }

    #[test]
    fn an_object_with_no_info_at_all_is_not_a_removal() {
        // The core object carries no `info` in a real dump, and reading its
        // absence as a removal would delete objects nobody asked to remove.
        let mut g = graph_of(&[SINK]);
        g.took_text(r#"[{ "id": 60, "type": "PipeWire:Interface:Node" }]"#)
            .expect("the fixture is JSON");
        assert_eq!(g.len(), 1);
        assert_eq!(g.snapshot().nodes[0].volume, 38, "the reading survived");
    }

    #[test]
    fn a_props_delta_replaces_and_a_params_delta_merges() {
        let mut g = graph_of(&[SINK]);
        let quieter = r#"[{ "id": 60, "type": "PipeWire:Interface:Node",
          "info": { "change-mask": ["params"],
                    "params": { "Props": [{ "channelVolumes": [1.0], "mute": true }] } } }]"#;
        g.took_text(quieter).expect("the fixture is JSON");
        let n = &g.snapshot().nodes[0];
        assert_eq!(n.volume, 100);
        assert!(n.muted);
        assert_eq!(n.description, "Speaker", "the props were not touched");
    }

    #[test]
    fn a_camera_is_not_a_card() {
        let webcam = r#"[{ "id": 53, "type": "PipeWire:Interface:Device",
          "info": { "props": { "media.class": "Video/Device",
                               "device.name": "v4l2_device.usb",
                               "device.description": "Integrated RGB Camera" } } }]"#;
        let s = graph_of(&[CARD, webcam]).snapshot();
        assert_eq!(s.devices.len(), 1);
        assert_eq!(s.devices[0].id, 52);
    }

    #[test]
    fn rubbish_on_the_stream_does_not_take_the_picture_down() {
        let mut g = graph_of(&[SINK]);
        g.took(&serde_json::json!("not an array"));
        g.took(&serde_json::json!([{ "no": "id" }]));
        g.took(&serde_json::json!([42]));
        assert_eq!(g.len(), 1);
        assert_eq!(g.snapshot().nodes.len(), 1);
    }

    #[test]
    fn a_node_with_no_props_is_passed_over_and_not_fatal() {
        // Six of a real dump's objects carry no props at all, and the first
        // of all of them carries no `node.name`.
        let s = graph_of(&[
            r#"[{ "id": 29, "type": "PipeWire:Interface:Node", "info": { "params": {} } }]"#,
            SINK,
        ])
        .snapshot();
        assert_eq!(s.nodes.len(), 1);
    }
}
