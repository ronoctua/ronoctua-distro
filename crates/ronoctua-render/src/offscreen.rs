//! Draws a frame into a TEXTURE instead of a surface, and writes it out as a
//! PNG.
//!
//! # Why this exists
//!
//! Until it did, the only way to LOOK at anything this renderer draws was to
//! boot a NixOS VM, start niri under `Xvfb`, screenshot the X root and crop it
//! by the geometry niri reported. Three minutes and a QEMU, to see whether a
//! title was touching a rule. The owner's own rule says green is not proof —
//! open it on the screen before saying it is done — and a rule that costs three
//! minutes every time is a rule that gets skipped.
//!
//! # Why it is not a second renderer
//!
//! Because it is the same one. [`Gpu`](crate::gpu::Gpu) is bound to a
//! `wgpu::Surface`, and that is the ONLY part of the path that needs Wayland:
//! the device, the limits, the atlas, the shaper, the quad pipeline and the
//! image pipeline all take a `TextureFormat` and do not care where it came
//! from. So this module supplies a device and a texture, and the caller drives
//! exactly the same `prepare`/`draw` sequence the footer drives. A PNG that
//! disagreed with the screen would mean this file diverged — not that the
//! screen is a mystery.
//!
//! # The format, and why it is not the one a Wayland surface gives
//!
//! Production takes the first sRGB format the SURFACE offers, which on Wayland
//! is normally `Bgra8UnormSrgb`. There is no surface here to ask, so the choice
//! is [`FORMAT`]: also sRGB — which is what matters, because the clear colour
//! goes through `to_linear()` and would come out washed on a linear target —
//! and with the channels already in the order a PNG wants.

use anyhow::{Context as _, Result};

use crate::gpu::limits_of;

/// The colour format drawn into.
///
/// sRGB, like the surface's: see the module's note. `Rgba` and not `Bgra` only
/// so the readback does not have to swap two bytes per pixel.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// `copy_texture_to_buffer` wants each row aligned to this.
const ROW_ALIGN: u32 = 256;

/// A device and a texture, with no surface and no compositor.
pub struct Offscreen {
    /// The device, for whoever builds the pipelines.
    pub device: wgpu::Device,
    /// The queue, likewise.
    pub queue: wgpu::Queue,
    /// The width actually used, after the GPU's ceiling.
    pub width: u32,
    /// The height actually used.
    pub height: u32,
    /// The largest dimension this device takes, which is what `width` and
    /// `height` were clamped against.
    ///
    /// Kept because the clamp is SILENT to whoever asked: without it a caller
    /// that wants to refuse an over-large picture can only report the size it
    /// GOT, and on the axis that was not clamped that number is the caller's
    /// own request dressed up as a limit.
    pub ceiling: u32,
    target: wgpu::Texture,
}

