//! The player: one process that plays everything, and the words to talk to it.
//!
//! # Two crates, and only one links libmpv
//!
//! This one is what the shell links: the protocol, the client and the pure
//! rules (lyrics, keeping in step, which decoder a format needs). The ENGINE —
//! libmpv, the render threads, the decoders' pipes — is
//! `ronoctua-player-engine`, the daemon's own crate. A cargo feature would not
//! have done: a workspace build unifies features, and the shell would have
//! linked libmpv the moment the daemon was built beside it.

#![deny(unsafe_code)]

pub mod client;
pub mod decoders;
pub mod lrclib;
pub mod lyrics;
pub mod prefs;
pub mod remote;
pub mod sponsorblock;
pub mod sync;
pub mod wire;
