//! `ronoctua-shell-video-metrics` — the video catalogue (`LEADER v`): the
//! person's YouTube channels, their videos scored, the rules that score them.
//!
//! One feature, one crate: the view (`video_metrics`, with its words), the
//! reader and writer every door runs (`video_metrics_feed`), the window
//! (`video_metrics_window`, with its camera), the region the footer hosts
//! (`video_metrics_region`) and the keys (`video_metrics::KEYS`) — and the
//! RANKING, the catalogue's videos best first as a page of the player
//! (`player_ranking`, `LEADER p k`), which the player's view holds through its
//! seam (`player_seam::Page`, the `PAGE` the binary installs).
//!
//! It stands on the kit, the door and the player: a line here recompiles this
//! crate and the binary, never the player's transport, its stage or its window.

#![deny(unsafe_code)]

// THE KIT, THE DOOR AND THE PLAYER, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`, `crate::player_rows::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, line, wake, when};
use ronoctua_shell_player::{
    player, player_find, player_metadata, player_posters, player_rows, player_seam, player_window,
    text_area,
};
// The tests hand a root gesture to a fake gate.
#[cfg(test)]
use ronoctua_shell_door::sudo;

pub mod key;
pub mod player_ranking;
pub mod video_metrics;
pub mod video_metrics_feed;
pub mod video_metrics_photograph;
// THE VIDEO CATALOGUE IN THE FOOTER'S REGION, and what the footer keeps of it.
pub mod video_metrics_region;
pub mod video_metrics_window;
pub mod video_metrics_words;

/// THE VIDEO CATALOGUE'S WINDOW the binary runs by name (`ronoctua-shell
/// video-metrics [page]`, `LEADER v w`): the SAME `video_metrics::View` the
/// footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "video-metrics",
    run: catalogue_door,
}];

/// THE VIDEO CATALOGUE'S WINDOW'S CAMERA (`photograph --door
/// video-metrics`), over a copy of the catalogue it is given.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "video-metrics",
    take: video_metrics_photograph::taken,
}];

/// `ronoctua-shell video-metrics [page]` — a page that is none is refused.
fn catalogue_door(said: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let said = said.all();
    let word = said.join(" ");
    let page = video_metrics_window::opening_of(&word).with_context(|| {
        format!(
            "usage: ronoctua-shell video-metrics [{}]",
            video_metrics_window::OPENINGS.join(" | ")
        )
    })?;
    video_metrics_window::host_it(page).context("host the video catalogue")
}
