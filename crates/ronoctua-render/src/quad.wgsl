// A flat-coloured rectangle — see `quad.rs`.
//
// No vertex buffer and no bind group: the four corners come out of
// `vertex_index`, and the conversion to NDC arrived ready from the CPU. The
// pipeline has no state at all beyond the instances.

struct Instance {
    // Top left corner, in NDC.
    @location(0) pos: vec2<f32>,
    // Size in NDC. The Y comes NEGATIVE: in NDC it grows upwards, and in
    // pixels it grows downwards.
    @location(1) size: vec2<f32>,
    @location(2) colour: vec4<f32>,
};

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, inst: Instance) -> Out {
    // `TriangleStrip` with four vertices, in this order:
    //   0 = top left       1 = bottom left
    //   2 = top right      3 = bottom right
    //
    // Bit 1 says whether it is on the right, bit 0 whether it is at the bottom.
    // A table would read better and would occupy a register for nothing.
    let right = f32(vi >> 1u);
    let down = f32(vi & 1u);

    var s: Out;
    s.position = vec4<f32>(
        inst.pos.x + inst.size.x * right,
        inst.pos.y + inst.size.y * down,
        0.0,
        1.0,
    );
    s.colour = inst.colour;
    return s;
}

@fragment
fn fs(frag: Out) -> @location(0) vec4<f32> {
    // Flat colour. No gradient, no border, no radius — invariant 15.
    return frag.colour;
}
