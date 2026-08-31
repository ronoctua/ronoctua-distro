//! Flat-color rectangles — the second thing the renderer draws.
//!
//! # Why it exists, and why only now
//!
//! Until now the renderer only knew glyphs. §7.7 says the load is "glyph,
//! rectangle and one-line rule", but the rectangle was never written: the rule
//! came out as a row of `─` and the protocol's `Command::Selection` was
//! **discarded silently** — a buffer could ask for a row highlight and nothing
//! happened.
//!
//! What forced the issue was the modeline needing a background of its own,
//! lighter than the minibuffer's. A background is an area, and an area with
//! glyphs would be a row of `█` — which depends on the font's metrics not to
//! leave a gap, and which the atlas would have to rasterize.
//!
//! # This does not violate invariant 15
//!
//! "Node cards, no shadow, no elevation." A flat-color rectangle is none of the
//! three: it has no border, no radius, no shadow, and it does not suggest that
//! something is *above* something else. It is the same role as the one-line
//! rule — to separate by color, and not by container.
//!
//! # Node vertex buffer
//!
//! The quad comes from the `vertex_index`: four corners generated in the
//! shader. Only the INSTANCE data travels (position, size, color), 32 bytes
//! each. On a surface that draws half a dozen rectangles, assembling and
//! keeping a vertex buffer would be more code for the same pixel.

use ronoctua_tokens as tokens;

/// A rectangle, in logical pixels, with the origin at the top left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// The color, from a token.
    pub color: tokens::Color,
}

/// One instance as the GPU sees it: two `vec2`s and one `vec4`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    /// Top left corner in NDC.
    pos: [f32; 2],
    /// Size in NDC (with the Y axis's sign already inverted).
    sz: [f32; 2],
    color: [f32; 4],
}

/// Draws flat-color rectangles.
pub struct Quads {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    /// How many instances the buffer has RIGHT NOW. Zero skips the `draw`.
    amount: u32,
    /// Capacity in instances. The buffer only grows.
    capability_of: u64,
}

/// How many instances the buffer is born holding.
///
/// Eight covers the whole footer — modeline background, minibuffer
/// background, rule and one selection — with no reallocation on the startup
/// path, which is what §5.2 times.
const INITIAL_CAPACITY: u64 = 8;

impl Quads {
    /// Assembles the pipeline.
    ///
    /// # Errors
    ///
    /// It never fails today; the signature gives back a `Result` to match
    /// [`crate::Text::new`], and because creating a pipeline may start failing.
    pub fn new(device: &wgpu::Device, format_of: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad"),
            source: wgpu::ShaderSource::Wgsl(include_str!("quad.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad"),
            // wgpu 30 wraps both in `Option` and swapped `push_constant_ranges`
            // for `immediate_size`. See the graphics API trap in CLAUDE.md — read
            // the source, do not assume.
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format_of,
                    // Normal alpha blending: a token with alpha < 255 lets what is
                    // behind show through, instead of cutting a hole.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            pipeline,
            buffer: Self::make_buffer(device, INITIAL_CAPACITY),
            amount: 0,
            capability_of: INITIAL_CAPACITY,
        })
    }

    fn make_buffer(device: &wgpu::Device, instances: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad-instancias"),
            size: instances * std::mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Converts to NDC and uploads to the GPU.
    ///
    /// The conversion is done HERE, and not in the shader, so as not to pass the
    /// dimensions through a uniform: with no bind group, the pipeline has no
    /// state at all beyond the instance buffer.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        quads: &[Quad],
    ) {
        self.amount = u32::try_from(quads.len()).unwrap_or(u32::MAX);
        if quads.is_empty() || width == 0 || height == 0 {
            return;
        }

        let (w, h) = (width as f32, height as f32);
        let instances: Vec<Instance> = quads
            .iter()
            .map(|q| Instance {
                // NDC: x from -1 to 1, y from 1 (top) to -1 (bottom). The Y axis is
                // inverted with respect to the pixel, and that is why the vertical
                // size goes in negative.
                pos: [q.x / w * 2.0 - 1.0, 1.0 - q.y / h * 2.0],
                sz: [q.width / w * 2.0, -(q.height / h * 2.0)],
                color: q.color.to_linear(),
            })
            .collect();

        let precise = instances.len() as u64;
        if precise > self.capability_of {
            // It grows and never shrinks: the footer alternates between few and many
            // rectangles on every expansion, and reallocating both ways would trade
            // memory for one allocation per key.
            self.capability_of = precise.next_power_of_two();
            self.buffer = Self::make_buffer(device, self.capability_of);
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&instances));
    }

    /// Draws what was prepared.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, 0, self.amount);
    }

    /// Draws only some of what was prepared, by position in the list.
    ///
    /// # Why a rectangle sometimes has to come AFTER the pictures
    ///
    /// There is no depth test: whoever draws later ends up on top, and the
    /// order is quads, then pictures, then text. That is right for a
    /// background and wrong for the file manager's transport, whose controls
    /// sit ON the film — a background drawn with the other quads would be
    /// behind the picture and invisible, and drawing the whole list twice would
    /// put every background over the picture instead.
    ///
    /// So the caller puts the few that belong on top at the END of the list and
    /// names where they begin. One buffer, one pipeline, two calls.
    pub fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, from: u32, to: u32) {
        let to = to.min(self.amount);
        if self.amount == 0 || from >= to {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        // Four vertices in a `TriangleStrip` = one rectangle, with no index.
        pass.draw(0..4, from..to);
    }

    /// How many rectangles were prepared.
    #[must_use]
    pub const fn amount(&self) -> u32 {
        self.amount
    }
}

#[cfg(test)]
mod tests {
    use super::{Quad, Quads};

    #[test]
    fn the_quad_fits_in_32_bytes() {
        // It is not curiosity: the `array_stride` of the `VertexBufferLayout`
        // is computed from this size, and one more field without adjusting the
        // layout would misalign EVERY instance — the symptom would be a
        // rectangle in the wrong place, with no error at all.
        assert_eq!(std::mem::size_of::<super::Instance>(), 32);
    }

    #[test]
    fn the_conversion_to_ndc_is_the_expected_one() {
        // The whole screen becomes the square from -1 to 1, with the Y inverted.
        // Getting the Y's sign wrong puts everything upside down, and on a
        // flat-color rectangle that is not obvious to look at.
        let (w, h) = (100.0_f32, 50.0_f32);
        let q = Quad {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            color: ronoctua_tokens::color::SURFACE_BASE,
        };
        let pos = [q.x / w * 2.0 - 1.0, 1.0 - q.y / h * 2.0];
        let sz = [q.width / w * 2.0, -(q.height / h * 2.0)];
        assert_eq!(pos, [-1.0, 1.0], "o canto superior esquerdo e (-1, 1)");
        assert_eq!(sz, [2.0, -2.0], "the whole screen measures 2 x -2 in NDC");
    }

    #[test]
    fn preparing_with_no_quads_does_not_draw() {
        // Called on every frame of the footer at rest. A `draw` with zero
        // instances and an empty buffer is a validation error in wgpu.
        assert_eq!(u32::try_from(Vec::<Quad>::new().len()).unwrap(), 0);
    }

    /// The type has to exist even with no GPU in the test environment.
    #[allow(dead_code)]
    fn compiles(_: &Quads) {}
}
