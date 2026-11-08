//! THE DRAWINGS of the protocols' manual: each device of a scene and each
//! sign a step shows, as SVG in isometric line art — a blueprint of the
//! network, in the theme's inks.
//!
//! # The inks are the theme's
//!
//! No drawing names a colour. Each paints with three placeholders, [`INK`]
//! (its lines), [`FILL`] (its faces, at the opacity each face is given) and
//! [`GLOW`] (what is lit on it: a screen, a light, an emblem), and the window
//! swaps them for the roles of the theme it wears — the device's ink, or the
//! colour of the packet that lights it ([`resolved`]). A face is told from
//! another by its opacity alone: no shadow, no elevation (invariant 15).
//!
//! # Isometric
//!
//! A device is modelled in world units, its footprint centred on the floor
//! point it stands on, and projected with [`project`]: `x` goes right and
//! down the screen, `y` left and down, `z` up. The floor's lattice is the
//! same projection's ([`HALF_ACROSS`], [`HALF_DOWN`]), so a device drawn here
//! stands where the scene's [`crate::Spot`] says. What a drawing gives back
//! ([`Art`]) is its SVG, its size, and where in it that floor point is — the
//! anchor the window places it by.

/// The placeholder of a drawing's lines.
pub const INK: &str = "@ink";
/// The placeholder of its faces.
pub const FILL: &str = "@fill";
/// The placeholder of what is lit on it.
pub const GLOW: &str = "@glow";

/// A floor tile's side, in world units.
pub const TILE: f32 = 64.0;

/// cos 30°: how far across the screen a world unit along `x` or `y` goes.
const ACROSS: f32 = 0.866_025_4;
/// sin 30°: how far down.
const DOWN: f32 = 0.5;

/// A lattice column — half a tile's width on the screen — in world units.
pub const HALF_ACROSS: f32 = TILE * ACROSS / 2.0;
/// A lattice row — half a tile's height on the screen.
pub const HALF_DOWN: f32 = TILE * DOWN / 2.0;

/// The width of a drawing's lines, in world units.
const STROKE: f32 = 1.3;

/// A world point on the screen, in world units: `x` right and down, `y` left
/// and down, `z` up.
#[must_use]
pub fn project(x: f32, y: f32, z: f32) -> (f32, f32) {
    ((x - y) * ACROSS, (x + y) * DOWN - z)
}

/// What a device is, and so how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A laptop.
    Laptop,
    /// A phone.
    Phone,
    /// A desktop computer: a screen on its stand.
    Desktop,
    /// A home router.
    Router,
    /// A switch.
    Switch,
    /// A wireless access point.
    AccessPoint,
    /// The internet, a cloud.
    Cloud,
    /// A provider's router, joining networks.
    CoreRouter,
    /// A rack of servers: a resolver, a name server.
    Rack,
    /// A server's tower, with the sign of what it serves on top.
    Tower(Emblem),
    /// A file server.
    Storage,
    /// A printer.
    Printer,
    /// A controller of a machine (a PLC).
    Controller,
}

/// The sign on top of a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Emblem {
    /// The web: a globe.
    Globe,
    /// An application: a gear.
    Gear,
    /// Mail: an envelope.
    Envelope,
    /// Time: a clock.
    Clock,
    /// A page — a portal's.
    Page,
    /// A key — who may enter.
    Key,
    /// Two arrows — what passes on for another.
    Arrows,
    /// A shield — a tunnel's end.
    Shield,
    /// A hub — what hands messages on.
    Hub,
    /// A mirror — what tells you how you look from outside.
    Mirror,
    /// An onion — a relay of layers.
    Onion,
    /// A list — who has what.
    List,
}

/// A sign a step shows beside a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prop {
    /// A padlock: encrypted from here.
    Lock,
    /// A key.
    Key,
    /// An envelope.
    Envelope,
    /// A certificate.
    Certificate,
    /// A clock.
    Clock,
    /// A page, drawn.
    Page,
    /// An onion's layers.
    Onion,
    /// A shield.
    Shield,
    /// A check: it holds.
    Check,
    /// A cross: refused.
    Cross,
}

/// A drawing: its SVG (with the placeholder inks), its size, and where in it
/// the floor point it stands on is — all in world units.
#[derive(Debug, Clone, PartialEq)]
pub struct Art {
    /// The SVG.
    pub svg: String,
    /// Its width.
    pub width: f32,
    /// Its height.
    pub height: f32,
    /// Where its floor point is, from its top left.
    pub anchor: (f32, f32),
}

/// A drawing being made: its elements, and how far they reach.
struct Pen {
    body: String,
    min: (f32, f32),
    max: (f32, f32),
}

