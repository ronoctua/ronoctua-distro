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

pub mod ambience;
pub mod arrange;
pub mod client;
pub mod decoders;
pub mod find;
pub mod limit;
pub mod lyrics;
pub mod prefs;
pub mod queue;
pub mod remote;
pub mod session;
pub mod subtitles;
pub mod sync;
pub mod tongues;
pub mod wire;

/// Sets, lanes, arrangements, streams and our two file formats — the
/// playlist crate's, named here so a door that links the player needs no
/// second path to them.
pub use ronoctua_playlist::own;

/// Why yt-dlp could not list a playlist — the closed set
/// `wire::Trouble::Listing` carries, named here for the same reason.
pub use ronoctua_ytdlp::Trouble as Listing;

/// The player's name on the session bus for MPRIS:
/// `org.mpris.MediaPlayer2.<instance>`, so the nested loop's player and the
/// installed one are two players on the bus and not a fight over one name.
///
/// HERE, in the crate both sides link: the daemon takes the name, and a door
/// that finds an OLDER daemon closes it through it (`client::quit_by_mpris`) —
/// the one way out every player ever built answers, since an old one may not
/// know the socket's `quit`.
#[must_use]
pub fn mpris_name() -> String {
    format!(
        "org.mpris.MediaPlayer2.{}",
        ronoctua_xdg::name().replace('-', "_")
    )
}
