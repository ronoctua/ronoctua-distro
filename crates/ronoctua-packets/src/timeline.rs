//! WHERE EVERYTHING IS AT A MOMENT — a protocol's steps laid on a clock, and
//! the [`Pose`] of the scene at any time on it.
//!
//! A [`Timeline`] is made once, from a protocol and a [`Pace`] (how long a
//! camera move, a hop, a typed character take — the window's tokens), and
//! [`Timeline::at`] is a pure function of the time: the window asks it for
//! the frame's moment, a photograph for any moment it names, and a test for
//! the moments that matter. Nothing here keeps a clock; nothing reads one.
//!
//! The steps follow one another: each first moves the camera to what it is
//! about (when that changed), then plays its beats in order — a packet
//! flies hop by hop along the links, an address is typed a character at a
//! time, a device lights — and rests before the next.

use crate::{Beat, Class, Protocol, art};

/// How long things take, in milliseconds of the animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pace {
    /// The camera moving from one framing to the next.
    pub camera: u32,
    /// A packet crossing one link.
    pub hop: u32,
    /// A character typed.
    pub key: u32,
    /// A device lighting, a sign appearing, a pulse spreading.
    pub light: u32,
    /// A rest, and the rest at the end of a step.
    pub rest: u32,
}

/// When a step plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// When it starts.
    pub start: u32,
    /// When it ends — the next one's start.
    pub end: u32,
}

/// What happens in a stretch of time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum What {
    /// The camera moves to the step's framing.
    Camera,
    /// A packet flies along `path`, a device to the next.
    Fly {
        /// The packet, by its place among all of the protocol's.
        packet: usize,
        /// Its class.
        class: Class,
        /// The devices it passes, from the sender to the receiver.
        path: Vec<usize>,
    },
    /// Something is typed beside a device.
    Type {
        /// The device.
        at: usize,
        /// How many characters.
        chars: usize,
    },
    /// A device lights, with a sign or none.
    Light {
        /// The device.
        at: usize,
        /// The sign.
        prop: Option<art::Prop>,
    },
    /// A pause.
    Rest,
}

/// Something that happens, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The step it is in.
    pub step: usize,
    /// When it starts.
    pub start: u32,
    /// When it ends.
    pub end: u32,
    /// What it is.
    pub what: What,
}

/// A protocol's steps on a clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    /// When each step plays.
    pub steps: Vec<Span>,
    /// Everything that happens, in order.
    pub events: Vec<Event>,
    /// When the last step ends.
    pub total: u32,
    /// The pace it was laid at — kept for [`Timeline::at`].
    pace: Pace,
}

/// A packet in the air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    /// The packet, by its place among all of the protocol's.
    pub packet: usize,
    /// Its class.
    pub class: Class,
    /// The device it left last, and the one it goes to.
    pub between: (usize, usize),
    /// How far between them, 0 to 1.
    pub along: f32,
}

/// A device lit, how much, and why.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lit {
    /// The device.
    pub at: usize,
    /// How lit, 0 to 1.
    pub strength: f32,
    /// The sign it shows.
    pub prop: Option<art::Prop>,
}

/// A pulse spreading from a device a packet reached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pulse {
    /// The device.
    pub at: usize,
    /// The packet's class.
    pub class: Class,
    /// How far it has spread, 0 to 1.
    pub along: f32,
}

/// The scene at a moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    /// The step playing.
    pub step: usize,
    /// The camera: the step it comes from, and how far it has come to this
    /// one's framing, eased, 0 to 1.
    pub camera: (usize, f32),
    /// The packets in the air.
    pub flights: Vec<Flight>,
    /// How many packets have left — the capture's rows.
    pub sent: usize,
    /// The packet the step is about: the last to leave, in this step.
    pub current: Option<usize>,
    /// The devices lit in this step.
    pub lit: Vec<Lit>,
    /// The pulses spreading.
    pub pulses: Vec<Pulse>,
    /// What is being typed: the device, how many characters are there, and
    /// whether the typing goes on.
    pub typed: Option<(usize, usize, bool)>,
}

