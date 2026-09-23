//! The studio: the one process that owns the camera.
//!
//! What a client links is the library half — [`scene`], [`compose`],
//! [`mix`], [`wire`], [`prefs`], [`client`] and [`preview`] — which is pure or
//! client-side. The daemon's half (`sources`, `screen`, `outputs`, `audio`,
//! `engine`) runs only in `ronoctua-studio`, the unit.

#![forbid(unsafe_code)]

pub mod audio;
pub mod client;
pub mod compose;
pub mod engine;
pub mod mix;
pub mod outputs;
pub mod prefs;
pub mod preview;
pub mod scene;
pub mod screen;
pub mod sources;
pub mod wire;
