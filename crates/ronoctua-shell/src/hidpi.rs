//! How a door follows the scale of the screen it is on.
//!
//! # Why this is one module and not ten copies
//!
//! Because it is one rule with three parts that have to agree, and ten
//! places to write it are ten places to get one part wrong. The rule is the
//! protocol's own: a surface says what its buffer scale is, the buffer has to
//! be exactly the surface's logical size times that number, and the glyphs in
//! it have to be rasterised at that number too. Any one of the three left
//! behind and the compositor either refuses the buffer or stretches it.
//!
//! # What was wrong before it existed
//!
//! `scale_factor_changed` was EMPTY in all nine doors of the day — the
//! footer, the terminal, the buffer host and the six view windows; the
//! screens' own window came after, and was born calling this, which is what
//! makes the count ten now and nine then. Nothing called `set_buffer_scale`,
//! so on a screen the compositor drives at scale 2 every door handed over a
//! buffer of logical size and the compositor stretched it over twice as many
//! pixels.
//!
//! MEASURED, on a niri output configured `scale 2` under Xvfb, over the power
//! window's own column (200x700 px at +30+40), the same picture with and
//! without the fix:
//!
//! | | stretched | drawn at the scale |
//! |---|---|---|
//! | pixels lit at all (grey >= 8%) | 20.19% | 11.35% |
//! | pixels at full ink (grey >= 60%) | 1.27% | 2.63% |
//! | halo, as a share of the lit ones | 93.7% | 76.9% |
//!
//! The same letters: half as many pixels touched, twice as many of them
//! actually inside a stroke. The share is what the eye reads as blur.
//!
//! Nothing on the screen said so. The columns were right, the layout was
//! right, the sizes were right; only the letters were soft, which reads as a
//! bad font rather than as a setting nobody wrote.
//!
//! # A door keeps TWO logical sizes, and they are different questions
//!
//! `asked` is what the compositor configured. `width`/`height` are what the
//! buffer can show. They are the same number on every machine anybody has —
//! and they come apart exactly where the GPU's ceiling bites, which is the
//! case [`resized`] exists for. One number cannot answer both: the size to
//! ask the GPU for is a function of what the COMPOSITOR said, and the size to
//! lay a picture out in is a function of what the BUFFER holds. Feeding a
//! door's kept size back in is a ratchet — a window clamped once at scale 3
//! could never grow back at scale 1, because the number it would ask for is
//! the number it was cut down to.
//!
//! # Everything else stays LOGICAL
//!
//! A `Span` is a column and a line. A `Quad` is a rectangle over the window's
//! own pixels. A pointer arrives in logical coordinates. `metrics::columns` is
//! logical width over logical cell, and it does not move when the scale does —
//! which is the point: the same screen shows the same number of columns
//! whether it is sharp or not.
//!
//! Only two things are physical, and both are here: the surface the GPU draws
//! into, and the size the rasteriser draws a glyph at.
//!
//! # What the clamp costs, and it is the pointer
//!
//! A pointer arrives in the COMPOSITOR's logical space, which is `asked`,
//! while the picture is laid out in `shown`. Where the two differ — only at
//! the GPU's texture ceiling — the rightmost `asked - shown` logical pixels of
//! a door hit-test to columns that were never drawn. Nothing is drawn there
//! either, so what a click in that strip reaches is a row's empty tail; it is
//! named here because a reader working out the clamped case will ask.

use wayland_client::Proxy as _;
use wayland_client::protocol::wl_surface;

/// The scale as a divisor — one conversion, and it was written four times in
/// the module whose thesis is that a rule written twice is a rule half-wrong.
///
/// FLOORED AT ONE. A compositor can say zero while a window is mapping, and
/// dividing by it is not an answer anybody wants.
fn by_of(scale: i32) -> u32 {
    u32::try_from(scale.max(1)).unwrap_or(1)
}

