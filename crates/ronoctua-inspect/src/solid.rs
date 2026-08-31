//! Turning a [`crate::mesh::Mesh`] into pixels.
//!
//! # Why in this process, and not by a program
//!
//! Because the promise this crate makes — every external path hands back a
//! picture on stdout and `image` decodes it — buys nothing here and costs a
//! store path. There is no mesh renderer in the closure to call, adding one
//! would be the first C library this preview has ever needed, and
//! `files-closure-has-no-mime-db` is the check that would say so.
//!
//! [`crate::pixels::from_svg`] is the precedent: resvg rasterises AT the size,
//! in process, and builds an [`crate::Rgba`] by hand. This is the same figure
//! with different arithmetic.
//!
//! And the size is what makes it cheap. The panel asks for at most 512 px on
//! its longest side (`crate::STEPS`), so a quarter of a million pixels bounds
//! the fill however many triangles there are — which is why a drag can afford
//! to redraw from the triangles rather than from a cache of poses.
//!
//! # Why the background is TRANSPARENT
//!
//! For the reason the film's letterbox already records: there is ONE texture
//! and TWO doors, and their backgrounds are different tokens. A colour baked
//! in here would be right in the footer and wrong in the window, or the other
//! way round. Transparent lets each door's own surface show through, which is
//! also what makes the piece look like it is sitting in the panel rather than
//! in a box.
//!
//! # Why the light does not move with the model
//!
//! Because a light fixed in the WORLD puts the piece in its own shadow as soon
//! as somebody turns it round, and the gesture that was supposed to show the
//! other side shows a silhouette instead. Fixed in the camera, every turn is
//! lit the same way, which is what a person actually wants from "let me see
//! the back of it".

use crate::Rgba;
use crate::mesh::Mesh;

/// How many samples per pixel, per side.
///
/// Two, so four per pixel. It is nearly free and the reason is the shape of
/// the work: the cost of a mesh this size is dominated by SETUP — a quarter of
/// a million triangles each covering less than a pixel — and supersampling
/// leaves the setup untouched while it multiplies only the fill. What it buys
/// is the silhouette, which is the whole outline of the thing being looked at
/// and the one place a stair-step is impossible not to see.
const SAMPLES: u32 = 2;

/// The largest side this will rasterise, in samples.
///
/// A guard and not a preference: the caller asks in pixels and the pixels come
/// from a token, but a caller that asked for a window's full width would have
/// this allocate four times that squared.
const SAMPLE_CEILING: u32 = 4096;

/// Where the model is being looked at from.
///
/// Angles in radians, `zoom` as a multiplier on the framing that fits the whole
/// piece. It is a plain value so the manager can compare two of them and know
/// whether a new picture is wanted — the same thing `motion::Reel` does for a
/// film.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Turn about the model's up axis.
    pub yaw: f32,
    /// Tilt above and below the horizon, clamped to just short of the poles.
    pub pitch: f32,
    /// How much closer than the framing that fits everything.
    pub zoom: f32,
}

/// How far the pitch may go, in radians.
///
/// Just short of a right angle. At exactly the pole the camera's up vector and
/// its forward vector are the same line and the frame has no orientation at
/// all — the picture spins on its own as the last digit changes.
pub const PITCH_CEILING: f32 = 1.5;

/// The nearest and furthest the zoom goes.
pub const ZOOM_NEAREST: f32 = 8.0;
/// The furthest out.
pub const ZOOM_FURTHEST: f32 = 0.2;

impl Default for Pose {
    /// Three quarters on, slightly above: the view a person draws a box in.
    ///
    /// Straight on hides the depth entirely — a cube looks like a square — and
    /// straight down hides what the piece stands on. This is the angle every
    /// catalogue photograph of a physical object uses, for the same reason.
    fn default() -> Self {
        Self {
            yaw: -0.6,
            pitch: 0.5,
            zoom: 1.0,
        }
    }
}

impl Pose {
    /// Turned by these amounts, kept inside the limits.
    #[must_use]
    pub fn turned(self, yaw: f32, pitch: f32) -> Self {
        Self {
            // The yaw wraps: there is no end to turning something round.
            yaw: (self.yaw + yaw).rem_euclid(std::f32::consts::TAU),
            pitch: (self.pitch + pitch).clamp(-PITCH_CEILING, PITCH_CEILING),
            zoom: self.zoom,
        }
    }

