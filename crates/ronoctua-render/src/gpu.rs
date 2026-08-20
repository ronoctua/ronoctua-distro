//! wgpu ↔ Wayland surface bridge — section 7.7.
//!
//! It isolates everything wgpu in one place, so `ronoctua-shell` can take care of
//! Wayland and the rest of the renderer can take care of text.
//!
//! # About the `unsafe`
//!
//! Creating a `wgpu::Surface` from raw Wayland handles requires
//! `unsafe`: wgpu has no way to prove the pointers outlive the
//! surface. This is the **only** point in the renderer that needs it, and that is
//! why the crate's `#![forbid(unsafe_code)]` makes an exception only here, with the
//! contract written out.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;

/// The limits we ask the device for, given the adapter that answered.
///
/// # Why it is not plain `downlevel_defaults()`
///
/// It fixes `max_texture_dimension_2d` at **2048**, and the presentation surface
/// IS a texture. With a 2048 ceiling, a 2112 px window takes the
/// `configure` down — and `Surface::configure` does not return an error: it KILLS the
/// process. The symptom was the window disappearing on its own when grown one step past the
/// screen (`Alt+=` on a 1920 monitor reaches 2112 on the first step), without a
/// single log line anywhere, because niri sends the output of what it spawns
/// to `/dev/null`.
///
/// The same ceiling broke the ENTIRE distro on any monitor larger than
/// 2048 px: the footer takes the screen's width, so on a 2560 or a 4K it
/// would die on the first configuration, before drawing the first pixel. It was
/// a compatibility limit turned into a supported-hardware limit.
///
/// `using_resolution` swaps **only** the three texture dimensions for the
/// adapter's and keeps everything else downlevel — which is exactly what one wants:
/// not requiring a modern GPU to draw a glyph and a quad, without inventing a resolution
/// ceiling the hardware does not have.
#[must_use]
pub fn limits_of(adapter: &wgpu::Adapter) -> wgpu::Limits {
    wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
}

/// Clamps a surface dimension to what the GPU accepts.
///
/// # Why it exists, if the ceiling now comes from the adapter
///
/// Because `Surface::configure` **does not return an error**: it returns `()`, and a
/// dimension above the limit becomes a fatal validation error — the process dies.
/// In a window that appears as the window closing on its own; in the footer, as the
/// distro not coming up. In both cases with no message, because niri sends the
/// output of what it spawns to `/dev/null`.
///
/// Asking for the adapter's resolution (see `new`) already removes the 2048 ceiling that
/// caused this in practice. This function is the net for what is left: a window
/// dragged beyond what the GPU can take gets its content CLIPPED, which is
/// visibly wrong but recoverable, instead of taking the process down.
///
/// The `max(1)` is the other end: a zero-dimension surface is also invalid,
/// and a compositor can send zero while the window is minimised.
fn fits(v: u32, ceiling: u32, eixo: &'static str) -> u32 {
    if v > ceiling {
        tracing::warn!(
            eixo,
            pedido = v,
            ceiling,
            "a superficie passou do que a GPU aceita; o conteudo vai sair cortado"
        );
    }
    v.clamp(1, ceiling)
}

/// A graphics context bound to a surface.
pub struct Gpu {
    /// The device.
    pub device: wgpu::Device,
    /// The command queue.
    pub queue: wgpu::Queue,
    /// The presentation surface.
    pub surface: wgpu::Surface<'static>,
    /// The current configuration (size, format).
    pub config: wgpu::SurfaceConfiguration,
    /// The largest texture dimension this device accepts. See [`fits`].
    ceiling: u32,
    /// Frames skipped in a row. See `frame`.
    skipped: u32,
    /// A pending measurement: what to time, and since when.
    ///
    /// Consumed on the next PRESENTED frame. See [`Gpu::mark_as`].
    milestone: Option<(&'static str, std::time::Instant)>,
}

impl Gpu {
    /// Times until the next presented frame.
    ///
    /// # What this number is, and what it is NOT
    ///
    /// It stops at `Queue::present`, which on Wayland does `attach` + `commit`.
    /// That is the instant the frame leaves OUR hands — not the instant
    /// it becomes light. What is missing is the compositor compositing and the panel scanning out,
    /// which at 60 Hz adds up to another 16.7 ms that do not depend on us.
    ///
    /// Measuring here is the honest choice: it is the boundary of what our code
    /// controls. A "to the pixel" number would require Wayland's
    /// `presentation-time` and would still measure the compositor along with us.
    ///
    /// An earlier unmeasured mark is OVERWRITTEN on purpose: if the leader
    /// opens twice before a frame comes out, what matters is the last one.
    pub fn mark_as(&mut self, name: &'static str, desde: std::time::Instant) {
        self.milestone = Some((name, desde));
    }

