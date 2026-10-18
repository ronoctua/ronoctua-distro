//! `ronoctua-shell-inspect` — the preview and the facts of one file.
//!
//! The file manager's two panels as a part of their own that any door showing
//! a file can hold (`inspection`, `inspection_sheet`), everything the info
//! panel can change (`inspection_edit`), the thumbnails' pool (`thumbs`), the
//! globe a place is shown on (`globe`), and the stage and the motion a video
//! plays with (`stage`, `motion`). The file manager, fast-access, the player
//! and the locale hold them. They step the manager with its report, and stand
//! on nothing else of the shell — not even the door, so a line of a window's
//! plumbing recompiles none of them.

#![deny(unsafe_code)]

// THE KIT AND THE REPORT, by the names these modules already use
// (`crate::fields::…`, `crate::report::…`).
use ronoctua_shell_kit::{chrome, fields, sizes, wake, watchdog};
use ronoctua_shell_report::report;

pub mod globe;
pub mod inspection;
// Everything the info panel can change — the tags through their own program,
// the details and the place through `ronoctua_inspect::amend`.
pub mod inspection_edit;
pub mod inspection_sheet;
pub mod key;
pub mod motion;
pub mod stage;
pub mod tag_words;
// The file manager's thumbnails: the workers that make them and what has been
// made. A pool and not the preview's single thread, because a grid wants every
// answer and the panel wants only the newest.
pub mod thumbs;
