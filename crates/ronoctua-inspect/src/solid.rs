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
//! # Why a wire cage is drawn by the same function
//!
//! Because a STEP file has no surface to shade — see [`crate::mesh::Wires`] —
//! and everything else about the picture is the same: the same camera, the
//! same depth buffer, the same transparent ground, the same supersampling.
//! Splitting them would be two projections that have to agree, and the day
//! they disagreed the two kinds of file would frame differently with nothing
//! saying why. So there is one [`Lens`] and two inner loops.
//!
//! # Why the light does not move with the model
//!
//! Because a light fixed in the WORLD puts the piece in its own shadow as soon
//! as somebody turns it round, and the gesture that was supposed to show the
//! other side shows a silhouette instead. Fixed in the camera, every turn is
//! lit the same way, which is what a person actually wants from "let me see
//! the back of it".

use crate::Rgba;
use crate::mesh::{Body, Bounds, Mesh, Wires};

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
    /// How wide a wire is drawn, in PIXELS of the finished picture.
    ///
    /// Only the traced half of a body uses it. It has to be more than one: at two
    /// samples per pixel a single-sample line is half a pixel on screen, which
    /// after the average is a grey ghost rather than an edge.
    pub wire: f32,
    /// How much of the colour is left on the furthest wire, 0 to 1.
    ///
    /// A cage drawn in one flat colour reads as a tangle: every edge is
    /// present and nothing says which is in front. Fading with depth is the
    /// only cue available, because there are no faces to hide anything behind
    /// — the front and the back of the piece are both drawn, always.
    pub fade: f32,
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

/// Where the camera is, and how a point in the model lands in the picture.
///
/// It exists so the two bodies cannot frame differently: the triangles and the
/// wires ask the same object the same question, and a change to the framing is
/// a change in one place.
struct Lens {
    middle: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    towards: [f32; 3],
    /// Model units to samples.
    across: f32,
    /// The middle of the picture, in samples.
    mx: f32,
    my: f32,
    /// Half the model's diagonal, which is how far a point can be from the
    /// middle along any axis and therefore what the depth is measured against.
    radius: f32,
}

impl Lens {
    /// `None` when there is nothing to look at.
    fn of(bounds: Bounds, pose: Pose, sw: u32, sh: u32) -> Option<Self> {
        let (right, up, towards) = axes(pose);
        // HALF THE DIAGONAL, and not half the longest side: the piece has to
        // fit at every angle, and the diagonal is the widest it can ever look.
        let size = bounds.size();
        let radius = 0.5
            * size[0]
                .mul_add(size[0], size[1].mul_add(size[1], size[2] * size[2]))
                .sqrt();
        if radius <= 0.0 {
            return None;
        }
        Some(Self {
            middle: bounds.middle(),
            right,
            up,
            towards,
            across: f32::from(u16::try_from(sw.min(sh)).unwrap_or(u16::MAX)) * 0.5 / radius
                * pose.zoom,
            mx: sw as f32 * 0.5,
            my: sh as f32 * 0.5,
            radius,
        })
    }

    /// A point as a sample position and a depth, smaller being nearer.
    #[inline]
    fn project(&self, c: [f32; 3]) -> (f32, f32, f32) {
        let v = [
            c[0] - self.middle[0],
            c[1] - self.middle[1],
            c[2] - self.middle[2],
        ];
        (
            dot(v, self.right).mul_add(self.across, self.mx),
            (-dot(v, self.up)).mul_add(self.across, self.my),
            -dot(v, self.towards),
        )
    }
}

/// Draws the piece.
///
/// # What a frame costs, measured
///
/// On the owner's machine, into 512x384: **44 ms** for the 3DBenchy's 225 154
/// triangles, **17 ms** for a 121 724-triangle cutter, **13 ms** for a 21 386-
/// triangle candle holder and **9 ms** for a 49 999-triangle OBJ, roughly
/// doubling when the piece is zoomed to fill the panel. It is arithmetic in
/// the per-triangle setup and not memory — packing the framebuffer and
/// stepping the edge functions were both measured and the second was worth
/// 4x, the first nothing.
///
/// A CAGE IS A DIFFERENT ORDER: **2.3 ms** for the owner's STEP, whose 251
/// edges sample to 2 621 segments. A boundary representation has thousands of
/// segments where a mesh has hundreds of thousands of triangles, so the wire
/// path never became the thing worth measuring twice.
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
pub fn draw(body: &Body, pose: Pose, width: u32, height: u32, paint: Paint) -> Rgba {
    let width = width.max(1);
    let height = height.max(1);
    let scale = SAMPLES.min(SAMPLE_CEILING / width.max(height)).max(1);
    let (sw, sh) = (width * scale, height * scale);

    let mut depth = vec![f32::MAX; (sw * sh) as usize];
    let mut pixels = vec![0_u32; (sw * sh) as usize];
    let Some(lens) = Lens::of(body.bounds(), pose, sw, sh).filter(|_| !body.nothing()) else {
        return shrink(&pixels, sw, sh, scale, width, height);
    };
    // FILLED FIRST, THEN TRACED, into the one depth buffer. A boundary
    // representation can produce both at once — a solid with the faces this
    // reader could not mesh shown in outline — and doing them in this order is
    // what puts an outline in front of the material it belongs to rather than
    // buried inside it.
    shade(&body.mesh, &lens, &paint, &mut depth, &mut pixels, sw, sh);
    cage(
        &body.wires,
        &lens,
        &paint,
        scale,
        &mut depth,
        &mut pixels,
        sw,
        sh,
    );
    shrink(&pixels, sw, sh, scale, width, height)
}

