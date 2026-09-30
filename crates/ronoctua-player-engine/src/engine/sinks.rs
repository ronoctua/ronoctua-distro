//! The sound outputs a lane can be sent to — read from `ronoctua-audio`,
//! the one crate that talks to PipeWire, and only while somebody needs them.
//!
//! # Only while needed
//!
//! A door that shows the outputs holds a connection open ([`Ask::Sinks`]),
//! and a lane sent to one needs to know when it goes away. While either is
//! true the engine holds a subscription to the audio crate's reading of the
//! graph; when neither is, the subscription is dropped and its monitor child
//! ends with it. A player at rest reads nothing.
//!
//! # An output that goes away
//!
//! A lane keeps the output it was sent to — the person's choice — and plays
//! on the machine's default while that output is not there, which is said
//! once ([`Trouble::SinkGone`]). When the output comes back the lane goes
//! back to it.
//!
//! [`Ask::Sinks`]: ronoctua_player::wire::Ask::Sinks

use ronoctua_player::{
    own::LaneKind,
    wire::{Sink, SinkKind, Trouble},
};

use super::Engine;

/// What the engine keeps of the outputs.
#[derive(Default)]
pub(crate) struct Sinks {
    /// The subscription, while one is needed.
    held: Option<ronoctua_audio::Subscription>,
    /// Which subscription this is: a reading from one dropped before is not
    /// this one's.
    generation: u64,
    /// Doors holding the list open ([`Ask::Sinks`]).
    ///
    /// [`Ask::Sinks`]: ronoctua_player::wire::Ask::Sinks
    pub holders: usize,
    /// The outputs, as last read.
    pub list: Vec<Sink>,
    /// A reading came since the subscription began: before it, an output
    /// nobody has read yet is not an output that is gone.
    known: bool,
    /// Outputs already said to be gone, so each is said once.
    said: Vec<String>,
}

/// The wire's kind of output, from the audio crate's.
#[must_use]
pub(super) const fn kind_of(kind: ronoctua_audio::OutputKind) -> SinkKind {
    match kind {
        ronoctua_audio::OutputKind::Headphones => SinkKind::Headphones,
        ronoctua_audio::OutputKind::Speakers => SinkKind::Speakers,
        ronoctua_audio::OutputKind::Bluetooth => SinkKind::Bluetooth,
        ronoctua_audio::OutputKind::Other => SinkKind::Other,
    }
}

/// The outputs of a reading of the graph, as the wire says them.
#[must_use]
pub(super) fn sinks_of(snapshot: &ronoctua_audio::Snapshot) -> Vec<Sink> {
    snapshot
        .of(ronoctua_audio::Kind::Sink)
        .into_iter()
        .map(|node| Sink {
            device: node.name.clone(),
            description: node.description.clone(),
            kind: kind_of(snapshot.output_kind(node)),
        })
        .collect()
}

impl Engine {
    /// Whether the output `device` is there — or not known yet, which is
    /// not the same as gone.
    pub(super) fn sink_is_there(&self, device: &str) -> bool {
        !self.sinks.known || self.sinks.list.iter().any(|s| s.device == device)
    }

    /// Says, once, that a lane's output is gone and it plays on the default.
    pub(super) fn sink_gone(&mut self, device: &str) {
        if self.sinks.said.iter().any(|d| d == device) {
            return;
        }
        self.sinks.said.push(device.to_owned());
        // WHAT IS WRONG NOW, as every trouble is said: an older refusal goes,
        // and outputs gone together are said together.
        self.state
            .troubles
            .retain(|t| matches!(t, Trouble::SinkGone { .. }));
        self.state.troubles.push(Trouble::SinkGone {
            device: device.to_owned(),
        });
        self.dirty = true;
    }

    /// Whether a lane of what plays is sent to an output of its own.
    fn a_lane_has_a_sink(&self) -> bool {
        let (lanes, _) = self.lanes_now();
        lanes
            .iter()
            .any(|l| l.enabled && l.kind == LaneKind::Audio && l.sink.is_some())
    }

    /// Holds the subscription while a door or a lane needs the outputs, and
    /// drops it — ending its monitor — when neither does.
    pub(super) fn mind_sinks(&mut self) {
        let wanted = self.sinks.holders > 0 || self.a_lane_has_a_sink();
        match (wanted, self.sinks.held.is_some()) {
            (true, false) => {
                let Ok(shared) = self.shared() else { return };
                self.sinks.generation += 1;
                let generation = self.sinks.generation;
                let weak = std::sync::Arc::downgrade(&shared);
                match ronoctua_audio::subscribe(move |snapshot| {
                    let Some(shared) = weak.upgrade() else { return };
                    let (lock, bell) = &*shared;
                    if let Ok(mut e) = lock.lock() {
                        e.took_sinks(generation, &snapshot);
                        if e.dirty {
                            e.broadcast();
                        }
                    }
                    bell.notify_all();
                }) {
                    Ok(held) => self.sinks.held = Some(held),
                    Err(e) => tracing::warn!(error = %e, "the outputs cannot be read"),
                }
            }
            (false, true) => {
                self.sinks.held = None;
                self.sinks.list.clear();
                self.sinks.known = false;
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// A reading of the outputs: kept, and every lane sent where it was
    /// sent — or to the default while its output is not there.
    fn took_sinks(&mut self, generation: u64, snapshot: &ronoctua_audio::Snapshot) {
        if self.sinks.held.is_none() || generation != self.sinks.generation {
            return;
        }
        let list = sinks_of(snapshot);
        if list == self.sinks.list && self.sinks.known {
            return;
        }
        self.sinks.list = list;
        self.sinks.known = true;
        // AN OUTPUT THAT CAME BACK may be said gone again when it next goes.
        let there: Vec<String> = self.sinks.list.iter().map(|s| s.device.clone()).collect();
        self.sinks.said.retain(|d| !there.contains(d));
        self.dirty = true;
        let (lanes, arrangement) = self.lanes_now();
        let covers = arrangement.unwrap_or_default().covers;
        let t = self.clock_now();
        if self.session.is_some()
            && let Err(super::Refused(t)) = self.cast(&lanes, &covers, t)
        {
            self.state.troubles = vec![t];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the list of outputs stops being the graph's sinks by their
    /// PipeWire names — what a lane keeps and mpv is told — or their kind
    /// stops being what the audio crate reads off the card.
    #[test]
    fn the_outputs_are_the_sinks_by_their_names() {
        let snapshot = ronoctua_audio::Snapshot {
            nodes: vec![
                ronoctua_audio::Node {
                    id: 60,
                    kind: Some(ronoctua_audio::Kind::Sink),
                    name: "bluez_output.AA_BB.1".into(),
                    description: "Headset".into(),
                    address: Some("AA:BB".into()),
                    ..ronoctua_audio::Node::default()
                },
                ronoctua_audio::Node {
                    id: 61,
                    kind: Some(ronoctua_audio::Kind::Source),
                    name: "mic".into(),
                    ..ronoctua_audio::Node::default()
                },
            ],
            ..ronoctua_audio::Snapshot::default()
        };
        assert_eq!(
            sinks_of(&snapshot),
            vec![Sink {
                device: "bluez_output.AA_BB.1".into(),
                description: "Headset".into(),
                kind: SinkKind::Bluetooth,
            }]
        );
    }
}
