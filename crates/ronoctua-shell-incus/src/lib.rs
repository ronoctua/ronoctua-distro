//! `ronoctua-shell-incus` — Incus (`LEADER s x`): the person's instances,
//! images, remotes, profiles, networks, storage and projects.
//!
//! One feature, one crate: the view (`incus`, with its words in
//! `incus_words`), its threads (`incus_feed`), the view open in a door and
//! its one errand handler (`incus_open`), its window (`incus_window`) and
//! camera (`incus_photograph`), its region in the footer (`incus_region`)
//! and its keys (`incus::KEYS`). It stands on the kit and the door alone: it
//! names no other feature and not the footer, which knows its region by its
//! id.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, places, queue, region};
use ronoctua_shell_kit::{
    bars, bindings, chrome, consent_rows, echo, fields, line, minibuffer, sizes, wheel, when,
};
// The tests reach the sudo bar's gate and a door's wake.
#[cfg(test)]
use ronoctua_shell_door::sudo;
#[cfg(test)]
use ronoctua_shell_kit::wake;

pub mod incus;
pub mod incus_feed;
// THE VIEW OPEN IN EITHER DOOR, and the one function both carry its errands
// out with.
pub mod incus_open;
pub mod incus_photograph;
// THE VIEW IN THE FOOTER'S REGION: the region the binary registers.
pub mod incus_region;
pub mod incus_window;
pub mod incus_words;
pub mod key;

/// INCUS IN A WINDOW the binary runs by name (`ronoctua-shell incus
/// [images|new]`, `LEADER s x w`): the SAME `incus::View` the footer's region
/// draws, over its own threads, opened on the instances, the images or a new
/// instance's draft.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "incus",
    run: incus_door,
}];

/// INCUS'S WINDOW'S CAMERA (`photograph --door incus`), over a scene handed
/// in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "incus",
    take: incus_photograph::taken,
}];

/// `ronoctua-shell incus [images|new]` — on the opening named.
fn incus_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let words = words.all();
    let opening = incus_window::Opening::of_words(&words)?;
    incus_window::host_it(opening).context("host Incus")
}
