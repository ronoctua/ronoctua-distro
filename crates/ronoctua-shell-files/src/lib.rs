//! `ronoctua-shell-files` — the file manager, in both of its doors.
//!
//! The view both doors draw (`files::View`) — a directory, its marks, its
//! panels, its menu, its sheets — its window (`files_window`), its region of
//! the footer (`files_region`, laid out in sections), the one handler of what
//! it asks in either door (`files_manager`), its archives (`files_archive`)
//! and its camera (`files_photograph`).
//!
//! # Why a crate
//!
//! Because the manager is the second largest feature of the shell, and it
//! changes often: inside the binary, every line of it recompiled every view
//! and the footer. Here a line of it recompiles it and the binary that
//! registers it. It stands on the foundation (the kit, the door, the report,
//! the keep, the work, the inspection panels) and on the git line below it
//! (`ronoctua-shell-git-line`), and on no other feature: what the security
//! centre does for its vault's row comes in through `vault::Flow`, and the
//! history of a repository is opened by its door's command.
//!
//! The modules keep the paths they had in the binary (`crate::files::View`,
//! `crate::chrome::Frame`): the foundation's modules are named here at the
//! root.

#![deny(unsafe_code)]

// THE FOUNDATION AND THE GIT LINE, by the names these modules already use.
use ronoctua_shell_door::{carry, door, file_picker, launch, region, sections, sudo};
use ronoctua_shell_git_line::git_line;
use ronoctua_shell_inspect::{
    globe, inspection, inspection_edit, inspection_sheet, motion, stage, tag_words, thumbs,
};
use ronoctua_shell_keep::keep;
use ronoctua_shell_kit::{
    bars, bindings, chrome, clipboard_copy, doors, echo, fields, fold, glyph, line, minibuffer,
    paint, security_words, sizes, slide, text_select, wake, wheel, when,
};
use ronoctua_shell_report::report;
use ronoctua_shell_work::{jobs, seal, vault};

pub mod files;
pub mod files_archive;
pub mod files_manager;
pub mod files_photograph;
pub mod files_region;
pub mod files_window;
pub mod key;
