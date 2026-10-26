//! The keyboard's safety net — §7.4.
//!
//! # Why this exists
//!
//! The leader is the Super key on its own, and whoever tells **tapping** from
//! **holding** is `keyd`, a daemon below the compositor
//! (`nix/modules/keyboard.nix`). If it goes down, Super goes back to being only
//! a modifier: tapping it no longer emits `F13`, and with that the command
//! list, the `M-x` and every sequence shortcut disappear.
//!
//! The system does not become unusable — the chords carry on, because they use
//! Super as a actual modifier — but the path to *every command by name* closes.
//! It is exactly when it is needed.
//!
//! # How one gets out of there
//!
//! Two binds outside the leader (`actions::emergency`), and this module is the
//! second one: `Ctrl+Shift+L` runs `keyboard-restore`, which restarts the
//! service. It **reports** what happened, always — restoring silently is
//! indistinguishable from not restoring, and whoever is left with a
//! half-broken keyboard has no patience for guessing.

use std::process::Command;

/// The `systemctl`, by path and not by name.
///
/// The footer runs as a user unit, and a unit's `PATH` is minimal — a bare
/// name would fail with "Node such file or directory" somewhere far from the
/// cause. Outside Nix (`cargo run`), it falls back to the name and the shell's
/// `PATH` resolves it.
const SYSTEMCTL_PATH: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// The unit `nix/modules/keyboard.nix` defines.
///
/// The polkit rule allows **this** unit and no other; changing the name here
/// without changing it there gives a password prompt on a screen with no
/// trustworthy keyboard.
const UNIT: &str = "keyd.service";

/// Restarts the keyboard daemon and says what happened.
///
/// # Errors
///
/// The `systemctl` could not be run, polkit refused, or the service did not
/// stay active after the restart. All three come back with the text systemd
/// gave — inventing a message of our own here would wipe the only clue.
pub fn restore_it() -> Result<(), String> {
    let output = Command::new(SYSTEMCTL_PATH)
        .args(["restart", UNIT])
        .output()
        .map_err(|e| format!("{SYSTEMCTL_PATH}: {e}"))?;

    if !output.status.success() {
        // systemctl's stderr is where polkit says it refused. Without it, the
        // message would be "it failed" and nothing else.
        let reason = String::from_utf8_lossy(&output.stderr);
        let reason = reason.trim();
        return Err(if reason.is_empty() {
            format!(
                "{SYSTEMCTL_PATH} restart {UNIT} exited with {}",
                output.status
            )
        } else {
            reason.to_owned()
        });
    }

    // RESTARTING IS NOT STANDING. A `restart` of a unit that dies right after
    // gives back success, and the keyboard would still be without a leader — with
    // a message saying it worked, which is worse than having no message.
    if !active() {
        return Err(format!("{UNIT} restarted and did not stay active"));
    }
    Ok(())
}

/// Is the unit active right now?
fn active() -> bool {
    active_unit(UNIT)
}

/// Whether keyd — Together mode's daemon — is active right now.
///
/// The footer asks before it reclaims Independent mode: if keyd took the
/// keyboards back (a `nixos-rebuild switch` starts it, it being enabled), the
/// remapper goes back on; if the remapper merely died, keyd is NOT active —
/// systemd's own `Restart`/`onFailure` answers that, and the footer keeps out
/// of it rather than trading places with keyd in a loop (`crate::surface`).
#[must_use]
pub fn keyd_is_active() -> bool {
    active_unit(UNIT)
}

fn active_unit(unit: &str) -> bool {
    Command::new(SYSTEMCTL_PATH)
        .args(["is-active", "--quiet", unit])
        .status()
        .is_ok_and(|s| s.success())
}

/// The unit `nix/modules/keys.nix` defines — Independent mode's remapper.
///
/// The polkit rule there names this and keyd, `start` and `stop`, and nothing
/// else; the two units conflict, so starting one stops the other.
const KEYS_UNIT: &str = "ronoctua-keys.service";

/// Puts the keyboards in `mode`: renders the remapper's rules when it is
/// Independent, then starts the unit that mode runs — which stops the other.
///
/// # Errors
///
/// The rules could not be written, polkit refused, or the unit did not stay
/// active; systemd's own words come back, as `restore_it`'s do.
pub fn switch_to(mode: ronoctua_keyboards::Mode) -> Result<(), String> {
    let unit = match mode {
        ronoctua_keyboards::Mode::Together => UNIT,
        ronoctua_keyboards::Mode::Independent => {
            render_keys()?;
            KEYS_UNIT
        }
    };
    if active_unit(unit) {
        return Ok(());
    }
    let output = Command::new(SYSTEMCTL_PATH)
        .args(["start", unit])
        .output()
        .map_err(|e| format!("{SYSTEMCTL_PATH}: {e}"))?;
    if !output.status.success() {
        let reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if reason.is_empty() {
            format!(
                "{SYSTEMCTL_PATH} start {unit} exited with {}",
                output.status
            )
        } else {
            reason
        });
    }
    if !active_unit(unit) {
        return Err(format!("{unit} started and did not stay active"));
    }
    Ok(())
}

/// Writes what the remapper reads, from the person's keyboard preferences
/// and the units plugged in now. The daemon reloads it on its own (inotify).
///
/// # Errors
///
/// The directory is not there — a machine built without `keys.nix`.
pub fn render_keys() -> Result<(), String> {
    let prefs = ronoctua_keyboards::store().read();
    let units = ronoctua_keyboards::units().unwrap_or_default();
    let config = ronoctua_keys::config::render(&prefs, &units);
    ronoctua_keys::config::write(ronoctua_keys::config::DIRECTORY, &config)
        .map_err(|e| format!("{e:#}"))
}

/// Applies the saved mode, once, when the session starts.
///
/// keyd is what the machine boots with; a person who chose Independent gets
/// the remapper when their session begins — never through an `enable` that
/// would outlive the choice (`keys.nix`).
pub fn apply_saved_mode() {
    let mode = ronoctua_keyboards::store().read().mode;
    if mode == ronoctua_keyboards::Mode::Independent
        && let Err(e) = switch_to(mode)
    {
        tracing::warn!(error = %e, "the saved keyboard mode could not be applied; keyd stays");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_unit_is_the_same_as_the_polkit_rules() {
        // The name travels to Nix as a literal in the polkit rule and to here as a
        // constant. They are two files, and a divergence between them is silent:
        // the `systemctl` would ask for a password instead of refusing. This assert
        // does not close the hole on its own — whoever closes it is the
        // `keyboard-vm`, which really restarts as the user —, but it anchors the name.
        assert_eq!(super::UNIT, "keyd.service");
        assert_eq!(super::KEYS_UNIT, "ronoctua-keys.service");
    }

    #[test]
    fn the_systemctl_path_is_not_empty() {
        // An `option_env!` giving back `Some("")` would give a `Command::new("")`,
        // which fails with "Node such file or directory" without saying what.
        assert!(!super::SYSTEMCTL_PATH.is_empty());
    }
}
