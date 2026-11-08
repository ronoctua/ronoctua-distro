//! THE NETWORK — a protocol's devices on their isometric floor, the packets
//! flying between them, and the camera framing what each step is about.
//!
//! # What is drawn how
//!
//! The floor and the links are DOTS — flat squares (`canvas.under`), as many
//! as the stage shows, placed afresh every frame: a dotted floor that pans
//! and zooms with the camera for nothing, and a link that "marches" in the
//! colour of the packet crossing it. The devices, the signs, the pulses and
//! the packets' labels are DRAWINGS (`ronoctua_packets::art`), rasterised
//! once per key — what is drawn, its inks, the camera's closeness it is
//! headed for, the palette and the screen's scale — and placed as sprites,
//! cut where the stage ends and faded as they leave its framing
//! (`image::Sprite`). Their words — a device's name, a packet's tag — are
//! text, over everything.
//!
//! # The camera
//!
//! Each step frames the devices it is about; the camera goes from the last
//! framing to this one, eased, in the step's first moments (`Pose::camera`).
//! While it moves, the drawings are scaled from what they were rasterised
//! for; they are rasterised for where the camera is going, so they are sharp
//! when it gets there.

use crate::packets_window::{Layout, Open, Paint, Rect, grid, ink_of, placed, quad, wide};
use ronoctua_i18n::Book;
use ronoctua_packets::art::{self, Art, HALF_ACROSS, HALF_DOWN, Kind, Prop};
use ronoctua_packets::timeline::Pose;
use ronoctua_packets::{Beat, Class, Packet, Protocol};
use ronoctua_render::image::{Rgba, Sprite};
use ronoctua_tokens as tokens;
use std::collections::HashMap;

/// The drawings made so far, by what they draw: an SVG is written once, and
/// rasterised once per key.
#[derive(Default)]
pub struct Drawings {
    arts: HashMap<String, Art>,
}

impl Drawings {
    /// The drawing `what` names, made the first time it is asked for.
    fn get(&mut self, what: &str, make: impl FnOnce() -> Art) -> Art {
        self.arts
            .entry(what.to_owned())
            .or_insert_with(make)
            .clone()
    }

    /// A device's drawing.
    fn device(&mut self, kind: Kind) -> Art {
        self.get(&format!("device:{kind:?}"), || art::device(kind))
    }
}

/// How a drawing is inked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ink {
    /// At rest.
    Idle,
    /// Lit by a step.
    Glow,
    /// In the colour of a packet's class.
    Class(Class),
    /// In one role, all of it.
    Plain(tokens::Color),
}

impl Ink {
    /// Its roles: the lines, the faces and what is lit.
    fn roles(self) -> (tokens::Color, tokens::Color, tokens::Color) {
        match self {
            Self::Idle => (
                tokens::color::TEXT_SECONDARY,
                tokens::color::TEXT_SECONDARY,
                tokens::color::TEXT_HEADING,
            ),
            Self::Glow => (
                tokens::color::TEXT_HEADING,
                tokens::color::TEXT_HEADING,
                tokens::color::TEXT_PRIMARY,
            ),
            Self::Class(c) => (ink_of(c), ink_of(c), ink_of(c)),
            Self::Plain(c) => (c, c, c),
        }
    }

    /// What a key says of it.
    fn word(self) -> String {
        match self {
            Self::Idle => "idle".into(),
            Self::Glow => "glow".into(),
            Self::Class(c) => format!("class{}", c.index()),
            Self::Plain(c) => format!("plain{c:?}"),
        }
    }
}

/// A drawing, rasterised in `ink` at `scale` pixels a unit, onto the GPU
/// under its key; the key, or `None` when it would not draw.
fn uploaded(
    paint: &mut Paint<'_, '_>,
    what: &str,
    drawing: &Art,
    ink: Ink,
    scale: f32,
) -> Option<String> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let bucket = (scale * 4.0).round().max(1.0) as u32;
    let key = format!(
        "packets:{what}:{}:{bucket}:{}",
        ink.word(),
        tokens::theme::palette()
    );
    let (line, fill, glow) = ink.roles();
    #[allow(clippy::cast_precision_loss)]
    let at = bucket as f32 / 4.0;
    let svg = &drawing.svg;
    paint
        .canvas
        .upload(&key, || {
            let resolved = art::resolved(svg, line.to_rgba8(), fill.to_rgba8(), glow.to_rgba8());
            art::raster(&resolved, at).map(|r| Rgba {
                width: r.width,
                height: r.height,
                pixels: r.pixels,
            })
        })
        .then_some(key)
}