/// A number as SVG writes it.
fn n(v: f32) -> String {
    format!("{v:.2}")
}

impl Pen {
    fn new() -> Self {
        Self {
            body: String::new(),
            min: (f32::MAX, f32::MAX),
            max: (f32::MIN, f32::MIN),
        }
    }

    fn reach(&mut self, (x, y): (f32, f32)) {
        self.min = (self.min.0.min(x), self.min.1.min(y));
        self.max = (self.max.0.max(x), self.max.1.max(y));
    }

    fn points(&mut self, pts: &[(f32, f32)]) -> String {
        let mut d = String::new();
        for (k, p) in pts.iter().enumerate() {
            self.reach(*p);
            d.push_str(if k == 0 { "M" } else { " L" });
            d.push_str(&format!("{} {}", n(p.0), n(p.1)));
        }
        d
    }

    /// A face: a closed polygon of world points, filled at `fill` and lined.
    fn face(&mut self, pts: &[(f32, f32, f32)], fill: f32) {
        let flat: Vec<_> = pts.iter().map(|&(x, y, z)| project(x, y, z)).collect();
        self.shape(&flat, fill, INK);
    }

    /// A closed polygon of screen points, filled at `fill` and lined in `ink`.
    fn shape(&mut self, flat: &[(f32, f32)], fill: f32, ink: &str) {
        let d = self.points(flat);
        self.body.push_str(&format!(
            r#"<path d="{d} Z" fill="{FILL}" fill-opacity="{}" stroke="{ink}" stroke-width="{}" stroke-linejoin="round"/>"#,
            n(fill),
            n(STROKE)
        ));
    }

    /// A closed polygon of screen points, filled with `ink` at `opacity`, no line.
    fn solid(&mut self, flat: &[(f32, f32)], ink: &str, opacity: f32) {
        let d = self.points(flat);
        self.body.push_str(&format!(
            r#"<path d="{d} Z" fill="{ink}" fill-opacity="{}"/>"#,
            n(opacity)
        ));
    }

    /// An open line through world points, in `ink`, `width` times the stroke.
    fn line(&mut self, pts: &[(f32, f32, f32)], ink: &str, width: f32) {
        let flat: Vec<_> = pts.iter().map(|&(x, y, z)| project(x, y, z)).collect();
        self.flat_line(&flat, ink, width);
    }

    /// An open line through screen points.
    fn flat_line(&mut self, flat: &[(f32, f32)], ink: &str, width: f32) {
        let d = self.points(flat);
        self.body.push_str(&format!(
            r#"<path d="{d}" fill="none" stroke="{ink}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            n(STROKE * width)
        ));
    }

    /// A dot at a world point.
    fn dot(&mut self, at: (f32, f32, f32), r: f32, ink: &str) {
        let (x, y) = project(at.0, at.1, at.2);
        self.reach((x - r, y - r));
        self.reach((x + r, y + r));
        self.body.push_str(&format!(
            r#"<circle cx="{}" cy="{}" r="{}" fill="{ink}"/>"#,
            n(x),
            n(y),
            n(r)
        ));
    }

    /// A box on the floor (or at `z`), centred on `(cx, cy)`: its three
    /// faces the eye sees — the top lightest, then the left, then the right.
    fn cuboid(&mut self, (cx, cy, z): (f32, f32, f32), (w, d, h): (f32, f32, f32)) {
        let (x0, x1, y0, y1, z1) = (
            cx - w / 2.0,
            cx + w / 2.0,
            cy - d / 2.0,
            cy + d / 2.0,
            z + h,
        );
        // THE TWO SIDES FIRST, then the top over their edges.
        self.face(&[(x0, y1, z), (x1, y1, z), (x1, y1, z1), (x0, y1, z1)], 0.1);
        self.face(
            &[(x1, y0, z), (x1, y1, z), (x1, y1, z1), (x1, y0, z1)],
            0.05,
        );
        self.face(
            &[(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)],
            0.16,
        );
    }

    /// A circle on a level plane at height `z`, as the points of its ellipse.
    fn ring(c: (f32, f32), r: f32, z: f32, steps: usize) -> Vec<(f32, f32)> {
        (0..steps)
            .map(|k| {
                #[allow(clippy::cast_precision_loss)]
                let a = std::f32::consts::TAU * k as f32 / steps as f32;
                project(c.0 + r * a.cos(), c.1 + r * a.sin(), z)
            })
            .collect()
    }

