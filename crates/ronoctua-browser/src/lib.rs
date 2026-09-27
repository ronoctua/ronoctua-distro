//! The person's RUNNING Chrome, and the bridge to it.
//!
//! # One border
//!
//! Everything that talks to a Chrome that is running lives here: the
//! native-messaging framing and the caller-origin argument Chrome passes
//! ([`native`]), the host process Chrome starts for our extension ([`host`]),
//! the wire the extension, the host and the footer share ([`wire`]), the
//! footer's socket ([`serve`]), a profile's files that no extension API reveals
//! ([`profile`]), the command line that opens a kiosk window ([`launch`]) and
//! which addresses a kiosk may be sent to ([`url`]).
//!
//! # What a kiosk is
//!
//! An ordinary window of the person's own Chrome — their profile, their
//! logins — whose tab strip and toolbar the COMPOSITOR hides by telling it it
//! is fullscreen while its tile keeps its size (`fullscreen application`).
//! Never Chromium's `--kiosk`, which would lock the whole browser.

pub mod host;
pub mod launch;
pub mod native;
pub mod profile;
pub mod serve;
pub mod url;
pub mod wire;
