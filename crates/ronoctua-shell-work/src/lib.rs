//! `ronoctua-shell-work` — the work a door hands the queue, and what it is
//! called.
//!
//! Encrypt anything's questions and the `Work::Seal` they hand the queue
//! (`seal`), the `/private` vault as the doors see it (`vault`), a job of the
//! queue in words (`jobs`) and the software centre's words
//! (`software_words`), with the person's programs as the footer says them too
//! — how a job of the engine ended, the modeline's two marks, the launcher's
//! words into the centre (`programs_words`). The file manager, the security
//! centre, the software centre, the queue's view, fast-access and the footer
//! say a job the same way because they say it here — and none of the
//! inspection panels, the player or the belongings stands on this crate, so a
//! line of the seal recompiles only the crates that hand work.

#![deny(unsafe_code)]

// THE DOOR, THE REPORT AND THE KEEP, by the names these modules already use
// (`crate::queue::…`, `crate::report::…`).
use ronoctua_shell_door::{file_picker, queue, region, sudo};
use ronoctua_shell_keep::belongings_words;
use ronoctua_shell_report::report;

pub mod jobs;
// THE PERSON'S PROGRAMS, view or no view: what the software centre and the
// footer, which lives while the session does, both say of them.
pub mod key;
pub mod programs_words;
pub mod seal;
pub mod software_words;
// THE /private VAULT, as the doors see it: the security centre's section, the
// sidebar's row, the work for its root unit.
pub mod vault;