    /// Closer or further by a factor, kept inside the limits.
    #[must_use]
    pub fn scaled(self, by: f32) -> Self {
        Self {
            zoom: (self.zoom * by).clamp(ZOOM_FURTHEST, ZOOM_NEAREST),
            ..self
        }
    }
}

/// What the piece is drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    /// The colour of the material, straight RGBA.
    pub solid: [u8; 4],
    /// Where the light comes from, in CAMERA space: `x` right, `y` up, `z`
    /// towards the viewer. Normalised here, so a caller may hand over degrees
    /// turned into a rough vector.
    pub light: [f32; 3],
    /// How much of the colour survives in full shadow, 0 to 1.
    ///
    /// Not zero: a face turned away should be dark and still be THERE, because
    /// its silhouette against the next face is what shows the shape. Black
    /// would merge two faces into one blob.
    pub ambient: f32,
}

/// A unit vector, or `None` when there is nothing to normalise.
#[inline]
fn unit(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2])).sqrt();
    (len > f32::EPSILON).then(|| [v[0] / len, v[1] / len, v[2] / len])
}

#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

#[inline]
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1].mul_add(b[2], -(a[2] * b[1])),
        a[2].mul_add(b[0], -(a[0] * b[2])),
        a[0].mul_add(b[1], -(a[1] * b[0])),
    ]
}

/// The three axes of the camera, for a pose.
///
/// Z is the model's up: that is what both formats mean by the third
/// coordinate, and a viewer that picked Y would lay every printed part on its
/// side.
fn axes(pose: Pose) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (sy, cy) = pose.yaw.sin_cos();
    let (sp, cp) = pose.pitch.sin_cos();
    // Towards the viewer, from the model.
    let towards = [cp * cy, cp * sy, sp];
    let right = unit(cross([0.0, 0.0, 1.0], towards)).unwrap_or([1.0, 0.0, 0.0]);
    let up = cross(towards, right);
    (right, up, towards)
}