/// The buffer's size for a window of this logical size at this scale.
///
/// SATURATING, so a window too big to double comes out at the ceiling rather
/// than wrapping to something small — a small buffer for a big window is one
/// drawn into a corner with the rest never painted.
///
/// AND THE SCALE IS FLOORED AT ONE, which the SIZE is not: a compositor can
/// configure a window at zero while it is mapping, and `(0, 0)` is the honest
/// answer for that. What each caller does with a zero belongs to it —
/// [`resized`] keeps the dimension it had, `Gpu::new` clamps to at least one —
/// and neither is this function's business.
#[must_use]
pub fn physical(logical: (u32, u32), scale: i32) -> (u32, u32) {
    let by = by_of(scale);
    (logical.0.saturating_mul(by), logical.1.saturating_mul(by))
}

/// The largest size no bigger than this one that the scale divides exactly.
///
/// `wl_surface.error.invalid_size` is "buffer size is not divisible by scale",
/// so a buffer the scale does not divide is a silent connection close: 16384
/// is not a multiple of 3.
///
/// NEVER BELOW ONE WHOLE LOGICAL PIXEL. A buffer of no pixels is not a buffer,
/// and [`shown_of`] would answer zero columns for it — so the floor is `by`
/// and not zero. That floor can only ever GROW a size, and only from a size
/// below one logical pixel, which is why the caller says "sized to a multiple"
/// rather than "cut".
fn cut_to(size: (u32, u32), by: u32) -> (u32, u32) {
    ((size.0 / by * by).max(by), (size.1 / by * by).max(by))
}

/// The buffer this window wants, with a zero keeping the buffer's own number.
///
/// The zero rule of [`resized`], separated so it can be tested: a compositor
/// answers width 0 for "you choose" and can send it while a window is mapping,
/// and `Gpu::resize` returns on a zero without applying EITHER dimension — so
/// a zero width used to take the new height down with it.
fn wanted_size(logical: (u32, u32), had: (u32, u32), scale: i32) -> (u32, u32) {
    let want = physical(logical, scale);
    (
        if logical.0 == 0 { had.0 } else { want.0 },
        if logical.1 == 0 { had.1 } else { want.1 },
    )
}

/// What a buffer of this size can show, in logical pixels.
///
/// The other half of [`physical`], and exact whenever [`cut_to`] has run — the
/// two together are what let a door lay a picture out in the space it actually
/// draws into.
fn shown_of(config: (u32, u32), by: u32) -> (u32, u32) {
    (config.0 / by, config.1 / by)
}

