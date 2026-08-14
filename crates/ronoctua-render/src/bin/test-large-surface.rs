//! Proves that the device accepts a surface the size of a monitor.
//!
//! # The defect this catches
//!
//! `Limits::downlevel_defaults()` fixes `max_texture_dimension_2d` at 2048, and
//! the presentation surface is a texture. With that ceiling:
//!
//! - `Alt+=` on a 1920 monitor takes the column to 2112 px on the FIRST step,
//!   and the window closes by itself — `Surface::configure` gives back no
//!   error, it kills the process;
//! - any monitor wider than 2048 px never had a footer, because it takes the
//!   screen's width and dies before the first pixel.
//!
//! # Why this binary, and not a unit test
//!
//! Because the criterion is "the DEVICE accepts", and a device is a runtime
//! thing. A test that compared numbers from `Limits` would be checking our
//! arithmetic against itself; this one asks for a actual adapter (lavapipe, in
//! the checks), asks for the device with the production limits, and tries to
//! create the texture. If someone gives back the pure `downlevel_defaults()`,
//! it fails here — which is the only cheap place before the owner's screen.
//!
//! # Why it does not configure a actual surface
//!
//! That would require a window, and a window requires a compositor. The
//! validation is the SAME: the `configure` checks `width`/`height` against
//! `max_texture_dimension_2d`, the same limit texture creation checks. What is
//! lost is the Wayland end, which the `footer-vm` already covers.

use anyhow::{Context as _, Result, bail};

/// The biggest monitor the distro commits to serving.
///
/// 4K is today's honest ceiling: it is what one buys, and the
/// `downlevel_defaults` failed even a common 2560. Raising this without
/// hardware to check it would be trading one made-up limit for another.
const WIDTH: u32 = 3840;
/// The height of the same monitor. It counts as much as the width: a 4K on
/// its side (2160 wide) already goes past the 2048 ceiling on the other axis.
const HEIGHT: u32 = 2160;

fn main() -> Result<()> {
    // The same two backends as production, in the same order. Asking only for
    // Vulkan would make the test fail for environment reasons on a machine with
    // no ICD, and the message would blame the limits.
    let mut adapter = None;
    for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        d.backends = backends;
        let instance = wgpu::Instance::new(d);
        if let Ok(a) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            ..Default::default()
        })) {
            adapter = Some(a);
            break;
        }
    }
    let adapter = adapter.context(
        "nenhum adaptador grafico, nem Vulkan nem GL.\n\
         \n\
         Nos checks o adaptador e o lavapipe, posto por VK_DRIVER_FILES.\n\
         Sem ele este teste nao consegue afirmar nada, e passar em silencio\n\
         seria pior do que falhar: o teto de 2048 voltaria sem ninguem ver.",
    )?;

    let limits = ronoctua_render::gpu::limits_of(&adapter);
    let ceiling = limits.max_texture_dimension_2d;
    println!(
        "adaptador: {:?} ({:?}); teto de textura 2D: {ceiling}",
        adapter.get_info().name,
        adapter.get_info().backend,
    );

    if ceiling < WIDTH || ceiling < HEIGHT {
        bail!(
            "o teto de textura 2D e {ceiling}, e um monitor de {WIDTH}x{HEIGHT} nao cabe.\n\
             \n\
             O adaptador informa {}. Se ele informa mais do que isto, quem\n\
             baixou o teto foi `limites_do` — provavelmente um\n\
             `downlevel_defaults()` sem `using_resolution`.",
            adapter.limits().max_texture_dimension_2d,
        );
    }

    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("test-large-surface"),
        required_features: wgpu::Features::empty(),
        required_limits: limits,
        ..Default::default()
    }))
    .context("o adaptador nao entregou dispositivo com os limites de producao")?;

    // AN ERROR SCOPE, and not a `catch_unwind`: a wgpu validation error goes to
    // the innermost scope instead of bringing the process down. Without it, the
    // regression would show up as "the test died", without saying in which
    // dimension. In wgpu 30 the `push_error_scope` gives back a GUARD, and it is
    // the guard that has the `pop()`; the `Device` no longer has `pop_error_scope`.
    let escopo = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("do tamanho de um monitor"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // The same usage as the presentation surface — and the validation that
        // matters is the dimension one, which does not depend on this.
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    if let Some(e) = pollster::block_on(escopo.pop()) {
        bail!("uma textura de {WIDTH}x{HEIGHT} foi recusada: {e}");
    }
    drop(texture);

    println!("uma superficie de {WIDTH}x{HEIGHT} cabe nos limites de producao.");
    Ok(())
}
