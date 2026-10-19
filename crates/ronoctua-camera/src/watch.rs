//! Cameras coming and going, by event — and only while somebody asked.
//!
//! # Raised by a gesture, never by a start
//!
//! A reading ([`scan`](crate::scan)) OPENS every node, and opening a UVC node
//! powers the camera up: its light blinks. So no program raises this watch
//! because it started — a footer, a lifeline, the studio or a door that only
//! came up opens no node. It is raised by what a person asked for: a view
//! that shows the cameras (the settings, in either door) while it is open,
//! and the studio while a camera is in use. Dropping the [`Handle`] ends it at
//! once — the thread wakes on its pipe closing — and a reading that was under
//! way when it was dropped is told to nobody. `camera-vm` watches `IN_OPEN`
//! on a node while a footer and a studio start, and sees none until a camera
//! is asked for.
//!
//! # Event, not clock
//!
//! An inotify watch on `/dev` for `video*` names: a plug is a `CREATE`, an
//! unplug a `DELETE`, and udev handing the node its group a moment later is an
//! `ATTRIB`. The thread sleeps in `poll` between them and costs nothing. Each
//! burst is answered with ONE fresh reading after a short quiet (`SETTLE`),
//! because a plug is several events — the node, its metadata sibling, the
//! permission — and a reading in the middle of them meets a node the current
//! user may not open yet. The watch is set BEFORE the first reading, so a
//! camera plugged while the first one is read is an event waiting, never one
//! missed.

use std::{
    io::{PipeReader, PipeWriter},
    os::fd::OwnedFd,
    path::{Path, PathBuf},
    time::Duration,
};

use rustix::{
    event::{PollFd, PollFlags, Timespec},
    fs::inotify::{self, CreateFlags, WatchFlags},
};

use crate::Camera;

/// The quiet after the last event before reading.
pub const SETTLE: Duration = Duration::from_millis(400);

/// The thread, and the end of the pipe whose closing ends it.
///
/// NEVER JOINED: a drop must not wait for a reading in flight — the loop that
/// drops it is a door's — and the thread tells nobody what it read after the
/// pipe closed.
pub struct Handle {
    _ring: Option<PipeWriter>,
    #[cfg(test)]
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Starts watching; `on_change` gets every camera, first once at once and
/// then after every change, until the [`Handle`] is dropped.
pub fn watch(on_change: impl Fn(Vec<Camera>) + Send + 'static) -> Handle {
    watch_in(PathBuf::from("/dev"), crate::scan, on_change)
}

/// [`watch`], over the folder `dir` and the reading `read` — a test's.
fn watch_in(
    dir: PathBuf,
    read: impl Fn() -> Vec<Camera> + Send + 'static,
    on_change: impl Fn(Vec<Camera>) + Send + 'static,
) -> Handle {
    let (ring_in, ring) = match std::io::pipe() {
        Ok(pair) => pair,
        Err(e) => {
            // NO PIPE, NO WATCH: a thread nothing can end would read the
            // cameras again on every plug after its view closed. One
            // reading, for the view that asked now.
            tracing::warn!(error = %e, "no pipe: the cameras are read once, and a camera plugged later appears when the view opens again");
            let spawned = std::thread::Builder::new()
                .name("camera-read".into())
                .spawn(move || on_change(read()));
            if let Err(e) = spawned {
                tracing::error!(error = %e, "could not read the cameras");
            }
            return Handle {
                _ring: None,
                #[cfg(test)]
                thread: None,
            };
        }
    };
    let spawned = std::thread::Builder::new()
        .name("camera-watch".into())
        .spawn(move || follow(&dir, &ring_in, &read, &on_change));
    match spawned {
        Ok(_thread) => Handle {
            _ring: Some(ring),
            #[cfg(test)]
            thread: Some(_thread),
        },
        Err(e) => {
            tracing::error!(error = %e, "could not start the camera watch");
            Handle {
                _ring: None,
                #[cfg(test)]
                thread: None,
            }
        }
    }
}

/// What one wait ended in.
enum Woke {
    /// The pipe closed: the handle was dropped.
    Over,
    /// The folder stirred.
    Stirred,
    /// The time ran out.
    Quiet,
}

/// The watch's thread: the first reading, then one after every burst of
/// `video*` events, until the pipe closes.
fn follow(
    dir: &Path,
    ring: &PipeReader,
    read: &dyn Fn() -> Vec<Camera>,
    on_change: &dyn Fn(Vec<Camera>),
) {
    let watched = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)
        .map_err(|e| tracing::warn!(error = %e, "no inotify: cameras plugged later will not appear until the view reopens"))
        .ok()
        .filter(|fd| {
            inotify::add_watch(
                fd,
                dir,
                WatchFlags::CREATE | WatchFlags::DELETE | WatchFlags::ATTRIB,
            )
            .map_err(|e| tracing::warn!(error = %e, dir = %dir.display(), "could not watch the devices' folder"))
            .is_ok()
        });
    tell(ring, read, on_change);
    let Some(fd) = watched else {
        return;
    };
    loop {
        match wait(ring, Some(&fd), None) {
            Woke::Over => return,
            Woke::Quiet => continue,
            Woke::Stirred => {}
        }
        if !drained(&fd) {
            continue;
        }
        // THE QUIET, during which only the pipe is heard: what the folder
        // says meanwhile is the same burst, drained before the reading.
        match wait(ring, None, Some(SETTLE)) {
            Woke::Over => return,
            Woke::Stirred | Woke::Quiet => {}
        }
        let _ = drained(&fd);
        tell(ring, read, on_change);
    }
}

