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
//! `"no-trial": "no-efi-variables"` joins them only while the system the last
//! switch made has no boot trial ([`NoTrial`]); the key is absent otherwise.
//!
//! # What `state.json` does not say: `last-apply.json` beside it
//!
//! The format stayed 1 while the last apply learnt more: the downloads a
//! build could not fetch (`missing`, 2026-10-25), the result `staged` and
//! the `toplevel` of the system an apply set up (2026-10-30). A reader
//! written before one of them refuses a record that carries it (`shape`:
//! `deny_unknown_fields` and a closed set of results).
//!
//! That reader is not gone from a machine at its next update: the boot
//! trial's way back can land on a system built before it — the generation
//! that brought the newer reader was switched to live with a new kernel,
//! its trial still pending, so the last good system predates it, and the
//! next update set a system for the next boot that did not start. Had
//! `state.json` said `staged`, that older system's bless and apply would
//! refuse the record: nothing would judge the trial, and every update would
//! fail at the rebuild until someone edited the file by hand.
//!
//! So `state.json` only ever says what the reader of the first published
//! release (`v2026.10.21-4d366d8`) reads ([`MachineState::split`]): its last
//! apply is `at` and `result` alone — an apply set for the next boot is
//! `switched`, the nearest result that reader knows (a system built and set
//! up). The last apply whole goes to `last-apply.json` beside it
//! ([`crate::names::LAST_APPLY`], [`Whole`]), which no older reader opens,
//! and a reader of today puts it back ([`MachineState::joined`]) only when
//! it is the apply `state.json` holds — an older system that wrote its own
//! apply since left a file beside that is of another apply, and
//! `state.json` is believed. A file beside that does not read (a newer
//! writer's) is believed the same way: the record is what an older reader
//! sees, never a refusal.

use std::path::Path;

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
    /// the one that waits. Never in `state.json` ([`MachineState::split`]).
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
    /// binary cache's push — reads it from here, never from the system
    /// profile. Never in `state.json` ([`MachineState::split`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toplevel: Option<String>,
    /// The downloads its build could not fetch ([`crate::frontier`]): the
    /// system's, when the build failed, or the hardware's that waits for a
    /// network, when the machine was built without it again. Each is what
    /// the update's page offers to hand in or bring on a stick. Empty (and
    /// absent from the file) when nothing was missing. Never in
    /// `state.json` ([`MachineState::split`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<crate::frontier::Download>,
}

impl LastApply {
    /// Whether the reader of the first published release reads it as it
    /// is: it carries no `staged`, no `toplevel` and no `missing`.
    #[must_use]
    pub fn older_reads_it(&self) -> bool {
        self.result != ApplyResult::Staged && self.toplevel.is_none() && self.missing.is_empty()
    }

    /// It as `state.json` tells the reader of the first published release:
    /// when and how, a system set for the next boot being a system set up
    /// (`switched`); which system, and what was missing, left out.
    #[must_use]
    pub fn for_older(&self) -> Self {
        Self {
            at: self.at.clone(),
            result: match self.result {
                ApplyResult::Staged => ApplyResult::Switched,
                other => other,
            },
            toplevel: None,
            missing: Vec::new(),
        }
    }

    /// Every value checked, at `/last-apply`.
    fn validate(&self) -> Result<(), Refusal> {
        // THE SYSTEM AN APPLY SET UP is a store path: what reads it (the
        // binary cache's push) joins a file to it.
        if let Some(top) = &self.toplevel
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
        for d in &self.missing {
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

/// The last apply whole, in `last-apply.json` beside `state.json`
/// ([`crate::names::LAST_APPLY`]) — written only while `state.json` cannot
/// tell it to an older reader ([`MachineState::split`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Whole {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The apply.
    pub last_apply: LastApply,
}

impl Document for Whole {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        self.last_apply.validate()
    }
}

impl MachineState {
    /// The record as it is written: what `state.json` holds, which every
    /// reader of format 1 reads, and — while its last apply carries what an
    /// older reader refuses ([`LastApply::older_reads_it`]) — that apply
    /// whole, for `last-apply.json` beside it. Pure.
    #[must_use]
    pub fn split(&self) -> (Self, Option<Whole>) {
        match &self.last_apply {
            Some(a) if !a.older_reads_it() => (
                Self {
                    last_apply: Some(a.for_older()),
                    ..self.clone()
                },
                Some(Whole {
                    format: crate::FORMAT,
                    last_apply: a.clone(),
                }),
            ),
            _ => (self.clone(), None),
        }
    }

    /// The record `state.json` held, its last apply put back whole from
    /// `beside` (`last-apply.json`'s bytes) when that is the same apply —
    /// the one `state.json` tells as [`LastApply::for_older`]. Otherwise,
    /// and when `beside` does not read, the record as `state.json` says it.
    /// Pure.
    #[must_use]
    pub fn joined(mut self, beside: Option<&[u8]>) -> Self {
        if let Some(whole) = beside.and_then(|b| crate::parse::<Whole>(b).ok())
            && self.last_apply.as_ref() == Some(&whole.last_apply.for_older())
        {
            self.last_apply = Some(whole.last_apply);
        }
        self
    }

