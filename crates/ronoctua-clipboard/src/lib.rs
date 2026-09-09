//! Clipboard history — the queue and the QR of what was copied.
//!
//! # What this crate does NOT do
//!
//! It does not speak Wayland. What watches the selection is the footer's daemon, which is already
//! a Wayland client with a seat: it receives `wl_data_device.selection` and hands
//! the result over here. It is the same split as `ronoctua-notify`, where the queue and the
//! policy live far from the bus — and it is what makes it possible to prove "a password does not
//! get in" and "a pinned entry survives the ceiling" without bringing up any compositor.
//!
//! # Why the capture is NOT `wl-paste --watch`
//!
//! Three reasons, and none is taste: one process per copy; the MIME types
//! would arrive through a second call (and it is from them that the password filter comes);
//! and `wl-paste` spawns by NAME, which under a systemd unit is the minimal-`PATH`
//! trap — it cost three rounds in this very session.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod decode;
pub mod image;
pub mod qr;
pub mod queue;

pub use decode::{Undecodable, decode};
pub use queue::{Content, Entry, Queue};