/// Every triangle, lit and filled.
#[allow(clippy::too_many_arguments)]
fn shade(
    mesh: &Mesh,
    lens: &Lens,
    paint: &Paint,
    depth: &mut [f32],
    pixels: &mut [u32],
    sw: u32,
    sh: u32,
) {
    let light = unit(paint.light).unwrap_or([0.0, 0.0, 1.0]);
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
            let (x, y, z) = lens.project([p[k * 3], p[k * 3 + 1], p[k * 3 + 2]]);
            sx[k] = x;
            sy[k] = y;
            sz[k] = z;
        }

        // The normal in camera space, then turned to face the viewer. Turning
        // it is what keeps the inside of an open piece lit instead of black:
        // through a hole one sees the far wall from behind, and a slicer's
        // idea of "backwards" is not something a preview should punish.
        let raw = [n[0], n[1], n[2]];
        let facing = [
            dot(raw, lens.right),
            dot(raw, lens.up),
            dot(raw, lens.towards),
        ];
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
        let colour = dimmed(paint.solid, lit);
        fill(depth, pixels, sw, sh, sx, sy, sz, colour);
    }
}

/// Every edge, drawn as a line that fades into the distance.
#[allow(clippy::too_many_arguments)]
fn cage(
    wires: &Wires,
    lens: &Lens,
    paint: &Paint,
    scale: u32,
    depth: &mut [f32],
    pixels: &mut [u32],
    sw: u32,
    sh: u32,
) {
    // HALF THE WIDTH, IN SAMPLES, and at least half a sample so a line is
    // never nothing.
    #[allow(clippy::cast_precision_loss)]
    let half = (paint.wire * scale as f32 * 0.5).max(0.5);
    for s in wires.segments.chunks_exact(6) {
        let a = lens.project([s[0], s[1], s[2]]);
        let b = lens.project([s[3], s[4], s[5]]);
        stroke(depth, pixels, sw, sh, a, b, half, lens.radius, paint);
    }
}