/// Sizes the buffer for a window the compositor configured at this logical
/// size, and answers what the buffer can actually SHOW, in logical pixels.
///
/// `logical` is the COMPOSITOR's number — the door's `asked`, never its kept
/// `width`/`height`, which is this function's own output; see the module's
/// "two logical sizes".
///
/// # Why it answers instead of only resizing
///
/// Because `Gpu::new` and `Gpu::resize` CLAMP at `max_texture_dimension_2d`
/// (16384 on the owner's machine) and warn — a surface past what the GPU takes
/// has its content cut, which is visibly wrong and recoverable rather than
/// fatal. A door that then goes on counting columns and placing scrollbars
/// against the size the COMPOSITOR gave it is laying a picture out wider than
/// the one it draws into: the text lands against the clamped buffer and the
/// rectangles against the unclamped window, and the two stop being one space.
/// On a 6016-logical output at scale 3 that is 557 columns of layout in a
/// buffer that can show 505, with the scrollbar mapped past the right edge and
/// clipped away.
///
/// There was a `drawn` that divided the clamped buffer back for the RECTANGLE
/// viewport alone — it fixed the half it could see and left the geometry and
/// the column count on the other side, which made the clamped case worse than
/// before it existed. ONE number is the answer: the door keeps what the buffer
/// can show, and everything downstream — `recount`, the bars, the hit tests —
/// is against that.
///
/// # ONE `resize`, and the clamp computed HERE
///
/// The size is clamped and cut BEFORE `Gpu::resize` sees it, rather than
/// asked for and then repaired. Asking first meant the clamped path never
/// settled: the configuration came back at the ceiling, which is not what
/// `physical` says, so every configure resized twice and warned again — for
/// ever, on a window nobody was touching. Now the target is one number, the
/// comparison against it is honest, and a window at rest does nothing.
///
/// # A ZERO KEEPS THE BUFFER'S OWN DIMENSION, per axis
///
/// Layer-shell answers width 0 for "you choose", and a compositor can send it
/// while a window is mapping. `Gpu::resize` returns on a zero without applying
/// EITHER dimension, so a zero width used to take the new height down with it
/// — the expansion's growth never reached the buffer. Kept per axis, the other
/// one gets through.
///
/// WHAT IS KEPT IS PHYSICAL, which matters when the scale is what moved: the
/// dimension comes back through `cut_to` and `shown_of` like any other, so it
/// is the same number of PIXELS at the new scale and therefore half or a third
/// of the logical size it was. That is the honest answer for "the compositor
/// did not say" — a door that has never been told its width has none — and it
/// is why the caller's `asked` must never be built from a zero: the footer
/// keeps the last width the compositor DID say, and the nine others cannot
/// be handed a zero at all (`Option<NonZeroU32>`).
///
/// CALLED ON EVERY CONFIGURE, ON EVERY SCALE CHANGE (through [`followed`]) AND
/// RIGHT AFTER `Gpu::new`: the constructor clamps too, and the first frame is
/// what sends the scale.
#[must_use]
pub fn resized(gpu: &mut ronoctua_render::gpu::Gpu, logical: (u32, u32), scale: i32) -> (u32, u32) {
    let by = by_of(scale);
    let had = (gpu.config.width, gpu.config.height);
    let want = wanted_size(logical, had, scale);
    let ceiling = gpu.ceiling();
    let target = cut_to((want.0.min(ceiling), want.1.min(ceiling)), by);
    if target != had {
        // AND THE WARNING IS ABOUT THE CEILING, which is the only part of this
        // a person can do anything about — the cut to a multiple takes at most
        // one logical pixel off and is this module's ordinary work. `fits` no
        // longer says it for `Gpu::resize`, because the size handed there is
        // already under the ceiling; it still says it once per axis for
        // `Gpu::new`, which a door calls with the unclamped `physical` because
        // there is no `Gpu` to ask for the ceiling yet.
        if want.0 > ceiling || want.1 > ceiling {
            tracing::warn!(
                width = want.0,
                height = want.1,
                buffer_width = target.0,
                buffer_height = target.1,
                ceiling,
                scale,
                "the surface went past what the GPU takes; the content will come out cut"
            );
        }
        gpu.resize(target.0, target.1);
    }
    shown_of((gpu.config.width, gpu.config.height), by)
}

