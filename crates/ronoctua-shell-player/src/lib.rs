//! `ronoctua-shell-player` — the player (`LEADER p`): one view, three doors.
//!
//! The view (`player`) and every page of what plays — the home, the queue as a
//! tree, a lane's page, the deck, the media in the file, the subtitles, the
//! ambient sounds, the layout, the metadata, the lookups, the directories, the
//! website, the other machines — written with one builder (`player_rows`);
//! the transport and the stage; the window and the column
//! (`player_window`, and its camera, `player_photograph`), the region the
//! footer hosts (`player_region`) and the keys (`player::KEYS`). Every door
//! holds the view and carries out what it asks through one function
//! (`player::carry_out`).
//!
//! # The seam
//!
//! Channels by theme with its listing and its sources, and the ranking of the
//! video catalogue, are pages of this view written in crates ABOVE it
//! (`ronoctua-shell-catalogue`, `ronoctua-shell-video-metrics`): the view
//! knows their words (`player_seam`) and asks each one through the `Page`
//! trait the binary installs at start. So a line of the catalogue recompiles
//! the catalogue and the binary, never the transport — and a line here
//! recompiles everything above it, which is the price of being below.
//!
//! It stands on the kit, the door and the inspection panels: it names no other
//! feature and not the footer, which knows its region by its id.

#![deny(unsafe_code)]

// THE KIT, THE DOOR AND THE INSPECTION PANELS, by the names these modules
// already use (`crate::chrome::…`, `crate::door::…`, `crate::globe::…`).
use ronoctua_shell_door::{column, door, file_picker, launch, region, right_column};
use ronoctua_shell_inspect::{globe, tag_words};
use ronoctua_shell_kit::{
    bars, bindings, chrome, daemon, echo, fields, foreign, line, security_words, sizes,
    text_select, wake, watchdog, wheel, when,
};
// The tests hand a root gesture to a fake gate.
#[cfg(test)]
use ronoctua_shell_door::sudo;
// WHAT A STATE OF THE PLAYER SAYS, under this view and the footer, which
// keeps a copy of the state for the modeline's pair (`player_mark`, beside it
// in the same leaf): the view says these words as its own (`player::loaded`).
use ronoctua_shell_player_words::player_state;

pub mod key;
pub mod player;
pub mod player_ambience;
pub mod player_directory;
pub mod player_find;
pub mod player_globe;
pub mod player_home;
pub mod player_internet;
pub mod player_kinds;
pub mod player_lane;
pub mod player_layout;
pub mod player_machines;
pub mod player_media;
pub mod player_metadata;
pub mod player_page;
// THE CAMERA OF THE WINDOW AND THE COLUMN (`ronoctua-shell photograph`),
// which the binary hands the doors that are not the player's.
pub mod player_photograph;
pub mod player_picture;
// THE CATALOGUE'S AND THE RANKING'S POSTERS: the pictures of a listing, which
// both pages above draw with the same pool.
pub mod player_posters;
pub mod player_question;
pub mod player_queue;
// THE VIEW IN THE FOOTER'S REGION: the region the binary registers, and what
// the footer keeps of the player for the session.
pub mod player_region;
pub mod player_rows;
// THE SEAM: the pages written above this crate, and the words the view names
// them by.
pub mod player_seam;
pub mod player_stage;
pub mod player_subtitles;
pub mod player_suggest;
pub mod player_timeline;
pub mod player_tracks;
pub mod player_translator;
pub mod player_transport;
pub mod player_website;
pub mod player_window;
// A text of several lines, edited where it is shown — the player's and the
// video catalogue's.
pub mod text_area;

/// THE PLAYER'S DOORS the binary runs by name: the player in a window
/// (`ronoctua-shell player [only|maximized|pip|listing]`, and `player
/// maximized --screen <connector>`), and what the `.desktop` entry runs to
/// play files and show the column (`play <file>…`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "player",
        run: player_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "play",
        run: play_door,
    },
];

/// THE PLAYER, in a window and in the column on the right: one host, two
/// kinds of surface (`player_window::Door`).
///
/// `player only`, `player maximized` and `player pip` open it with the
/// picture alone: in the window, on the whole screen, or small in the corner.
///
/// `player maximized --screen <connector>` is one of the windows "show on
/// every screen" opens: whole-screen on THAT output, showing what is routed
/// there (`player_stage::everywhere`).
fn player_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let words = words.all();
    let stage = match words.first().map(String::as_str) {
        Some("only") => player_window::Stage::Only,
        Some("maximized") => player_window::Stage::Maximized,
        Some("pip") => player_window::Stage::Small,
        // A LISTING OF THE CATALOGUE on the whole screen, on the one the door
        // that asked handed over (`player_catalog`).
        Some("listing") => player_window::Stage::Listing,
        _ => player_window::Stage::Off,
    };
    let screen = match words.get(1..) {
        Some([flag, connector]) if flag == player_stage::SCREEN => Some(connector.clone()),
        Some([]) | None => None,
        Some(other) => anyhow::bail!(
            "`player {}` takes `{} <connector>` and nothing else, not `{}`",
            words.first().map_or("", String::as_str),
            player_stage::SCREEN,
            other.join(" ")
        ),
    };
    anyhow::ensure!(
        screen.is_none() || stage == player_window::Stage::Maximized,
        "a screen of its own is for `player maximized` alone"
    );
    player_window::host_window(stage, screen).context("the player's window")
}

/// WHAT THE `.desktop` ENTRY RUNS: play these, and show the column — a file
/// opened from anywhere is a file somebody wants to see play.
///
/// RIGHT AFTER WHAT PLAYS, and the queue kept (the owner's rule): the player
/// puts them there and plays the first. A folder goes whole — the player makes
/// it a playlist named after it — and a subtitle file goes over the film it is
/// named after.
fn play_door(items: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let items = items.all();
    ronoctua_player::client::ensure().context("the player did not start")?;
    let what = items
        .iter()
        .map(|i| {
            let path = std::path::Path::new(i);
            ronoctua_player::wire::Thing::Location(if i.contains("://") || path.is_absolute() {
                i.clone()
            } else {
                std::env::current_dir()
                    .map(|d| d.join(path).to_string_lossy().into_owned())
                    .unwrap_or_else(|_| i.clone())
            })
        })
        .collect();
    ronoctua_player::client::tell(&ronoctua_player::wire::Ask::Put {
        what,
        after: None,
        into: None,
        play: true,
        replace: false,
        once: false,
    })?;
    // A COLUMN ALREADY UP, on any workspace, is left where it is.
    if right_column::holder(right_column::Tenant::Player).is_none() {
        let me = std::env::current_exe().context("where this program is")?;
        std::process::Command::new(me)
            .arg("column")
            .stdin(std::process::Stdio::null())
            .spawn()
            .context("raising the column")?;
    }
    Ok(())
}
