//! `ronoctua-shell-icons` — the icons app (`LEADER m i`).
//!
//! The view both doors draw (`icons`: the three screens, the grid, the
//! search; `icons_page`, the page of one icon), the window
//! (`icons_window`), its camera (`icons_photograph`) and the region the
//! footer hosts (`icons_region`) — with the ONE errand handler the window and
//! the region share (`icons_region::carry_out`).
//!
//! On the kit and the door only.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use.
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{
    bars, bindings, chrome, fields, glyph, line, minibuffer, text_select, wheel,
};

pub mod icons;
pub mod icons_page;
pub mod icons_photograph;
pub mod icons_region;
pub mod icons_window;
pub mod key;

/// THE ICONS APP'S WINDOW the binary runs by name (`ronoctua-shell icons
/// [nerd-fonts|lucide|unicode]`, `LEADER m i w`), on the screen named: the
/// SAME `icons::View` the footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "icons",
    run: icons_door,
}];

/// THE ICONS APP'S WINDOW'S CAMERA (`photograph --door icons`).
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "icons",
    take: icons_photograph::taken,
}];

/// `ronoctua-shell icons [screen]` — a word that names no screen is refused.
fn icons_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let screen = words
        .first()
        .map(|w| {
            icons::screen_named(&w).with_context(|| {
                format!("`{w}` is not a screen; the screens are nerd-fonts, lucide and unicode")
            })
        })
        .transpose()?
        .unwrap_or(ronoctua_iconset::Collection::NerdFonts);
    icons_window::host_it(screen).context("host the icons app")
}