    /// [`Self::joined`] with the `last-apply.json` beside `file`, the
    /// `state.json` this record was read from — what every reader of the
    /// machine's record does after [`crate::parse`].
    #[must_use]
    pub fn completed(self, file: &Path) -> Self {
        let beside = std::fs::read(file.with_file_name(crate::names::LAST_APPLY)).ok();
        self.joined(beside.as_deref())
    }
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
        self.last_apply.as_ref().map_or(Ok(()), LastApply::validate)
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

    const TOP: &str = "/nix/store/bcdefghijklmnopqrstuvwxyz0123456-nixos-system-noctua-26.11";

    fn applied(at: &str, result: ApplyResult, toplevel: Option<&str>) -> MachineState {
        MachineState {
            last_apply: Some(LastApply {
                at: Stamp::parse(at).unwrap(),
                result,
                toplevel: toplevel.map(str::to_owned),
                missing: Vec::new(),
            }),
            ..MachineState::default()
        }
    }

    fn a_download() -> crate::frontier::Download {
        crate::frontier::Download {
            drv: "/nix/store/dddddddddddddddddddddddddddddddd-crate-x-0.1.0.tar.gz.drv".to_owned(),
            out: "/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-crate-x-0.1.0.tar.gz".to_owned(),
            name: "crate-x-0.1.0.tar.gz".to_owned(),
            hash: "sha256-AAECAw==".to_owned(),
            mode: crate::frontier::Mode::Flat,
            urls: vec!["https://static.crates.io/crates/x/x-0.1.0.crate".to_owned()],
        }
    }

    const EVERY_RESULT: [ApplyResult; 7] = [
        ApplyResult::Switched,
        ApplyResult::SwitchedWithFailures,
        ApplyResult::BuildFailed,
        ApplyResult::SwitchFailed,
        ApplyResult::Rehearsed,
        ApplyResult::Refused,
        ApplyResult::Staged,
    ];

    /// FAILS IF: `state.json` is ever written with a key or a result that the
    /// reader of the first published release (v2026.10.21-4d366d8, older
    /// than `missing`, `staged` and `toplevel` — one a boot trial's way back
    /// can land on) refuses; if what it cannot say is not beside it, whole;
    /// or if the two do not read back as the record that was split.
    #[test]
    fn state_json_says_only_what_an_older_reader_reads() {
        // That reader's last apply: these keys, these results, and
        // `deny_unknown_fields` and a closed enum around them.
        let keys = ["at", "result"];
        let results = [
            "switched",
            "switched-with-failures",
            "build-failed",
            "switch-failed",
            "rehearsed",
            "refused",
        ];
        let cases = EVERY_RESULT.into_iter().flat_map(|r| {
            [None, Some(TOP)]
                .into_iter()
                .flat_map(move |t| [(r, t, false), (r, t, true)])
        });
        for (result, toplevel, missing) in cases {
            let mut state = applied("2026-10-30T12:00:00Z", result, toplevel);
            if missing {
                state.last_apply.as_mut().unwrap().missing = vec![a_download()];
            }
            let (told, whole) = state.split();
            let text = crate::to_json(&told);
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            let last = value["last-apply"].as_object().unwrap();
            assert!(last.keys().all(|k| keys.contains(&k.as_str())), "{text}");
            assert!(
                results.contains(&last["result"].as_str().unwrap()),
                "{text}"
            );
            let needs = result == ApplyResult::Staged || toplevel.is_some() || missing;
            assert_eq!(whole.is_some(), needs, "{result:?} {toplevel:?} {missing}");
            let beside = whole.map(|w| crate::to_json(&w));
            let read = crate::parse::<MachineState>(text.as_bytes()).unwrap();
            assert_eq!(read.joined(beside.as_deref().map(str::as_bytes)), state);
        }
    }

    /// FAILS IF: `last-apply.json` overrides a `state.json` an older writer
    /// rewrote with its own apply since, a file beside that does not read
    /// (a newer writer's) makes the record refused instead of read as
    /// `state.json` says it, or a record written whole into `state.json`
    /// (before the split) does not read as it is.
    #[test]
    fn last_apply_json_is_believed_only_for_its_own_apply() {
        let staged = applied("2026-10-30T12:00:00Z", ApplyResult::Staged, Some(TOP));
        let (_, whole) = staged.split();
        let beside = crate::to_json(&whole.unwrap());

        let since = applied("2026-11-02T08:00:00Z", ApplyResult::Switched, None);
        assert_eq!(since.clone().joined(Some(beside.as_bytes())), since);
        let none = MachineState::default();
        assert_eq!(none.clone().joined(Some(beside.as_bytes())), none);

        let (told, _) = staged.split();
        for unread in [
            "{",
            r#"{"format": 2, "last-apply": {}}"#,
            r#"{"format": 1, "last-apply": {"at": "2026-10-30T12:00:00Z", "result": "staged", "more": 1}}"#,
        ] {
            assert_eq!(told.clone().joined(Some(unread.as_bytes())), told);
        }
        assert_eq!(told.clone().joined(None), told);

        let written_whole = crate::to_json(&staged);
        let read = crate::parse::<MachineState>(written_whole.as_bytes()).unwrap();
        assert_eq!(read.clone().joined(Some(beside.as_bytes())), staged);
        assert_eq!(read.joined(None), staged);
    }
}
