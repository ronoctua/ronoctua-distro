//! `/var/lib/ronoctua-machine/state.json` — the machine's own record, written
//! by `ronoctua-machine` and published to `plain/person/machines/<hostname>.json`.
//!
//! ```json
//! {"format": 1,
//!  "last-good": {"toplevel": "/nix/store/…", "entry": "nixos-generation-41.conf",
//!                "distro-rev": "…", "blessed-at": "2026-10-05T12:00:00Z"},
//!  "trial": {"toplevel": "/nix/store/…", "outcome": "pending"},
//!  "hold": null,
//!  "last-apply": {"at": "2026-10-05T12:00:00Z", "result": "switched"}}
//! ```
//!
//! A last apply whose build could not fetch something lists it under
//! `"missing"` ([`crate::frontier::Download`]); the key is absent otherwise.
//! One that set a system up — switched to, or `"staged"` for the next boot —
//! names it under `"toplevel"`; the key is absent otherwise.
//!
//! # Fields that joined format 1
//!
//! `missing`, `no-trial`, `toplevel` and the result `staged` came after the
//! first records were written, and the format stayed 1: an absent key reads
//! as it always did, and every record written without them reads back byte
//! for byte. A reader older than one of them refuses a record that carries
//! it as `shape`.
//!
//! Across machines, that reader is gone at each one's next update. On ONE
//! machine it is not always: the boot trial's way back can land on a system
//! built before the reader knew `staged` — the generation that first brought
//! it was switched to live with a new kernel, its trial still pending, so
//! the last good system predates it, and the next update set a system for
//! the next boot that did not start. That older system's bless and apply
//! refuse the record (`unreadable`): nothing judges the trial, and every
//! update fails at the rebuild until `last-apply` is taken out of the
//! machine's `state.json` by hand (or the file, which reads as a new record).
//! Keeping format 1 with that window, writing `staged` only once the last
//! good system reads it, or a new format is the owner's decision
//! (`docs/ARCHITECTURE.md`, "O formato 1 tem uma janela em aberto").
//!
//! `"no-trial": "no-efi-variables"` joins them only while the system the last
//! switch made has no boot trial ([`NoTrial`]); the key is absent otherwise.

use serde::{Deserialize, Serialize};

use crate::{Document, Refusal, Stamp, Why, check};

/// The record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MachineState {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The last generation that booted and was blessed; `None` before the
    /// first bless.
    pub last_good: Option<LastGood>,
    /// The generation on trial, if one is.
    pub trial: Option<Trial>,
    /// The distro revision the machine is held at, if it is.
    pub hold: Option<Hold>,
    /// The last apply, if one ran.
    pub last_apply: Option<LastApply>,
    /// Why the system the last switch made has no boot trial — no automatic
    /// way back but the boot menu — until it has booted once; `None` (and
    /// absent from the file) otherwise. Applies that switch nothing leave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_trial: Option<NoTrial>,
}

impl Default for MachineState {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            last_good: None,
            trial: None,
            hold: None,
            last_apply: None,
            no_trial: None,
        }
    }
}

/// Why a switch armed no boot trial (`machine-no-trial-<id>` on screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoTrial {
    /// The EFI variables cannot be read or written (no EFI, read-only
    /// `efivarfs`, a boot loader `bootctl` does not know).
    NoEfiVariables,
    /// The boot menu lists the new system or the last good one under no
    /// entry `bootctl` can name.
    NotInMenu,
    /// No last good system is known to fall back to.
    NoLastGood,
}

impl NoTrial {
    /// Every one.
    pub const ALL: [Self; 3] = [Self::NoEfiVariables, Self::NotInMenu, Self::NoLastGood];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoEfiVariables => "no-efi-variables",
            Self::NotInMenu => "not-in-menu",
            Self::NoLastGood => "no-last-good",
        }
    }
}

/// The last good generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LastGood {
    /// Its toplevel.
    pub toplevel: String,
    /// Its boot entry (`bootctl list`'s id, or GRUB's title path).
    pub entry: String,
    /// The distro revision it was built from.
    pub distro_rev: String,
    /// When it was blessed.
    pub blessed_at: Stamp,
}

/// How a boot trial ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrialOutcome {
    /// Armed (`bootctl set-oneshot`), not booted yet.
    Pending,
    /// Booted and blessed.
    Good,
    /// Did not boot well: the machine went back to the last good one.
    Failed,
    /// Kernel, initrd and parameters identical to the last good one: blessed
    /// at once, no reboot needed to know.
    BootIdentical,
}

/// The generation on trial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Trial {
    /// Its toplevel.
    pub toplevel: String,
    /// How it stands.
    pub outcome: TrialOutcome,
}

/// A hold at a distro revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Hold {
    /// The revision.
    pub rev: String,
    /// Since when.
    pub since: Stamp,
}

/// How an apply ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplyResult {
    /// Built and switched.
    Switched,
    /// Switched, and some units the switch started failed (exit 4).
    SwitchedWithFailures,
    /// The build failed; the lock was restored.
    BuildFailed,
    /// The switch failed.
    SwitchFailed,
    /// A rehearsal: nothing changed.
    Rehearsed,
    /// Refused before anything ran (a hold, the last good one would fall out
    /// of the boot menu, another run).
    Refused,
    /// Built and set for the NEXT BOOT (`switch-to-configuration boot`):
    /// nothing that runs was stopped or restarted, and the new system is
    /// used from the next restart, the boot trial armed for it. Until then
    /// the machine runs the system it ran, and [`LastApply::toplevel`] names
    /// the one that waits.
    Staged,
}

