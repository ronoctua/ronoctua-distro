//! An image drawn in an area of the screen — the third thing the renderer
//! knows how to do.
//!
//! # Why it exists
//!
//! The clipboard history has to show what was copied, and what one
//! copies is not always text. Until now the renderer knew glyphs and flat-colour
//! rectangles: a pasted screenshot showed up as
//! `copy_of kdl_of kind unsupported ignored` in the log and nothing on screen.
//!
//! # This does not violate invariant 15
//!
//! "Node cards, no shadow, no elevation." An image drawn in an area is
//! none of the three: it has no border, no radius, no shadow, and does not
//! suggest that something is *above* something else. It plays the same role as the quad —
//! occupying an area of the same surface.
//!
//! # Several per frame, with a key
//!
//! It was **one** — the clipboard's focused-item preview — and the app
//! launcher needs one per visible row. Two separate mechanisms
//! would diverge; what exists is one, with a KEY.
//!
//! The key decides when the transfer happens: uploading a texture costs
//! megabytes in the preview's case, and `missing()` answers without touching the GPU if it
//! is already there. Comparing dimensions would not be enough — two screenshots of the same
//! monitor have the same size and are different images.
//!
//! What is **not** here is a disk cache or a decoding one: this keeps
//! what is already a texture. What decides what to decode is the host.

use ronoctua_tokens as tokens;
use std::collections::HashMap;

/// Where the image goes, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Area {
    /// The `(w, h)` pixel area **centred** inside this one, without enlarging.
    ///
    /// # Why it differs from [`Area::fit_into`]
    ///
    /// Because `fit_into` **grows** to the edge, and there is a case in which growing
    /// erases the information: the Lucide preview draws the icon at the size the
    /// person chose, and stretching it to fill the box leaves the 24 px one
    /// identical to the 512 px one. Whoever is choosing a size needs to see the size.
    ///
    /// Shrinking still applies — a drawing larger than the box fits reduced,
    /// which is what any viewer does.
    #[must_use]
    pub fn centre(self, w: u32, h: u32) -> Self {
        if w == 0 || h == 0 {
            return Self {
                width: 0.0,
                height: 0.0,
                ..self
            };
        }
        // IT DOES NOT FIT: the fit applies, which already reduces and already centres.
        if w as f32 > self.width || h as f32 > self.height {
            return self.fit_into(w, h);
        }
        Self {
            x: self.x + (self.width - w as f32) / 2.0,
            y: self.y + (self.height - h as f32) / 2.0,
            width: w as f32,
            height: h as f32,
        }
    }

    /// The largest area with `(w, h)`'s aspect ratio that fits inside this one.
    ///
    /// **Centred**, and never stretched: a deformed screenshot is
    /// worse than a smaller one — whoever looks does not know whether what is skewed is the
    /// image or the preview.
    #[must_use]
    pub fn fit_into(self, w: u32, h: u32) -> Self {
        if w == 0 || h == 0 {
            return Self {
                width: 0.0,
                height: 0.0,
                ..self
            };
        }
        let scale = (self.width / w as f32).min(self.height / h as f32);
        let (width, height) = (w as f32 * scale, h as f32 * scale);
        Self {
            x: self.x + (self.width - width) / 2.0,
            y: self.y + (self.height - height) / 2.0,
            width,
            height,
        }
    }
}

/// An instance as the GPU sees it: two `vec2`s.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    pos: [f32; 2],
    sz: [f32; 2],
}

/// How many images fit in a frame.
///
/// The ceiling of the expanded region in rows — one icon per visible row — plus
/// one for the clipboard's preview, which occupies the right panel and coexists with the
/// list on the left. The number is the token's because it is the SAME limit: what
/// does not fit in the region has no icon to draw.
///
/// # And why the file manager gets TWO on top of that
///
/// Because it draws two pictures at once now: the preview's, in the right-hand
/// panel, and the globe's, at the top of the info panel beside it. A second
/// view could be showing a globe of its own.
///
/// THIS IS THE ONE NUMBER IN THE FEATURE THAT FAILS SILENTLY. `prepare` takes
/// what fits and DROPS the rest with no error anywhere — so a ceiling one too
/// low means the globe simply never reaches the GPU on a full region, and
/// nothing on screen or in any log says why. `files-panels-have-two-pictures`
/// is what keeps it honest.
const CEILING: usize =
    tokens::space::EXPANSION_MAXLINES as usize + 1 + crate::image::MANAGER_PICTURES;

/// How many pictures the file manager can have on screen at once.
///
/// The preview's and the globe's. It is named rather than inlined because the
/// ceiling above has to be able to say WHY it is not just the region's rows.
pub const MANAGER_PICTURES: usize = 2;

