//! The screen, or one window, as frames — `ext-image-copy-capture-v1`.
//!
//! # Why a Wayland client and not a program
//!
//! `LEADER m s` takes ONE picture and `grim` is right for that. A recording
//! needs frames continuously, and the compositor hands them out only when
//! something changed (the frame's `ready` waits for damage), which is what
//! makes a static screen cost nothing. And only this protocol captures ONE
//! WINDOW (`ext-foreign-toplevel-image-capture-source-v1`), which is what a
//! person sharing a single program expects.
//!
//! This is a standard Wayland protocol, spoken the way `carry.rs` speaks
//! `wl_data_device`: it is not the compositor's IPC and it names no compositor
//! (invariant 3 — the exception the owner approved with the plan). The pinned
//! compositor was measured to announce all four globals used here.
//!
//! # Buffers
//!
//! Shared memory only: a memfd, one `wl_buffer` of the size the session
//! states, read back with `pread`. The four 32-bit formats in both byte
//! orders are accepted; anything else is refused and said.
//!
//! # And when the compositor does not announce it
//!
//! The pinned compositor announces `ext-image-copy-capture-v1`; another may
//! not. A MONITOR is then captured through `zwlr_screencopy_v1`, which every
//! wlroots compositor has spoken for years: a frame object per picture, the
//! same shared memory, a `y_invert` flag honoured ([`run_wlr`]). A WINDOW has
//! no such fallback — only the `ext` protocol captures one — and is refused
//! as it was.

use std::os::fd::{AsFd, OwnedFd};

use anyhow::{Context as _, Result};
use ronoctua_effects::Frame;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy as _, QueueHandle, event_created_child,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer::WlBuffer,
        wl_output::{self, WlOutput},
        wl_registry::WlRegistry,
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
    },
};
use wayland_protocols::ext::{
    foreign_toplevel_list::v1::client::{
        ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
        ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
    },
    image_capture_source::v1::client::{
        ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
        ext_image_capture_source_v1::ExtImageCaptureSourceV1,
        ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
    },
    image_copy_capture::v1::client::{
        ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
        ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
        ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
    },
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

/// What to capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A monitor by name, or the first.
    Output(Option<String>),
    /// A window whose app id is, or whose title contains, the words.
    Window(String),
}

/// Why capture could not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// No compositor, or it lacks the protocol.
    NoScreen,
    /// No window matches.
    NoWindow,
}

#[derive(Default)]
struct Toplevel {
    handle: Option<ExtForeignToplevelHandleV1>,
    title: String,
    app_id: String,
}

#[derive(Default)]
struct St {
    outputs: Vec<(WlOutput, String)>,
    toplevels: Vec<Toplevel>,
    size: Option<(u32, u32)>,
    formats: Vec<wl_shm::Format>,
    session_done: bool,
    stopped: bool,
    frame: Option<bool>,
    /// The buffer a `zwlr_screencopy_frame_v1` asks for: format, width,
    /// height and stride.
    wlr_buffer: Option<(wl_shm::Format, u32, u32, u32)>,
    /// Every buffer it accepts has been said (version 3).
    wlr_done: bool,
    /// The picture arrived upside down.
    wlr_inverted: bool,
}

impl Dispatch<WlRegistry, GlobalListContents> for St {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlOutput, usize> for St {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        at: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event
            && let Some(o) = state.outputs.get_mut(*at)
        {
            o.1 = name;
        }
    }
}

