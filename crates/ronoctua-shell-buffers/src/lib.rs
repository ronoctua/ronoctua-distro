//! `ronoctua-shell-buffers` — the host of the WASM buffers: a native buffer
//! in an `xdg_toplevel` of its own (section 7.5).
//!
//! The component sees no token, no language and no Wayland (invariant 11);
//! this crate resolves all three — the display list drawn through the kit's
//! painter, the keys and the pointer turned into the buffer's inputs, the
//! machine's facts a buffer is handed (the disks' rack, the queue in words).
//! A door of the binary's own (`ronoctua-shell buffer <component>`), which
//! hands it the queue's words (`buffer::QueueWords`): it stands on the kit
//! and the door, and on nothing of the footer or of any other view.

// `deny`, not `forbid`: the window's GPU surface passes Wayland's pointers to
// wgpu through the door, which holds its one unsafe block.
#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{brainlink, door, queue};
use ronoctua_shell_kit::{
    bars, chrome, clipboard_copy, ipc, keyboard, language, line, mind_wire, paint, seats, wheel,
};

pub mod buffer;
mod disks;
pub mod key;