/// A rounded label `w` by `h` logical pixels, lined and dotted in `glow`
/// and filled with the window's ground, onto the GPU at the screen's scale;
/// its key and drawing.
fn pill(
    paint: &mut Paint<'_, '_>,
    drawings: &mut Drawings,
    w: f32,
    h: f32,
    glow: tokens::Color,
) -> Option<(String, Art)> {
    let drawing = drawings.get(&format!("pill:{w:.0}:{h:.0}"), || art::pill(w, h));
    let ground = tokens::color::SURFACE_CHROME;
    let key = format!(
        "packets:pill{w:.0}x{h:.0}:{glow:?}:{}:{}",
        paint.scale,
        tokens::theme::palette()
    );
    #[allow(clippy::cast_precision_loss)]
    let scale = paint.scale as f32;
    let svg = drawing.svg.clone();
    paint
        .canvas
        .upload(&key, || {
            let resolved = art::resolved(&svg, glow.to_rgba8(), ground.to_rgba8(), glow.to_rgba8());
            art::raster(&resolved, scale).map(|r| Rgba {
                width: r.width,
                height: r.height,
                pixels: r.pixels,
            })
        })
        .then_some((key, drawing))
}

/// Where a device stands, in the scene's units: its floor point.
#[must_use]
pub fn spot(p: &Protocol, d: usize) -> (f32, f32) {
    let s = p.scene.devices[d].at;
    (f32::from(s.col) * HALF_ACROSS, f32::from(s.row) * HALF_DOWN)
}

/// Where the camera looks, and how close.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// The middle of what it frames, in the scene's units.
    pub x: f32,
    /// Likewise, down.
    pub y: f32,
    /// Logical pixels a unit.
    pub zoom: f32,
}

impl Camera {
    /// A point of the scene on the screen.
    #[must_use]
    pub fn on(self, stage: Rect, p: (f32, f32)) -> (f32, f32) {
        (
            stage.x + stage.w / 2.0 + (p.0 - self.x) * self.zoom,
            stage.y + stage.h / 2.0 + (p.1 - self.y) * self.zoom,
        )
    }
}

/// The air left around what a camera frames, in the scene's units: the
/// labels under the devices and a little more.
const FRAME_AIR: (f32, f32, f32) = (24.0, 20.0, 46.0);

/// The camera that frames `focus` (every device when it is empty) on `stage`.
fn framing(p: &Protocol, focus: &[usize], drawings: &mut Drawings, stage: Rect) -> Camera {
    let every: Vec<usize> = (0..p.scene.devices.len()).collect();
    let devices: &[usize] = if focus.is_empty() { &every } else { focus };
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &d in devices {
        let (sx, sy) = spot(p, d);
        let a = drawings.device(p.scene.devices[d].kind);
        x0 = x0.min(sx - a.anchor.0);
        y0 = y0.min(sy - a.anchor.1);
        x1 = x1.max(sx - a.anchor.0 + a.width);
        y1 = y1.max(sy - a.anchor.1 + a.height);
    }
    let (side, top, bottom) = FRAME_AIR;
    let (x0, y0, x1, y1) = (x0 - side, y0 - top, x1 + side, y1 + bottom);
    let least = f32::from(tokens::space::PACKETS_ZOOMLEASTTENTHS) / 10.0;
    let most = f32::from(tokens::space::PACKETS_ZOOMMOSTTENTHS) / 10.0;
    let zoom = (stage.w / (x1 - x0).max(1.0))
        .min(stage.h / (y1 - y0).max(1.0))
        .clamp(least, most);
    Camera {
        x: (x0 + x1) / 2.0,
        y: (y0 + y1) / 2.0,
        zoom,
    }
}

