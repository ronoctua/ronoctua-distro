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

/// The limits we ask the device for, given the adapter that answered.
///
/// # Why it is not plain `downlevel_defaults()`
///
/// It fixes `max_texture_dimension_2d` at **2048**, and the presentation surface
/// IS a texture. With a 2048 ceiling, a 2112 px window takes the
/// `configure` down — and `Surface::configure` does not return an error: it KILLS the
/// process. The symptom was the window disappearing on its own when grown one step past the
/// screen (`Alt+=` on a 1920 monitor reaches 2112 on the first step), without a
/// single log line where anybody was looking: the footer is a unit, so what it
/// says on its way out lands in the journal of a service nobody opens while a
/// window is vanishing.
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
/// distro not coming up. In both cases with no message where anybody was
/// looking, for the reason above.
///
/// Asking for the adapter's resolution (see `new`) already removes the 2048 ceiling that
/// caused this in practice. This function is the net for what is left: a window
/// dragged beyond what the GPU can take gets its content CLIPPED, which is
/// visibly wrong but recoverable, instead of taking the process down.
///
/// The `max(1)` is the other end: a zero-dimension surface is also invalid,
/// and a compositor can send zero while the window is minimised.
fn fits(v: u32, ceiling: u32, axis: &'static str) -> u32 {
    if v > ceiling {
        tracing::warn!(
            axis,
            request = v,
            ceiling,
            "the surface went past what the GPU takes; the content will come out cut"
        );
    }
    v.clamp(1, ceiling)
}

/// The alpha mode a surface is configured with, out of what it offers.
///
/// # Why the footer asks and the windows do not
///
/// The footer's two hidden modes lay it over the windows on a veil the
/// window shows through, and leave one pixel of nothing at the screen's edge
/// (`surface.veil`, `surface.clear`). A surface the compositor composes as
/// OPAQUE shows its clear colour whatever alpha it carries — the hole comes
/// out as a strip of chrome. So the footer asks to be see-through and gets the PREMULTIPLIED
/// mode when the surface offers it; every other door asks nothing and keeps
/// the first mode offered, which is what all of them got before and is
/// opaque in practice.
///
/// # And when the driver does not offer it
///
/// The first mode, as an opaque surface would have, and the caller is told by
/// [`Gpu::transparent`] answering `false` — the footer says so on the screen
/// once, rather than looking as if hiding did nothing.
#[must_use]
pub fn alpha_mode_for(
    offered: &[wgpu::CompositeAlphaMode],
    transparent: bool,
) -> wgpu::CompositeAlphaMode {
    if transparent && offered.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
        return wgpu::CompositeAlphaMode::PreMultiplied;
    }
    offered
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
}