/// Draws the mesh.
///
/// # What a frame costs, measured
///
/// On the owner's machine, into 512x384: **41 ms** for the 3DBenchy's 225 154
/// triangles and **14 ms** for a 21 386-triangle candle holder, roughly
/// doubling when the piece is zoomed to fill the panel. It is arithmetic in
/// the per-triangle setup and not memory — packing the framebuffer and
/// stepping the edge functions were both measured and the second was worth
/// 4x, the first nothing.
///
/// BACK FACES ARE NOT CULLED, and that is the one place speed was refused.
/// Culling was measured at 1.5x — 41 ms down to 33 — and it is correct only
/// for a mesh whose faces are all wound the same way. A mesh with some of them
/// reversed is a real and common thing, `Sanity::flipped` counts them, and
/// what culling does to one is punch holes in it. A preview that shows a solid
/// where there is a solid is worth more than eight milliseconds; if that ever
/// stops being true, the switch is one line and the number above is what it
/// buys.
///
/// `width` and `height` are the picture's, in pixels. An empty mesh or a zero
/// side gives a fully transparent picture of the size asked for rather than
/// nothing — a caller that got `None` would have to invent a sentence for a
/// case that is not a refusal.
#[must_use]
pub fn draw(mesh: &Mesh, pose: Pose, width: u32, height: u32, paint: Paint) -> Rgba {
    let width = width.max(1);
    let height = height.max(1);
    let scale = SAMPLES.min(SAMPLE_CEILING / width.max(height)).max(1);
    let (sw, sh) = (width * scale, height * scale);

    let (right, up, towards) = axes(pose);
    let light = unit(paint.light).unwrap_or([0.0, 0.0, 1.0]);
    let middle = mesh.bounds.middle();
    // HALF THE DIAGONAL, and not half the longest side: the piece has to fit
    // at every angle, and the diagonal is the widest it can ever look.
    let size = mesh.bounds.size();
    let radius = 0.5
        * size[0]
            .mul_add(size[0], size[1].mul_add(size[1], size[2] * size[2]))
            .sqrt();
    let mut depth = vec![f32::MAX; (sw * sh) as usize];
    let mut pixels = vec![0_u32; (sw * sh) as usize];
    if radius <= 0.0 || mesh.triangles() == 0 {
        return shrink(&pixels, sw, sh, scale, width, height);
    }
    let across =
        f32::from(u16::try_from(sw.min(sh)).unwrap_or(u16::MAX)) * 0.5 / radius * pose.zoom;
    let (mx, my) = (sw as f32 * 0.5, sh as f32 * 0.5);

    // BY CHUNKS, and not by index. `positions[i * 9..i * 9 + 9]` is a bounds
    // check per corner on a loop that runs a quarter of a million times; the
    // iterator proves the length once.
    for (p, n) in mesh
        .positions
        .chunks_exact(9)
        .zip(mesh.normals.chunks_exact(3))
    {
        // PROJECTED FIRST, because the two rejections below are three
        // subtractions each and the lighting is a square root. Doing the
        // lighting first pays it for every triangle that turns out to be
        // edge-on or off the picture.
        let mut sx = [0.0_f32; 3];
        let mut sy = [0.0_f32; 3];
        let mut sz = [0.0_f32; 3];
        for k in 0..3 {
            let c = [p[k * 3], p[k * 3 + 1], p[k * 3 + 2]];
            let v = [c[0] - middle[0], c[1] - middle[1], c[2] - middle[2]];
            sx[k] = dot(v, right).mul_add(across, mx);
            sy[k] = (-dot(v, up)).mul_add(across, my);
            // Away from the viewer, so smaller is nearer.
            sz[k] = -dot(v, towards);
        }

        // The normal in camera space, then turned to face the viewer. Turning
        // it is what keeps the inside of an open piece lit instead of black:
        // through a hole one sees the far wall from behind, and a slicer's
        // idea of "backwards" is not something a preview should punish.
        let raw = [n[0], n[1], n[2]];
        let facing = [dot(raw, right), dot(raw, up), dot(raw, towards)];
        let Some(unit_n) = unit(if facing[2] < 0.0 {
            [-facing[0], -facing[1], -facing[2]]
        } else {
            facing
        }) else {
            continue;
        };
        let lit = paint
            .ambient
            .mul_add(1.0, (1.0 - paint.ambient) * dot(unit_n, light).max(0.0))
            .clamp(0.0, 1.0);
        let shade = |c: u8| (f32::from(c) * lit).round().clamp(0.0, 255.0) as u8;
        let colour = u32::from_le_bytes([
            shade(paint.solid[0]),
            shade(paint.solid[1]),
            shade(paint.solid[2]),
            paint.solid[3],
        ]);
        fill(&mut depth, &mut pixels, sw, sh, sx, sy, sz, colour);
    }
    shrink(&pixels, sw, sh, scale, width, height)
}