macro_rules! quiet {
    ($($t:ty),* $(,)?) => {$(
        impl Dispatch<$t, ()> for St {
            fn event(_: &mut Self, _: &$t, _: <$t as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
quiet!(
    WlShm,
    WlShmPool,
    WlBuffer,
    ExtOutputImageCaptureSourceManagerV1,
    ExtForeignToplevelImageCaptureSourceManagerV1,
    ExtImageCaptureSourceV1,
    ExtImageCopyCaptureManagerV1,
    ZwlrScreencopyManagerV1
);

impl Dispatch<ZwlrScreencopyFrameV1, ()> for St {
    fn event(
        state: &mut Self,
        _: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format: wayland_client::WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                // THE FIRST 32-BIT ONE is kept: the event repeats, one per
                // format the compositor accepts.
                if state.wlr_buffer.is_none() && byte_order(format).is_some() {
                    state.wlr_buffer = Some((format, width, height, stride));
                }
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => state.wlr_done = true,
            zwlr_screencopy_frame_v1::Event::Flags {
                flags: wayland_client::WEnum::Value(flags),
            } => {
                state.wlr_inverted = flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert);
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => state.frame = Some(true),
            zwlr_screencopy_frame_v1::Event::Failed => state.frame = Some(false),
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for St {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.toplevels.push(Toplevel {
                handle: Some(toplevel),
                ..Toplevel::default()
            });
        }
    }

    event_created_child!(St, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for St {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(t) = state
            .toplevels
            .iter_mut()
            .find(|t| t.handle.as_ref() == Some(handle))
        else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Title { title } => t.title = title,
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => t.app_id = app_id,
            ext_foreign_toplevel_handle_v1::Event::Closed => t.handle = None,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for St {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                state.size = Some((width, height))
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat {
                format: wayland_client::WEnum::Value(f),
            } => state.formats.push(f),
            ext_image_copy_capture_session_v1::Event::Done => state.session_done = true,
            ext_image_copy_capture_session_v1::Event::Stopped => state.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for St {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_image_copy_capture_frame_v1::Event::Ready => state.frame = Some(true),
            ext_image_copy_capture_frame_v1::Event::Failed { .. } => state.frame = Some(false),
            _ => {}
        }
    }
}

/// Byte order of a format: `true` is B,G,R,A in memory (the `ARGB`/`XRGB`
/// little-endian formats), `false` is R,G,B,A (`ABGR`/`XBGR`).
fn byte_order(f: wl_shm::Format) -> Option<(bool, bool)> {
    match f {
        wl_shm::Format::Argb8888 => Some((true, true)),
        wl_shm::Format::Xrgb8888 => Some((true, false)),
        wl_shm::Format::Abgr8888 => Some((false, true)),
        wl_shm::Format::Xbgr8888 => Some((false, false)),
        _ => None,
    }
}

/// Waits for events without blocking past a tick, so a stop is heard.
fn pump(
    queue: &mut EventQueue<St>,
    st: &mut St,
    wanted: &dyn Fn() -> bool,
    until: impl Fn(&St) -> bool,
) -> Result<bool> {
    loop {
        queue.dispatch_pending(st)?;
        if until(st) {
            return Ok(true);
        }
        if !wanted() || st.stopped {
            return Ok(false);
        }
        queue.flush()?;
        if let Some(guard) = queue.prepare_read() {
            let ready = {
                let fd = guard.connection_fd();
                let mut fds = [rustix::event::PollFd::new(
                    &fd,
                    rustix::event::PollFlags::IN,
                )];
                let tick = rustix::event::Timespec {
                    tv_sec: 0,
                    tv_nsec: 50_000_000,
                };
                rustix::event::poll(&mut fds, Some(&tick))
                    .map(|_| !fds[0].revents().is_empty())
                    .unwrap_or(false)
            };
            if ready {
                guard.read()?;
            }
        }
    }
}

/// Every monitor's name, and every window's `(app id, title)`.
pub type Listing = (Vec<String>, Vec<(String, String)>);

/// Every monitor's name and every window's title, for the settings view's
/// pickers. Empty without a compositor.
pub fn list() -> Listing {
    let run = || -> Result<Listing> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut queue) = registry_queue_init::<St>(&conn)?;
        let qh = queue.handle();
        let mut st = St::default();
        bind_outputs(&globals, &qh, &mut st);
        let _list: Option<ExtForeignToplevelListV1> = globals.bind(&qh, 1..=1, ()).ok();
        queue.roundtrip(&mut st)?;
        queue.roundtrip(&mut st)?;
        Ok((
            st.outputs.iter().map(|o| o.1.clone()).collect(),
            st.toplevels
                .iter()
                .filter(|t| t.handle.is_some())
                .map(|t| (t.app_id.clone(), t.title.clone()))
                .collect(),
        ))
    };
    run().unwrap_or_default()
}

fn bind_outputs(globals: &wayland_client::globals::GlobalList, qh: &QueueHandle<St>, st: &mut St) {
    let names: Vec<(u32, u32)> = globals.contents().with_list(|l| {
        l.iter()
            .filter(|g| g.interface == "wl_output")
            .map(|g| (g.name, g.version))
            .collect()
    });
    for (name, version) in names {
        let at = st.outputs.len();
        let output: WlOutput = globals.registry().bind(name, version.min(4), qh, at);
        st.outputs.push((output, String::new()));
    }
}

/// Captures `target` until `wanted()` says stop, handing each frame to
/// `publish` already RGBA and resized to fit `max` (never enlarged).
pub fn run(
    target: &Target,
    max: (u32, u32),
    cursor: bool,
    fps: u32,
    wanted: &dyn Fn() -> bool,
    publish: &mut dyn FnMut(Frame),
) -> Result<(), Refused> {
    let mut run = || -> Result<Result<(), Refused>> {
        let conn = Connection::connect_to_env().map_err(|_| anyhow::anyhow!("no display"))?;
        let (globals, mut queue) = registry_queue_init::<St>(&conn)?;
        let qh = queue.handle();
        let mut st = St::default();
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let Ok(copy) = globals.bind::<ExtImageCopyCaptureManagerV1, _, _>(&qh, 1..=1, ()) else {
            // THE OLDER PROTOCOL, for a monitor: see the module's header.
            return match target {
                Target::Output(name) => {
                    let Ok(wlr) = globals.bind::<ZwlrScreencopyManagerV1, _, _>(&qh, 1..=3, ())
                    else {
                        return Ok(Err(Refused::NoScreen));
                    };
                    bind_outputs(&globals, &qh, &mut st);
                    queue.roundtrip(&mut st)?;
                    let output = name
                        .as_ref()
                        .and_then(|n| st.outputs.iter().find(|o| &o.1 == n))
                        .or_else(|| st.outputs.first())
                        .map(|o| o.0.clone());
                    let Some(output) = output else {
                        return Ok(Err(Refused::NoScreen));
                    };
                    run_wlr(
                        &mut queue,
                        &mut st,
                        &Wlr {
                            shm: &shm,
                            manager: &wlr,
                            output: &output,
                        },
                        max,
                        cursor,
                        fps,
                        wanted,
                        publish,
                    )
                }
                Target::Window(_) => Ok(Err(Refused::NoScreen)),
            };
        };
        bind_outputs(&globals, &qh, &mut st);
        let source: ExtImageCaptureSourceV1 = match target {
            Target::Output(name) => {
                let Ok(mgr) =
                    globals.bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(&qh, 1..=1, ())
                else {
                    return Ok(Err(Refused::NoScreen));
                };
                queue.roundtrip(&mut st)?;
                let output = name
                    .as_ref()
                    .and_then(|n| st.outputs.iter().find(|o| &o.1 == n))
                    .or_else(|| st.outputs.first())
                    .map(|o| o.0.clone());
                let Some(output) = output else {
                    return Ok(Err(Refused::NoScreen));
                };
                mgr.create_source(&output, &qh, ())
            }
            Target::Window(matching) => {
                let (Ok(mgr), Ok(_list)) = (
                    globals.bind::<ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(
                        &qh,
                        1..=1,
                        (),
                    ),
                    globals.bind::<ExtForeignToplevelListV1, _, _>(&qh, 1..=1, ()),
                ) else {
                    return Ok(Err(Refused::NoScreen));
                };
                queue.roundtrip(&mut st)?;
                queue.roundtrip(&mut st)?;
                let wanted_words = matching.to_lowercase();
                let found = st
                    .toplevels
                    .iter()
                    .filter(|t| t.handle.is_some())
                    .find(|t| t.app_id.eq_ignore_ascii_case(matching))
                    .or_else(|| {
                        st.toplevels
                            .iter()
                            .filter(|t| t.handle.is_some())
                            .find(|t| t.title.to_lowercase().contains(&wanted_words))
                    })
                    .and_then(|t| t.handle.clone());
                let Some(handle) = found else {
                    return Ok(Err(Refused::NoWindow));
                };
                mgr.create_source(&handle, &qh, ())
            }
        };
        let options = if cursor {
            Options::PaintCursors
        } else {
            Options::empty()
        };
        let session = copy.create_session(&source, options, &qh, ());
        loop {
            st.session_done = false;
            st.formats.clear();
            if !pump(&mut queue, &mut st, wanted, |s| s.session_done)? {
                return Ok(Ok(()));
            }
            let Some((w, h)) = st.size else {
                return Ok(Err(Refused::NoScreen));
            };
            let Some((format, (bgr, alpha))) = st
                .formats
                .iter()
                .find_map(|f| byte_order(*f).map(|o| (*f, o)))
            else {
                tracing::warn!(formats = ?st.formats, "no 32-bit shared-memory format offered");
                return Ok(Err(Refused::NoScreen));
            };
            // The constraints are read: from here a `done` is NEW constraints
            // (a resolution change), and only that.
            st.session_done = false;
            let stride = w * 4;
            let bytes = u64::from(stride) * u64::from(h);
            let fd: OwnedFd = rustix::fs::memfd_create(
                "ronoctua-studio-screen",
                rustix::fs::MemfdFlags::CLOEXEC,
            )?;
            rustix::fs::ftruncate(&fd, bytes)?;
            let pool = shm.create_pool(
                fd.as_fd(),
                i32::try_from(bytes).context("screen too large")?,
                &qh,
                (),
            );
            let buffer = pool.create_buffer(0, w as i32, h as i32, stride as i32, format, &qh, ());
            let mut raw = vec![0u8; bytes as usize];
            let pace = std::time::Duration::from_secs(1) / fps.max(1);
            let mut last = std::time::Instant::now() - pace;
            let resized = 'frames: loop {
                let wait = pace.saturating_sub(last.elapsed());
                if !wait.is_zero() {
                    std::thread::sleep(wait);
                }
                if !wanted() {
                    break 'frames false;
                }
                last = std::time::Instant::now();
                st.frame = None;
                let frame = session.create_frame(&qh, ());
                frame.attach_buffer(&buffer);
                frame.damage_buffer(0, 0, w as i32, h as i32);
                frame.capture();
                if !pump(&mut queue, &mut st, wanted, |s| {
                    s.frame.is_some() || s.session_done
                })? {
                    frame.destroy();
                    break 'frames false;
                }
                frame.destroy();
                if st.session_done && st.frame.is_none() {
                    // New constraints (a resolution change): start over with a
                    // buffer of the new size.
                    break 'frames true;
                }
                if st.frame != Some(true) {
                    if st.stopped {
                        break 'frames false;
                    }
                    continue;
                }
                let mut got = 0usize;
                while got < raw.len() {
                    let n = rustix::io::pread(&fd, &mut raw[got..], got as u64)?;
                    if n == 0 {
                        break;
                    }
                    got += n;
                }
                let mut rgba = Frame {
                    width: w as usize,
                    height: h as usize,
                    rgba: raw.clone(),
                };
                for px in rgba.rgba.as_chunks_mut::<4>().0 {
                    if bgr {
                        px.swap(0, 2);
                    }
                    if !alpha {
                        px[3] = 255;
                    }
                }
                let (tw, th) = crate::compose::fit_within(w, h, max.0, max.1);
                publish(crate::compose::resize(&rgba, tw, th));
            };
            buffer.destroy();
            pool.destroy();
            if !resized {
                break;
            }
        }
        session.destroy();
        source.destroy();
        Ok(Ok(()))
    };
    match run() {
        Ok(answer) => answer,
        Err(e) => {
            tracing::warn!(error = %e, "screen capture");
            Err(Refused::NoScreen)
        }
    }
}

/// What the older protocol captures with.
struct Wlr<'a> {
    shm: &'a WlShm,
    manager: &'a ZwlrScreencopyManagerV1,
    output: &'a WlOutput,
}

/// A monitor through `zwlr_screencopy_v1`, until `wanted()` says stop: a
/// frame object per picture, at the program's pace, each read out of one
/// shared buffer made for the size the first frame asks for.
/// The memory one wlr frame is copied into, kept while its shape holds:
/// the file, its pool, the buffer, and the shape (format, width, height,
/// stride) it was made for.
type Made = (
    OwnedFd,
    WlShmPool,
    WlBuffer,
    (wl_shm::Format, u32, u32, u32),
);

#[allow(clippy::too_many_arguments)]
fn run_wlr(
    queue: &mut EventQueue<St>,
    st: &mut St,
    wlr: &Wlr<'_>,
    max: (u32, u32),
    cursor: bool,
    fps: u32,
    wanted: &dyn Fn() -> bool,
    publish: &mut dyn FnMut(Frame),
) -> Result<Result<(), Refused>> {
    let qh = queue.handle();
    let version = wlr.manager.version();
    let pace = std::time::Duration::from_secs(1) / fps.max(1);
    let mut last = std::time::Instant::now() - pace;
    let mut made: Option<Made> = None;
    let mut raw = Vec::new();
    loop {
        let wait = pace.saturating_sub(last.elapsed());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        if !wanted() {
            break;
        }
        last = std::time::Instant::now();
        st.frame = None;
        st.wlr_buffer = None;
        st.wlr_done = false;
        st.wlr_inverted = false;
        let frame = wlr
            .manager
            .capture_output(i32::from(cursor), wlr.output, &qh, ());
        // THE SIZE FIRST: every buffer it takes is said, and from version 3
        // the list ends with `buffer_done`.
        if !pump(queue, st, wanted, |s| {
            s.frame == Some(false) || (s.wlr_buffer.is_some() && (version < 3 || s.wlr_done))
        })? {
            frame.destroy();
            break;
        }
        let Some(asked) = st.wlr_buffer else {
            frame.destroy();
            tracing::warn!("no 32-bit shared-memory format offered by the screencopy");
            return Ok(Err(Refused::NoScreen));
        };
        // A NEW BUFFER only when the size or the format moved.
        if made.as_ref().is_none_or(|m| m.3 != asked) {
            if let Some((_, pool, buffer, _)) = made.take() {
                buffer.destroy();
                pool.destroy();
            }
            let (format, w, h, stride) = asked;
            let bytes = u64::from(stride) * u64::from(h);
            let fd: OwnedFd = rustix::fs::memfd_create(
                "ronoctua-studio-screen",
                rustix::fs::MemfdFlags::CLOEXEC,
            )?;
            rustix::fs::ftruncate(&fd, bytes)?;
            let pool = wlr.shm.create_pool(
                fd.as_fd(),
                i32::try_from(bytes).context("screen too large")?,
                &qh,
                (),
            );
            let buffer = pool.create_buffer(
                0,
                i32::try_from(w).context("screen too wide")?,
                i32::try_from(h).context("screen too tall")?,
                i32::try_from(stride).context("stride too wide")?,
                format,
                &qh,
                (),
            );
            raw = vec![0u8; usize::try_from(bytes).context("screen too large")?];
            made = Some((fd, pool, buffer, asked));
        }
        let Some((fd, _, buffer, (format, w, h, stride))) = made.as_ref() else {
            break;
        };
        frame.copy(buffer);
        if !pump(queue, st, wanted, |s| s.frame.is_some())? {
            frame.destroy();
            break;
        }
        frame.destroy();
        if st.frame != Some(true) {
            continue;
        }
        let mut got = 0usize;
        while got < raw.len() {
            let n = rustix::io::pread(fd, &mut raw[got..], got as u64)?;
            if n == 0 {
                break;
            }
            got += n;
        }
        let Some((bgr, alpha)) = byte_order(*format) else {
            return Ok(Err(Refused::NoScreen));
        };
        publish(unpacked(
            &raw,
            *w,
            *h,
            *stride,
            bgr,
            alpha,
            st.wlr_inverted,
            max,
        ));
    }
    if let Some((_, pool, buffer, _)) = made.take() {
        buffer.destroy();
        pool.destroy();
    }
    Ok(Ok(()))
}

/// A shared buffer's bytes as an RGBA frame: rows of `stride` cut to the
/// width, the byte order undone, an opaque alpha where the format has none,
/// the rows turned when the picture arrived upside down — then fitted.
#[allow(clippy::too_many_arguments)]
fn unpacked(
    raw: &[u8],
    w: u32,
    h: u32,
    stride: u32,
    bgr: bool,
    alpha: bool,
    inverted: bool,
    max: (u32, u32),
) -> Frame {
    let (wide, rows, stride) = (w as usize * 4, h as usize, stride as usize);
    let mut rgba = Vec::with_capacity(wide * rows);
    for y in 0..rows {
        let from = if inverted { rows - 1 - y } else { y } * stride;
        let row = raw.get(from..from + wide).unwrap_or(&[]);
        rgba.extend_from_slice(row);
    }
    rgba.resize(wide * rows, 0);
    for px in rgba.as_chunks_mut::<4>().0 {
        if bgr {
            px.swap(0, 2);
        }
        if !alpha {
            px[3] = 255;
        }
    }
    let frame = Frame {
        width: w as usize,
        height: h as usize,
        rgba,
    };
    let (tw, th) = crate::compose::fit_within(w, h, max.0, max.1);
    crate::compose::resize(&frame, tw, th)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_protocols_buffer_is_cut_to_its_width_and_turned_when_inverted() {
        // TWO ROWS OF TWO PIXELS, a stride of twelve: the padding is not a
        // pixel, the byte order is undone, and an inverted picture is turned.
        // (Two by two because the program's sizes are even.)
        let raw = [
            1, 2, 3, 0, 1, 2, 3, 0, 9, 9, 9, 9, // row 0: B G R X twice, padding
            4, 5, 6, 0, 4, 5, 6, 0, 9, 9, 9, 9, // row 1
        ];
        let f = unpacked(&raw, 2, 2, 12, true, false, false, (2, 2));
        assert_eq!(f.rgba[..8], [3, 2, 1, 255, 3, 2, 1, 255]);
        assert_eq!(f.rgba[8..], [6, 5, 4, 255, 6, 5, 4, 255]);
        let turned = unpacked(&raw, 2, 2, 12, true, false, true, (2, 2));
        assert_eq!(turned.rgba[..4], [6, 5, 4, 255]);
        assert_eq!(turned.rgba[8..12], [3, 2, 1, 255]);
    }

    #[test]
    fn the_four_32_bit_formats_are_read_in_their_byte_order() {
        assert_eq!(byte_order(wl_shm::Format::Xrgb8888), Some((true, false)));
        assert_eq!(byte_order(wl_shm::Format::Abgr8888), Some((false, true)));
        assert_eq!(byte_order(wl_shm::Format::Rgb565), None);
    }
}
