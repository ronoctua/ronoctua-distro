//! THE SAMPLE MODULE (brain.md §7, S0) — what a module's `module.rs` looks
//! like, small enough to read at once.
//!
//! The entry [`new`], one type that answers the contract, and a sibling file
//! declared with `mod rules;` and found BESIDE this one — which is the whole
//! point of the sample: the shim loads this file by an absolute `#[path]`, and
//! a module that could not split itself into files would be one file forever.
//!
//! It marks the modeline while the disk is nearly full. It is never
//! registered — it lives under `tests/modules/`, which the loader does not
//! read — and the brain's own test is the only thing that compiles it.

mod rules;

use crate::module::{Birth, Face, Look, Manifest, Mark, Module, Run, Scheduled, Tone};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::task::{Due, Outcome};

/// The entry every module has: what the core calls, once, at start.
///
/// `+ use<>` IS PART OF THE ENTRY: under Rust 2024 an `impl Trait` captures
/// every lifetime of its arguments, so without it the module would borrow the
/// `Birth` it was made from and could never be kept in the registry.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    PilotSample
}

/// Where rustc found the sibling file — what the pilot test reads.
#[must_use]
pub fn rules_live_in() -> &'static str {
    rules::FILE
}

/// The module. No state of its own: the mark is a function of the pulse.
struct PilotSample;

impl Module for PilotSample {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn face(&self, look: &Look<'_>) -> Face {
        let mut face = Face::default();
        // A MARK EXISTS ONLY WHEN IT SAYS SOMETHING (invariant 14): at rest
        // the face is empty and the modeline draws nothing for this module.
        if rules::nearly_full(look.pulse.disk_free_per_mille) {
            face.marks.push(Mark {
                id: super::mark::FULL,
                tone: Tone::Warning,
                count: 0,
                act: None,
            });
        }
        face
    }
}

impl Scheduled for PilotSample {
    fn due(&self, _now: Moment, _mine: &TaskPrefs, _last: Option<Moment>) -> Due {
        // A WORD OF ITS OWN, declared in the manifest's `whys` and drawn as
        // `pilot-sample-why-when-empty` — never a core text that names it.
        Due::Not {
            why: super::why::WHEN_EMPTY,
        }
    }

    fn run(&self, _run: &Run) -> Outcome {
        Outcome::Skipped {
            why: super::why::WHEN_EMPTY.to_owned(),
        }
    }
}
