//! The one boundary with a media file's tags and covers: read them all,
//! write what a person changed.
//!
//! # What lives here
//!
//! - [`sheet`]: what a file says about itself — the schema that crosses the
//!   player's socket. Pure, and all a door needs (`default-features =
//!   false`).
//! - [`can`]: which writer each container gets, the one table.
//! - [`stream`]: the read-only sheet of something that plays from the
//!   network, out of what it says while it plays. Pure too.
//! - [`read`] (feature `engine`): a whole sheet out of lofty, ffprobe,
//!   exiftool and Matroska's own tools.
//! - [`write`] (feature `engine`): a change, written into a draft beside the
//!   file and renamed over it; covers in, out and away.
//!
//! # Who calls it
//!
//! The player's daemon, on a worker thread of its own: every function under
//! `engine` blocks (programs, disk), and nothing here starts a thread. No
//! door calls the engine half — a door asks the daemon, which is the one
//! process that knows what is playing and can tell every other door the file
//! changed.
//!
//! # What it never does
//!
//! Edit a file in place. mpv may be playing it, and a rewrite that moves the
//! sound under a reader is a crash in the reader. Every write goes to
//! `.<name>.ronoctua-<pid>.part` in the same folder — a reflink when the
//! filesystem clones, a copy when it does not — and ends in one `rename`,
//! after which mpv still holds the old inode and plays on.

pub mod can;
pub mod sheet;
pub mod stream;

#[cfg(feature = "engine")]
mod matroska;
#[cfg(feature = "engine")]
mod native;
#[cfg(feature = "engine")]
mod picture;
#[cfg(feature = "engine")]
mod probe;
#[cfg(feature = "engine")]
pub mod read;
#[cfg(feature = "engine")]
mod run;
#[cfg(feature = "engine")]
pub mod write;

#[cfg(all(test, feature = "engine"))]
mod fixture;

#[cfg(feature = "engine")]
pub use run::Tools;
pub use sheet::*;
