//! The word every door agrees on about a module the system switched off.
//!
//! The registry ITSELF — every module the brain knows, built once and read by
//! every thread, and the refusals that happen at construction — is the
//! daemon's (`ronoctua_brain_daemon::registry`). What crosses is the word a
//! switched-off module answers with, which a door compares and translates.

/// The word a module the SYSTEM switched off gives instead of a moment — its
/// row's reason, `brain-why-system-off`, and what every ask about it is
/// refused with.
pub const SYSTEM_OFF: &str = "system-off";