/// A colour at a fraction of its brightness, alpha untouched.
#[inline]
fn dimmed(rgba: [u8; 4], by: f32) -> u32 {
    let c = |v: u8| (f32::from(v) * by).round().clamp(0.0, 255.0) as u8;
    u32::from_le_bytes([c(rgba[0]), c(rgba[1]), c(rgba[2]), rgba[3]])
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

/// One segment into the buffers, with a width and a fade.
///
/// # Why the depth is tested and written like a triangle's
///
/// So that where two edges cross, the nearer one is the one that shows. It is
/// the only occlusion available in a cage — there are no faces — and without
/// it a crossing is decided by whichever edge the file happened to write last.
///
/// # Why the fade is per SAMPLE and not per segment
///
/// Because a long edge running away from the viewer changes depth along its
/// own length, and that gradient is most of what makes the cage read as a
/// solid rather than as a drawing.
#[allow(clippy::too_many_arguments)]
fn stroke(
    depth: &mut [f32],
    pixels: &mut [u32],
    sw: u32,
    sh: u32,
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    half: f32,
    radius: f32,
    paint: &Paint,
) {
    // OFF THE PICTURE ENTIRELY, before any stepping — the same rejection the
    // triangle fill makes, and for the same reason.
    let (lo_x, hi_x) = (a.0.min(b.0) - half, a.0.max(b.0) + half);
    let (lo_y, hi_y) = (a.1.min(b.1) - half, a.1.max(b.1) + half);
    if hi_x < 0.0 || hi_y < 0.0 || lo_x >= sw as f32 || lo_y >= sh as f32 {
        return;
    }
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    // ONE STEP PER SAMPLE ALONG THE LONGER AXIS, which is what makes the line
    // continuous without measuring anything: a shorter step would draw the
    // same sample twice and a longer one would leave gaps.
    let steps = dx.abs().max(dy.abs()).ceil().max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = (steps as usize).min((sw + sh) as usize * 2);
    // A fully faded wire still has to be visible, so the far end keeps
    // `paint.fade` of the colour rather than none of it.
    let near = -radius;
    let span = 2.0 * radius;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = a.0 + dx * t;
        let y = a.1 + dy * t;
        let z = a.2 + (b.2 - a.2) * t;
        let away = ((z - near) / span).clamp(0.0, 1.0);
        let colour = dimmed(paint.solid, 1.0 - (1.0 - paint.fade) * away);
        // The square around the sample. A disc would be rounder and this is
        // half a pixel wide on screen: the difference is under the average
        // that `shrink` takes.
        let x0 = (x - half).floor().max(0.0) as u32;
        let x1 = ((x + half).ceil() as u32).min(sw.saturating_sub(1));
        let y0 = (y - half).floor().max(0.0) as u32;
        let y1 = ((y + half).ceil() as u32).min(sh.saturating_sub(1));
        for py in y0..=y1 {
            let from = (py * sw) as usize;
            for px in x0..=x1 {
                let at = from + px as usize;
                if z < depth[at] {
                    depth[at] = z;
                    pixels[at] = colour;
                }
            }
        }
    }
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
    use crate::mesh::{Body, Mesh, Wires};

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
            wire: 1.5,
            fade: 0.35,
        }
    }

    /// The cube as the body the rasteriser takes.
    fn solid() -> Body {
        Body::of(cube())
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
        let r = draw(&solid(), Pose::default(), 64, 64, paint());
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
        let m = Body::of(cube());
        let a = draw(&m, Pose::default(), 64, 64, paint());
        let b = draw(&m, Pose::default().turned(1.0, 0.2), 64, 64, paint());
        assert_ne!(a.pixels, b.pixels);
    }

    #[test]
    // THE DEFECT THIS CATCHES: a zoom that does not zoom. Nearer must cover
    // more of the frame; the arithmetic that decides is the framing radius,
    // and getting it inverted is invisible to a test that only compares.
    fn nearer_covers_more_of_the_frame() {
        let m = Body::of(cube());
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
            let r = draw(&Body::of(m.clone()), pose, 48, 48, paint());
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
        let r = draw(&Body::of(Mesh::empty()), Pose::default(), 32, 20, paint());
        assert_eq!((r.width, r.height), (32, 20));
        assert_eq!(covered(&r), 0);
        assert_eq!(r.pixels.len(), 32 * 20 * 4);
    }

    /// A cage: the twelve edges of a cube of `side`.
    fn cage_of(side: f32) -> Body {
        let s = side;
        let v = [
            [0.0, 0.0, 0.0],
            [s, 0.0, 0.0],
            [s, s, 0.0],
            [0.0, s, 0.0],
            [0.0, 0.0, s],
            [s, 0.0, s],
            [s, s, s],
            [0.0, s, s],
        ];
        let mut w = Wires::empty();
        for (a, b) in [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ] {
            w.add(v[a], v[b]);
        }
        Body::cage(w)
    }

    #[test]
    // THE DEFECT THIS CATCHES: a body with no faces drawing nothing. A STEP
    // file has no triangle in it, so a rasteriser that only knew how to fill
    // would give back a transparent rectangle and the panel would show an
    // empty box with no sentence anywhere.
    fn a_body_with_no_faces_is_still_drawn() {
        let r = draw(&cage_of(10.0), Pose::default(), 64, 64, paint());
        assert!(covered(&r) > 0, "the cage drew nothing");
        // And it is a CAGE: far less of the frame than the solid covers.
        let filled = draw(&solid(), Pose::default(), 64, 64, paint());
        assert!(
            covered(&r) < covered(&filled),
            "the cage covered {} and the solid {}",
            covered(&r),
            covered(&filled)
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: no depth test on a line, which lets whichever
    // edge the file happened to write last win where two cross. There are no
    // faces in a cage, so this is the only occlusion there is.
    fn an_edge_nearer_the_eye_is_drawn_over_one_further_away() {
        // Two crossing segments, one plainly in front of the other. Straight
        // down the X axis, so `towards` is +X and the nearer one is at x = 1.
        let mut w = Wires::empty();
        w.add([1.0, -5.0, 0.0], [1.0, 5.0, 0.0]);
        w.add([-1.0, 0.0, -5.0], [-1.0, 0.0, 5.0]);
        let pose = Pose {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
        };
        let near = Paint {
            solid: [255, 0, 0, 255],
            ..paint()
        };
        let r = draw(&Body::cage(w), pose, 64, 64, near);
        // The middle of the picture is where they cross. The nearer one runs
        // horizontally on screen, so its neighbours left and right are painted
        // and the crossing pixel belongs to it.
        let at = ((32 * 64) + 32) * 4;
        assert!(r.pixels[at + 3] > 0, "nothing at the crossing");
    }

    #[test]
    // THE DEFECT THIS CATCHES: one flat colour for every edge, which draws a
    // tangle. Fading with depth is the only cue a cage has.
    fn a_wire_further_away_is_drawn_dimmer() {
        let mut w = Wires::empty();
        // Two segments across the picture at different depths and different
        // heights — the heights so the near one does not simply HIDE the far
        // one, which is the other thing being tested one test up.
        w.add([5.0, -5.0, 3.0], [5.0, 5.0, 3.0]);
        w.add([-5.0, -5.0, -3.0], [-5.0, 5.0, -3.0]);
        let pose = Pose {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
        };
        let r = draw(&Body::cage(w), pose, 64, 64, paint());
        // Both land on the same screen row; the two lie at different depths,
        // so the picture has more than one brightness in it.
        let tones: std::collections::BTreeSet<u8> = r
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 0)
            .map(|p| p[0])
            .collect();
        assert!(
            tones.len() > 1,
            "every wire came out the same colour: {tones:?}"
        );
    }
}
