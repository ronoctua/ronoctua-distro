//! `try-again`: the system whose boot trial failed is tried once more, at the
//! next apply that builds it — the person's answer to a trial that failed by
//! chance (the power cut during the boot that tried it).
//!
//! # Why a note, and not a word in `state.json`
//!
//! The apply refuses for ever the toplevel whose trial failed
//! ([`crate::Why::DidNotBoot`]): without it the next update would switch
//! into it again and the next restart fail the same way. Forgetting that
//! refusal must not forget the failure itself — the rescue's rollback still
//! skips that generation, and the page still says it did not start — and it
//! cannot be said in `state.json`: a trial's outcome is a closed set, and
//! `deny_unknown_fields` everywhere, read by the first published release's
//! reader that a trial's way back can land on
//! (`ronoctua_person::state::MachineState::split`). So the record keeps
//! `failed`, and a small note beside it, which no older reader opens, names
//! the toplevel the person asked to try again ([`AGAIN`]). An older system
//! booted meanwhile does not read it, and refuses as before: the request
//! waits for a system that knows it.
//!
//! # Once
//!
//! The note forgives the refusal only while the record's failed trial IS the
//! toplevel it names ([`forgiven`]), and the apply that sets up any system
//! takes it away ([`spent`]): from there the record's trial is the new one,
//! and a retry that fails again is refused again until the person asks
//! again. A note about a trial since replaced names nothing and is believed
//! by nobody — the page says it only while it forgives ([`asked`]).

use ronoctua_person::Stamp;
use ronoctua_person::state::{MachineState, TrialOutcome};
use serde::{Deserialize, Serialize};

use crate::{Machine, Places, Refusal, Why, disk, guard, say};

/// The note beside `state.json`: the failed toplevel the person asked to try
/// again.
pub const AGAIN: &str = "trial-again.json";

/// What the note says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Again {
    /// The toplevel whose trial failed, tried again.
    toplevel: String,
    /// When the person asked.
    asked_at: Stamp,
}

/// The note under `places`, when there is one that reads.
fn read(places: &Places) -> Option<Again> {
    let file = disk::snapshot(places, AGAIN);
    let bytes = std::fs::read(&file).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(again) => Some(again),
        Err(e) => {
            say(4, &format!("{} does not read: {e}", file.display()));
            None
        }
    }
}

/// The toplevel of the trial `state` says failed, if one did.
fn failed(state: &MachineState) -> Option<&str> {
    state
        .trial
        .as_ref()
        .filter(|t| t.outcome == TrialOutcome::Failed)
        .map(|t| t.toplevel.as_str())
}

/// `try-again`: the note written for the trial the record says failed.
pub(crate) fn try_again(m: &Machine) -> Result<(), Refusal> {
    let _lock = guard(m)?;
    let state = disk::read_state(&m.places)?;
    let Some(toplevel) = failed(&state) else {
        say(
            3,
            "no system failed its boot trial on this machine: there is nothing to try again \
             (no-failed-trial)",
        );
        return Err(Refusal::of(Why::NoFailedTrial));
    };
    let note = Again {
        toplevel: toplevel.to_owned(),
        asked_at: disk::now(),
    };
    let text = serde_json::to_string_pretty(&note).unwrap_or_default() + "\n";
    disk::keep_snapshot(&m.places, AGAIN, text.as_bytes())
        .map_err(|e| disk::failed(&disk::snapshot(&m.places, AGAIN), &e))?;
    say(
        6,
        &format!(
            "{toplevel} did not start when it was tried: the next update that builds it tries it again"
        ),
    );
    Ok(())
}

/// Whether the person asked to try `toplevel` again, and it is the trial the
/// record says failed — what lets the apply build it past `did-not-boot`.
pub(crate) fn forgiven(m: &Machine, state: &MachineState, toplevel: &str) -> bool {
    failed(state) == Some(toplevel) && read(&m.places).is_some_and(|a| a.toplevel == toplevel)
}

/// The note goes: an apply set a system up, and the record's trial is about
/// to be that system's.
pub(crate) fn spent(m: &Machine) {
    let file = disk::snapshot(&m.places, AGAIN);
    if let Err(e) = disk::remove(&file) {
        say(4, &format!("{}: {e}", file.display()));
    }
}

/// Whether the system whose trial `state` says failed is tried again at the
/// next update — the person asked, and the note still names it. What the
/// update's page says in place of the gesture that asks.
#[must_use]
pub fn asked(places: &Places, state: &MachineState) -> bool {
    failed(state).is_some_and(|top| read(places).is_some_and(|a| a.toplevel == top))
}
