//! The program's frames, shared with the windows that show them — the ring
//! is `ronoctua-frames`, shared with the player; this is the studio's words
//! around it.
//!
//! # Why memory, and never the virtual camera
//!
//! The app could read the program back from our own `v4l2loopback` device —
//! and so could every other program on the machine. A scene with the SCREEN in
//! it would then be readable by anything that opens a camera, while the person
//! only thought they were looking at a preview. So the preview travels in a
//! memfd handed over the socket to whoever asked, and the loopback gets frames
//! only while the virtual camera is ON.

use std::os::unix::net::UnixStream;

use anyhow::{Context as _, Result};
pub use ronoctua_frames::{Feed, HEADER, SLOTS};

use crate::wire::Say;

/// The daemon's side.
pub struct Ring(ronoctua_frames::Ring);

impl Ring {
    /// A ring for frames of `width × height`.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        Ok(Self(ronoctua_frames::Ring::new(
            "ronoctua-studio-preview",
            width,
            height,
        )?))
    }

    /// The frame size.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        self.0.size()
    }

    /// Writes a frame into the next slot and says which slot it was.
    pub fn publish(&mut self, rgba: &[u8]) -> Result<u8> {
        self.0.publish(rgba)
    }

    /// Hands the ring to a client with the studio's answer line.
    pub fn hand_over(&self, to: &UnixStream) -> Result<()> {
        let (width, height) = self.0.size();
        let mut line = serde_json::to_vec(&Say::Preview {
            width,
            height,
            slots: SLOTS,
        })?;
        line.push(b'\n');
        self.0.hand_over(to, &line)
    }
}

/// Asks for the preview on a fresh connection.
pub fn open(path: &std::path::Path) -> Result<Feed> {
    let mut socket =
        UnixStream::connect(path).with_context(|| format!("no studio at {}", path.display()))?;
    crate::wire::write_line(&mut socket, &crate::wire::Ask::Preview)?;
    let (line, fd) = ronoctua_frames::receive(&socket)?;
    let say: Say = serde_json::from_str(&line).context("the answer is not a preview")?;
    let Say::Preview {
        width,
        height,
        slots,
    } = say
    else {
        anyhow::bail!("the studio refused the preview: {line}");
    };
    let fd = fd.context("the preview came without its memory")?;
    Ok(Feed::new(socket, fd, width, height, slots))
}
