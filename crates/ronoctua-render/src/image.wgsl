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
};

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
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
    return s;
}

@fragment
fn fs(frag: Out) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, frag.uv);
}
