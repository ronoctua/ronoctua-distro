//! `ronoctua-shell-player-words` — the player as the shell says it on the
//! modeline.
//!
//! What a state of the player says and what a door makes of what the
//! player's watch heard (`player_state`), and the modeline's pair drawn from
//! it (`player_mark`): the footer keeps a copy of the player's state for the
//! whole session and names no feature, so these stand under the footer and
//! the player's view (`ronoctua-shell-player`), on no other shell crate.

#![deny(unsafe_code)]

pub mod key;
pub mod player_mark;
pub mod player_state;
