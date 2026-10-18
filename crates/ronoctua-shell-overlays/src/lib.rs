//! `ronoctua-shell-overlays` — the two overlays invariant 14 allows.
//!
//! The screen capture's choice (`capture`): a layer in `overlay` that exists
//! only while what to keep is chosen, and is gone before the file is written.
//! The resizer's curtains (`resizer`, `LEADER / r`): two surfaces in
//! `overlay`, on the right and below, that exist only while it is on and cover
//! only what it cut. Both stand on the kit and the door, and nothing of the
//! footer.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names these modules already use
// (`crate::chrome::…`, `crate::room::…`).
use ronoctua_shell_door::{capture_wire, cursor_shape, door, photo, room};
use ronoctua_shell_kit::{bindings, chrome, daemon, text_select, wheel};

pub mod capture;
pub mod key;
pub mod resizer;

/// THE OVERLAYS' DOORS the binary runs by name: the capture's choice
/// (`ronoctua-shell capture`), the resizer's keeper (`resizer`) and one of
/// its curtains, raised by the keeper alone (`resizer-curtain <edge>`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "capture",
        run: capture_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "resizer",
        run: resizer_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: resizer::CURTAIN_WORD,
        run: curtain_door,
    },
];

/// THE CAMERA OF THE RESIZER'S CURTAIN (`photograph --door curtain`), over a
/// scene handed in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "curtain",
    take: resizer::camera,
}];

/// THE SCREEN CAPTURE: a frozen picture of the screen with the focus, and a
/// region, a window or all of it chosen on top — see `capture`. The footer
/// starts it and reads back where the file went.
fn capture_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    capture::host_it().context("the screen capture")
}

/// THE SCREEN RESIZER'S KEEPER (`LEADER / r`): it measures the screen with
/// the focus, raises the two curtains and ends both when either ends — see
/// `resizer`. The footer starts it and reads how it ended.
fn resizer_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    resizer::keep_it().context("the screen resizer")
}

/// ONE CURTAIN OF THE RESIZER, `right` or `bottom` — raised by the keeper and
/// by nothing else: it reads its parent's command line.
fn curtain_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let edge = words
        .first()
        .as_deref()
        .and_then(resizer::Edge::of_word)
        .with_context(|| {
            format!(
                "usage: ronoctua-shell {} {}",
                resizer::CURTAIN_WORD,
                resizer::Edge::ALL.map(resizer::Edge::word).join("|")
            )
        })?;
    resizer::curtain_it(edge).context("a curtain of the screen resizer")
}