/// How many textures stay on the GPU between frames.
///
/// More than one frame's ceiling, so scrolling the list back does not re-pay the
/// transfer; and bounded, because the clipboard's preview is **33 MB** of
/// RGBA per screenshot — an unbounded cache would become the footer's memory.
const TEXTURE_CEILING: usize = CEILING * 3;

/// A texture already on the GPU.
struct Upload {
    group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// Draws images in an area of the screen, by key.
pub struct Images {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    buffer: wgpu::Buffer,
    /// What is already on the GPU, by key.
    uploads: HashMap<String, Upload>,
    /// The textures behind them, kept so a frame can be written into one again.
    ///
    /// A bind group holds a VIEW and cannot hand back the texture it was built
    /// from, so the texture is kept beside it. Thirty-nine of them, the same
    /// ceiling `uploads` has, and each one is the memory its picture already
    /// occupies — nothing new is held.
    textures: HashMap<String, wgpu::Texture>,
    /// The keys to draw this frame, in instance order.
    batch: Vec<String>,
}

impl Images {
    /// Builds the pipeline.
    ///
    /// # Errors
    ///
    /// It never fails today; the signature follows [`crate::quad::Quads::new`].
    pub fn new(device: &wgpu::Device, format_of: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image"),
            source: wgpu::ShaderSource::Wgsl(include_str!("image.wgsl").into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("image"),
            // An `Option` in wgpu 30, like the pipeline's `layout`. See the
            // graphics-API trap in AGENTS.md.
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("image"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format_of,
                    // Normal alpha: a PNG with transparency lets the footer's background
                    // show through, instead of cutting out a black hole.
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image"),
            // LINEAR in both directions: the preview almost always SHRINKS a
            // screenshot, and shrinking with `Nearest` aliases text to the point
            // of making the copied window unrecognisable.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            pipeline,
            layout,
            sampler,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("image-instance"),
                size: (std::mem::size_of::<Instance>() * CEILING) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            uploads: HashMap::new(),
            textures: HashMap::new(),
            batch: Vec::new(),
        })
    }

    /// Is the image `key` NOT on the GPU yet?
    ///
    /// It is what avoids the transfer: the host asks before decoding, and
    /// a 33 MB preview does not go up again on every frame.
    #[must_use]
    pub fn missing(&self, key: &str) -> bool {
        !self.uploads.contains_key(key)
    }

    /// Uploads an RGBA8 image under a key.
    ///
    /// Called only when [`Images::missing`] says it is not there —
    /// uploading the same texture on every frame would put a transfer of
    /// megabytes into the frame budget.
    pub fn upload_it(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: &str,
        rgba: &crate::image::Rgba,
    ) {
        if rgba.width == 0 || rgba.height == 0 {
            // A zero-dimension image is not a texture: registering it would make
            // `missing()` lie and the drawing ask for a 0 px `create_texture`,
            // which wgpu refuses at the last moment.
            return;
        }
        let size = wgpu::Extent3d {
            width: rgba.width,
            height: rgba.height,
            depth_or_array_layers: 1,
        };
        // THE SAME TEXTURE AGAIN WHEN THE SHAPE HAS NOT CHANGED.
        //
        // A moving file arrives under one key, thirty times a second, at one
        // size — and this used to allocate a texture, a view and a bind group
        // for every one of them, then drop the lot on the next. `write_texture`
        // is the only part a new frame actually needs.
        if let Some(had) = self.textures.get(key)
            && self
                .uploads
                .get(key)
                .is_some_and(|u| u.width == rgba.width && u.height == rgba.height)
        {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: had,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(rgba.width * 4),
                    rows_per_image: Some(rgba.height),
                },
                size,
            );
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // `Srgb`: the PNG's bytes are sRGB, and it is the GPU that undoes the curve
            // when sampling. Using `Unorm` would lighten everything — the same trap as
            // `to_linear` recorded in AGENTS.md, from the other side.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rgba.width * 4),
                rows_per_image: Some(rgba.height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.uploads.insert(
            key.to_owned(),
            Upload {
                group,
                width: rgba.width,
                height: rgba.height,
            },
        );
        self.textures.insert(key.to_owned(), texture);
    }

    /// Forgets everything that was drawn. The textures stay on the GPU.
    pub fn clear(&mut self) {
        self.batch.clear();
    }

    /// The size of an already-uploaded image, for whoever computes the fit.
    #[must_use]
    pub fn dimensions(&self, key: &str) -> Option<(u32, u32)> {
        self.uploads.get(key).map(|s| (s.width, s.height))
    }

