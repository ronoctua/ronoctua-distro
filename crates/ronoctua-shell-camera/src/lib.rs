//! `ronoctua-shell-camera` — the camera.
//!
//! The studio's app (`camera`, `camera_window`): the picture and its layers,
//! the mixer and the bottom row, told to the studio daemon. The camera's
//! settings (`camera_settings`): one view, two doors — the footer's region
//! (`camera_region`, a `region::Region` the footer hosts by its id) and a
//! window of its own (`camera_settings_window`) — and one handler of what
//! either is asked (`camera_settings::carry_in`). The app's keys work from
//! anywhere, with no window open (`camera_region`).
//!
//! A feature crate: it stands on the kit and the door, and names neither the
//! footer nor another feature.

#![deny(unsafe_code)]

// THE FOUNDATION, by the names these modules already use (`crate::chrome::…`).
use ronoctua_shell_camera_words::studio_words;
use ronoctua_shell_door::{door, file_picker, floating, photo, region};
use ronoctua_shell_kit::{
    bindings, chrome, echo, fields, ipc, line, sizes, text_select, wake, wheel,
};
// What only the tests name: the doorway they build.
#[cfg(test)]
use ronoctua_shell_door::sudo;

pub mod camera;
pub mod camera_region;
pub mod camera_settings;
pub mod camera_settings_window;
pub mod camera_window;
pub mod key;

/// THE CAMERA'S DOORS the binary runs by name: the camera app
/// (`ronoctua-shell camera`) and its settings in a window
/// (`camera-settings`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "camera",
        run: camera_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "camera-settings",
        run: settings_door,
    },
];

/// THE CAMERA SETTINGS' WINDOW'S CAMERA (`photograph --door
/// camera-settings`), over a planted machine.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "camera-settings",
    take: camera_settings_window::camera,
}];

/// `ronoctua-shell camera` — the camera app.
fn camera_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    camera_window::host_it().context("the camera app")
}

/// `ronoctua-shell camera-settings` — the camera's settings in a window.
fn settings_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    camera_settings_window::host_it().context("the camera settings window")
}
