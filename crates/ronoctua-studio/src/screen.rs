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

use std::os::fd::{AsFd, OwnedFd};

use anyhow::{Context as _, Result};
use ronoctua_effects::Frame;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, event_created_child,
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
    ExtImageCopyCaptureManagerV1
);

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
            return Ok(Err(Refused::NoScreen));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_32_bit_formats_are_read_in_their_byte_order() {
        assert_eq!(byte_order(wl_shm::Format::Xrgb8888), Some((true, false)));
        assert_eq!(byte_order(wl_shm::Format::Abgr8888), Some((false, true)));
        assert_eq!(byte_order(wl_shm::Format::Rgb565), None);
    }
}