/// Tells the surface its buffer scale, in the commit that carries the buffer.
///
/// # Why this is not in `followed`
///
/// Because `set_buffer_scale` is DOUBLE-BUFFERED: it applies on the next
/// `wl_surface.commit`, whichever that turns out to be. Sent when the
/// compositor's event arrives, it sits pending — and the frame that was meant
/// to carry it is skipped whenever the surface is occluded, timed out, lost or
/// out of date (`Gpu::frame` answers `Ok(())` for all four). The next bare
/// commit then applies the new scale over the buffer that is still attached,
/// whose height is not a multiple of it: `wl_surface.error.invalid_size`, and
/// the compositor closes the connection with no message.
///
/// So it goes in `Gpu::frame`'s hook, which runs only on a frame that presents
/// — and presenting on Wayland is the attach and the commit. `told` is what
/// the surface has been sent, `scale` what the compositor said. They differ
/// until the next frame that PRESENTS — which on a surface being skipped
/// (`Occluded` is the case this hook exists for) can be the whole time a
/// window is covered. That is safe rather than merely tolerable: the old
/// buffer and the old scale still agree, and nothing is sent until a buffer of
/// the new size is going out with it.
///
/// # And that last sentence is MEASURED, not assumed
///
/// The attach and the commit are issued by mesa's Vulkan WSI and not by us —
/// so "presenting is the attach and the commit" is a claim about somebody
/// else's code. Under `WAYLAND_DEBUG=client`, on a niri output configured
/// `scale 2`, the power window's own wire (the `{…}` tag is the event queue's
/// name, which is what `wl_event_queue_set_name` puts there):
///
/// ```text
/// [2566578.059]                            -> wl_surface#5.set_buffer_scale(2)
/// [2566589.084] {mesa vk display queue}    -> wl_surface#5.attach(wl_buffer#48, 0, 0)
/// [2566589.108] {mesa vk display queue}    -> wl_surface#5.damage_buffer(…)
/// [2566589.118] {mesa vk display queue}    -> wl_surface#5.frame(…)
/// [2566589.124] {mesa vk display queue}    -> wl_surface#5.commit()
/// ```
///
/// No commit between the two, which is the whole of what has to be true. It
/// holds because `Queue::present` blocks until the WSI has submitted and
/// nothing else in a door commits while a frame is being drawn.
///
/// WHAT IS NOT PROVED, and is worth naming: a `present` that returns WITHOUT
/// attaching — mesa declining a swapchain that went out of date between the
/// acquire and the present — would leave the request pending for whichever
/// bare commit came next. Twelve commits exist in the shell; ten are the
/// creation of a surface that has no `Gpu` yet and cannot be in this window,
/// so the two that can are `apply_geometry` and the `layer.commit` at the end
/// of the footer's `configure`. The window is between one acquire and its
/// present; the measurement above says nothing about it, and closing it would
/// mean sending the scale from inside the WSI, which is not ours to do.
///
/// AND IT WOULD BE PERMANENT, which is worth saying rather than leaving to be
/// worked out: `told` is advanced BEFORE the request goes out, so a scale that
/// was never actually applied short-circuits every later call and the surface
/// stays at the old one for the life of the process. Advancing it afterwards
/// would not help — `Queue::present` answers `()`, so there is nothing to
/// advance it on.
pub fn told_before_present(scale: i32, told: &mut i32, surface: &wl_surface::WlSurface) {
    if scale <= 0 || scale == *told {
        return;
    }
    *told = scale;
    surface.set_buffer_scale(scale);
}