/// The clear of a frame, in the surface's own alpha convention.
///
/// # Why premultiplied, and only there
///
/// A premultiplied surface is composed as `ours + behind × (1 − α)`. A token
/// is a colour AND an alpha — `surface.clear` is the chrome at alpha 0 — and
/// cleared as it is written it would be the chrome ADDED to whatever window
/// is behind: a glow where a hole was meant. Multiplied through, alpha 0 is
/// nothing at all and the veil's four fifths is four fifths.
///
/// An opaque surface ignores the alpha, and there the colour is what shows:
/// kept as it is, the clear of a footer that could not be transparent is
/// the chrome, which is what it painted before any of this existed.
///
/// # What draws on top of it, measured in the pinned sources
///
/// The quads and the pictures blend with `BlendState::ALPHA_BLENDING`
/// (`quad.rs`, `image.rs`), and so does glyphon 0.12's text pipeline
/// (`cache.rs:235`, a straight colour out of `shader.wgsl`'s `fs_main`). Its
/// colour half is `src × αs + dst × (1 − αs)` and its alpha half is
/// `αs + αd × (1 − αs)`: over a premultiplied destination that is the
/// premultiplied `over` — IN LINEAR VALUES.
///
/// The target is sRGB, and that is where it stops being exact. The blend
/// runs on linear values and the store encodes them, so a partial alpha is
/// stored as `encode(c × α)`, where a compositor blending ENCODED values
/// (wlroots' GLES2 and pixman renderers) wants `encode(c) × α`. At alpha 0
/// and 1 the two agree. In between:
///
/// - ON THE VEIL, the veil comes out a little lighter than its number says.
///   Its alpha is a recipe in `colors.nix`, tuned by eye, so the bias is
///   inside what the tuning already absorbs.
/// - ON NOTHING (a clear of alpha 0), a letter is NOT validly
///   premultiplied: at a glyph's edge the stored colour exceeds its alpha,
///   and over anything but black it comes out as a white fringe round the
///   glyph. So nothing is lettered on nothing: the rows of a hidden footer
///   that push stand on the desk's colour, OPAQUE, and the rows that lie
///   over the windows on the veil (`ronoctua-shell`'s
///   `hidden::Shape::grounds`). Alpha 0 is left only where no letter is —
///   the one-pixel edge a hidden footer keeps.
///
///   They stood on nothing until 2026-09-28, the desk the windows moved off
///   showing through, and the fringe was the price; the owner asked for the
///   desk painted instead — the same look, opaque.
#[must_use]
pub fn clear_of(colour: [f32; 4], mode: wgpu::CompositeAlphaMode) -> wgpu::Color {
    let [r, g, b, a] = colour;
    let k = if mode == wgpu::CompositeAlphaMode::PreMultiplied {
        a
    } else {
        1.0
    };
    wgpu::Color {
        r: f64::from(r * k),
        g: f64::from(g * k),
        b: f64::from(b * k),
        a: f64::from(a),
    }
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
    pub fn mark_as(&mut self, name: &'static str, since: std::time::Instant) {
        self.milestone = Some((name, since));
    }

    /// Builds the context over handles already validated by the caller.
    ///
    /// `transparent` asks for a surface the compositor composes with its
    /// alpha — see [`alpha_mode_for`]; whether it was granted is
    /// [`Self::transparent`].
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
        transparent: bool,
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
            tracing::info!(
                stage = q,
                ms = t.elapsed().as_secs_f64() * 1000.0,
                "start-up"
            );
            *t = std::time::Instant::now();
        };

        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        // VULKAN FIRST, GL only if there is no adapter.
        //
        // Asking for both at once makes wgpu initialise the GL backend —
        // opening `libGL`/EGL and creating a context — only to find out that
        // Vulkan will do. Measured in the footer inside the nested session: 780 ms
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
        let attempt = |backends: wgpu::Backends| {
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

        let (_instance, surface, adapter) = match attempt(wgpu::Backends::VULKAN) {
            Some(v) => v,
            None => {
                tracing::warn!("no Vulkan adapter; trying the GL backend");
                attempt(wgpu::Backends::GL).context(
                    "no graphics adapter will do, neither by Vulkan nor by GL.\n\
                     \n\
                     The footer draws with wgpu over Vulkan (section 7.7). On a\n\
                     machine with no GPU, Mesa's lavapipe works — it is what\n\
                     the tests use.",
                )?
            }
        };
        step("instance-surface-adapter", &mut t);

        let limits = limits_of(&adapter);
        let ceiling = limits.max_texture_dimension_2d;

        let (device, queue) =
            pollster_shim::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("ronoctua"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            }))
            .context("the adapter handed over no device")?;
        step("request-device", &mut t);

        let caps = surface.get_capabilities(&adapter);
        // The capabilities go into the log because a wrong choice here does not give an
        // error: the surface is created, the frames are presented, and the
        // compositor simply shows nothing.
        tracing::debug!(
            formats = ?caps.formats,
            alpha = ?caps.alpha_modes,
            present = ?caps.present_modes,
            "the surface's capabilities"
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
            width: fits(width, ceiling, "width"),
            height: fits(height, ceiling, "height"),
            // `Fifo` = vsync. The footer does not animate; drawing more than the
            // refresh rate would only burn battery (section 2 targets laptops).
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: alpha_mode_for(&caps.alpha_modes, transparent),
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
        // ASKED AND REFUSED is said, once, where it was decided: the caller
        // shows it on the screen, and this is what a journal reader finds.
        if transparent && config.alpha_mode != wgpu::CompositeAlphaMode::PreMultiplied {
            tracing::warn!(
                offered = ?caps.alpha_modes,
                "the surface cannot be see-through here; it stays opaque"
            );
        }

        tracing::info!(
            adapter = ?adapter.get_info().name,
            backend = ?adapter.get_info().backend,
            format_of = ?format_of,
            "graphics context ready"
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

    /// The largest dimension this device takes for a texture — and the
    /// presentation surface IS a texture.
    ///
    /// # Why a caller wants it before asking for a size
    ///
    /// Because [`resize`](Self::resize) clamps, and the clamp is SILENT to
    /// whoever asked: the surface comes back smaller than the number handed in
    /// and nothing in the return value says so. A caller that has to keep the
    /// buffer divisible by a scale (`crate::gpu` does not; `ronoctua-shell`'s
    /// `hidpi` does) cannot compute the size it wants without this number, and
    /// asking for one above it and then reading `config` back is two
    /// reconfigurations for one size.
    #[must_use]
    pub const fn ceiling(&self) -> u32 {
        self.ceiling
    }

    /// Whether the compositor composes this surface with its alpha — asked
    /// for at [`Self::new`], and granted only where the surface offers the
    /// premultiplied mode.
    #[must_use]
    pub fn transparent(&self) -> bool {
        self.config.alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied
    }

    /// Reconfigures after a resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = fits(width, self.ceiling, "width");
        self.config.height = fits(height, self.ceiling, "height");
        self.surface.configure(&self.device, &self.config);
    }

    /// Draws a frame, and PRESENTS it — which on Wayland is the attach and the
    /// commit. It clears with `clear` (a token's `to_linear()`, as the
    /// offscreen twin takes it) and lets `draw` paint on top.
    ///
    /// THE CLEAR IS THE CALLER'S, and it was a constant: every door cleared
    /// with the chrome. Now a door clears with the ground the person chose
    /// for it, and the footer's hidden modes clear with nothing
    /// (`surface.clear`) and lay their grounds over it; the colour is
    /// multiplied through by its alpha on a premultiplied surface — see
    /// [`clear_of`].
    ///
    /// The clear happens EVEN when there is no text — otherwise the compositor
    /// shows rubbish on the first frame.
    ///
    /// `draw` receives the render pass already open. Passing a closure instead
    /// of returning the pass avoids having to expose wgpu's lifetimes
    /// to `ronoctua-shell`, which should not need to know them.
    /// # Why there is a hook before the present
    ///
    /// Because `present` is the only commit this door makes that carries a
    /// buffer, and some surface state has to travel in exactly that commit.
    /// `wl_surface.set_buffer_scale` is the case it was written for: it is
    /// double-buffered, so it applies on the NEXT commit whichever that is —
    /// and this function returns `Ok(())` without presenting on `Timeout`,
    /// `Occluded`, `Outdated` and `Lost`. Sent outside the frame, the scale
    /// could sit pending until some other bare `commit` applied it over the
    /// buffer that was already attached, whose size is then not a multiple of
    /// it: `wl_surface.error.invalid_size`, which is the compositor closing
    /// the connection with no message at all.
    ///
    /// So the hook runs after the texture is in hand and the drawing is
    /// submitted, and never on a frame that is skipped.
    pub fn frame<B, F>(&mut self, clear: [f32; 4], before_present: B, draw: F) -> Result<()>
    where
        B: FnOnce(),
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
                tracing::debug!(reason = ?otherwise, skipped = self.skipped, "frame skipped");
                return Ok(());
            }
            otherwise @ (wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost) => {
                tracing::debug!(reason = ?otherwise, "the surface is out of date; reconfiguring");
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                anyhow::bail!("the wgpu surface went invalid");
            }
        };
        self.skipped = 0;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("footer"),
            });

        {
            let cleared = clear_of(clear, self.config.alpha_mode);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(cleared),
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

        // IN THE SAME COMMIT AS THE BUFFER — see the doc. This is the last
        // moment at which a request can be certain of travelling with it.
        before_present();

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
        if let Some((name, since)) = self.milestone.take() {
            tracing::info!(
                milestone = name,
                ms = since.elapsed().as_secs_f64() * 1000.0,
                "5.2's target measured (up to `present`, no compositor and no scan-out)"
            );
        }

        tracing::trace!(
            width = self.config.width,
            height = self.config.height,
            "frame presented"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{alpha_mode_for, clear_of};
    use wgpu::CompositeAlphaMode as Mode;

    /// FAILS IF: a surface that asked to be see-through is handed an opaque
    /// mode while the compositor offers the premultiplied one, or a surface
    /// that did not ask stops getting what it got before (the first offered).
    #[test]
    fn a_transparent_surface_takes_premultiplied_when_offered() {
        let offered = [Mode::Opaque, Mode::PreMultiplied];
        assert_eq!(alpha_mode_for(&offered, true), Mode::PreMultiplied);
        assert_eq!(alpha_mode_for(&offered, false), Mode::Opaque);
        // THE FIRST OFFERED, as before, for everything that stays opaque —
        // whatever order the driver lists them in.
        assert_eq!(
            alpha_mode_for(&[Mode::PreMultiplied, Mode::Opaque], false),
            Mode::PreMultiplied
        );
    }

    /// FAILS IF: a driver without the premultiplied mode makes the surface
    /// pick something other than what an opaque one would, or panics on an
    /// empty list.
    #[test]
    fn without_premultiplied_the_surface_stays_what_it_was() {
        assert_eq!(alpha_mode_for(&[Mode::Opaque], true), Mode::Opaque);
        assert_eq!(
            alpha_mode_for(&[Mode::Opaque, Mode::PostMultiplied], true),
            Mode::Opaque
        );
        assert_eq!(alpha_mode_for(&[], true), Mode::Opaque);
    }

    /// FAILS IF: the clear of a premultiplied surface keeps its colour at
    /// alpha 0 — which the compositor ADDS to what is behind (a glow, not a
    /// hole) — or an opaque surface loses the colour its alpha would scale.
    #[test]
    fn the_clear_is_premultiplied_only_where_the_surface_is() {
        let chrome_at_zero = [0.2, 0.4, 0.6, 0.0];
        let c = clear_of(chrome_at_zero, Mode::PreMultiplied);
        assert_eq!((c.r, c.g, c.b, c.a), (0.0, 0.0, 0.0, 0.0));
        let half = clear_of([0.2, 0.4, 0.8, 0.5], Mode::PreMultiplied);
        assert!((half.r - 0.1).abs() < 1e-6 && (half.b - 0.4).abs() < 1e-6);
        // OPAQUE: the alpha is ignored by the compositor, and the colour is
        // what shows — the chrome, which is what an opaque fallback draws.
        let o = clear_of(chrome_at_zero, Mode::Opaque);
        assert!((o.g - 0.4).abs() < 1e-6);
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
