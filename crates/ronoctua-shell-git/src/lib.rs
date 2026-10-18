//! `ronoctua-shell-git` — git, as the shell shows it.
//!
//! The history of a repository (the file manager's `C-h`): the view both
//! doors draw (`git_browser`), the window (`git_browser_window`), its
//! camera (`git_photograph`) and the region the footer hosts (`git_region`)
//! — with the ONE errand handler the window and the region share
//! (`git_region::carry_out`). Beside them, the distro's own clone
//! (`dev_clone`, `LEADER / c`).
//!
//! On the kit, the door and the git line only (`ronoctua-shell-git-line`:
//! the file manager draws the line, the history reads its words and watches
//! the same git files — it lives below both).

#![deny(unsafe_code)]

// THE KIT, THE DOOR AND THE GIT LINE, by the names the modules already use.
use ronoctua_shell_door::{door, launch, region};
use ronoctua_shell_git_line::git_line;
use ronoctua_shell_kit::{
    bars, bindings, chrome, clipboard_copy, echo, fields, line, text_select, wheel, when,
};

pub mod dev_clone;
pub mod git_browser;
pub mod git_browser_window;
pub mod git_photograph;
pub mod git_region;
pub mod key;

/// THE GIT BROWSER'S WINDOW the binary runs by name (`ronoctua-shell
/// git-browser [<folder>]`): the history of the repository the folder is in
/// (`C-h` in the file manager's window), the SAME `git_browser::View` the
/// footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "git-browser",
    run: history_door,
}];

/// THE GIT BROWSER'S WINDOW'S CAMERA (`photograph --door git`), over the
/// repository `--dir` is in.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "git",
    take: git_photograph::taken,
}];

/// `ronoctua-shell git-browser [<folder>]` — the folder given, or this one.
fn history_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let dir = words
        .first()
        .map_or_else(std::env::current_dir, |d| Ok(std::path::PathBuf::from(d)))
        .context("no folder to read the history of")?;
    git_browser_window::host_it(&dir).context("host the git browser")
}
