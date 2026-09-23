//! Cameras coming and going, by event.
//!
//! # Event, not clock
//!
//! An inotify watch on `/dev` for `video*` names: a plug is a `CREATE`, an
//! unplug a `DELETE`, and udev handing the node its group a moment later is an
//! `ATTRIB`. The thread sleeps in `read` between them and costs nothing. Each
//! burst is answered with ONE fresh [`scan`](crate::scan) after a short quiet
//! (`SETTLE`), because a plug is several events — the node, its metadata
//! sibling, the permission — and a scan in the middle of them reads a node
//! the current user may not open yet.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use rustix::fs::inotify::{self, CreateFlags, WatchFlags};

use crate::Camera;

/// The quiet after the last event before scanning.
pub const SETTLE: Duration = Duration::from_millis(400);

/// The thread, and the switch that ends it.
pub struct Handle {
    stop: Arc<AtomicBool>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Starts watching; `on_change` gets every camera, first once at once and
/// then after every change.
pub fn watch(on_change: impl Fn(Vec<Camera>) + Send + 'static) -> Handle {
    let stop = Arc::new(AtomicBool::new(false));
    let gone = stop.clone();
    let spawned = std::thread::Builder::new()
        .name("camera-watch".into())
        .spawn(move || {
            on_change(crate::scan());
            let Ok(fd) = inotify::init(CreateFlags::CLOEXEC) else {
                tracing::warn!(
                    "no inotify: cameras plugged later will not appear until the view reopens"
                );
                return;
            };
            if inotify::add_watch(
                &fd,
                "/dev",
                WatchFlags::CREATE | WatchFlags::DELETE | WatchFlags::ATTRIB,
            )
            .is_err()
            {
                tracing::warn!("could not watch /dev");
                return;
            }
            let mut buf = [std::mem::MaybeUninit::<u8>::uninit(); 4096];
            loop {
                if gone.load(Ordering::SeqCst) {
                    return;
                }
                let mut reader = inotify::Reader::new(&fd, &mut buf);
                let mut ours = false;
                loop {
                    match reader.next() {
                        Ok(event) => {
                            if event
                                .file_name()
                                .is_some_and(|n| n.to_bytes().starts_with(b"video"))
                            {
                                ours = true;
                            }
                        }
                        Err(rustix::io::Errno::INTR) => continue,
                        Err(_) => break,
                    }
                    if reader.is_buffer_empty() {
                        break;
                    }
                }
                if ours && !gone.load(Ordering::SeqCst) {
                    std::thread::sleep(SETTLE);
                    on_change(crate::scan());
                }
            }
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start the camera watch");
    }
    Handle { stop }
}