    /// Builds the context over handles already validated by the caller.
    ///
    /// # Safety
    ///
    /// The caller guarantees that `display` and `window` stay valid
    /// for as long as this `Gpu` lives. In practice that holds because
    /// `ronoctua-shell` keeps the `Connection` and the `LayerSurface` for the same
    /// time as the `Gpu` — if one day they stop being kept together, this
    /// becomes use after free.
    #[allow(unsafe_code)]
    pub unsafe fn new(
        display: raw_window_handle_shim::RawDisplayHandle,
        window: raw_window_handle_shim::RawWindowHandle,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        // wgpu 30's `InstanceDescriptor` does not implement `Default` (it has a
        // field `display: Option<Box<dyn ...>>` that could not be
        // derived) and `Instance::new` takes it BY VALUE. The constructor is
        // `new_without_display_handle`.
        // STEP BY STEP: without this, "the graphics context took 748 ms" does not say
        // whether the cost is wgpu's, the driver's or waiting for the compositor. Each
        // stage here is a different optimisation candidate.
        let mut t = std::time::Instant::now();
        let step = |q: &'static str, t: &mut std::time::Instant| {
            tracing::info!(fase = q, ms = t.elapsed().as_secs_f64() * 1000.0, "partida");
            *t = std::time::Instant::now();
        };

        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        // VULKAN FIRST, GL only if there is no adapter.
        //
        // Asking for both at once makes wgpu initialise the GL backend —
        // opening `libGL`/EGL and creating a context — only to find out that
        // Vulkan will do. Measured in the footer inside the nested niri: 780 ms
        // for `Instance::new`, against 5.5 ms for `request_adapter` and 4.4 ms
        // for `request_device`. The cost was in ENUMERATING, not in talking to
        // the GPU.
        //
        // The net still exists: if Vulkan returns no adapter
        // (a machine with no ICD), the second attempt turns GL on. On a machine with
        // Vulkan, which is the case of the tests' lavapipe, it never runs.
        desc.backends = wgpu::Backends::VULKAN;

