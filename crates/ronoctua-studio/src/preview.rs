//! The program's frames, shared with the windows that show them.
//!
//! # Why memory, and never the virtual camera
//!
//! The app could read the program back from our own `v4l2loopback` device —
//! and so could every other program on the machine. A scene with the SCREEN in
//! it would then be readable by anything that opens a camera, while the person
//! only thought they were looking at a preview. So the preview travels in a
//! memfd handed over the socket (`SCM_RIGHTS`) to whoever asked, and the
//! loopback gets frames only while the virtual camera is ON.
//!
//! # The ring
//!
//! [`SLOTS`] slots of `HEADER + width·height·4` bytes. The daemon writes the
//! slot AFTER the one it last announced, then sends one byte naming it; a
//! window reads the named slot and uploads it. The header's sequence number is
//! written as `u64::MAX` before the pixels and as the frame's number after, and
//! a reader checks it before AND after copying — a slot overwritten under the
//! copy (a window two frames behind) is dropped rather than shown torn.
//!
//! No `mmap` and no `unsafe`: both sides use `pread`/`pwrite` on the memfd,
//! and the one copy a reader makes is the copy the upload needed anyway.

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

use crate::wire::Say;

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
    /// A ring for frames of `width × height`.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let fd =
            rustix::fs::memfd_create("ronoctua-studio-preview", rustix::fs::MemfdFlags::CLOEXEC)?;
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
    pub fn size(&self) -> (u32, u32) {
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
        Ok(slot as u8)
    }

    /// Hands the ring to a client: the answer line and the memfd in ONE
    /// message, so the descriptor cannot be separated from the line that
    /// describes it by a buffered reader on the other side.
    pub fn hand_over(&self, to: &UnixStream) -> Result<()> {
        let mut line = serde_json::to_vec(&Say::Preview {
            width: self.width,
            height: self.height,
            slots: SLOTS,
        })?;
        line.push(b'\n');
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut cmsg = SendAncillaryBuffer::new(&mut space);
        let fds = [std::os::fd::AsFd::as_fd(&self.fd)];
        if !cmsg.push(SendAncillaryMessage::ScmRights(&fds)) {
            anyhow::bail!("no room for the descriptor");
        }
        let sent = sendmsg(to, &[IoSlice::new(&line)], &mut cmsg, SendFlags::NOSIGNAL)?;
        if sent != line.len() {
            anyhow::bail!("the preview answer was cut");
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

/// A window's side.
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
    /// Asks for the preview on a fresh connection.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let mut socket = UnixStream::connect(path)
            .with_context(|| format!("no studio at {}", path.display()))?;
        crate::wire::write_line(&mut socket, &crate::wire::Ask::Preview)?;
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut cmsg = RecvAncillaryBuffer::new(&mut space);
        let mut buf = [0u8; 512];
        let got = {
            let mut iov = [IoSliceMut::new(&mut buf)];
            recvmsg(&socket, &mut iov, &mut cmsg, RecvFlags::CMSG_CLOEXEC)?.bytes
        };
        let mut fd = None;
        for m in cmsg.drain() {
            if let RecvAncillaryMessage::ScmRights(mut fds) = m {
                fd = fds.next();
            }
        }
        let line = std::str::from_utf8(&buf[..got]).context("the answer is not text")?;
        let say: Say =
            serde_json::from_str(line.trim_end()).context("the answer is not a preview")?;
        let Say::Preview {
            width,
            height,
            slots,
        } = say
        else {
            anyhow::bail!("the studio refused the preview: {line}");
        };
        let fd = fd.context("the preview came without its memory")?;
        Ok(Self {
            socket,
            fd,
            width,
            height,
            slots,
        })
    }

    /// Waits for the next frame and copies it into `into`. `Ok(false)` is a
    /// torn slot, dropped; an error is the studio gone.
    pub fn next(&mut self, into: &mut Vec<u8>) -> Result<bool> {
        let mut slot = [0u8; 1];
        self.socket
            .read_exact(&mut slot)
            .context("the studio went away")?;
        // Only the newest: a window that fell behind skips to the last frame
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
        let mut ring = Ring::new(2, 1).expect("ring");
        let listener = std::thread::spawn(move || {
            // The client half, reading its answer the way `Feed::open` does.
            let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut cmsg = RecvAncillaryBuffer::new(&mut space);
            let mut buf = [0u8; 512];
            let got = {
                let mut iov = [IoSliceMut::new(&mut buf)];
                recvmsg(&theirs, &mut iov, &mut cmsg, RecvFlags::CMSG_CLOEXEC)
                    .expect("recv")
                    .bytes
            };
            let mut fd = None;
            for m in cmsg.drain() {
                if let RecvAncillaryMessage::ScmRights(mut fds) = m {
                    fd = fds.next();
                }
            }
            let say: Say =
                serde_json::from_str(std::str::from_utf8(&buf[..got]).expect("text").trim_end())
                    .expect("json");
            assert_eq!(
                say,
                Say::Preview {
                    width: 2,
                    height: 1,
                    slots: SLOTS
                }
            );
            let mut feed = Feed {
                socket: theirs,
                fd: fd.expect("a descriptor"),
                width: 2,
                height: 1,
                slots: SLOTS,
            };
            let mut frame = Vec::new();
            assert!(feed.next(&mut frame).expect("a frame"));
            frame
        });
        ring.hand_over(&ours).expect("hand over");
        let slot = ring.publish(&[1, 2, 3, 4, 5, 6, 7, 8]).expect("publish");
        use std::io::Write as _;
        (&ours).write_all(&[slot]).expect("announce");
        assert_eq!(
            listener.join().expect("joins"),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}
