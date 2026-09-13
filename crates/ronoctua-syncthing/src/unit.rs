//! The systemd unit — the one thing about Syncthing the REST API cannot do.
//!
//! # Why this needs a polkit rule
//!
//! Everything else on the screen goes through `127.0.0.1`: pausing, resuming,
//! rescanning, adding a folder, writing an option, even restarting the process.
//! None of it is privileged. What is privileged is telling systemd to STOP the
//! unit, and there is no way around it: `services.syncthing` puts a system
//! unit on the machine, running as the owner, and `manage-units` on a system
//! unit is root's.
//!
//! Without the rule, "turn Syncthing off" would be the only thing in this view
//! that answered by telling the person to open a terminal — which is the
//! failure mode the whole shell exists to remove.
//!
//! The rule is `nix/modules/syncthing.nix`, in the exact shape
//! `nix/modules/tor.nix` set: the `manage-units` action, the verbs `start` and
//! `stop`, THIS unit, that one user, and a refusal for everything else.
//! `syncthing-polkit-is-narrow` reads the installed rule and [`UNIT`] and
//! compares them in BOTH directions, so a rule that grew and a rule that
//! stopped matching this file each fail.
//!
//! There is no polkit agent in this session, so the rule answers `YES` rather
//! than asking. What buys that width back is that the door is one millimetre
//! wide.

/// The unit the rule names, and the only one this crate ever touches.
pub const UNIT: &str = "syncthing.service";

/// `systemctl`, baked at build time.
///
/// By its STORE PATH and not by name: in a systemd unit the `PATH` is minimal,
/// and the failure would show up far from the cause (AGENTS.md).
pub const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};

/// Whether the unit is up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Standing {
    /// systemd says it is active.
    Running,
    /// systemd says it is not.
    Stopped,
    /// Coming up or going down.
    Moving,
    /// There is no such unit on this machine.
    ///
    /// NOT the same as stopped, and the difference reaches the modeline: a
    /// machine that never installed Syncthing has nothing to say about it, and
    /// a mark there would be a glyph about a program that does not exist. Only
    /// `LoadState` tells the two apart — `is-active` answers `inactive` for
    /// both.
    Absent,
    /// `systemctl` could not be asked.
    ///
    /// NOT the same as stopped either: a build sandbox and a container without
    /// systemd would otherwise be one word with a laptop whose service is off,
    /// and the screen would tell two of them something false.
    #[default]
    Unknown,
}

impl Standing {
    /// The id, which is the stem of the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Moving => "moving",
            Self::Absent => "absent",
            Self::Unknown => "unknown",
        }
    }

    /// What `systemctl show -p LoadState -p ActiveState --value` said.
    ///
    /// Two lines, in that order. `loaded` plus an active state is the ordinary
    /// case; `not-found` is a machine that never installed it, whatever the
    /// second line says.
    #[must_use]
    pub fn of(said: &str) -> Self {
        let mut lines = said.trim().lines();
        let loaded = lines.next().unwrap_or("").trim();
        let active = lines.next().unwrap_or("").trim();
        if loaded == "not-found" {
            return Self::Absent;
        }
        if loaded.is_empty() {
            return Self::Unknown;
        }
        match active {
            "active" => Self::Running,
            "inactive" | "failed" => Self::Stopped,
            "activating" | "deactivating" | "reloading" => Self::Moving,
            _ => Self::Unknown,
        }
    }
}

/// Asks systemd how the unit is.
///
/// TWO QUESTIONS IN ONE CALL, and the first one is the one that is easy to
/// miss. `is-active` answers `inactive` both for a unit that is stopped and
/// for a unit that DOES NOT EXIST; only `LoadState` tells them apart, and the
/// difference reaches the modeline — a machine that never installed Syncthing
/// would otherwise carry a mark about a program it does not have.
///
/// The exit code is not the answer either: `is-active` exits non-zero for an
/// inactive unit, so reading it would make "stopped" and "systemctl is not
/// here" the same thing.
#[must_use]
pub fn standing() -> Standing {
    match std::process::Command::new(SYSTEMCTL)
        .args([
            "show",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            "--value",
            UNIT,
        ])
        .output()
    {
        Ok(out) => Standing::of(&String::from_utf8_lossy(&out.stdout)),
        Err(e) => {
            tracing::debug!(error = %e, "systemctl could not be asked about syncthing");
            Standing::Unknown
        }
    }
}

/// Starts the unit.
///
/// # Errors
///
/// systemd refused, or `systemctl` is not there.
pub fn start() -> anyhow::Result<()> {
    verb("start")
}

/// Stops the unit.
///
/// # Errors
///
/// systemd refused, or `systemctl` is not there.
pub fn stop() -> anyhow::Result<()> {
    verb("stop")
}

/// One verb, and systemd's own words when it says no.
fn verb(what: &'static str) -> anyhow::Result<()> {
    let out = std::process::Command::new(SYSTEMCTL)
        .args([what, UNIT])
        .output()
        .map_err(|e| anyhow::anyhow!("{SYSTEMCTL}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    if said.is_empty() {
        anyhow::bail!("systemctl {what} {UNIT} exited with {}", out.status);
    }
    anyhow::bail!("{said}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_stopped_and_unaskable_are_three_different_words() {
        // THE ONE THAT COST A VM RUN: `is-active` answers `inactive` both for a
        // unit that is stopped and for one that does not exist, so a machine
        // that never installed Syncthing carried a mark about a program it does
        // not have. Only `LoadState` tells them apart.
        assert_eq!(Standing::of("loaded\nactive"), Standing::Running);
        assert_eq!(Standing::of("loaded\ninactive\n"), Standing::Stopped);
        assert_eq!(Standing::of("loaded\nfailed"), Standing::Stopped);
        assert_eq!(Standing::of("loaded\nactivating"), Standing::Moving);
        assert_eq!(Standing::of("not-found\ninactive"), Standing::Absent);
        // AND A BUILD SANDBOX, where there is no systemd to ask at all.
        assert_eq!(Standing::of(""), Standing::Unknown);
        assert_eq!(Standing::of("loaded\nsomething new"), Standing::Unknown);
        assert_eq!(Standing::default(), Standing::Unknown);
    }

    #[test]
    fn the_unit_is_named_once_and_the_polkit_check_reads_it_here() {
        // `syncthing-polkit-is-narrow` compares the installed rule against
        // THIS constant, in both directions. A second spelling anywhere would
        // make the two agree with each other and with nothing else.
        assert_eq!(UNIT, "syncthing.service");
        let source = include_str!("unit.rs");
        let code = source.split("#[cfg(test)]").next().unwrap();
        assert_eq!(
            code.matches("\"syncthing.service\"").count(),
            1,
            "the unit name is written more than once"
        );
    }

    #[test]
    fn systemctl_is_a_path_when_nix_built_this() {
        // Outside Nix the fallback is a bare name, and that is deliberate:
        // `cargo build` keeps working. Inside, it must be a store path.
        if option_env!("RONOCTUA_SYSTEMCTL").is_some() {
            assert!(
                SYSTEMCTL.starts_with('/'),
                "a bare name fails at EXEC inside a unit, far from the cause"
            );
        }
    }
}
