//! Reading whole frames of raw pixels from a child, with a deadline.
//!
//! # Whose this is
//!
//! Every process in the tree that turns a child's stdout into pictures: the
//! camera (`capture.rs`), the studio's screen and media sources, and the file
//! manager's miniplayer (`shell/motion.rs`), which is where the two traps below
//! were learnt. It lives in this crate because the camera is the first caller
//! outside the shell; it knows nothing of cameras.
//!
//! # The two traps
//!
//! 1. `read_exact` has NO DEADLINE. A child that opens the pipe and never
//!    writes — a device that went away, a mount that hung — holds the reading
//!    thread for the rest of the session. So the read is a `poll` with a tick,
//!    and the deadline and the caller's "still wanted?" are asked every tick.
//! 2. A SHORT frame is never handed on: half a picture read as a whole one is
//!    a smear, and a smear on a live camera reads as a broken camera.

use std::{
    io::Read,
    os::fd::AsFd,
    process::Child,
    time::{Duration, Instant},
};

/// A child that is killed and reaped whatever happens next.
///
/// A guard and not calls at each exit: the exit somebody adds tomorrow is the
/// one that would leak a process holding a camera open — and a camera held
/// open is a camera whose light stays on.
pub struct Reaped(pub Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// How a read of one whole frame turned out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filled {
    /// Every byte of it.
    Whole,
    /// The pipe closed first.
    Short,
    /// Nothing came inside the deadline.
    Starved,
    /// The caller stopped wanting it.
    Left,
}

/// How often the wait looks up, which bounds how long a stop takes.
pub const TICK: Duration = Duration::from_millis(20);

/// Reads exactly `into.len()` bytes, giving up rather than blocking for ever.
pub fn fill<R: Read + AsFd>(
    pipe: &mut R,
    into: &mut [u8],
    starved: Duration,
    wanted: impl Fn() -> bool,
) -> Filled {
    let deadline = Instant::now() + starved;
    let tick = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: i64::try_from(TICK.as_nanos()).unwrap_or(20_000_000),
    };
    let mut got = 0usize;
    while got < into.len() {
        if !wanted() {
            return Filled::Left;
        }
        if Instant::now() >= deadline {
            return Filled::Starved;
        }
        let ready = {
            let fd = pipe.as_fd();
            let mut fds = [rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN,
            )];
            match rustix::event::poll(&mut fds, Some(&tick)) {
                Ok(_) => fds[0].revents(),
                Err(rustix::io::Errno::INTR) => continue,
                Err(_) => return Filled::Short,
            }
        };
        if ready.is_empty() {
            continue;
        }
        match pipe.read(&mut into[got..]) {
            Ok(0) => return Filled::Short,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return Filled::Short,
        }
    }
    Filled::Whole
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn a_whole_frame_is_read_and_a_short_one_is_refused() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "printf abcdefg"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("/bin/sh");
        let mut out = child.stdout.take().expect("pipe");
        let _guard = Reaped(child);
        let mut frame = [0u8; 4];
        assert_eq!(
            fill(&mut out, &mut frame, Duration::from_secs(5), || true),
            Filled::Whole
        );
        assert_eq!(&frame, b"abcd");
        assert_eq!(
            fill(&mut out, &mut frame, Duration::from_secs(5), || true),
            Filled::Short,
            "three bytes are not a frame"
        );
    }

    #[test]
    fn a_silent_child_starves_instead_of_hanging() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 5"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("/bin/sh");
        let mut out = child.stdout.take().expect("pipe");
        let _guard = Reaped(child);
        let began = Instant::now();
        let mut frame = [0u8; 4];
        assert_eq!(
            fill(&mut out, &mut frame, Duration::from_millis(100), || true),
            Filled::Starved
        );
        assert!(began.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_stop_is_heard_inside_a_tick() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 5"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("/bin/sh");
        let mut out = child.stdout.take().expect("pipe");
        let _guard = Reaped(child);
        let mut frame = [0u8; 4];
        assert_eq!(
            fill(&mut out, &mut frame, Duration::from_secs(5), || false),
            Filled::Left
        );
    }
}