    /// A standing cylinder: its side, then its top.
    fn cylinder(&mut self, c: (f32, f32), r: f32, z: f32, h: f32) {
        let bottom = Self::ring(c, r, z, 48);
        let top = Self::ring(c, r, z + h, 48);
        // THE SIDE: the bottom's front half, from its left end to its right
        // one, then the top's front half back.
        let (_, middle) = project(c.0, c.1, z);
        let by_x = |a: &(f32, f32), b: &(f32, f32)| a.0.total_cmp(&b.0);
        let left = bottom
            .iter()
            .copied()
            .min_by(by_x)
            .unwrap_or(project(c.0, c.1, z));
        let right = bottom
            .iter()
            .copied()
            .max_by(by_x)
            .unwrap_or(project(c.0, c.1, z));
        let mut front: Vec<(f32, f32)> = bottom.iter().copied().filter(|p| p.1 > middle).collect();
        front.sort_by(by_x);
        let mut side = vec![left];
        side.extend(front);
        side.push(right);
        let mut up: Vec<(f32, f32)> = side.iter().rev().map(|&(x, y)| (x, y - h)).collect();
        side.append(&mut up);
        self.shape(&side, 0.08, INK);
        self.shape(&top, 0.16, INK);
    }

    /// A shape in a square of `[-10, 10]²` laid on a level plane at `z`,
    /// centred on `(cx, cy)` and `s` world units a side unit: an emblem
    /// printed on a top.
    fn on_top(c: (f32, f32), z: f32, s: f32, pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
        pts.iter()
            .map(|&(u, v)| project(c.0 + u * s, c.1 + v * s, z))
            .collect()
    }

    /// The SVG, framed with room for its lines.
    fn finish(self) -> Art {
        let pad = STROKE * 2.0;
        let (x0, y0) = (self.min.0 - pad, self.min.1 - pad);
        let (w, h) = (self.max.0 - x0 + pad, self.max.1 - y0 + pad);
        Art {
            svg: format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}">{}</svg>"#,
                n(w),
                n(h),
                n(x0),
                n(y0),
                n(w),
                n(h),
                self.body
            ),
            width: w,
            height: h,
            anchor: (-x0, -y0),
        }
    }
}