impl Offscreen {
    /// Opens a device with no surface and allocates the target.
    ///
    /// The backend falls back the same way `Gpu::new` does — Vulkan, then GL —
    /// because the environment this runs in most often is a check with no GPU,
    /// where what answers is Mesa's lavapipe.
    ///
    /// # Errors
    ///
    /// No adapter at all, or the device refused the downlevel limits.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let adapter_of = |backends| {
            // The same construction as `Gpu::new`: `InstanceDescriptor` has no
            // `Default` in wgpu 30, and the constructor that skips the display
            // handle is on the DESCRIPTOR, not on the instance.
            let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
            d.backends = backends;
            let instance = wgpu::Instance::new(d);
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()
        };
        let adapter = match adapter_of(wgpu::Backends::VULKAN) {
            Some(a) => a,
            None => adapter_of(wgpu::Backends::GL).context(
                "no graphics adapter will do, neither by Vulkan nor by GL.\n\
                 In an environment with no GPU, Mesa's lavapipe works — it is what\n\
                 the tests use.",
            )?,
        };

        // THE SAME LIMITS AS THE SCREEN'S, and for the same reason: asking for
        // the adapter's resolution and nothing else keeps the 2048 ceiling of
        // `downlevel_defaults` from deciding how big a window may be.
        let limits = limits_of(&adapter);
        let ceiling = limits.max_texture_dimension_2d;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ronoctua-offscreen"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            ..Default::default()
        }))
        .context("the adapter handed over no device")?;

        let width = width.clamp(1, ceiling);
        let height = height.clamp(1, ceiling);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            // `COPY_SRC` is the whole point: without it the drawing exists and
            // cannot be read back, which looks exactly like a drawing that did
            // not happen.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        Ok(Self {
            device,
            queue,
            width,
            height,
            ceiling,
            target,
        })
    }

    /// Clears with `background` and lets `draw` paint on top.
    ///
    /// The same shape as `Gpu::frame`, closure and all, so the caller's drawing
    /// code is the same line for line. `background` is linear — the callers
    /// have a token and call `to_linear()` on it, as the footer does.
    ///
    /// # Errors
    ///
    /// Whatever `draw` returns.
    pub fn frame<F>(&mut self, background: [f32; 4], draw: F) -> Result<()>
    where
        F: FnOnce(&mut wgpu::RenderPass<'_>) -> Result<()>,
    {
        let view = self
            .target
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen"),
            });
        {
            let [r, g, b, a] = background;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(r),
                            g: f64::from(g),
                            b: f64::from(b),
                            a: f64::from(a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            draw(&mut pass)?;
        }
        self.queue.submit(Some(enc.finish()));
        Ok(())
    }

    /// Reads the texture back as tightly packed RGBA.
    ///
    /// # Errors
    ///
    /// The mapping failed, which on a software adapter means the device was
    /// lost.
    pub fn read_rgba(&self) -> Result<Vec<u8>> {
        // EACH ROW PADDED TO 256 BYTES on the way out, and unpadded here. wgpu
        // requires the alignment of the copy and says nothing about it
        // afterwards: a caller that trusted `width * 4` would get an image
        // sheared a little further to the right on every row, which reads as a
        // font problem and is not one.
        let unpadded = self.width * 4;
        let padded = unpadded.div_ceil(ROW_ALIGN) * ROW_ALIGN;
        let scratch = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("reading"),
            size: u64::from(padded) * u64::from(self.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("reading"),
            });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &scratch,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(enc.finish()));

        let slice = scratch.slice(..);
        let (said, heard) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = said.send(r);
        });
        // `Wait` and not `Poll`: the mapping only completes when the queue
        // drains, and without blocking here the receive below waits forever on
        // work nobody asked the device to finish.
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        heard
            .recv()
            .context("the read mapping never answered")?
            .context("mapping the read buffer failed")?;

        let padded_bytes = slice
            .get_mapped_range()
            .context("the mapped range did not come")?;
        let mut out = Vec::with_capacity((unpadded * self.height) as usize);
        for row in 0..self.height {
            let from = (row * padded) as usize;
            out.extend_from_slice(&padded_bytes[from..from + unpadded as usize]);
        }
        drop(padded_bytes);
        scratch.unmap();
        Ok(out)
    }
}

/// Encodes RGBA into a PNG, with no compression and no dependency.
///
/// # Why by hand
///
/// Because the alternative is making `png` a direct dependency, and in a build
/// where crane compiles the dependency layer separately that costs a full
/// rebuild of every dependency — minutes, on every machine, forever — to save
/// forty lines here. This is a tool that writes a picture somebody is about to
/// look at once: the file being three times larger than it needs to be costs
/// nothing, and `deflate` has a **stored** block type that exists precisely so
/// that a writer can skip compressing.
///
/// # Errors
///
/// Writing to `path` failed.
pub fn write_png(path: &std::path::Path, rgba: &[u8], width: u32, height: u32) -> Result<()> {
    /// The one table both checksums here need — CRC-32, as PNG defines it.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for b in bytes {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        let mut crc_over = Vec::with_capacity(4 + body.len());
        crc_over.extend_from_slice(kind);
        crc_over.extend_from_slice(body);
        out.extend_from_slice(&crc32(&crc_over).to_be_bytes());
    }

    // EACH ROW GETS A FILTER BYTE, and it is 0 — "no filter". Filters exist to
    // make the row compress better, and nothing here compresses.
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0);
        let from = row * width as usize * 4;
        raw.extend_from_slice(&rgba[from..from + width as usize * 4]);
    }

    // zlib: two header bytes, stored deflate blocks of at most 65 535 bytes,
    // and Adler-32 over the uncompressed data.
    let mut z = vec![0x78, 0x01];
    for (n, block) in raw.chunks(0xFFFF).enumerate() {
        let last = u8::from((n + 1) * 0xFFFF >= raw.len());
        let len = u16::try_from(block.len()).unwrap_or(u16::MAX);
        z.push(last);
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut head = Vec::with_capacity(13);
    head.extend_from_slice(&width.to_be_bytes());
    head.extend_from_slice(&height.to_be_bytes());
    // 8 bits per channel, colour type 6 (RGBA), deflate, no filter, no
    // interlacing.
    head.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &head);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, png).with_context(|| format!("could not write {}", path.display()))
}