/// Takes a new scale from the compositor and makes the three parts agree.
///
/// `asked` is the logical size the COMPOSITOR configured this door at, and the
/// answer is what the buffer can show at the new scale — `Some` exactly when
/// something moved, which is when the door has to keep the new size, recount
/// and draw again. `None` means nothing moved and nothing is to be written.
///
/// # Why it answers a size and not a `bool`
///
/// Because it calls [`resized`], and for one round of this deliverable it
/// threw that answer away. Every door then went on laying out against the size
/// it had before the scale changed: on a clamped buffer that is the whole
/// defect `resized` was written to remove, on the one path — a window dragged
/// between two monitors of different scales, with no configure after it —
/// that the module exists for. A `bool` cannot carry the number, so the number
/// could not be kept; `#[must_use]` is what makes dropping it a build error
/// rather than a silent line.
///
/// A factor of zero or less is refused rather than clamped: the protocol says
/// a buffer scale is a positive integer, and a zero would ask for a buffer of
/// no pixels — a window that draws nothing, which looks like a crash and is
/// not one.
#[must_use]
pub fn followed(
    scale: &mut i32,
    factor: i32,
    surface: &wl_surface::WlSurface,
    gpu: Option<&mut ronoctua_render::gpu::Gpu>,
    text: Option<&mut ronoctua_render::Text>,
    asked: (u32, u32),
) -> Option<(u32, u32)> {
    // A SCALE NO SCREEN HAS IS REFUSED, at both ends, before anything moves.
    //
    // Zero and below are not scales at all. And above the bound the clamp stops
    // being the extreme case and becomes the ordinary one: at 9, an ordinary
    // 1920-logical window wants a 17280 px buffer against a ceiling of 16384,
    // so the door would be laying its picture out in the fraction of itself
    // the buffer can show — 1820 logical pixels of a 1920 window — on hardware
    // that is not exotic. Everything downstream stays correct there, because
    // that is what `resized` answers; it is just not a window anybody would
    // recognise as theirs.
    //
    // EIGHT, and the number itself is `ronoctua_render::MOST_SCALE` — the one
    // place it is written, which `render-png --scale` reads too. It used to be
    // written here and there, each comment claiming to be the other's bound.
    if factor <= 0 || factor > ronoctua_render::MOST_SCALE {
        tracing::warn!(factor, "a buffer scale no screen has; staying as we are");
        return None;
    }
    // AND THE REFUSAL IS HERE, BEFORE ANYTHING MOVES.
    //
    // `wl_surface.set_buffer_scale` is a version-3 request while
    // `wl_surface.enter` — which is what makes the toolkit call
    // `scale_factor_changed` — exists at version 1, so a compositor offering
    // `wl_compositor` v1 or v2 can send the event and then be sent an opcode
    // its object does not have: `invalid_method`, and the connection closes.
    //
    // It used to be checked in `told_before_present`, which is the LAST of the
    // three parts: the buffer had already been resized and the glyphs already
    // re-rasterised, so refusing there left two of the three moved and the
    // surface at the old scale — a door twice the size it was configured to
    // be. Not following at all is the only answer that keeps them together,
    // and it is the answer this module exists to give. niri offers v6, which
    // is what makes this a guard rather than a path anybody walks.
    if surface.version() < 3 {
        tracing::warn!(
            version = surface.version(),
            "this compositor's surfaces cannot be told a buffer scale; staying one to one"
        );
        return None;
    }
    if factor == *scale {
        return None;
    }
    *scale = factor;
    // THE SURFACE IS NOT TOLD HERE — see `told_before_present`, which is the
    // one place that sends it, and which runs inside the frame.
    //
    // AND WITH NO GPU THERE IS NOTHING TO CLAMP: what the compositor asked for
    // is what the door would show, which is what `configure` writes on that
    // same path before the GPU is assembled.
    let shown = match gpu {
        Some(g) => resized(g, asked, factor),
        None => asked,
    };
    if let Some(t) = text {
        // THE SIZE STAYS LOGICAL and the scale is said separately: a person
        // chose the size, and it means the same thing on every screen.
        t.draw_scaled(ronoctua_render::rasterised_at(factor));
    }
    Some(shown)
}

#[cfg(test)]
mod tests {
    use super::{cut_to, physical, shown_of, wanted_size};

    #[test]
    fn a_buffer_is_the_window_times_the_scale() {
        assert_eq!(physical((900, 600), 1), (900, 600));
        assert_eq!(physical((900, 600), 2), (1800, 1200));
        assert_eq!(physical((900, 600), 3), (2700, 1800));
    }

    #[test]
    fn a_scale_that_cannot_be_one_is_treated_as_one() {
        // A COMPOSITOR CAN SAY ZERO while a window is being mapped, and a
        // buffer of no pixels is not a buffer. It is treated as one rather
        // than refused here, because this half is arithmetic — the refusal
        // with a reason lives in `followed`, which is where the compositor's
        // number arrives.
        assert_eq!(physical((900, 600), 0), (900, 600));
        assert_eq!(physical((900, 600), -4), (900, 600));
    }