/// The last apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LastApply {
    /// When it ended.
    pub at: Stamp,
    /// How.
    pub result: ApplyResult,
    /// The system the apply set up — switched to, or set for the next boot
    /// ([`ApplyResult::Staged`]); `None` (and absent from the file) for an
    /// apply that set up none. What reads the system an update made — the
    /// binary cache's push, the parts' plan of the next boot — reads it
    /// from here, never from the system profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toplevel: Option<String>,
    /// The downloads its build could not fetch ([`crate::frontier`]): the
    /// system's, when the build failed, or the hardware's that waits for a
    /// network, when the machine was built without it again. Each is what
    /// the update's page offers to hand in or bring on a stick. Empty (and
    /// absent from the file) when nothing was missing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<crate::frontier::Download>,
}

impl Document for MachineState {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if let Some(g) = &self.last_good {
            if !check::store_path(&g.toplevel) {
                return Err(Refusal::new(
                    Why::Invalid,
                    "/last-good/toplevel",
                    g.toplevel.as_str(),
                ));
            }
            if !check::entry(&g.entry) {
                return Err(Refusal::new(
                    Why::Invalid,
                    "/last-good/entry",
                    g.entry.as_str(),
                ));
            }
        }
        if let Some(t) = &self.trial
            && !check::store_path(&t.toplevel)
        {
            return Err(Refusal::new(
                Why::Invalid,
                "/trial/toplevel",
                t.toplevel.as_str(),
            ));
        }
        if let Some(h) = &self.hold
            && !check::token(&h.rev)
        {
            return Err(Refusal::new(Why::Invalid, "/hold/rev", h.rev.as_str()));
        }
        // THE SYSTEM AN APPLY SET UP is a store path: what reads it (the
        // binary cache's push, the parts' plan) joins a file to it.
        if let Some(top) = self.last_apply.as_ref().and_then(|a| a.toplevel.as_ref())
            && !check::store_path(top)
        {
            return Err(Refusal::new(
                Why::Invalid,
                "/last-apply/toplevel",
                top.as_str(),
            ));
        }
        // A DOWNLOAD NAMES TWO STORE PATHS that root hands to Nix (`accept`,
        // `import`): nothing else may stand where they are.
        for d in self.last_apply.iter().flat_map(|a| &a.missing) {
            for (at, path) in [("drv", &d.drv), ("out", &d.out)] {
                if !check::store_path(path) {
                    return Err(Refusal::new(
                        Why::Invalid,
                        format!("/last-apply/missing/{at}"),
                        path.as_str(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a record without `no-trial` does not read, gains the key when
    /// written back, or a reason outside the closed set is read at all.
    #[test]
    fn no_trial_is_read_written_only_when_set_and_closed() {
        let plain = r#"{"format":1,"last-good":null,"trial":null,"hold":null,"last-apply":null}"#;
        let state: MachineState = serde_json::from_str(plain).unwrap();
        assert_eq!(state.no_trial, None);
        assert!(!serde_json::to_string(&state).unwrap().contains("no-trial"));
        for why in NoTrial::ALL {
            let set = MachineState {
                no_trial: Some(why),
                ..MachineState::default()
            };
            let json = serde_json::to_string(&set).unwrap();
            assert!(
                json.contains(&format!(r#""no-trial":"{}""#, why.id())),
                "{json}"
            );
            assert_eq!(serde_json::from_str::<MachineState>(&json).unwrap(), set);
        }
        let unknown = plain.replace('}', r#","no-trial":"no-reason"}"#);
        assert!(serde_json::from_str::<MachineState>(&unknown).is_err());
    }

    /// FAILS IF: an apply set for the next boot does not read and write back
    /// whole (`staged`, the system it set up), a last apply without a
    /// toplevel gains the key when written, or a toplevel that is no store
    /// path is taken in — the push and the parts join a file to it.
    #[test]
    fn a_staged_apply_names_its_system_and_reads_back() {
        let top = "/nix/store/bcdefghijklmnopqrstuvwxyz0123456-nixos-system-noctua-26.11";
        let staged = MachineState {
            last_apply: Some(LastApply {
                at: Stamp::parse("2026-10-30T12:00:00Z").unwrap(),
                result: ApplyResult::Staged,
                toplevel: Some(top.to_owned()),
                missing: Vec::new(),
            }),
            ..MachineState::default()
        };
        let text = crate::to_json(&staged);
        assert!(text.contains(r#""result": "staged""#), "{text}");
        assert!(text.contains(&format!(r#""toplevel": "{top}""#)), "{text}");
        assert_eq!(crate::parse::<MachineState>(text.as_bytes()), Ok(staged));

        let plain = MachineState {
            last_apply: Some(LastApply {
                at: Stamp::parse("2026-10-30T12:00:00Z").unwrap(),
                result: ApplyResult::Refused,
                toplevel: None,
                missing: Vec::new(),
            }),
            ..MachineState::default()
        };
        let text = crate::to_json(&plain);
        assert!(!text.contains("toplevel"), "{text}");
        assert_eq!(crate::parse::<MachineState>(text.as_bytes()), Ok(plain));

        let elsewhere = text.replace(
            r#""result": "refused""#,
            r#""result": "staged", "toplevel": "/tmp/x""#,
        );
        assert_eq!(
            crate::parse::<MachineState>(elsewhere.as_bytes()).map_err(|r| r.why),
            Err(Why::Invalid)
        );
    }
}