/// Points of a circle in the `[-10, 10]²` square of an emblem.
fn circle(c: (f32, f32), r: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|k| {
            #[allow(clippy::cast_precision_loss)]
            let a = std::f32::consts::TAU * k as f32 / steps as f32;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

/// An emblem, as lines in the `[-10, 10]²` square.
fn emblem_lines(e: Emblem) -> Vec<Vec<(f32, f32)>> {
    match e {
        Emblem::Globe => vec![
            circle((0.0, 0.0), 8.0, 32),
            (0..=32)
                .map(|k| {
                    #[allow(clippy::cast_precision_loss)]
                    let a = std::f32::consts::TAU * k as f32 / 32.0;
                    (3.5 * a.cos(), 8.0 * a.sin())
                })
                .collect(),
            vec![(-8.0, 0.0), (8.0, 0.0)],
            vec![(0.0, -8.0), (0.0, 8.0)],
        ],
        Emblem::Gear => {
            let mut teeth = Vec::new();
            for k in 0..8 {
                #[allow(clippy::cast_precision_loss)]
                let a = std::f32::consts::TAU * k as f32 / 8.0;
                teeth.push(vec![
                    (5.5 * a.cos(), 5.5 * a.sin()),
                    (8.5 * a.cos(), 8.5 * a.sin()),
                ]);
            }
            teeth.push(circle((0.0, 0.0), 5.5, 24));
            teeth.push(circle((0.0, 0.0), 2.2, 16));
            teeth
        }
        Emblem::Envelope => vec![
            vec![
                (-8.0, -5.5),
                (8.0, -5.5),
                (8.0, 5.5),
                (-8.0, 5.5),
                (-8.0, -5.5),
            ],
            vec![(-8.0, -5.5), (0.0, 1.0), (8.0, -5.5)],
        ],
        Emblem::Clock => vec![
            circle((0.0, 0.0), 8.0, 32),
            vec![(0.0, -5.5), (0.0, 0.0), (4.0, 2.0)],
        ],
        Emblem::Page => vec![
            vec![
                (-6.0, -8.0),
                (3.0, -8.0),
                (6.0, -5.0),
                (6.0, 8.0),
                (-6.0, 8.0),
                (-6.0, -8.0),
            ],
            vec![(-3.5, -3.0), (3.5, -3.0)],
            vec![(-3.5, 0.5), (3.5, 0.5)],
            vec![(-3.5, 4.0), (1.5, 4.0)],
        ],
        Emblem::Key => vec![
            circle((-4.0, 0.0), 3.6, 20),
            vec![(-0.4, 0.0), (8.5, 0.0)],
            vec![(5.0, 0.0), (5.0, 3.0)],
            vec![(7.5, 0.0), (7.5, 2.5)],
        ],
        Emblem::Arrows => vec![
            vec![(-8.0, -3.0), (7.0, -3.0)],
            vec![(4.0, -6.0), (7.5, -3.0), (4.0, 0.0)],
            vec![(8.0, 3.0), (-7.0, 3.0)],
            vec![(-4.0, 0.0), (-7.5, 3.0), (-4.0, 6.0)],
        ],
        Emblem::Shield => vec![
            vec![
                (0.0, -8.5),
                (7.0, -5.5),
                (6.0, 2.5),
                (0.0, 8.5),
                (-6.0, 2.5),
                (-7.0, -5.5),
                (0.0, -8.5),
            ],
            vec![(-3.0, 0.0), (-0.5, 3.0), (4.0, -3.0)],
        ],
        Emblem::Hub => {
            let mut spokes = vec![circle((0.0, 0.0), 2.5, 16)];
            for k in 0..6 {
                #[allow(clippy::cast_precision_loss)]
                let a = std::f32::consts::TAU * k as f32 / 6.0;
                spokes.push(vec![
                    (2.5 * a.cos(), 2.5 * a.sin()),
                    (7.0 * a.cos(), 7.0 * a.sin()),
                ]);
                spokes.push(circle((8.0 * a.cos(), 8.0 * a.sin()), 1.2, 10));
            }
            spokes
        }
        Emblem::Mirror => vec![
            vec![(0.0, -8.0), (0.0, 8.0)],
            vec![(-8.0, -4.0), (-2.0, 0.0), (-8.0, 4.0)],
            vec![(8.0, -4.0), (2.0, 0.0), (8.0, 4.0)],
        ],
        Emblem::Onion => vec![
            circle((0.0, 1.5), 7.0, 28),
            circle((0.0, 1.5), 4.6, 24),
            circle((0.0, 1.5), 2.2, 16),
            vec![(0.0, -5.5), (0.0, -9.0)],
        ],
        Emblem::List => vec![
            vec![(-7.0, -6.0), (7.0, -6.0)],
            vec![(-7.0, -2.0), (7.0, -2.0)],
            vec![(-7.0, 2.0), (7.0, 2.0)],
            vec![(-7.0, 6.0), (3.0, 6.0)],
        ],
    }
}

/// The drawing of a device.
#[must_use]
pub fn device(kind: Kind) -> Art {
    let mut p = Pen::new();
    match kind {
        Kind::Laptop => {
            // THE BASE, its keys and its pad.
            p.cuboid((0.0, 0.0, 0.0), (36.0, 26.0, 2.6));
            for row in 0..3 {
                for col in 0..6 {
                    #[allow(clippy::cast_precision_loss)]
                    let (x, y) = (-12.0 + row as f32 * 4.4, -10.5 + col as f32 * 3.6);
                    p.face(
                        &[
                            (x, y, 2.6),
                            (x + 3.4, y, 2.6),
                            (x + 3.4, y + 2.8, 2.6),
                            (x, y + 2.8, 2.6),
                        ],
                        0.10,
                    );
                }
            }
            p.face(
                &[
                    (4.0, -4.5, 2.6),
                    (12.0, -4.5, 2.6),
                    (12.0, 4.5, 2.6),
                    (4.0, 4.5, 2.6),
                ],
                0.06,
            );
            // THE SCREEN, standing at the back and leaning back a little,
            // and what is on it.
            let (x0, lean) = (-18.0, -4.0);
            p.face(
                &[
                    (x0, -13.0, 2.6),
                    (x0, 13.0, 2.6),
                    (x0 + lean, 13.0, 27.0),
                    (x0 + lean, -13.0, 27.0),
                ],
                0.16,
            );
            let inset = |t: f32, s: f32| (x0 + lean * t, s, 2.6 + 24.4 * t);
            let screen = [
                inset(0.12, -11.0),
                inset(0.12, 11.0),
                inset(0.9, 11.0),
                inset(0.9, -11.0),
            ];
            let flat: Vec<_> = screen.iter().map(|&(x, y, z)| project(x, y, z)).collect();
            p.solid(&flat, GLOW, 0.22);
            for (k, len) in [(0.7, 14.0), (0.55, 9.0), (0.4, 16.0), (0.25, 7.0)] {
                p.line(&[inset(k, -8.5), inset(k, -8.5 + len)], GLOW, 0.9);
            }
        }
        Kind::Phone => {
            p.cuboid((0.0, 0.0, 0.0), (5.0, 15.0, 28.0));
            let x = 2.5;
            let flat: Vec<_> = [
                (x, -6.0, 3.5),
                (x, 6.0, 3.5),
                (x, 6.0, 25.5),
                (x, -6.0, 25.5),
            ]
            .iter()
            .map(|&(a, b, c)| project(a, b, c))
            .collect();
            p.solid(&flat, GLOW, 0.24);
            p.dot((x, 0.0, 1.7), 0.8, GLOW);
        }
        Kind::Desktop => {
            p.cuboid((0.0, 0.0, 0.0), (12.0, 16.0, 1.8));
            p.line(&[(0.0, 0.0, 1.8), (0.0, 0.0, 9.0)], INK, 2.0);
            p.cuboid((0.0, 0.0, 9.0), (3.0, 36.0, 24.0));
            let x = 1.5;
            let flat: Vec<_> = [
                (x, -16.0, 11.0),
                (x, 16.0, 11.0),
                (x, 16.0, 31.0),
                (x, -16.0, 31.0),
            ]
            .iter()
            .map(|&(a, b, c)| project(a, b, c))
            .collect();
            p.solid(&flat, GLOW, 0.2);
        }
        Kind::Router => {
            p.cuboid((0.0, 0.0, 0.0), (32.0, 24.0, 9.0));
            for y in [-8.0, 8.0] {
                p.line(&[(-12.0, y, 9.0), (-14.0, y, 27.0)], INK, 1.4);
                p.dot((-14.0, y, 27.0), 1.5, INK);
            }
            // ITS LIGHTS on the front, and its ports on the side.
            for k in 0..4 {
                #[allow(clippy::cast_precision_loss)]
                p.dot((-9.0 + k as f32 * 5.0, 12.0, 5.0), 1.1, GLOW);
            }
            for k in 0..3 {
                #[allow(clippy::cast_precision_loss)]
                let y = -8.0 + k as f32 * 6.0;
                p.face(
                    &[
                        (16.0, y, 2.5),
                        (16.0, y + 3.5, 2.5),
                        (16.0, y + 3.5, 6.0),
                        (16.0, y, 6.0),
                    ],
                    0.2,
                );
            }
        }
        Kind::Switch => {
            p.cuboid((0.0, 0.0, 0.0), (44.0, 20.0, 7.0));
            for k in 0..8 {
                #[allow(clippy::cast_precision_loss)]
                let x = -19.0 + k as f32 * 5.0;
                p.face(
                    &[
                        (x, 10.0, 1.5),
                        (x + 3.4, 10.0, 1.5),
                        (x + 3.4, 10.0, 4.4),
                        (x, 10.0, 4.4),
                    ],
                    0.18,
                );
                p.dot((x + 1.7, 10.0, 5.8), 0.6, GLOW);
            }
        }
        Kind::AccessPoint => {
            p.cylinder((0.0, 0.0), 14.0, 0.0, 4.0);
            p.dot((0.0, 0.0, 4.0), 1.6, GLOW);
            let (cx, cy) = project(0.0, 0.0, 10.0);
            for (k, r) in [6.0_f32, 11.0, 16.0].iter().enumerate() {
                let arc: Vec<_> = (0..=12)
                    .map(|s| {
                        #[allow(clippy::cast_precision_loss)]
                        let a = std::f32::consts::PI * (1.2 + 0.6 * s as f32 / 12.0);
                        (cx + r * a.cos(), cy + r * 0.8 * a.sin())
                    })
                    .collect();
                #[allow(clippy::cast_precision_loss)]
                p.flat_line(&arc, GLOW, 1.2 - k as f32 * 0.2);
            }
        }
        Kind::Cloud => {
            // A CLOUD has no depth: its outline, and the globe in it.
            let base = -16.0;
            let lobes: [((f32, f32), f32); 5] = [
                ((-26.0, base + 2.0), 12.0),
                ((-11.0, base - 9.0), 15.0),
                ((7.0, base - 12.0), 16.0),
                ((24.0, base - 1.0), 12.0),
                ((0.0, base + 6.0), 14.0),
            ];
            let mut outline = Vec::new();
            for k in 0..=96 {
                #[allow(clippy::cast_precision_loss)]
                let a = std::f32::consts::TAU * k as f32 / 96.0;
                let (dx, dy) = (a.cos(), a.sin());
                // The farthest any lobe reaches in that direction, from the
                // cloud's middle.
                let mut far = 0.0_f32;
                for &((lx, ly), r) in &lobes {
                    let (ox, oy) = (lx, ly - base);
                    let b = ox * dx + oy * dy;
                    let c = ox * ox + oy * oy - r * r;
                    let disc = b * b - c;
                    if disc >= 0.0 {
                        far = far.max(b + disc.sqrt());
                    }
                }
                outline.push((dx * far, base + dy * far * 0.75));
            }
            p.shape(&outline, 0.08, INK);
            let g = (0.0, base);
            let globe: Vec<_> = circle((0.0, 0.0), 9.0, 32)
                .iter()
                .map(|&(x, y)| (g.0 + x, g.1 + y))
                .collect();
            p.flat_line(&globe, GLOW, 1.0);
            let meridian: Vec<_> = (0..=32)
                .map(|k| {
                    #[allow(clippy::cast_precision_loss)]
                    let a = std::f32::consts::TAU * k as f32 / 32.0;
                    (g.0 + 4.0 * a.cos(), g.1 + 9.0 * a.sin())
                })
                .collect();
            p.flat_line(&meridian, GLOW, 0.8);
            p.flat_line(&[(g.0 - 9.0, g.1), (g.0 + 9.0, g.1)], GLOW, 0.8);
        }
        Kind::CoreRouter => {
            p.cylinder((0.0, 0.0), 17.0, 0.0, 11.0);
            // FOUR ARROWS on its top, two in and two out: what a router of
            // routers does.
            let z = 11.0;
            let arrows: [[(f32, f32); 3]; 4] = [
                [(4.0, 0.0), (12.0, 0.0), (9.0, 3.0)],
                [(-4.0, 0.0), (-12.0, 0.0), (-9.0, -3.0)],
                [(0.0, 12.0), (0.0, 4.0), (3.0, 7.0)],
                [(0.0, -12.0), (0.0, -4.0), (-3.0, -7.0)],
            ];
            for a in arrows {
                let pts: Vec<_> = a.iter().map(|&(u, v)| project(u, v, z)).collect();
                p.flat_line(&pts[..2], GLOW, 1.3);
                p.flat_line(&[pts[1], pts[2]], GLOW, 1.3);
            }
        }
        Kind::Rack => {
            for k in 0..3 {
                #[allow(clippy::cast_precision_loss)]
                let z = k as f32 * 11.0;
                p.cuboid((0.0, 0.0, z), (28.0, 28.0, 9.0));
                p.line(&[(-10.0, 14.0, z + 4.5), (4.0, 14.0, z + 4.5)], INK, 0.8);
                p.dot((8.0, 14.0, z + 4.5), 1.0, GLOW);
                p.dot((11.0, 14.0, z + 4.5), 1.0, GLOW);
            }
        }
        Kind::Tower(emblem) => {
            p.cuboid((0.0, 0.0, 0.0), (26.0, 26.0, 42.0));
            // ITS SLOTS on the front, its vents on the side, its light.
            for k in 0..5 {
                #[allow(clippy::cast_precision_loss)]
                let z = 34.0 - k as f32 * 4.5;
                p.line(&[(-9.0, 13.0, z), (7.0, 13.0, z)], INK, 0.8);
            }
            p.dot((9.5, 13.0, 34.0), 1.1, GLOW);
            for row in 0..3 {
                for col in 0..4 {
                    #[allow(clippy::cast_precision_loss)]
                    p.dot(
                        (13.0, -8.0 + col as f32 * 5.0, 10.0 + row as f32 * 4.0),
                        0.6,
                        INK,
                    );
                }
            }
            for l in emblem_lines(emblem) {
                let pts = Pen::on_top((0.0, 0.0), 42.0, 0.9, &l);
                p.flat_line(&pts, GLOW, 1.0);
            }
        }
        Kind::Storage => {
            p.cuboid((0.0, 0.0, 0.0), (38.0, 28.0, 20.0));
            for k in 0..4 {
                #[allow(clippy::cast_precision_loss)]
                let x = -16.0 + k as f32 * 8.5;
                p.face(
                    &[
                        (x, 14.0, 3.0),
                        (x + 7.0, 14.0, 3.0),
                        (x + 7.0, 14.0, 16.0),
                        (x, 14.0, 16.0),
                    ],
                    0.12,
                );
                p.dot((x + 3.5, 14.0, 5.5), 0.8, GLOW);
            }
            let disk = Pen::on_top((0.0, 0.0), 20.0, 1.0, &circle((0.0, 0.0), 8.0, 32));
            p.flat_line(&disk, GLOW, 1.0);
            let hub = Pen::on_top((0.0, 0.0), 20.0, 1.0, &circle((0.0, 0.0), 2.0, 16));
            p.flat_line(&hub, GLOW, 1.0);
        }
        Kind::Printer => {
            p.cuboid((0.0, 0.0, 0.0), (30.0, 26.0, 13.0));
            // THE SHEET, coming up at the back.
            p.face(
                &[
                    (-11.0, -9.0, 13.0),
                    (-11.0, 9.0, 13.0),
                    (-17.0, 9.0, 26.0),
                    (-17.0, -9.0, 26.0),
                ],
                0.1,
            );
            for k in 0..3 {
                #[allow(clippy::cast_precision_loss)]
                let t = 0.3 + k as f32 * 0.2;
                p.line(
                    &[
                        (-11.0 - 6.0 * t, -6.0, 13.0 + 13.0 * t),
                        (-11.0 - 6.0 * t, 5.0, 13.0 + 13.0 * t),
                    ],
                    GLOW,
                    0.8,
                );
            }
            p.face(
                &[
                    (15.0, -9.0, 3.0),
                    (21.0, -9.0, 3.0),
                    (21.0, 9.0, 3.0),
                    (15.0, 9.0, 3.0),
                ],
                0.1,
            );
            p.dot((10.0, 13.0, 9.5), 1.0, GLOW);
        }
        Kind::Controller => {
            p.cuboid((0.0, 0.0, 0.0), (40.0, 14.0, 24.0));
            // ITS MODULES side by side on the rail, each with its lights.
            for k in 0..5 {
                #[allow(clippy::cast_precision_loss)]
                let x = -20.0 + k as f32 * 8.0;
                if k > 0 {
                    p.line(&[(x, 7.0, 0.0), (x, 7.0, 24.0)], INK, 0.8);
                }
                p.dot((x + 4.0, 7.0, 19.0), 0.9, GLOW);
                p.dot((x + 4.0, 7.0, 15.5), 0.9, GLOW);
                p.line(&[(x + 1.5, 7.0, 5.0), (x + 6.5, 7.0, 5.0)], INK, 0.8);
            }
        }
    }
    p.finish()
}

/// The drawing of a sign, centred on its own middle (its anchor).
#[must_use]
pub fn prop(prop: Prop) -> Art {
    let mut p = Pen::new();
    // SIGNS ARE FLAT, drawn facing the eye in a square of about 24 units.
    match prop {
        Prop::Lock => {
            p.shape(
                &[(-8.0, -1.0), (8.0, -1.0), (8.0, 11.0), (-8.0, 11.0)],
                0.2,
                GLOW,
            );
            let shackle: Vec<_> = (0..=16)
                .map(|k| {
                    #[allow(clippy::cast_precision_loss)]
                    let a = std::f32::consts::PI * (1.0 + k as f32 / 16.0);
                    (5.0 * a.cos(), -1.0 + 7.0 * a.sin())
                })
                .collect();
            p.flat_line(&shackle, GLOW, 1.4);
            p.dot((0.0, 0.0, -5.0), 1.4, GLOW);
        }
        Prop::Key => {
            for l in emblem_lines(Emblem::Key) {
                let pts: Vec<_> = l.iter().map(|&(x, y)| (x * 1.2, y * 1.2)).collect();
                p.flat_line(&pts, GLOW, 1.4);
            }
        }
        Prop::Envelope => {
            p.shape(
                &[(-11.0, -7.5), (11.0, -7.5), (11.0, 7.5), (-11.0, 7.5)],
                0.2,
                GLOW,
            );
            p.flat_line(&[(-11.0, -7.5), (0.0, 1.5), (11.0, -7.5)], GLOW, 1.2);
        }
        Prop::Certificate => {
            p.shape(
                &[(-9.0, -11.0), (9.0, -11.0), (9.0, 11.0), (-9.0, 11.0)],
                0.16,
                GLOW,
            );
            for (k, len) in [(-6.0, 12.0), (-2.5, 12.0), (1.0, 8.0)] {
                p.flat_line(&[(-6.0, k), (-6.0 + len, k)], GLOW, 0.9);
            }
            let seal: Vec<_> = circle((4.0, 6.5), 3.2, 20);
            p.shape(&seal, 0.5, GLOW);
            p.flat_line(&[(2.5, 9.0), (1.5, 14.0)], GLOW, 1.0);
            p.flat_line(&[(5.5, 9.0), (6.5, 14.0)], GLOW, 1.0);
        }
        Prop::Clock => {
            p.shape(&circle((0.0, 0.0), 10.0, 40), 0.16, GLOW);
            p.flat_line(&[(0.0, -6.5), (0.0, 0.0), (4.5, 2.5)], GLOW, 1.4);
        }
        Prop::Page => {
            p.shape(
                &[(-10.0, -13.0), (10.0, -13.0), (10.0, 13.0), (-10.0, 13.0)],
                0.14,
                GLOW,
            );
            p.solid(
                &[(-10.0, -13.0), (10.0, -13.0), (10.0, -9.0), (-10.0, -9.0)],
                GLOW,
                0.5,
            );
            p.shape(
                &[(-7.0, -6.0), (7.0, -6.0), (7.0, 1.0), (-7.0, 1.0)],
                0.3,
                GLOW,
            );
            for y in [4.5, 7.5, 10.5] {
                p.flat_line(&[(-7.0, y), (y - 1.0, y)], GLOW, 0.9);
            }
        }
        Prop::Onion => {
            for l in emblem_lines(Emblem::Onion) {
                let pts: Vec<_> = l.iter().map(|&(x, y)| (x * 1.3, y * 1.3)).collect();
                p.flat_line(&pts, GLOW, 1.3);
            }
        }
        Prop::Shield => {
            for l in emblem_lines(Emblem::Shield) {
                let pts: Vec<_> = l.iter().map(|&(x, y)| (x * 1.3, y * 1.3)).collect();
                p.flat_line(&pts, GLOW, 1.4);
            }
        }
        Prop::Check => {
            p.shape(&circle((0.0, 0.0), 10.0, 40), 0.18, GLOW);
            p.flat_line(&[(-5.0, 0.5), (-1.5, 4.5), (5.5, -4.0)], GLOW, 1.8);
        }
        Prop::Cross => {
            p.shape(&circle((0.0, 0.0), 10.0, 40), 0.18, GLOW);
            p.flat_line(&[(-4.5, -4.5), (4.5, 4.5)], GLOW, 1.8);
            p.flat_line(&[(4.5, -4.5), (-4.5, 4.5)], GLOW, 1.8);
        }
    }
    p.finish()
}

/// A ring on the floor around a device, `r` world units out: the pulse a
/// device gives as a packet reaches it.
#[must_use]
pub fn pulse(r: f32) -> Art {
    let mut p = Pen::new();
    let ring = Pen::ring((0.0, 0.0), r, 0.0, 64);
    let mut closed = ring.clone();
    closed.push(ring[0]);
    p.flat_line(&closed, GLOW, 1.6);
    p.finish()
}

/// A rounded box `w` by `h` world units, centred: the ground of a packet's
/// label, lined in [`GLOW`] (its class's colour) and filled with [`FILL`],
/// with a dot of its colour at its start.
#[must_use]
pub fn pill(w: f32, h: f32) -> Art {
    let mut p = Pen::new();
    let r = h / 2.0;
    let mut outline = Vec::new();
    for k in 0..=12 {
        #[allow(clippy::cast_precision_loss)]
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
        outline.push((-w / 2.0 + r + r * a.cos(), r * a.sin()));
    }
    for k in 0..=12 {
        #[allow(clippy::cast_precision_loss)]
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
        outline.push((w / 2.0 - r + r * a.cos(), r * a.sin()));
    }
    let d = p.points(&outline);
    p.body.push_str(&format!(
        r#"<path d="{d} Z" fill="{FILL}" stroke="{GLOW}" stroke-width="{}" stroke-linejoin="round"/>"#,
        n(STROKE)
    ));
    let dot = (-w / 2.0 + r, 0.0);
    p.body.push_str(&format!(
        r#"<circle cx="{}" cy="{}" r="{}" fill="{GLOW}"/>"#,
        n(dot.0),
        n(dot.1),
        n(h * 0.17)
    ));
    p.finish()
}

/// A drawing's SVG with its placeholders swapped for colours (`#rrggbb`), and
/// the fill's opacity scaled by `fill_alpha` — the window's, from the theme's
/// roles.
#[must_use]
pub fn resolved(svg: &str, ink: [u8; 4], fill: [u8; 4], glow: [u8; 4]) -> String {
    let hex = |c: [u8; 4]| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
    svg.replace(INK, &hex(ink))
        .replace(FILL, &hex(fill))
        .replace(GLOW, &hex(glow))
}

/// Pixels, RGBA and straight — what the renderer takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    /// Width, in pixels.
    pub width: u32,
    /// Height, in pixels.
    pub height: u32,
    /// The pixels, row by row.
    pub pixels: Vec<u8>,
}

/// A drawing's SVG (its inks resolved) rasterised at `scale` pixels a world
/// unit; `None` for an SVG that does not read or a size of nothing.
#[must_use]
pub fn raster(svg: &str, scale: f32) -> Option<Raster> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &options).ok()?;
    let size = tree.size();
    if scale <= 0.0 || size.width() <= 0.0 || size.height() <= 0.0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = (
        (size.width() * scale).ceil().max(1.0) as u32,
        (size.height() * scale).ceil().max(1.0) as u32,
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(Raster {
        width,
        height,
        // STRAIGHT, as the renderer takes it: tiny-skia's is premultiplied.
        pixels: pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let d = p.demultiply();
                [d.red(), d.green(), d.blue(), d.alpha()]
            })
            .collect(),
    })
}
