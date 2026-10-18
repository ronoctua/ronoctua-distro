//! `ronoctua-shell-locale` — the language and region view (`LEADER s l`) and
//! the regions whose streets are installed (`LEADER s g`).
//!
//! Two views, each with what it knows ([`locale`], [`geo`]), its window
//! ([`locale_window`], [`geo_window`]) and its region of the footer
//! ([`locale_region`], [`geo_region`]) — the two doors of each carrying out
//! its errands through one handler (`locale_region::carry_out`,
//! `geo_region::carry_out`). One crate for the two because the language
//! view's keys hold the regions' (`locale::KEYS`), and the regions are where
//! a place of the language view gets its streets.

#![forbid(unsafe_code)]

// THE KIT, THE DOOR AND THE GLOBE, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`, `crate::globe::…`).
use ronoctua_shell_door::{door, photo, region};
use ronoctua_shell_inspect::globe;
use ronoctua_shell_kit::{bindings, chrome, echo, fields, language, line, mind_wire, sizes, when};

pub mod geo;
pub mod geo_region;
pub mod geo_window;
pub mod key;
pub mod locale;
pub mod locale_region;
pub mod locale_window;

/// THE LANGUAGE AND REGION'S WINDOWS the binary runs by name
/// (`ronoctua-shell locale`, and the regions' `geo`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "locale",
        run: locale_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "geo",
        run: geo_door,
    },
];

/// THE LANGUAGE AND REGION'S WINDOW'S CAMERA (`photograph --door locale`),
/// over a planted machine.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "locale",
    take: locale_window::camera,
}];

/// `ronoctua-shell locale` — the language and region in a window.
fn locale_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    locale_window::host_it().context("host the language and region view")
}

/// `ronoctua-shell geo` — the regions' view in a window.
fn geo_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    geo_window::host_it().context("host the regions' view")
}