/// The camera now, and where it is going.
fn cameras(open: &Open, pose: &Pose, drawings: &mut Drawings, stage: Rect) -> (Camera, Camera) {
    let p = open.protocol;
    let to = framing(p, p.steps[pose.step].focus, drawings, stage);
    let (from_step, u) = pose.camera;
    if u >= 1.0 {
        return (to, to);
    }
    // THE FIRST STEP COMES FROM THE WHOLE NETWORK — what the overview shows
    // before it plays — and closes in on what it is about.
    let from = if from_step == pose.step && pose.step == 0 {
        framing(p, &[], drawings, stage)
    } else if from_step == pose.step {
        return (to, to);
    } else {
        framing(p, p.steps[from_step].focus, drawings, stage)
    };
    let now = Camera {
        x: from.x + (to.x - from.x) * u,
        y: from.y + (to.y - from.y) * u,
        zoom: from.zoom * (to.zoom / from.zoom).powf(u),
    };
    (now, to)
}

/// How much a device is in the step's framing, 0 to 1 — eased between the
/// framing the camera comes from and the one it goes to.
fn presence(open: &Open, pose: &Pose, d: usize) -> f32 {
    let p = open.protocol;
    let framed = |s: usize| {
        let focus = p.steps[s].focus;
        if focus.is_empty() || focus.contains(&d) {
            1.0
        } else {
            DIMMED
        }
    };
    let (from, u) = pose.camera;
    // Before the first step, every device is there: the whole network.
    let before = if from == pose.step && pose.step == 0 {
        1.0
    } else {
        framed(from)
    };
    before + (framed(pose.step) - before) * u
}

/// How a device out of the step's framing is drawn.
const DIMMED: f32 = 0.3;

/// How much of `area` the stage shows, 0 to 1 — a drawing cut by the
/// stage's edge fades with what is cut, so no sliver of one stands at it.
fn inside(stage: Rect, area: &ronoctua_render::image::Area) -> f32 {
    let w = (area.x + area.width).min(stage.x + stage.w) - area.x.max(stage.x);
    let h = (area.y + area.height).min(stage.y + stage.h) - area.y.max(stage.y);
    if w <= 0.0 || h <= 0.0 || area.width <= 0.0 || area.height <= 0.0 {
        return 0.0;
    }
    ((w * h) / (area.width * area.height))
        .clamp(0.0, 1.0)
        .powi(3)
}

/// Dots along a line from `a` to `b`, `gap` apart, starting `phase` in,
/// leaving `trim` free at each end: flat squares of `side`, inside `stage`
/// and off the words in `keep_off`.
#[allow(clippy::too_many_arguments)]
fn dots(
    out: &mut Vec<ronoctua_render::quad::Quad>,
    a: (f32, f32),
    b: (f32, f32),
    gap: f32,
    phase: f32,
    trim: f32,
    side: f32,
    color: tokens::Color,
    stage: Rect,
    keep_off: &[Rect],
) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    if length <= 2.0 * trim || gap <= 0.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let mut s = trim + phase.rem_euclid(gap);
    while s < length - trim {
        let (x, y) = (a.0 + ux * s, a.1 + uy * s);
        if stage.holds(x, y) && !keep_off.iter().any(|r| r.holds(x, y)) {
            out.push(quad(
                Rect {
                    x: x - side / 2.0,
                    y: y - side / 2.0,
                    w: side,
                    h: side,
                },
                color,
            ));
        }
        s += gap;
    }
}