/// Slow at both ends: a move that starts and stops.
#[must_use]
pub fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// How far `t` is from `start` to `end`, 0 to 1.
fn progress(t: u32, start: u32, end: u32) -> f32 {
    if end <= start {
        return 1.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let u = (t.saturating_sub(start)) as f32 / (end - start) as f32;
    u.clamp(0.0, 1.0)
}

impl Timeline {
    /// A protocol's steps on a clock, at `pace`.
    #[must_use]
    pub fn of(protocol: &Protocol, pace: Pace) -> Self {
        let mut t = 0_u32;
        let mut steps = Vec::with_capacity(protocol.steps.len());
        let mut events = Vec::new();
        let mut packet = 0;
        let mut framing: Option<&[usize]> = None;
        for (s, step) in protocol.steps.iter().enumerate() {
            let start = t;
            if framing != Some(step.focus) {
                events.push(Event {
                    step: s,
                    start: t,
                    end: t + pace.camera,
                    what: What::Camera,
                });
                t += pace.camera;
                framing = Some(step.focus);
            }
            for beat in step.beats {
                let (lasts, what) = match beat {
                    Beat::Send(p) => {
                        let path = protocol
                            .path(p.from, p.to)
                            .unwrap_or_else(|| vec![p.from, p.to]);
                        #[allow(clippy::cast_possible_truncation)]
                        let hops = path.len().saturating_sub(1).max(1) as u32;
                        let what = What::Fly {
                            packet,
                            class: p.class,
                            path,
                        };
                        packet += 1;
                        (pace.hop * hops, what)
                    }
                    Beat::Type { at, wire } => {
                        let chars = wire.chars().count();
                        #[allow(clippy::cast_possible_truncation)]
                        let lasts = pace.key * chars as u32 + pace.rest;
                        (lasts, What::Type { at: *at, chars })
                    }
                    Beat::Light { at, prop } => (
                        pace.light,
                        What::Light {
                            at: *at,
                            prop: *prop,
                        },
                    ),
                    Beat::Rest => (pace.rest, What::Rest),
                };
                events.push(Event {
                    step: s,
                    start: t,
                    end: t + lasts,
                    what,
                });
                t += lasts;
            }
            t += pace.rest;
            steps.push(Span { start, end: t });
        }
        Self {
            steps,
            events,
            total: t,
            pace,
        }
    }

    /// When step `s` starts.
    #[must_use]
    pub fn start_of(&self, s: usize) -> u32 {
        self.steps.get(s).map_or(self.total, |span| span.start)
    }

    /// When packet `p` leaves.
    #[must_use]
    pub fn leaves(&self, p: usize) -> Option<u32> {
        self.events.iter().find_map(|e| match &e.what {
            What::Fly { packet, .. } if *packet == p => Some(e.start),
            _ => None,
        })
    }

    /// The step playing at `t`.
    #[must_use]
    pub fn step_at(&self, t: u32) -> usize {
        self.steps
            .iter()
            .rposition(|span| span.start <= t)
            .unwrap_or(0)
    }

    /// The scene at `t` (held inside the timeline).
    #[must_use]
    pub fn at(&self, t: u32) -> Pose {
        let t = t.min(self.total);
        let step = self.step_at(t);
        let mut pose = Pose {
            step,
            camera: (step, 1.0),
            flights: Vec::new(),
            sent: 0,
            current: None,
            lit: Vec::new(),
            pulses: Vec::new(),
            typed: None,
        };
        // THE CAMERA comes from the last step that framed something else.
        let came_from = self.events[..]
            .iter()
            .filter(|e| e.what == What::Camera && e.step < step)
            .map(|e| e.step)
            .next_back()
            .unwrap_or(step);
        if let Some(cam) = self
            .events
            .iter()
            .find(|e| e.step == step && e.what == What::Camera)
        {
            pose.camera = (came_from, ease(progress(t, cam.start, cam.end)));
        }
        for e in &self.events {
            if e.start > t {
                break;
            }
            match &e.what {
                What::Fly {
                    packet,
                    class,
                    path,
                } => {
                    pose.sent += 1;
                    if e.step == step {
                        pose.current = Some(*packet);
                    }
                    if t < e.end {
                        let hops = path.len().saturating_sub(1).max(1);
                        #[allow(clippy::cast_precision_loss)]
                        let along = ease(progress(t, e.start, e.end)) * hops as f32;
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let hop = (along.floor() as usize).min(hops - 1);
                        #[allow(clippy::cast_precision_loss)]
                        let within = along - hop as f32;
                        let a = path[hop.min(path.len() - 1)];
                        let b = path[(hop + 1).min(path.len() - 1)];
                        pose.flights.push(Flight {
                            packet: *packet,
                            class: *class,
                            between: (a, b),
                            along: within.clamp(0.0, 1.0),
                        });
                    } else if t < e.end + self.pace.light && e.step == step {
                        pose.pulses.push(Pulse {
                            at: *path.last().unwrap_or(&0),
                            class: *class,
                            along: progress(t, e.end, e.end + self.pace.light),
                        });
                    }
                }
                What::Type { at, chars } if e.step == step => {
                    let shown = ((t - e.start) / self.pace.key.max(1)) as usize;
                    pose.typed = Some((*at, shown.min(*chars), t < e.end));
                }
                What::Light { at, prop } if e.step == step => {
                    pose.lit.push(Lit {
                        at: *at,
                        strength: progress(t, e.start, e.start + self.pace.light.max(1)),
                        prop: *prop,
                    });
                }
                _ => {}
            }
        }
        pose
    }
}
