//! The player's ceilings, as a refusal a door can say.
//!
//! One type for the queue's and the session's, so the wire has one
//! `too-many` trouble and a door one sentence per [`Limit`], with the number
//! in it — never a request that silently did nothing.

use serde::{Deserialize, Serialize};

/// What there would have been too many of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Limit {
    /// Things to play in the queue ([`crate::queue::LEAVES_MOST`]).
    Leaves,
    /// Playlists inside playlists ([`crate::queue::DEPTH_MOST`]).
    Depth,
    /// Lanes in one set ([`crate::session::LANES_MOST`]).
    Lanes,
    /// Subtitle lanes over one picture
    /// ([`crate::session::SUBTITLES_PER_PICTURE`]).
    Subtitles,
    /// Subtitles drawn as pictures (PGS, VobSub) over one picture
    /// ([`crate::subtitles::show::IMAGES_PER_PICTURE`]): mpv shows them on
    /// its secondary track, one at a time.
    SubtitlePictures,
    /// Media players running for one set ([`crate::session::VOICES_MOST`]).
    Voices,
    /// Pictures of one set ([`crate::session::PICTURES_MOST`]).
    Pictures,
    /// Ambient sounds playing together (the catalogue's own number,
    /// `atOnce` in `assets/ambience.nix`).
    Ambience,
    /// Several-line fields open in the editor at once
    /// ([`crate::wire::DRAFTS_MOST`]).
    Drafts,
    /// Changes to files waiting behind the one being written
    /// ([`crate::wire::WRITES_WAITING_MOST`]).
    Writes,
    /// Takes of one desk waiting behind the one being fetched
    /// ([`crate::wire::TAKES_WAITING_MOST`]).
    Takes,
}

/// A request refused because it would pass a ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TooMany {
    /// What there would have been too many of.
    pub what: Limit,
    /// The most there may be.
    pub most: usize,
}
