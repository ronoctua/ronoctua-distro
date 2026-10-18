//! `ronoctua-shell-git-line` — the git line, under both features that show it.
//!
//! One row under the file manager's path when its folder is inside a
//! repository, read on a thread of its own and watched by the repository's
//! git files ([`git_line::Keeper`]); and the words the history of a
//! repository shares with it — a short commit id, how long ago, why git said
//! nothing. Two features use it, so it lives below both: the manager and the
//! history stand on it, and on each other not at all.

#![deny(unsafe_code)]

// THE KIT AND THE REPORT, by the names the module already uses
// (`crate::fields::Row`, `crate::report::Line`).
use ronoctua_shell_kit::fields;
use ronoctua_shell_report::report;

pub mod git_line;
pub mod key;
