//! Frames shared between a daemon and the windows that show them.
//!
//! # Why memory, and never a device
//!
//! A daemon that decodes a picture once and shows it in several places — the
//! studio's program, the player's film in the footer, a window and the side
//! column at once — hands each viewer a memfd rather than writing to anything
//! another program could open. The viewer gets it over the socket
//! (`SCM_RIGHTS`) that asked for it, and nobody else ever does.
//!
//! # The ring
//!
//! [`SLOTS`] slots of `HEADER + width·height·4` bytes. The daemon writes the
//! slot AFTER the one it last announced, then sends one byte naming it; a
//! viewer reads the named slot. The header's sequence number is written as
//! `u64::MAX` before the pixels and as the frame's number after, and a reader
//! checks it before AND after copying — a slot overwritten under the copy (a
//! viewer two frames behind) is dropped rather than shown torn.
//!
//! No `mmap` and no `unsafe`: both sides use `pread`/`pwrite`.
//!
//! # What is NOT here
//!
//! The words. Each daemon speaks its own protocol, so the ring sends the
//! answer line it is GIVEN, and gives back the one it received; parsing it is
//! the caller's.

use std::{
    io::{IoSlice, IoSliceMut, Read as _},
    mem::MaybeUninit,
    os::{fd::OwnedFd, unix::net::UnixStream},
};

use anyhow::{Context as _, Result};
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};

/// Slots in the ring.
pub const SLOTS: u32 = 3;
/// Bytes before a slot's pixels: sequence (8), width (4), height (4).
pub const HEADER: usize = 16;

const WRITING: u64 = u64::MAX;

/// The daemon's side.
pub struct Ring {
    fd: OwnedFd,
    width: u32,
    height: u32,
    next: u32,
    seq: u64,
}

impl Ring {
    /// A ring for frames of `width × height`, named `name` in `/proc`.
    pub fn new(name: &str, width: u32, height: u32) -> Result<Self> {
        let fd = rustix::fs::memfd_create(name, rustix::fs::MemfdFlags::CLOEXEC)?;
        let size = u64::from(SLOTS) * (HEADER as u64 + u64::from(width) * u64::from(height) * 4);
        rustix::fs::ftruncate(&fd, size)?;
        Ok(Self {
            fd,
            width,
            height,
            next: 0,
            seq: 0,
        })
    }

    /// The frame size.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn slot_bytes(&self) -> u64 {
        HEADER as u64 + u64::from(self.width) * u64::from(self.height) * 4
    }

    /// Writes a frame into the next slot and says which slot it was.
    pub fn publish(&mut self, rgba: &[u8]) -> Result<u8> {
        let slot = self.next;
        self.next = (self.next + 1) % SLOTS;
        self.seq += 1;
        let at = u64::from(slot) * self.slot_bytes();
        rustix::io::pwrite(&self.fd, &WRITING.to_le_bytes(), at)?;
        let mut dims = [0u8; 8];
        dims[..4].copy_from_slice(&self.width.to_le_bytes());
        dims[4..].copy_from_slice(&self.height.to_le_bytes());
        rustix::io::pwrite(&self.fd, &dims, at + 8)?;
        write_all_at(&self.fd, rgba, at + HEADER as u64)?;
        rustix::io::pwrite(&self.fd, &self.seq.to_le_bytes(), at)?;
        Ok(u8::try_from(slot).unwrap_or(0))
    }

    /// Hands the ring to a client: `line` (the daemon's own answer, ending in
    /// a newline) and the memfd in ONE message, so the descriptor cannot be
    /// separated from the line that describes it by a buffered reader.
    pub fn hand_over(&self, to: &UnixStream, line: &[u8]) -> Result<()> {
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut cmsg = SendAncillaryBuffer::new(&mut space);
        let fds = [std::os::fd::AsFd::as_fd(&self.fd)];
        if !cmsg.push(SendAncillaryMessage::ScmRights(&fds)) {
            anyhow::bail!("no room for the descriptor");
        }
        let sent = sendmsg(to, &[IoSlice::new(line)], &mut cmsg, SendFlags::NOSIGNAL)?;
        if sent != line.len() {
            anyhow::bail!("the answer was cut");
        }
        Ok(())
    }
}

fn write_all_at(fd: &OwnedFd, mut bytes: &[u8], mut at: u64) -> Result<()> {
    while !bytes.is_empty() {
        let n = rustix::io::pwrite(fd, bytes, at)?;
        if n == 0 {
            anyhow::bail!("the ring would not take more");
        }
        bytes = &bytes[n..];
        at += n as u64;
    }
    Ok(())
}