        // Instance, surface and adapter move together: switching backend
        // forces redoing all three, because the surface belongs to the instance.
        let tentar = |backends: wgpu::Backends| {
            let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
            d.backends = backends;
            let instance = wgpu::Instance::new(d);
            // SAFETY: the function's contract — the handles live as long as the Gpu does.
            let surface = unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    // An `Option` in wgpu 30: there are targets with no display handle.
                    raw_display_handle: Some(display),
                    raw_window_handle: window,
                })
            }
            .ok()?;
            let adapter =
                pollster_shim::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                    ..Default::default()
                }))
                // wgpu 30's `request_adapter` returns a `Result`, not an
                // `Option`: the error says WHY there was no adapter. Here the
                // reason does not matter — if Vulkan will not do, we try GL.
                .ok()?;
            Some((instance, surface, adapter))
        };

        let (_instance, surface, adapter) = match tentar(wgpu::Backends::VULKAN) {
            Some(v) => v,
            None => {
                tracing::warn!("sem adaptador Vulkan; tentando o backend GL");
                tentar(wgpu::Backends::GL).context(
                    "nenhum adaptador grafico serve, nem por Vulkan nem por GL.\n\
                     \n\
                     O rodape desenha com wgpu sobre Vulkan (secao 7.7). Num\n\
                     ambiente sem GPU, o lavapipe do Mesa funciona — e o que\n\
                     os testes usam.",
                )?
            }
        };
        step("instancia-superficie-adaptador", &mut t);

        let limits = limits_of(&adapter);
        let ceiling = limits.max_texture_dimension_2d;

        let (device, queue) =
            pollster_shim::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("ronoctua"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            }))
            .context("o adaptador nao entregou dispositivo")?;
        step("request-device", &mut t);

        let caps = surface.get_capabilities(&adapter);
        // The capabilities go into the log because a wrong choice here does not give an
        // error: the surface is created, the frames are presented, and the
        // compositor simply shows nothing.
        tracing::debug!(
            formatos = ?caps.formats,
            alpha = ?caps.alpha_modes,
            present = ?caps.present_modes,
            "capacidades da superficie"
        );
        let format_of = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: format_of,
            width: fits(width, ceiling, "largura"),
            height: fits(height, ceiling, "altura"),
            // `Fifo` = vsync. The footer does not animate; drawing more than the
            // refresh rate would only burn battery (section 2 targets laptops).
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            // ONE FRAME IN FLIGHT, AND NOT TWO.
            //
            // The deeper queue exists so that an animation keeps the GPU fed
            // while the compositor is still showing the previous frame — it
            // buys THROUGHPUT with LATENCY. Nothing here animates: every frame
            // this surface draws is the answer to something that just happened,
            // and the only thing that matters about it is how soon it is seen.
            //
            // With two, the frame a pointer movement produced could sit behind
            // one already queued — a whole refresh of delay, ~16 ms at 60 Hz,
            // added to a gesture whose whole point is following the hand.
            desired_maximum_frame_latency: 1,
            // A new field in wgpu 30, with no `Default` on the struct. `Srgb` matches
            // the sRGB format we chose above.
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        surface.configure(&device, &config);
        step("configure", &mut t);

        tracing::info!(
            adapter = ?adapter.get_info().name,
            backend = ?adapter.get_info().backend,
            format_of = ?format_of,
            "contexto grafico pronto"
        );

        Ok(Self {
            device,
            queue,
            surface,
            config,
            ceiling,
            skipped: 0,
            milestone: None,
        })
    }

    /// Reconfigures after a resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = fits(width, self.ceiling, "largura");
        self.config.height = fits(height, self.ceiling, "altura");
        self.surface.configure(&self.device, &self.config);
    }

    /// Draws a frame: clears with the chrome colour and lets `draw`
    /// paint on top.
    ///
    /// The clear happens EVEN when there is no text — otherwise the compositor
    /// shows rubbish on the first frame.
    ///
    /// `draw` receives the render pass already open. Passing a closure instead
    /// of returning the pass avoids having to expose wgpu's lifetimes
    /// to `ronoctua-shell`, which should not need to know them.
    pub fn frame<F>(&mut self, draw: F) -> Result<()>
    where
        F: FnOnce(&mut wgpu::RenderPass<'_>) -> Result<()>,
    {
        // wgpu 30's `get_current_texture` returns an ENUM, not a
        // `Result`. Handling each case matters: `Timeout` and `Occluded` are
        // normal (the surface is hidden, for example) and skipping the frame
        // is right; `Outdated` and `Lost` call for reconfiguration. Turning
        // everything into an error would make the footer die when covered by a window.
        // Skipping a frame is NOT silent. A `Timeout` on every round is
        // indistinguishable from "all fine" by the return value, and the symptom
        // is a footer that never appears with not a single error anywhere —
        // exactly what happened when this was wired up in a actual compositor.
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            otherwise @ (wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded) => {
                // Normal when the surface is hidden; abnormal when it
                // repeats. The count is what separates the two cases.
                self.skipped += 1;
                tracing::debug!(reason = ?otherwise, skipped = self.skipped, "frame pulado");
                return Ok(());
            }
            otherwise @ (wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost) => {
                tracing::debug!(reason = ?otherwise, "superficie desatualizada; reconfigurando");
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                anyhow::bail!("a superficie wgpu ficou invalida");
            }
        };
        self.skipped = 0;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rodape"),
            });

        {
            let [r, g, b, a] = tokens::color::SURFACE_CHROME.to_linear();
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("limpar"),
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

        // PRESENTING is `Queue::present`, not dropping the `SurfaceTexture`.
        //
        // In wgpu 30 the `Drop` of an unpresented `SurfaceTexture` calls
        // `texture_discard()`: the frame is DISCARDED, silently, with no error and
        // no return value. From `Queue::present`'s documentation: "on
        // Wayland, present will attach a wl_buffer to the underlying
        // wl_surface and commit". Without it no `attach` goes out — the compositor
        // reserves the exclusive zone, lists our layer, and draws its own
        // background in the footer's place.
        //
        // This cost an afternoon: `get_current_texture` worked, the drawing
        // gave no error, and the footer simply did not exist on screen. What
        // pointed at the cause was `WAYLAND_DEBUG=client`, where
        // `wl_shm_pool.create_buffer` and `wl_surface.commit` appeared but no
        // `wl_surface.attach`.
        self.queue.present(frame);

        // After the `present`, and not before: it is what delivers the frame.
        if let Some((name, desde)) = self.milestone.take() {
            tracing::info!(
                milestone = name,
                ms = desde.elapsed().as_secs_f64() * 1000.0,
                "alvo da 5.2 medido (ate `present`, sem compositor nem varredura)"
            );
        }

        tracing::trace!(
            width = self.config.width,
            height = self.config.height,
            "frame apresentado"
        );
        Ok(())
    }
}

/// Re-exports the handle types, so the shell does not need the crate directly.
pub mod raw_window_handle_shim {
    pub use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
}

/// `block_on` without dragging in a whole async runtime.
mod pollster_shim {
    pub use pollster::block_on;
}
