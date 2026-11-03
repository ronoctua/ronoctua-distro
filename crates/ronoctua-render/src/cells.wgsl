// A grid of cells — see `cells.rs`.
//
// One pipeline for everything a grid draws: a flat rectangle (a background,
// the cursor, a band), a raster of coverage in the cell's colour (a glyph, a
// sprite, a line), or a raster of an emoji's own colours. The instance says
// which (`kind`), and the four corners come out of `vertex_index`, as in
// `quad.wgsl`.
//
// The positions are PHYSICAL pixels from the grid's origin, and the origin and
// the surface's size are a uniform: moving the grid, or the surface growing,
// rewrites sixteen bytes and not one instance.

struct View {
    // The surface, in physical pixels.
    size: vec2<f32>,
    // Where the grid's top-left corner is on it.
    origin: vec2<f32>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var masks: texture_2d<f32>;
@group(0) @binding(2) var colours: texture_2d<f32>;

struct Instance {
    // x, y, width, height — physical pixels from the grid's origin.
    @location(0) rect: vec4<f32>,
    // LINEAR, as the `Srgb` target wants it.
    @location(1) colour: vec4<f32>,
    // The cell's ground under a raster of coverage, LINEAR too (`cells.rs`'s
    // `Instance`): what the coverage is blended against.
    @location(4) ground: vec4<f32>,
    // The raster's top-left texel in its atlas.
    @location(2) texel: vec2<f32>,
    // 0 a flat rectangle, 1 coverage, 2 an emoji's colours.
    @location(3) kind: u32,
};

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
    // From the fragment's pixel to its texel: the raster's top-left minus
    // where it sits on the surface. Flat, so every fragment of an instance
    // reads the SAME offset and lands on its texel exactly, with no
    // interpolation to round wrong.
    @location(1) @interpolate(flat) shift: vec2<f32>,
    @location(2) @interpolate(flat) kind: u32,
    @location(3) @interpolate(flat) ground: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instance) -> Out {
    // `TriangleStrip`, four vertices: bit 1 is right, bit 0 is down.
    let corner = vec2<f32>(f32(vi >> 1u), f32(vi & 1u));
    let at = view.origin + inst.rect.xy;
    let px = at + inst.rect.zw * corner;
    var out: Out;
    // NDC: x from -1 to 1, y from 1 (top) to -1 (bottom).
    out.position = vec4<f32>(
        px.x / view.size.x * 2.0 - 1.0,
        1.0 - px.y / view.size.y * 2.0,
        0.0,
        1.0,
    );
    out.colour = inst.colour;
    out.shift = inst.texel - at;
    out.kind = inst.kind;
    out.ground = inst.ground;
    return out;
}

// sRGB's own curve, one channel each way.
fn encoded(c: f32) -> f32 {
    if c <= 0.0031308 {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn decoded(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

// A linear colour's luminance (Rec. 709, sRGB's primaries).
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// THE ALPHA THAT BLENDS LIKE THE SCREEN'S SPACE: the target blends linearly,
// and coverage `a` of `ink` over `ground` blended so reads a weight heavier
// than the face (light on dark) or lighter (dark on light). The luminance the
// screen's space would give is mixed in sRGB and brought back, and the alpha
// that reaches it linearly is solved for — Ghostty's look, by the same
// arithmetic its `linear-corrected` blending does.
fn corrected(a: f32, ink: vec3<f32>, ground: vec3<f32>) -> f32 {
    let li = luminance(ink);
    let lg = luminance(ground);
    if abs(li - lg) < 0.0001 {
        return a;
    }
    let wanted = decoded(mix(encoded(lg), encoded(li), a));
    return clamp((wanted - lg) / (li - lg), 0.0, 1.0);
}

@fragment
fn fs(frag: Out) -> @location(0) vec4<f32> {
    // THE FRAGMENT'S OWN PIXEL, which is a pixel centre: floored after the
    // shift, it is the texel the raster put there — one to one, because a
    // raster is drawn at the size it was rasterised.
    let texel = vec2<i32>(floor(frag.position.xy + frag.shift));
    // BOTH LOADS, and the choice after them: a load has no uniformity rule,
    // and two are cheaper than the branch's reasoning.
    let coverage = textureLoad(masks, texel, 0).r;
    let own = textureLoad(colours, texel, 0);
    if frag.kind == 1u {
        let a = corrected(frag.colour.a * coverage, frag.colour.rgb, frag.ground.rgb);
        return vec4<f32>(frag.colour.rgb, a);
    }
    if frag.kind == 2u {
        return vec4<f32>(own.rgb, own.a * frag.colour.a);
    }
    return frag.colour;
}
