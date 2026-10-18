//! Syncthing's refusals, in the person's language — said by the footer, whose
//! thread carries every order of its own and of the Syncthing view's region,
//! and by the view's window, whose thread carries its own.
//!
//! Below both because the footer says them with no view open (pause every
//! folder, resume them), and it names no feature: what both speak lives below
//! both — in a crate of its own, and not in the kit, so a line of the
//! Syncthing crate never recompiles the foundation.

/// The sentence one refusal gets.
///
/// ONE PLACE, so the two doors — and the footer, which says what its own
/// thread was refused — cannot disagree about what a failure was.
#[must_use]
pub const fn refusal_key(why: ronoctua_syncthing::Refusal) -> &'static str {
    use crate::key;
    match why {
        ronoctua_syncthing::Refusal::NotRunning => key::ECHO_SYNCTHING_NOT_RUNNING,
        ronoctua_syncthing::Refusal::NotAllowed => key::ECHO_SYNCTHING_NOT_ALLOWED,
        ronoctua_syncthing::Refusal::Refused | ronoctua_syncthing::Refusal::Unknown => {
            key::ECHO_SYNCTHING_UNKNOWN
        }
    }
}