    /// What to draw this frame. It converts to NDC, like `quad`.
    ///
    /// A key that is not on the GPU is **silently ignored**, and that is the
    /// design: an application's icon arrives on a thread, and the row exists
    /// before it does. Failing here would be the list waiting for the icon, which is
    /// exactly what it cannot do.
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        items: &[(String, Area)],
    ) {
        self.batch.clear();
        if width == 0 || height == 0 {
            return;
        }
        let (w, h) = (width as f32, height as f32);
        let mut instances: Vec<Instance> = Vec::with_capacity(items.len().min(CEILING));
        for (key, area) in items {
            if instances.len() >= CEILING {
                break;
            }
            if area.width <= 0.0 || area.height <= 0.0 || !self.uploads.contains_key(key) {
                continue;
            }
            instances.push(Instance {
                pos: [area.x / w * 2.0 - 1.0, 1.0 - area.y / h * 2.0],
                sz: [area.width / w * 2.0, -(area.height / h * 2.0)],
            });
            self.batch.push(key.clone());
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&instances));
        }
        self.dump();
    }

    /// Releases the textures that are not in the current frame, when there are too many.
    ///
    /// A coarse policy on purpose: it only acts above the ceiling, and then keeps
    /// exactly what is on screen. A actual LRU would need a clock
    /// and per-frame sorting to save transfers of a kilobyte
    /// — the bookkeeping would cost more than what it avoids.
    fn dump(&mut self) {
        if self.uploads.len() <= TEXTURE_CEILING {
            return;
        }
        let alive: std::collections::HashSet<&str> =
            self.batch.iter().map(String::as_str).collect();
        self.uploads.retain(|k, _| alive.contains(k.as_str()));
        // AND THE TEXTURES GO WITH THEM. Kept separately they would be a second
        // map growing without a ceiling — which is the thing this function
        // exists to stop.
        self.textures.retain(|k, _| alive.contains(k.as_str()));
    }

    /// Draws what was prepared.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.batch.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        for (i, key) in self.batch.iter().enumerate() {
            let Some(s) = self.uploads.get(key) else {
                continue;
            };
            // ONE CALL PER IMAGE: each has its own texture, and the bind
            // group cannot change in the middle of a `draw`. There are at most thirteen
            // per frame — the cost is in the transfer, not here.
            pass.set_bind_group(0, &s.group, &[]);
            let n = i as u32;
            pass.draw(0..4, n..n + 1);
        }
    }
}

/// Pixels ready for the GPU.
///
/// It lives here, and not in the clipboard crate, because it is the shape the renderer
/// consumes. What **decodes** is `ronoctua_clipboard::image`, which does not
/// depend on a GPU and can therefore be tested without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes, RGBA8.
    pub pixels: Vec<u8>,
}

impl Rgba {
    /// A picture, or nothing at all when the bytes do not make one.
    ///
    /// # Why the count is CHECKED and not assumed
    ///
    /// Because the player reads its frames out of a pipe, and a pipe closing
    /// mid-frame hands back fewer bytes than a picture needs. Uploaded anyway,
    /// that is the smear that slides a little further sideways every frame —
    /// the defect the owner photographed, in its second form. One place asks,
    /// and a partial frame becomes `None` instead of a texture.
    #[must_use]
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let want = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (pixels.len() == want).then_some(Self {
            width,
            height,
            pixels,
        })
    }
}

/// The colour the image's area is cleared with before drawing.
///
/// An image with transparency would let what was in the previous frame show
/// through; the buffer's background is what it should show underneath.
#[must_use]
pub const fn background() -> tokens::Color {
    tokens::color::SURFACE_BUFFER
}

#[cfg(test)]
mod tests {
    use super::Area;

    fn area() -> Area {
        Area {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        }
    }

    #[test]
    fn the_fit_preserves_the_ratio_and_centres() {
        // STRETCHING WOULD BE WORSE THAN SHRINKING. A deformed screenshot
        // makes whoever looks doubt what they copied.
        let a = area().fit_into(200, 200);
        assert!((a.width - 50.0).abs() < 0.01, "width: {}", a.width);
        assert!((a.height - 50.0).abs() < 0.01, "height: {}", a.height);
        // Centred horizontally, which is where the space was left over.
        assert!((a.x - 35.0).abs() < 0.01, "x: {}", a.x);
        assert!((a.y - 20.0).abs() < 0.01, "y: {}", a.y);
    }

    #[test]
    fn a_wide_image_is_bounded_by_the_width() {
        let a = area().fit_into(400, 100);
        assert!((a.width - 100.0).abs() < 0.01);
        assert!((a.height - 25.0).abs() < 0.01);
        assert!((a.y - 32.5).abs() < 0.01, "it did not centre vertically");
    }

    #[test]
    fn an_image_of_zero_dimension_does_not_draw() {
        // A decoder that returns 0x0 must not become a division by
        // zero — and zero area is the sign for "do not draw".
        let a = area().fit_into(0, 10);
        assert_eq!(a.width, 0.0);
        assert_eq!(a.height, 0.0);
    }
}
