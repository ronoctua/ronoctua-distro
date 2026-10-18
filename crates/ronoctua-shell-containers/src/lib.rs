//! `ronoctua-shell-containers` — the person's containers (`LEADER s o`):
//! Podman's containers, pods, compose projects, images, volumes and
//! networks.
//!
//! One feature, one crate: the view (`containers`, with its words in
//! `containers_words`), its threads (`containers_feed`), the view open in a
//! door and its one errand handler (`containers_open`), its window
//! (`containers_window`) and camera (`containers_photograph`), its region in
//! the footer (`containers_region`) and its keys (`containers::KEYS`). It
//! stands on the kit and the door alone: it names no other feature and not
//! the footer, which knows its region by its id.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, places, queue, region};
use ronoctua_shell_kit::{
    bars, bindings, chrome, echo, fields, line, minibuffer, sizes, wheel, when,
};
// The tests reach the sudo bar's gate and a door's wake.
#[cfg(test)]
use ronoctua_shell_door::sudo;
#[cfg(test)]
use ronoctua_shell_kit::wake;

pub mod containers;
pub mod containers_feed;
// THE VIEW OPEN IN EITHER DOOR, and the one function both carry its errands
// out with.
pub mod containers_open;
pub mod containers_photograph;
// THE VIEW IN THE FOOTER'S REGION: the region the binary registers.
pub mod containers_region;
pub mod containers_window;
pub mod containers_words;
pub mod key;

/// THE PERSON'S CONTAINERS IN A WINDOW the binary runs by name
/// (`ronoctua-shell containers`, `LEADER s o w`): the SAME
/// `containers::View` the footer's region draws, over its own threads.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "containers",
    run: containers_door,
}];

/// THE CONTAINERS' WINDOW'S CAMERA (`photograph --door containers`), over a
/// stand-in podman.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "containers",
    take: containers_photograph::taken,
}];

/// `ronoctua-shell containers` — the person's containers in a window.
fn containers_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    containers_window::host_it().context("host the containers")
}
