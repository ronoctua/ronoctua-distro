//! `ronoctua-shell-fast-access` — the search over everything (`LEADER d`):
//! the view, its sources and its feed, the detail of what was found, the
//! three doors — the footer's region, the window (floating or not) and the
//! column — and their cameras and keys.
//!
//! The view decides (`fast_access`); what every door holds around it is
//! `fast_access_window::Search`; the doors are the window and the column
//! (`fast_access_window`) and the footer's region (`fast_access_region`),
//! which the binary registers by its id. A line of this crate recompiles
//! this crate and the binary, and no other feature.

#![deny(unsafe_code)]

// THE FOUNDATION AND WHAT THE SEARCH SHOWS, by the names these modules
// already use (`crate::chrome::…`, `crate::icons_page::…`): a `use` at the
// root is seen by every module below it and is no part of this crate's API.
use ronoctua_shell_calc::calc;
use ronoctua_shell_door::{column, door, floating, launch, queue, region, right_column};
use ronoctua_shell_icons::icons_page;
use ronoctua_shell_inspect::{inspection, inspection_edit, motion, stage};
use ronoctua_shell_kit::{
    actions, bars, bindings, chrome, consent_rows, echo, fields, glyph, line, minibuffer, sizes,
    text_select, wheel, when,
};
use ronoctua_shell_report::report;
use ronoctua_shell_work::software_words;

pub mod fast_access;
pub mod fast_access_detail;
pub mod fast_access_online;
pub mod fast_access_photograph;
pub mod fast_access_region;
pub mod fast_access_rows;
pub mod fast_access_search;
pub mod fast_access_videos;
pub mod fast_access_window;
pub mod key;

/// THE SEARCH'S WINDOW the binary runs by name (`ronoctua-shell fast-access
/// [--float|--column] [query]`): in a window (`LEADER d w`), floating over the
/// whole room (`LEADER SPC`), or in the column (`LEADER d c`) — the SAME
/// `fast_access::View` the footer's region draws (`LEADER d d`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "fast-access",
    run: search_door,
}];

/// THE SEARCH'S WINDOW AND COLUMN'S CAMERA (`photograph --door
/// fast-access`), over an index built for it and a fixture for what the
/// network would answer.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "fast-access",
    take: fast_access_photograph::taken,
}];

/// `ronoctua-shell fast-access [--float|--column] [query]`.
fn search_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let words = words.all();
    fast_access_window::invoked(&words).context("host the search")
}