/// Paints the network.
#[allow(clippy::too_many_lines)]
pub fn paint(
    paint: &mut Paint<'_, '_>,
    layout: &Layout,
    book: &Book,
    open: &Open,
    pose: &Pose,
    drawings: &mut Drawings,
) {
    let stage = layout.stage_rect();
    let p = open.protocol;
    let (cam, target) = cameras(open, pose, drawings, stage);
    #[allow(clippy::cast_precision_loss)]
    let scale = paint.scale as f32;
    let (cell, line) = grid();
    let tenth = |t: u16| f32::from(t) / 10.0;

    // THE FLOOR: a dot at every other point of the lattice the camera sees.
    let half = (stage.w / 2.0 / cam.zoom, stage.h / 2.0 / cam.zoom);
    #[allow(clippy::cast_possible_truncation)]
    let (c0, c1) = (
        ((cam.x - half.0) / HALF_ACROSS).floor() as i32 - 1,
        ((cam.x + half.0) / HALF_ACROSS).ceil() as i32 + 1,
    );
    #[allow(clippy::cast_possible_truncation)]
    let (r0, r1) = (
        ((cam.y - half.1) / HALF_DOWN).floor() as i32 - 1,
        ((cam.y + half.1) / HALF_DOWN).ceil() as i32 + 1,
    );
    let floor_dot = tenth(tokens::space::PACKETS_FLOORDOTTENTHS);
    for row in r0..=r1 {
        for col in c0..=c1 {
            if (col + row).rem_euclid(4) != 0 || row.rem_euclid(2) != 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let (x, y) = cam.on(stage, (col as f32 * HALF_ACROSS, row as f32 * HALF_DOWN));
            if stage.holds(x, y) {
                paint.canvas.under.push(quad(
                    Rect {
                        x: x - floor_dot / 2.0,
                        y: y - floor_dot / 2.0,
                        w: floor_dot,
                        h: floor_dot,
                    },
                    tokens::color::RULE_DEFAULT,
                ));
            }
        }
    }

    // WHERE THE DEVICES' WORDS GO, before anything is drawn: the links keep
    // off them.
    let mut labels: Vec<Rect> = Vec::new();
    for d in 0..p.scene.devices.len() {
        let device = p.scene.devices[d];
        let drawing = drawings.device(device.kind);
        let (x, y) = cam.on(stage, spot(p, d));
        let below = y + (drawing.height - drawing.anchor.1) * cam.zoom;
        let mut said = vec![book.t(device.name)];
        said.extend(device.address.text());
        for (k, text) in said.iter().enumerate() {
            let w = wide(text) + cell;
            #[allow(clippy::cast_precision_loss)]
            let mid = below + line * (0.6 + k as f32);
            labels.push(Rect {
                x: x - w / 2.0,
                y: mid - line / 2.0,
                w,
                h: line,
            });
        }
    }

    // THE LINKS, dotted — and the one a packet crosses, in its colour,
    // marching the way it goes.
    let gap = f32::from(tokens::space::PACKETS_LINKGAPPX);
    let link_dot = tenth(tokens::space::PACKETS_LINKDOTTENTHS);
    let trim = 26.0 * cam.zoom;
    #[allow(clippy::cast_precision_loss)]
    let march = open.t as f32 * 0.05;
    for l in p.scene.links {
        let crossing = pose.flights.iter().find(|f| {
            (f.between.0 == l.a && f.between.1 == l.b) || (f.between.0 == l.b && f.between.1 == l.a)
        });
        let (a, b) = match crossing {
            Some(f) => (f.between.0, f.between.1),
            None => (l.a, l.b),
        };
        let (pa, pb) = (cam.on(stage, spot(p, a)), cam.on(stage, spot(p, b)));
        match crossing {
            Some(f) => dots(
                &mut paint.canvas.under,
                pa,
                pb,
                gap,
                march,
                trim,
                link_dot * 1.3,
                ink_of(f.class),
                stage,
                &labels,
            ),
            None => dots(
                &mut paint.canvas.under,
                pa,
                pb,
                gap,
                0.0,
                trim,
                link_dot,
                tokens::color::TEXT_DIM,
                stage,
                &labels,
            ),
        }
    }

    let packets: Vec<&Packet> = p.packets().map(|(_, packet)| packet).collect();

    // THE PULSES, on the floor, under the devices.
    for pulse in &pose.pulses {
        let ring = drawings.get("pulse", || art::pulse(30.0));
        let Some(key) = uploaded(
            paint,
            "pulse",
            &ring,
            Ink::Class(pulse.class),
            target.zoom * scale,
        ) else {
            continue;
        };
        let (x, y) = cam.on(stage, spot(p, pulse.at));
        let grow = cam.zoom * (0.6 + pulse.along);
        paint.canvas.sprites.push(Sprite {
            key,
            area: ronoctua_render::image::Area {
                x: x - ring.anchor.0 * grow,
                y: y - ring.anchor.1 * grow,
                width: ring.width * grow,
                height: ring.height * grow,
            },
            clip: stage.area(),
            alpha: 1.0 - pulse.along,
        });
    }

    // THE DEVICES, back to front.
    let mut order: Vec<usize> = (0..p.scene.devices.len()).collect();
    order.sort_by(|a, b| {
        let (sa, sb) = (spot(p, *a), spot(p, *b));
        sa.1.total_cmp(&sb.1).then(sa.0.total_cmp(&sb.0))
    });
    for d in order {
        let device = p.scene.devices[d];
        let drawing = drawings.device(device.kind);
        let flying_here = pose.flights.iter().find(|f| {
            packets
                .get(f.packet)
                .is_some_and(|pk| pk.from == d || pk.to == d)
        });
        let ink = if let Some(f) = flying_here {
            Ink::Class(f.class)
        } else if pose.pulses.iter().any(|pu| pu.at == d) {
            Ink::Class(
                pose.pulses
                    .iter()
                    .find(|pu| pu.at == d)
                    .map_or(Class::Data, |pu| pu.class),
            )
        } else if pose.lit.iter().any(|l| l.at == d) {
            Ink::Glow
        } else {
            Ink::Idle
        };
        let what = format!("{:?}", device.kind);
        let Some(key) = uploaded(paint, &what, &drawing, ink, target.zoom * scale) else {
            continue;
        };
        let (x, y) = cam.on(stage, spot(p, d));
        let area = ronoctua_render::image::Area {
            x: x - drawing.anchor.0 * cam.zoom,
            y: y - drawing.anchor.1 * cam.zoom,
            width: drawing.width * cam.zoom,
            height: drawing.height * cam.zoom,
        };
        let alpha = presence(open, pose, d) * inside(stage, &area);
        paint.canvas.sprites.push(Sprite {
            key,
            area,
            clip: stage.area(),
            alpha,
        });
        // ITS NAME AND ADDRESS under it, whole inside the stage or not at all.
        let name = book.t(device.name);
        let below = y + (drawing.height - drawing.anchor.1) * cam.zoom;
        let mut lines = vec![(
            name,
            if alpha > 0.6 {
                tokens::color::TEXT_PRIMARY
            } else {
                tokens::color::TEXT_DIM
            },
        )];
        if let Some(address) = device.address.text() {
            lines.push((address, tokens::color::TEXT_DIM));
        }
        for (k, (text, color)) in lines.into_iter().enumerate() {
            let w = wide(&text);
            #[allow(clippy::cast_precision_loss)]
            let mid = below + line * (0.6 + k as f32);
            let r = Rect {
                x: x - w / 2.0,
                y: mid - line / 2.0,
                w,
                h: line,
            };
            if stage.holds(r.x, r.y) && stage.holds(r.x + r.w, r.y + r.h) {
                paint.canvas.spans.push(placed(text, r.x, mid, color));
            }
        }
    }

    // THE SIGNS a step shows over its devices.
    for lit in &pose.lit {
        let Some(prop) = lit.prop else {
            continue;
        };
        let sign = drawings.get(&format!("prop:{prop:?}"), || art::prop(prop));
        let ink = match prop {
            Prop::Check => Ink::Plain(tokens::color::STATE_SUCCESS),
            Prop::Cross => Ink::Plain(tokens::color::STATE_ERROR),
            _ => Ink::Plain(tokens::color::TEXT_HEADING),
        };
        let Some(key) = uploaded(
            paint,
            &format!("prop{prop:?}"),
            &sign,
            ink,
            target.zoom * scale,
        ) else {
            continue;
        };
        let device = drawings.device(p.scene.devices[lit.at].kind);
        let (x, y) = cam.on(stage, spot(p, lit.at));
        let top = y - device.anchor.1 * cam.zoom - (sign.height - sign.anchor.1 + 6.0) * cam.zoom;
        paint.canvas.sprites.push(Sprite {
            key,
            area: ronoctua_render::image::Area {
                x: x - sign.anchor.0 * cam.zoom,
                y: top - sign.anchor.1 * cam.zoom,
                width: sign.width * cam.zoom,
                height: sign.height * cam.zoom,
            },
            clip: stage.area(),
            alpha: lit.strength,
        });
    }

    // WHAT IS TYPED, in a bubble over its device, the caret blinking.
    if let Some((d, chars, typing)) = pose.typed {
        let wire = p.steps[pose.step].beats.iter().find_map(|b| match b {
            Beat::Type { at, wire } if *at == d => Some(*wire),
            _ => None,
        });
        if let Some(wire) = wire {
            let shown: String = wire.chars().take(chars).collect();
            let (w, h) = (wide(wire) + 3.0 * cell, line * 1.3);
            if let Some((key, bubble)) = pill(paint, drawings, w, h, tokens::color::TEXT_HEADING) {
                let device = drawings.device(p.scene.devices[d].kind);
                let (x, y) = cam.on(stage, spot(p, d));
                let mid = y - device.anchor.1 * cam.zoom - h;
                let left = x - w / 2.0;
                let r = Rect {
                    x: left,
                    y: mid - h / 2.0,
                    w,
                    h,
                };
                paint.canvas.sprites.push(Sprite {
                    key,
                    area: ronoctua_render::image::Area {
                        x: x - bubble.anchor.0,
                        y: mid - bubble.anchor.1,
                        width: bubble.width,
                        height: bubble.height,
                    },
                    clip: stage.area(),
                    alpha: 1.0,
                });
                let text_x = left + h * 0.9;
                if stage.holds(r.x, r.y) && stage.holds(r.x + r.w, r.y + r.h) {
                    paint.canvas.spans.push(placed(
                        shown.clone(),
                        text_x,
                        mid,
                        tokens::color::TEXT_PRIMARY,
                    ));
                    let blink = (open.t / 450).is_multiple_of(2);
                    if typing && blink {
                        paint.canvas.over.push(quad(
                            Rect {
                                x: text_x + wide(&shown) + 1.0,
                                y: mid - line * 0.35,
                                w: 1.5,
                                h: line * 0.7,
                            },
                            tokens::color::TEXT_PRIMARY,
                        ));
                    }
                }
            }
        }
    }

    // THE PACKETS IN THE AIR: a label in their colour, a little above the
    // links, and a trail behind.
    let lift = f32::from(tokens::space::PACKETS_LIFT) * cam.zoom;
    for f in &pose.flights {
        let Some(packet) = packets.get(f.packet) else {
            continue;
        };
        let (a, b) = (
            cam.on(stage, spot(p, f.between.0)),
            cam.on(stage, spot(p, f.between.1)),
        );
        let at = |u: f32| (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u - lift);
        let (x, y) = at(f.along);
        for k in 1..=3 {
            #[allow(clippy::cast_precision_loss)]
            let (tx, ty) = at((f.along - 0.035 * k as f32).max(0.0));
            #[allow(clippy::cast_precision_loss)]
            let side = 5.0 - k as f32;
            if stage.holds(tx, ty) {
                paint.canvas.over.push(quad(
                    Rect {
                        x: tx - side / 2.0,
                        y: ty - side / 2.0,
                        w: side,
                        h: side,
                    },
                    ink_of(f.class),
                ));
            }
        }
        let (w, h) = (wide(packet.tag) + 2.6 * cell, line * 1.2);
        if let Some((key, label)) = pill(paint, drawings, w, h, ink_of(f.class)) {
            paint.canvas.sprites.push(Sprite {
                key,
                area: ronoctua_render::image::Area {
                    x: x - label.anchor.0,
                    y: y - label.anchor.1,
                    width: label.width,
                    height: label.height,
                },
                clip: stage.area(),
                alpha: 1.0,
            });
        }
        let r = Rect {
            x: x - w / 2.0,
            y: y - h / 2.0,
            w,
            h,
        };
        if stage.holds(r.x, r.y) && stage.holds(r.x + r.w, r.y + r.h) {
            paint.canvas.spans.push(placed(
                packet.tag.to_owned(),
                r.x + h * 0.85,
                y,
                tokens::color::TEXT_PRIMARY,
            ));
        }
    }
}
