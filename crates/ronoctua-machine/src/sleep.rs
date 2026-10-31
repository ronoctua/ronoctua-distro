//! `hibernate pre | post`: a hibernation does not spend the boot trial
//! (`/etc/systemd/system-sleep`, `nix/modules/machine.nix`).
//!
//! # Why
//!
//! A trial is a one-shot the boot loader reads at the next FIRMWARE boot,
//! and the resume from a hibernation is one: the loader would start the
//! trial's entry. Its kernel, when it is another, cannot resume the image
//! the old one wrote — the session is lost; when it is the same, it resumes
//! the old system and the one-shot is spent on nothing, so the real restart
//! after it comes up on the default, and the bless calls the trial failed.
//! A system set for the next boot waits days for that restart, and the
//! machine hibernates on its own on a low battery (`nix/modules/power.nix`).
//!
//! So, before a hibernation (`pre`), the trial's one-shot is put aside: the
//! resume boots the default — the last good system, the one whose kernel
//! runs — and after it (`post`) the one-shot is put back while the trial is
//! still pending. What was put aside is written down ([`ASLEEP`]), and a
//! machine that never came back through its resume (a resume that failed,
//! the power lost) finds it at its next bless: the trial was not tried, and
//! is armed again for the next boot ([`lost_resume`]).
//!
//! Neither half waits for root's lock of the person's data — a sleep must
//! not wait for an update — and neither changes the record: they move only
//! the one-shot and their own note.

use serde::{Deserialize, Serialize};

use ronoctua_person::state::Trial;

use crate::loader::Loader;
use crate::{Machine, Refusal, boot, boots, disk, must_be_root, say, trial};

/// The note of a one-shot put aside for a hibernation, under the state
/// folder: the machine's own bookkeeping, as [`boots`]' are.
pub const ASLEEP: &str = "trial-asleep.json";

/// A one-shot put aside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Asleep {
    /// The toplevel on trial.
    toplevel: String,
    /// Its entry, as the one-shot named it.
    entry: String,
}

fn read(m: &Machine) -> Option<Asleep> {
    let bytes = std::fs::read(disk::snapshot(&m.places, ASLEEP)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// The note goes: the trial was judged, disarmed or replaced.
pub(crate) fn forget(m: &Machine) {
    let _ = disk::remove(&disk::snapshot(&m.places, ASLEEP));
}

/// `hibernate pre` (`resumed` false) or `hibernate post`; the line it ends
/// with.
pub(crate) fn hibernate(m: &Machine, resumed: bool) -> Result<String, Refusal> {
    must_be_root(m)?;
    Ok(if resumed { woke(m) } else { before(m) })
}

/// Before a hibernation: the pending trial's one-shot, put aside.
fn before(m: &Machine) -> String {
    let Some(t) = disk::read_state(&m.places)
        .ok()
        .as_ref()
        .and_then(trial::pending)
    else {
        return "no trial waits: nothing put aside".to_owned();
    };
    let Some(loader) = Loader::of(&m.system) else {
        return "no boot loader is named: nothing put aside".to_owned();
    };
    let Some(oneshot) = loader.oneshot().filter(|o| !o.is_empty()) else {
        return "no one-shot is set: nothing put aside".to_owned();
    };
    let ours = loader
        .entries()
        .is_some_and(|listed| boot::entry_of(&listed, &t.toplevel) == Some(oneshot.as_str()));
    if !ours {
        return "the one-shot is not the trial's: left as it is".to_owned();
    }
    let note = Asleep {
        toplevel: t.toplevel,
        entry: oneshot.clone(),
    };
    let text = serde_json::to_string_pretty(&note).unwrap_or_default() + "\n";
    if let Err(e) = disk::keep_snapshot(&m.places, ASLEEP, text.as_bytes()) {
        say(
            4,
            &format!("{ASLEEP} was not written: the one-shot stays, and the resume boots it ({e})"),
        );
        return "nothing put aside".to_owned();
    }
    if !loader.set_oneshot("") {
        forget(m);
        say(
            4,
            "the one-shot could not be unset: the resume boots the trial's entry",
        );
        return "nothing put aside".to_owned();
    }
    say(
        6,
        &format!(
            "the trial's one-shot ({oneshot}) is put aside: the resume boots the system that runs"
        ),
    );
    format!("put aside {oneshot}")
}

/// After the resume: the one-shot back, while its trial is still pending.
fn woke(m: &Machine) -> String {
    let Some(note) = read(m) else {
        return "nothing was put aside".to_owned();
    };
    let pending = disk::read_state(&m.places)
        .ok()
        .as_ref()
        .and_then(trial::pending);
    forget(m);
    if !pending.is_some_and(|t| t.toplevel == note.toplevel) {
        return "the trial put aside is no longer pending: nothing put back".to_owned();
    }
    if set_again(m, &note.entry) {
        format!("put back {}", note.entry)
    } else {
        "nothing put back".to_owned()
    }
}

/// The one-shot on `entry` again; whether it was set.
fn set_again(m: &Machine, entry: &str) -> bool {
    let set = Loader::of(&m.system).is_some_and(|loader| loader.set_oneshot(entry));
    if set {
        say(
            6,
            &format!("the trial's one-shot ({entry}) is back: the next boot tries it"),
        );
    } else {
        say(
            4,
            "the trial's one-shot could not be set again: the next boot starts the default",
        );
    }
    set
}

/// Whether `t`'s one-shot is put aside for a hibernation (the note names
/// it).
pub(crate) fn put_aside_for(m: &Machine, t: &Trial) -> bool {
    read(m).is_some_and(|note| note.toplevel == t.toplevel)
}

/// Whether `t`'s one-shot was put aside by a hibernation the machine never
/// came back from through its resume — a resume that failed, the power lost
/// —, so the boot `boot` did not try it: the one-shot is set again, `boot`
/// is the one that armed it ([`boots::arm`]), and the note goes. Run by the
/// bless and the apply before they judge.
pub(crate) fn lost_resume(m: &Machine, t: &Trial, boot: &str) -> bool {
    let Some(note) = read(m).filter(|n| n.toplevel == t.toplevel) else {
        return false;
    };
    say(
        4,
        "the trial's one-shot was put aside for a hibernation the machine did not resume from: \
         it was not tried, and is armed again",
    );
    set_again(m, &note.entry);
    boots::arm(m, &t.toplevel, Some(boot));
    forget(m);
    true
}