fn read_all_at(fd: &OwnedFd, mut into: &mut [u8], mut at: u64) -> Result<()> {
    while !into.is_empty() {
        let n = rustix::io::pread(fd, &mut *into, at)?;
        if n == 0 {
            anyhow::bail!("the ring is shorter than it said");
        }
        into = &mut into[n..];
        at += n as u64;
    }
    Ok(())
}

/// Reads the daemon's answer and the memfd that came with it. `None` for the
/// descriptor when the answer was a refusal, which carries none.
pub fn receive(socket: &UnixStream) -> Result<(String, Option<OwnedFd>)> {
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut cmsg = RecvAncillaryBuffer::new(&mut space);
    let mut buf = [0u8; 1024];
    let got = {
        let mut iov = [IoSliceMut::new(&mut buf)];
        recvmsg(socket, &mut iov, &mut cmsg, RecvFlags::CMSG_CLOEXEC)?.bytes
    };
    let mut fd = None;
    for m in cmsg.drain() {
        if let RecvAncillaryMessage::ScmRights(mut fds) = m {
            fd = fds.next();
        }
    }
    let line = std::str::from_utf8(&buf[..got]).context("the answer is not text")?;
    Ok((line.trim_end().to_owned(), fd))
}

/// A viewer's side.
pub struct Feed {
    socket: UnixStream,
    fd: OwnedFd,
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
    slots: u32,
}

impl Feed {
    /// A feed over `socket`, whose ring is `fd`.
    #[must_use]
    pub const fn new(socket: UnixStream, fd: OwnedFd, width: u32, height: u32, slots: u32) -> Self {
        Self {
            socket,
            fd,
            width,
            height,
            slots,
        }
    }

    /// How long [`Feed::next`] waits for a frame before it answers "none".
    ///
    /// `None` waits for ever, which is right for a camera — frames never stop
    /// while it is open. A paused film sends none, and a viewer that has to
    /// notice something else in the meantime (its own size changed) sets a
    /// patience instead.
    ///
    /// # Errors
    ///
    /// The socket refused the option.
    pub fn patience(&self, wait: Option<std::time::Duration>) -> Result<()> {
        self.socket.set_read_timeout(wait)?;
        Ok(())
    }

    /// Waits for the next frame and copies it into `into`. `Ok(false)` is a
    /// torn slot, dropped — or, under a [`Feed::patience`], no frame in time;
    /// an error is the daemon gone.
    pub fn next(&mut self, into: &mut Vec<u8>) -> Result<bool> {
        let mut slot = [0u8; 1];
        if let Err(e) = self.socket.read_exact(&mut slot) {
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) {
                return Ok(false);
            }
            return Err(e).context("the daemon went away");
        }
        // Only the newest: a viewer that fell behind skips to the last frame
        // announced rather than playing a backlog.
        self.socket.set_nonblocking(true)?;
        let mut more = [0u8; 16];
        while let Ok(n) = self.socket.read(&mut more) {
            if n == 0 {
                break;
            }
            slot[0] = more[n - 1];
        }
        self.socket.set_nonblocking(false)?;
        let s = u64::from(slot[0]) % u64::from(self.slots.max(1));
        let pixels = self.width as usize * self.height as usize * 4;
        let at = s * (HEADER as u64 + pixels as u64);
        let mut head = [0u8; HEADER];
        read_all_at(&self.fd, &mut head, at)?;
        let before = u64::from_le_bytes(head[..8].try_into().unwrap_or([0; 8]));
        into.resize(pixels, 0);
        read_all_at(&self.fd, into, at + HEADER as u64)?;
        let mut again = [0u8; 8];
        read_all_at(&self.fd, &mut again, at)?;
        Ok(before != WRITING && before == u64::from_le_bytes(again))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_goes_round_the_ring_and_back() {
        let (ours, theirs) = UnixStream::pair().expect("pair");
        let mut ring = Ring::new("test", 2, 1).expect("ring");
        let listener = std::thread::spawn(move || {
            let (line, fd) = receive(&theirs).expect("received");
            assert_eq!(line, "hello");
            let mut feed = Feed::new(theirs, fd.expect("a descriptor"), 2, 1, SLOTS);
            let mut frame = Vec::new();
            assert!(feed.next(&mut frame).expect("a frame"));
            frame
        });
        ring.hand_over(&ours, b"hello\n").expect("hand over");
        let slot = ring.publish(&[1, 2, 3, 4, 5, 6, 7, 8]).expect("publish");
        use std::io::Write as _;
        (&ours).write_all(&[slot]).expect("announce");
        assert_eq!(
            listener.join().expect("joins"),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}