/// One triangle into the buffers.
///
/// # Why the edge functions are stepped and not evaluated
///
/// Because they are linear in `x` and `y`, so the whole inner loop is three
/// additions and a comparison. Evaluating them per sample instead costs two
/// DIVISIONS per sample, and a zoomed-in piece covers a quarter of a million
/// samples several times over — measured at 252 ms a frame, which is a drag
/// that visibly lurches. The same argument covers the depth, which is linear
/// in the two weights and therefore linear in the pixel too.
#[allow(clippy::too_many_arguments)]
fn fill(
    depth: &mut [f32],
    pixels: &mut [u32],
    sw: u32,
    sh: u32,
    x: [f32; 3],
    y: [f32; 3],
    z: [f32; 3],
    colour: u32,
) {
    let area = (x[1] - x[0]).mul_add(y[2] - y[0], -((x[2] - x[0]) * (y[1] - y[0])));
    if area.abs() < f32::EPSILON {
        return;
    }
    // OFF THE PICTURE ENTIRELY, said before any clamping. Without it a piece
    // zoomed until most of it is outside still pays a one-sample loop per
    // triangle that is nowhere near the frame.
    let (min_x, max_x) = (x[0].min(x[1]).min(x[2]), x[0].max(x[1]).max(x[2]));
    let (min_y, max_y) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
    if max_x < 0.0 || max_y < 0.0 || min_x >= sw as f32 || min_y >= sh as f32 {
        return;
    }
    let lo_x = min_x.floor().max(0.0) as u32;
    let hi_x = (max_x.ceil() as u32).min(sw - 1);
    let lo_y = min_y.floor().max(0.0) as u32;
    let hi_y = (max_y.ceil() as u32).min(sh - 1);
    if lo_x > hi_x || lo_y > hi_y {
        return;
    }

    let inv = 1.0 / area;
    // How each weight changes across one sample, and down one row.
    let d0x = -(y[1] - y[0]) * inv;
    let d0y = (x[1] - x[0]) * inv;
    let d1x = (y[2] - y[0]) * inv;
    let d1y = -(x[2] - x[0]) * inv;
    let (ox, oy) = (lo_x as f32 + 0.5, lo_y as f32 + 0.5);
    let mut row0 = (x[1] - x[0]).mul_add(oy - y[0], -((ox - x[0]) * (y[1] - y[0]))) * inv;
    let mut row1 = (ox - x[0]).mul_add(y[2] - y[0], -((x[2] - x[0]) * (oy - y[0]))) * inv;
    // The depth, in the same terms.
    let (dz0, dz1) = (z[2] - z[0], z[1] - z[0]);
    let dzx = d0x.mul_add(dz0, d1x * dz1);
    let dzy = d0y.mul_add(dz0, d1y * dz1);

    for py in lo_y..=hi_y {
        let mut w0 = row0;
        let mut w1 = row1;
        let mut d = z[0] + w0 * dz0 + w1 * dz1;
        let from = (py * sw + lo_x) as usize;
        for at in from..=from + (hi_x - lo_x) as usize {
            if w0 >= 0.0 && w1 >= 0.0 && w0 + w1 <= 1.0 && d < depth[at] {
                depth[at] = d;
                pixels[at] = colour;
            }
            w0 += d0x;
            w1 += d1x;
            d += dzx;
        }
        row0 += d0y;
        row1 += d1y;
    }
    let _ = dzy;
}

