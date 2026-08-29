//! The one thing the brain asks the machine to do — and it asks, it does not
//! do.
//!
//! # It runs no privileged command
//!
//! What is here is `systemctl start <unit>`, and nothing else. The unit is the
//! flake's: what an update IS — which channel, which command, in which order —
//! is declared in Nix and stays there (invariant 5). The brain decides WHEN,
//! and a narrow polkit rule decides whether this user may start that one unit.
//! Nothing here reads a password, holds one, or checks one: there is no polkit
//! agent in the session, so every call is made in a way that cannot ask.
//!
//! # By STORE PATH, never by name
//!
//! `RONOCTUA_SYSTEMCTL` is baked at build time by `nix/rust.nix`, which is what
//! `ronoctua-rescue` already does with the same variable. Under a systemd unit
//! the `PATH` is minimal, and a bare `systemctl` fails far from its cause — the
//! failure would land on the screen somebody opened BECAUSE something was
//! already wrong.
//!
//! # `--no-block`, and why the brain still learns what happened
//!
//! `systemctl start` on a oneshot waits for the job, and waiting is a thread
//! held for the length of a rebuild. `--no-block` returns as soon as systemd
//! has taken the job, and what the run did comes back through the report file
//! the unit writes ([`crate::task::Report`]) — one `stat` per beat, instead of
//! `journalctl` spawned on a clock to parse prose upstream is free to reword.

use std::process::Command;

/// `systemctl`, by store path.
const SYSTEMCTL: Option<&str> = option_env!("RONOCTUA_SYSTEMCTL");

/// What went wrong asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// The build did not say where `systemctl` is.
    ///
    /// Its own case, and not a failure: it means `nix/rust.nix` did not set the
    /// variable, which is a defect in the flake and not in the machine. The
    /// name of the variable travels with it, because that is the fix.
    Missing(&'static str),
    /// It ran and refused. The text is the last thing it said.
    Refused(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(variable) => write!(f, "{variable} was not set at compile time"),
            Self::Refused(said) => write!(f, "{said}"),
        }
    }
}

/// Asks systemd to start a unit, and does not wait for it to finish.
///
/// # Errors
///
/// The build did not bake `systemctl`'s path, or systemd refused the job —
/// which on this machine means polkit did, because there is no agent in the
/// session to ask.
pub fn start(unit: &str) -> Result<(), Trouble> {
    tracing::info!(unit, "asking systemd to start");
    ask("start", unit)
}

/// The arguments a verb takes, so the shape can be checked without a machine.
///
/// # Why this is its own function
///
/// Because the test that used to check it CALLED `start`, and `start` spawns.
/// On a system unit `systemctl start` is authorised by polkit BEFORE systemd
/// even reports that the unit does not exist — so a unit test asking for a
/// deliberately absent unit still raised a real authentication prompt on the
/// owner's laptop, once per run. It was noticed the only way it could be: he
/// was asked for his fingerprint five times and came to ask what was going on.
///
/// A test may not reach the machine. `argv` is what the shape test looks at,
/// and `systemd-is-not-in-a-test` is what keeps it that way.
/// # `--user`, and it is not decoration
///
/// Both units this crate names — the update and its rehearsal — are declared
/// in `nix/modules/update.nix` as `systemd.user.services`. Without the flag,
/// `systemctl` talks to the SYSTEM bus, where neither of them exists.
///
/// It would not merely fail. On the system bus `systemctl start` is authorised
/// by polkit BEFORE systemd looks the unit up, and there is no polkit agent in
/// this session — so the call would sit there until it timed out, raising an
/// authentication request nobody asked for. The owner reported exactly that
/// shape of prompt, from a unit test in this file doing the same thing; this
/// one would have fired every time an update came due, on a machine he might
/// not be sitting at. `updates-never-ask` guards the runner's side of that
/// promise, and this flag is the brain's side of it.
#[must_use]
pub fn argv<'a>(verb: &'a str, unit: &'a str) -> [&'a str; 4] {
    ["--user", "--no-block", verb, unit]
}

/// Runs one verb against one unit. The ONLY place this crate spawns anything.
fn ask(verb: &str, unit: &str) -> Result<(), Trouble> {
    let Some(path) = SYSTEMCTL else {
        return Err(Trouble::Missing("RONOCTUA_SYSTEMCTL"));
    };
    let out = Command::new(path)
        .args(argv(verb, unit))
        .output()
        .map_err(|e| Trouble::Refused(e.to_string()))?;
    if out.status.success() {
        return Ok(());
    }
    // The LAST non-empty line: polkit's refusal is the last thing it says, and
    // the lines before it are the invocation nobody needs to read again.
    let said = String::from_utf8_lossy(&out.stderr);
    let last = said.lines().rev().find(|l| !l.trim().is_empty());
    Err(Trouble::Refused(last.unwrap_or("").to_owned()))
}

/// Asks systemd to stop a unit, and does not wait.
///
/// # Why aborting has to reach systemd at all
///
/// Because the run is not ours. Dropping the watch would leave the rebuild
/// going with nothing on screen saying so — the screen would show *"nothing is
/// happening"* while the machine compiled for an hour. Giving up is what STOPS
/// it, which is the same promise the transfer queue makes about a copy.
///
/// # Errors
///
/// The build did not bake `systemctl`'s path, or systemd refused.
pub fn stop(unit: &str) -> Result<(), Trouble> {
    tracing::info!(unit, "asking systemd to stop");
    ask("stop", unit)
}

/// Whether this build knows where `systemctl` is.
///
/// The screen asks, so it can say *"this build cannot start anything"* instead
/// of offering a key that fails every time it is pressed.
#[must_use]
pub const fn reachable() -> bool {
    SYSTEMCTL.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shape_is_checked_without_touching_the_machine() {
        // THIS TEST USED TO CALL `start`, AND THAT WAS THE DEFECT.
        //
        // It asked for `ronoctua-does-not-exist.service`, reasoning that a unit
        // which is not there cannot do any harm. But `systemctl start` on a
        // SYSTEM unit is authorised by polkit before systemd ever looks the
        // unit up — so every run of `cargo test` raised a real authentication
        // prompt on the owner's laptop. Nothing failed and nothing was logged;
        // it surfaced because he was asked for his fingerprint five times in a
        // row and came to ask why.
        //
        // A unit test may not reach the machine. What is worth checking here is
        // the shape of the invocation, and the shape is a pure function.
        assert_eq!(
            argv("start", "u.service"),
            ["--user", "--no-block", "start", "u.service"]
        );
        assert_eq!(
            argv("stop", "u.service"),
            ["--user", "--no-block", "stop", "u.service"]
        );
        // `--user` FIRST, and the whole argument is on `argv`: both units are
        // user units, and on the SYSTEM bus this call goes through polkit
        // before systemd looks the unit up — which, with no agent in the
        // session, is a prompt that hangs until it times out.
        assert_eq!(argv("start", "u.service")[0], "--user");
        // `--no-block` before the verb: without it `systemctl start` on a
        // oneshot waits for the whole job, which is a thread held for the
        // length of a rebuild.
        assert_eq!(argv("start", "u.service")[1], "--no-block");
        // And the build's own answer about whether it can reach systemd at all
        // — a `const fn` over an `option_env!`, which spawns nothing.
        assert_eq!(reachable(), SYSTEMCTL.is_some());
    }
}