/// One reading, told — unless the handle was dropped before or while it was
/// read: those cameras are nobody's business any more.
fn tell(ring: &PipeReader, read: &dyn Fn() -> Vec<Camera>, on_change: &dyn Fn(Vec<Camera>)) {
    if closed(ring) {
        return;
    }
    let cameras = read();
    if !closed(ring) {
        on_change(cameras);
    }
}

/// Whether the handle was dropped, asked without waiting.
fn closed(ring: &PipeReader) -> bool {
    matches!(wait(ring, None, Some(Duration::ZERO)), Woke::Over)
}

/// Waits on the pipe and, when given, the inotify descriptor — for `within`,
/// or for ever.
fn wait(ring: &PipeReader, watched: Option<&OwnedFd>, within: Option<Duration>) -> Woke {
    let deadline = within.map(|d| Timespec {
        tv_sec: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(d.subsec_nanos()),
    });
    loop {
        let mut fds = vec![PollFd::new(ring, PollFlags::IN)];
        if let Some(fd) = watched {
            fds.push(PollFd::new(fd, PollFlags::IN));
        }
        match rustix::event::poll(&mut fds, deadline.as_ref()) {
            Ok(0) => return Woke::Quiet,
            // THE WRITING END IS GONE — dropping the handle closed it, and
            // `HUP` is how that arrives. Nothing is ever written to it.
            Ok(_) if !fds[0].revents().is_empty() => return Woke::Over,
            Ok(_) => return Woke::Stirred,
            // A signal is a wake-up, not a failure.
            Err(rustix::io::Errno::INTR) => continue,
            Err(e) => {
                tracing::warn!(error = %e, "the camera watch's wait failed; it stops");
                return Woke::Over;
            }
        }
    }
}

/// Every event waiting, read — `true` when one of them was about a `video*`
/// name.
fn drained(fd: &OwnedFd) -> bool {
    let mut buf = [std::mem::MaybeUninit::<u8>::uninit(); 4096];
    let mut reader = inotify::Reader::new(fd, &mut buf);
    let mut ours = false;
    loop {
        match reader.next() {
            Ok(event) => {
                ours |= event
                    .file_name()
                    .is_some_and(|n| n.to_bytes().starts_with(b"video"));
            }
            Err(rustix::io::Errno::INTR) => {}
            // `AGAIN`: nothing more waiting.
            Err(_) => return ours,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    use std::time::Instant;

    /// A folder of the test's own under the temporary one.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-camera-watch-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    /// Whether the thread ended within a few seconds of the handle's pipe
    /// closing.
    fn ends(mut handle: Handle) -> bool {
        let thread = handle.thread.take().expect("a thread");
        drop(handle);
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if thread.is_finished() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// FAILS IF: a burst of `video*` events is not answered with exactly one
    /// reading, a name that is not a camera's is, or the thread outlives its
    /// handle — a footer that opened and closed the settings fifty times kept
    /// fifty threads and fifty inotify descriptors, each ready to read every
    /// camera at the next plug.
    #[test]
    fn a_burst_is_one_reading_and_the_handle_ends_the_thread() {
        let dir = folder("burst");
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let (told, heard) = mpsc::channel();
        let handle = watch_in(
            dir.clone(),
            move || {
                counted.fetch_add(1, Ordering::SeqCst);
                Vec::new()
            },
            move |c: Vec<Camera>| {
                let _ = told.send(c.len());
            },
        );
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("the first reading, at once");
        for name in ["video0", "video1", "video1-meta"] {
            std::fs::write(dir.join(name), b"").expect("a node");
        }
        heard
            .recv_timeout(SETTLE * 10)
            .expect("a reading after the burst");
        assert!(
            heard.recv_timeout(SETTLE * 3).is_err(),
            "one burst was read twice"
        );
        std::fs::write(dir.join("sda"), b"").expect("a disk");
        assert!(
            heard.recv_timeout(SETTLE * 3).is_err(),
            "a name that is not a camera's was read"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert!(ends(handle), "the thread outlived its handle");
        std::fs::write(dir.join("video2"), b"").expect("a node");
        std::thread::sleep(SETTLE * 2);
        assert_eq!(
            reads.load(Ordering::SeqCst),
            2,
            "a camera was read after the handle was dropped"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a reading under way when the handle is dropped is still
    /// told — the studio let go of its watch and then heard of cameras nobody
    /// asked for any more.
    #[test]
    fn a_reading_in_flight_when_dropped_is_told_to_nobody() {
        let dir = folder("flight");
        let (go, gate) = mpsc::channel::<()>();
        let gate = std::sync::Mutex::new(gate);
        let (started, reading) = mpsc::channel::<()>();
        let (told, heard) = mpsc::channel();
        let mut handle = watch_in(
            dir.clone(),
            move || {
                let _ = started.send(());
                if let Ok(g) = gate.lock() {
                    let _ = g.recv_timeout(Duration::from_secs(5));
                }
                Vec::new()
            },
            move |c: Vec<Camera>| {
                let _ = told.send(c.len());
            },
        );
        reading
            .recv_timeout(Duration::from_secs(5))
            .expect("the first reading starts");
        let thread = handle.thread.take().expect("a thread");
        drop(handle);
        let _ = go.send(());
        let until = Instant::now() + Duration::from_secs(5);
        while !thread.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(thread.is_finished(), "the thread outlived its handle");
        assert!(
            heard.try_recv().is_err(),
            "a reading finished after the drop was told"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