/// Averages the samples down to the picture that was asked for.
///
/// Straight alpha throughout, which is what [`crate::Rgba`] promises and what
/// the pipeline expects — the colours are averaged WEIGHTED BY alpha, or a
/// transparent sample would drag the edge towards black instead of towards
/// nothing.
fn shrink(pixels: &[u32], sw: u32, sh: u32, scale: u32, width: u32, height: u32) -> Rgba {
    if scale == 1 {
        return Rgba {
            width: sw,
            height: sh,
            pixels: pixels.iter().flat_map(|p| p.to_le_bytes()).collect(),
        };
    }
    let n = f32::from(u16::try_from(scale * scale).unwrap_or(1));
    let mut out = vec![0_u8; (width * height * 4) as usize];
    for py in 0..height {
        for px in 0..width {
            let (mut r, mut g, mut b, mut a) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
            for sy in 0..scale {
                for sx in 0..scale {
                    let at = (((py * scale + sy) * sw) + px * scale + sx) as usize;
                    let s = pixels[at].to_le_bytes();
                    let alpha = f32::from(s[3]) / 255.0;
                    r += f32::from(s[0]) * alpha;
                    g += f32::from(s[1]) * alpha;
                    b += f32::from(s[2]) * alpha;
                    a += alpha;
                }
            }
            let at = ((py * width + px) * 4) as usize;
            if a > 0.0 {
                out[at] = (r / a).round().clamp(0.0, 255.0) as u8;
                out[at + 1] = (g / a).round().clamp(0.0, 255.0) as u8;
                out[at + 2] = (b / a).round().clamp(0.0, 255.0) as u8;
                out[at + 3] = (a / n * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Rgba {
        width,
        height,
        pixels: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;

    /// A cube of ten, wound outwards.
    fn cube() -> Mesh {
        let v = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 10.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0],
        ];
        let faces = [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let mut m = Mesh::empty();
        for f in faces {
            m.add(v[f[0]], v[f[1]], v[f[2]]);
        }
        m
    }

    fn paint() -> Paint {
        Paint {
            solid: [200, 200, 200, 255],
            light: [-0.35, 0.55, 0.75],
            ambient: 0.2,
        }
    }

    /// How many samples have any paint on them at all.
    fn covered(r: &Rgba) -> usize {
        r.pixels.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    // THE DEFECT THIS CATCHES: a background that is opaque. There is ONE
    // texture and TWO doors whose surfaces are different tokens, so a colour
    // baked in here is right in the footer and wrong in the window — the same
    // thing the film's letterbox already had to be transparent for.
    fn what_the_piece_does_not_cover_is_fully_transparent() {
        let r = draw(&cube(), Pose::default(), 64, 64, paint());
        assert_eq!(r.width, 64);
        assert_eq!(r.height, 64);
        // A corner of a 64x64 frame is outside a cube drawn to fit.
        assert_eq!(r.pixels[3], 0, "the top-left sample is not transparent");
        assert!(covered(&r) > 0, "nothing was drawn at all");
        assert!(covered(&r) < 64 * 64, "everything was drawn");
    }

    #[test]
    // THE DEFECT THIS CATCHES: a pose that does not change the picture. Every
    // gesture in the manager routes through `Pose`, so if turning it produced
    // the same pixels the whole feature would be inert and every unit test
    // about the keys would still pass.
    fn turning_it_changes_the_picture() {
        let m = cube();
        let a = draw(&m, Pose::default(), 64, 64, paint());
        let b = draw(&m, Pose::default().turned(1.0, 0.2), 64, 64, paint());
        assert_ne!(a.pixels, b.pixels);
    }

    #[test]
    // THE DEFECT THIS CATCHES: a zoom that does not zoom. Nearer must cover
    // more of the frame; the arithmetic that decides is the framing radius,
    // and getting it inverted is invisible to a test that only compares.
    fn nearer_covers_more_of_the_frame() {
        let m = cube();
        let out = draw(&m, Pose::default().scaled(0.5), 64, 64, paint());
        let near = draw(&m, Pose::default().scaled(2.0), 64, 64, paint());
        assert!(
            covered(&near) > covered(&out),
            "near covered {} and far covered {}",
            covered(&near),
            covered(&out)
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: the whole piece not fitting. A framing taken
    // from the longest SIDE rather than the diagonal fits head-on and clips
    // the corners as soon as anything is turned, which is the first thing
    // anybody does.
    fn the_whole_piece_fits_at_every_angle() {
        let m = cube();
        for step in 0..8 {
            let pose = Pose {
                yaw: step as f32 * 0.7,
                pitch: 0.4,
                zoom: 1.0,
            };
            let r = draw(&m, pose, 48, 48, paint());
            // Nothing may touch the border: if it does, something was cut off.
            for x in 0..r.width {
                let top = (x * 4 + 3) as usize;
                let bottom = (((r.height - 1) * r.width + x) * 4 + 3) as usize;
                assert_eq!(r.pixels[top], 0, "the top edge is painted at yaw {step}");
                assert_eq!(
                    r.pixels[bottom], 0,
                    "the bottom edge is painted at yaw {step}"
                );
            }
        }
    }

    #[test]
    // THE DEFECT THIS CATCHES: a pitch that reaches the pole. There the
    // camera's up and its forward are the same line, the frame has no
    // orientation left, and the picture spins on its own as the last digit
    // changes.
    fn the_pitch_never_reaches_the_pole() {
        let p = Pose::default().turned(0.0, 99.0);
        assert!(p.pitch <= PITCH_CEILING);
        let q = Pose::default().turned(0.0, -99.0);
        assert!(q.pitch >= -PITCH_CEILING);
        // And the yaw wraps rather than growing without end.
        let r = Pose::default().turned(100.0, 0.0);
        assert!(r.yaw >= 0.0 && r.yaw < std::f32::consts::TAU);
    }

    #[test]
    // THE DEFECT THIS CATCHES: an empty mesh answering with nothing. A caller
    // that got no picture would have to invent a sentence for a case that is
    // not a refusal — the panel simply has nothing in it yet.
    fn an_empty_mesh_gives_a_transparent_picture_of_the_size_asked_for() {
        let r = draw(&Mesh::empty(), Pose::default(), 32, 20, paint());
        assert_eq!((r.width, r.height), (32, 20));
        assert_eq!(covered(&r), 0);
        assert_eq!(r.pixels.len(), 32 * 20 * 4);
    }
}