    #[test]
    fn what_the_buffer_can_show_divides_back_to_the_window() {
        // THE FUNCTIONS THEMSELVES, not a copy of the division: `resized` is
        // these plus a `Gpu`, and a `Gpu` in a unit test is a test of wgpu.
        // What is left out is the `Gpu::resize` between them, which nothing
        // asserts anywhere — `tests/display-vm.nix` turns a scale knob and so
        // walks the path, but it reads the FRAGMENT and not the footer's
        // buffer. What proves that half is a photograph.
        assert_eq!(shown_of(physical((900, 600), 2), 2), (900, 600));
        assert_eq!(shown_of(physical((901, 601), 2), 2), (901, 601));
        // AT THE CEILING the buffer is smaller than the window asked for, and
        // dividing that is what keeps the rectangles in the same space as the
        // text.
        assert_eq!(shown_of((16384, 16384), 2), (8192, 8192));
        // AND A SCALE THAT IS NOT ONE IS TREATED AS ONE, as everywhere else.
        assert_eq!(shown_of((1920, 1080), 1), (1920, 1080));
    }

    #[test]
    fn a_clamped_buffer_comes_down_to_a_multiple_of_the_scale() {
        // THE ODD SCALE IS THE WHOLE POINT. 16384 is not a multiple of 3, and
        // a buffer the scale does not divide is `invalid_size` and a closed
        // connection — so the ceiling has to come down to 16383, which then
        // divides back to 5461 whole logical pixels.
        assert_eq!(cut_to((16384, 16384), 3), (16383, 16383));
        assert_eq!(shown_of(cut_to((16384, 16384), 3), 3), (5461, 5461));
        // AN EVEN ONE IS ALREADY THERE and must not move.
        assert_eq!(cut_to((16384, 16384), 2), (16384, 16384));
        assert_eq!(cut_to((900, 600), 1), (900, 600));
    }

    #[test]
    fn a_buffer_below_one_logical_pixel_is_grown_to_one() {
        // `Gpu::new` clamps a dimension to at least ONE — so a window mapped
        // at zero can leave a configuration of 1 behind, and 1 / 3 is a
        // buffer of no logical pixels at all. The floor is `by`, which is one
        // whole logical pixel, and it is the only case where this GROWS.
        assert_eq!(cut_to((1, 1), 3), (3, 3));
        assert_eq!(shown_of(cut_to((1, 1), 3), 3), (1, 1));
    }

    #[test]
    fn a_zero_keeps_the_buffer_it_had_on_that_axis_alone() {
        // A COMPOSITOR ANSWERS WIDTH 0 FOR "YOU CHOOSE", and can send it while
        // a window is mapping. `Gpu::resize` returns on a zero without
        // applying EITHER dimension, so a zero width took the new height down
        // with it — the footer's expansion never reached the buffer.
        assert_eq!(wanted_size((0, 53), (1920, 53), 1), (1920, 53));
        assert_eq!(wanted_size((1920, 0), (1920, 53), 1), (1920, 53));
        assert_eq!(wanted_size((0, 106), (1920, 53), 1), (1920, 106));
        // AND WHAT IS KEPT IS THE BUFFER'S OWN NUMBER, which is PHYSICAL: at
        // a new scale it is the same pixels and so half the logical size. The
        // caller is what must not build its `asked` out of a zero.
        assert_eq!(wanted_size((0, 53), (1920, 53), 2), (1920, 106));
        // AND WITHOUT A ZERO IT IS JUST `physical`.
        assert_eq!(wanted_size((900, 600), (1, 1), 2), (1800, 1200));
    }

    #[test]
    fn a_window_too_big_to_double_does_not_wrap_around() {
        // Saturating and not wrapping. `3_000_000_000 * 2` wraps to
        // 1_705_032_704 in release, and a buffer far smaller than the window
        // is one drawn into a corner of it with the rest never painted.
        // (`u32::MAX * 2` wraps to `u32::MAX - 1`, which is why it is the one
        // example that does NOT show the hazard.)
        assert_eq!(physical((u32::MAX, u32::MAX), 2), (u32::MAX, u32::MAX));
        assert_eq!(physical((3_000_000_000, 1), 2), (u32::MAX, 2));
    }
}
