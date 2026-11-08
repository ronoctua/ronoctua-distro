// One image over an area of the screen — see `image.rs`.
//
// The same economy as `quad.wgsl`: with no vertex buffer, the four corners come
// out of `vertex_index`. The difference is that here every corner also carries
// its texture coordinate, and there is a bind group with the texture and the
// sampler.

struct Instance {
    // Top left corner, in NDC.
    @location(0) pos: vec2<f32>,
    // Size in NDC. The Y comes NEGATIVE: in NDC it grows upwards, and in
    // pixels it grows downwards.
    @location(1) size: vec2<f32>,
    // What of the screen it may cover, in NDC: left, bottom, right, top. The
    // whole screen for a picture; its stage for a sprite (`image::Sprite`).
    @location(2) clip: vec4<f32>,
    // How opaque it is drawn: 1 for a picture.
    @location(3) alpha: f32,
};

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Where the fragment is, in NDC, and the instance's clip and opacity.
    @location(1) ndc: vec2<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
    @location(3) @interpolate(flat) alpha: f32,
};

@group(0) @binding(0) var image_texture: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instance) -> Out {
    let right = f32(vi >> 1u);
    let down = f32(vi & 1u);

    var s: Out;
    s.position = vec4<f32>(
        inst.pos.x + inst.size.x * right,
        inst.pos.y + inst.size.y * down,
        0.0,
        1.0,
    );
    // The UV follows the corner: (0,0) at the image's top left. It does NOT
    // flip along with the NDC's Y — the image already arrives with its first
    // row on top, the way every file format delivers it.
    s.uv = vec2<f32>(right, down);
    s.ndc = s.position.xy;
    s.clip = inst.clip;
    s.alpha = inst.alpha;
    return s;
}

@fragment
fn fs(frag: Out) -> @location(0) vec4<f32> {
    let c = textureSample(image_texture, image_sampler, frag.uv);
    // CUT AT THE CLIP: what falls outside its box is not drawn at all.
    if (frag.ndc.x < frag.clip.x || frag.ndc.x > frag.clip.z
        || frag.ndc.y < frag.clip.y || frag.ndc.y > frag.clip.w) {
        discard;
    }
    // The texture is STRAIGHT alpha, and the blend is too: the opacity
    // multiplies the alpha alone.
    return vec4<f32>(c.rgb, c.a * frag.alpha);
}
