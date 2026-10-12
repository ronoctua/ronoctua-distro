//! Which boot armed the trial, and which boot first tried it.
//!
//! A trial is judged by the FIRST boot after the one that armed it that
//! lasted to the bless — never by whichever boot the bless happens to run in,
//! and never by the clock. Two small records beside `state.json` say so; they
//! are the machine's own bookkeeping (a boot's identity means nothing on
//! another machine), so they are not part of the published record:
//!
//! - [`ARMED`], written by the apply that armed the trial: its toplevel and
//!   the boot it was armed in (`/proc/sys/kernel/random/boot_id`). A bless or
//!   an apply in that same boot judges nothing: the trial was not tried.
//!   Later applies that arm nothing (nothing to switch, a failed build, a
//!   refusal) do not touch it.
//! - [`TRIED`], written by the first bless (or apply) of a LATER boot, before
//!   it waits for root's lock of the person's data: the boot it was and the
//!   system that boot booted. A bless that then waits and never gets the lock
//!   — an update holds it, the machine is turned off — has still said what
//!   booted, and the next one judges by it, not by the boot it runs in.
//!   Written once per armed trial.
//!
//! The second names the first (the trial's toplevel and the boot that armed
//! it), so a note about a trial an apply has since replaced names a trial that
//! is no longer armed and is ignored: the two writers need no lock between
//! them, and each file is written whole and renamed into place.

use serde::{Deserialize, Serialize};

use crate::{Machine, disk, say};

/// The record of the boot that armed the trial.
pub const ARMED: &str = "trial-armed.json";

/// The record of the first later boot that lasted to the bless.
pub const TRIED: &str = "trial-tried.json";

/// What a boot is called when no record says which one armed the trial (a
/// record lost, a trial armed by a version that kept none).
const UNKNOWN: &str = "unknown";

/// The trial and the boot it was armed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Armed {
    /// The toplevel on trial.
    pub toplevel: String,
    /// The boot that armed it.
    pub boot: String,
}

/// The first boot after the arming one that lasted to the bless.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Tried {
    /// The toplevel on trial ([`Armed::toplevel`]).
    pub toplevel: String,
    /// The boot that armed it ([`Armed::boot`]).
    pub armed_in: String,
    /// The boot that tried it.
    pub boot: String,
    /// The toplevel that boot booted.
    pub booted: String,
}

impl Tried {
    /// Whether this note is about `armed`.
    fn about(&self, armed: &Armed) -> bool {
        self.toplevel == armed.toplevel && self.armed_in == armed.boot
    }
}

fn read<T: for<'de> Deserialize<'de>>(m: &Machine, name: &str) -> Option<T> {
    let file = disk::snapshot(&m.places, name);
    let bytes = std::fs::read(&file).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(e) => {
            say(4, &format!("{} does not read: {e}", file.display()));
            None
        }
    }
}

fn write<T: Serialize>(m: &Machine, name: &str, value: &T) {
    let text = serde_json::to_string_pretty(value).unwrap_or_default() + "\n";
    if let Err(e) = disk::keep_snapshot(&m.places, name, text.as_bytes()) {
        say(4, &format!("{name} was not written: {e}"));
    }
}

/// The record of the boot that armed `toplevel`'s trial; one that names
/// another toplevel is about an older trial and answers nothing.
pub(crate) fn armed(m: &Machine, toplevel: &str) -> Option<Armed> {
    read::<Armed>(m, ARMED).filter(|a| a.toplevel == toplevel)
}

/// `toplevel`'s trial was armed in the boot `boot`; whatever was noted about
/// an earlier trial goes.
pub(crate) fn arm(m: &Machine, toplevel: &str, boot: Option<&str>) {
    write(
        m,
        ARMED,
        &Armed {
            toplevel: toplevel.to_owned(),
            boot: boot.unwrap_or(UNKNOWN).to_owned(),
        },
    );
    let _ = disk::remove(&disk::snapshot(&m.places, TRIED));
}

/// The record of a trial armed in a boot no record names.
pub(crate) fn unknown(toplevel: &str) -> Armed {
    Armed {
        toplevel: toplevel.to_owned(),
        boot: UNKNOWN.to_owned(),
    }
}

/// Whether `armed` was armed in the boot `boot`.
pub(crate) fn armed_in(armed: &Armed, boot: &str) -> bool {
    armed.boot != UNKNOWN && armed.boot == boot
}

/// Whether no record says which boot armed `armed`.
pub(crate) fn in_unknown_boot(armed: &Armed) -> bool {
    armed.boot == UNKNOWN
}

/// What the first boot after `armed`'s booted, if one has said it.
pub(crate) fn tried(m: &Machine, armed: &Armed) -> Option<Tried> {
    read::<Tried>(m, TRIED).filter(|t| t.about(armed))
}

/// The first boot after `armed`'s: the note already written, else this boot
/// (`boot`, which booted `booted`), written now.
pub(crate) fn note(m: &Machine, armed: &Armed, boot: &str, booted: &str) -> Tried {
    if let Some(earlier) = tried(m, armed) {
        return earlier;
    }
    let now = Tried {
        toplevel: armed.toplevel.clone(),
        armed_in: armed.boot.clone(),
        boot: boot.to_owned(),
        booted: booted.to_owned(),
    };
    write(m, TRIED, &now);
    say(
        6,
        &format!(
            "the first boot since the trial {} was armed booted {booted}",
            armed.toplevel
        ),
    );
    now
}

/// Both records go: the trial was judged, disarmed or replaced by none.
pub(crate) fn forget(m: &Machine) {
    for name in [ARMED, TRIED] {
        let _ = disk::remove(&disk::snapshot(&m.places, name));
    }
}
