//! Raising the shields, as the person, with no password: the one loosening-free
//! change of the firewall, by the narrow polkit YES `security.nix` declares —
//! `start` of one unit, for one user.
//!
//! Tightening is free and loosening goes through the sudo door: lowering the
//! shields is a wish staged and applied through [`crate::authority`]; raising
//! them is this, from the page and from the modeline's click alike.

use crate::tools::{Tools, given};
use crate::{Refusal, Why, refused};

/// The unit that raises them (`ronoctua-firewall-shields.service`).
pub const UNIT: &str = "ronoctua-firewall-shields.service";

/// `systemctl`'s arguments: never asking (there is no agent in the session,
/// and a question nobody sees would be a wait for nothing), and starting the
/// one unit the rule lets. Pure.
#[must_use]
pub const fn arguments() -> [&'static str; 3] {
    ["--no-ask-password", "start", UNIT]
}

/// Raises the shields. Blocks until the unit is done (a oneshot): a thread.
///
/// # Errors
///
/// [`Why::Unstartable`] without `systemctl`, [`Why::Failed`] when polkit or
/// the unit refused (the journal has the words).
pub fn raise() -> Result<(), Refusal> {
    let tools = Tools::baked();
    let systemctl = given(tools.systemctl.as_ref())?;
    crate::tools::run(systemctl, &arguments(), None)
        .map(|_| ())
        .map_err(|r| {
            if r.why == Why::Unstartable {
                r
            } else {
                refused(Why::Failed)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the call could stop to ask a password nobody sees, or
    /// starts anything but the one unit the polkit rule lets.
    #[test]
    fn the_shields_are_one_unit_started_without_asking() {
        assert_eq!(
            arguments(),
            [
                "--no-ask-password",
                "start",
                "ronoctua-firewall-shields.service"
            ]
        );
    }
}
