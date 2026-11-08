//! EVERY WINDOW'S SHORTCUTS ARE ONE KEY AFTER `LEADER Alt` — proven over the
//! tables each window's hint is built from, every mode's at once.
//!
//! Here and not in each crate because this binary is the one place that
//! stands on every window: the list below is also the list of what
//! `LEADER Alt` reaches. A window's own keys are its crate's; what is
//! asserted is the key `chrome::leader_alt_key` gives each of them, with the
//! window's exceptions (`Content::leader_alt_keys`).
//!
//! A set that REPLACES a window's hint while it is up — the file picker's, a
//! text being edited — is a census of its own: its keys are never listed
//! beside the window's. The buffers (`, x`, `ronoctua_protocol::Descriptor`)
//! and the kiosk's grid (`kiosk_view`) are proven where they are declared,
//! and so is a window whose name is a brain module's, which this binary may
//! not say (`brain-modules-are-sealed`): the video metrics', in its crate.

use ronoctua_shell_kit::chrome::{OF_THE_WAY_BACK, leader_alt_census};
use ronoctua_shell_kit::consent_rows::OF_THE_CONSENT;

/// A table's `(notation, id)`, whatever else its rows carry.
macro_rules! rows {
    ($table:expr) => {
        $table
            .iter()
            .map(|r| (r.0, r.1))
            .collect::<Vec<(&str, &str)>>()
    };
}

/// One window: its name, the tables its hint is built from, and its
/// exceptions.
fn window(name: &str, tables: &[&[(&str, &str)]], exceptions: &[&[(&str, &str)]]) {
    if let Err(wrong) = leader_alt_census(tables, exceptions) {
        panic!("the {name} window offers what is not one key after LEADER Alt:\n{wrong}");
    }
}

/// FAILS IF: a key any window teaches has no one key after `LEADER Alt`, or
/// lands on a key another of its commands already has — the window's table
/// grew a capital, a chord, or a letter its exceptions did not see coming.
#[test]
fn every_window_shortcut_is_one_key() {
    use ronoctua_shell_agents::agents;
    use ronoctua_shell_audio::audio;
    use ronoctua_shell_belongings::belongings;
    use ronoctua_shell_bluetooth::bluetooth;
    use ronoctua_shell_calc::calc;
    use ronoctua_shell_camera::{camera, camera_settings};
    use ronoctua_shell_clipboard::clipboard_view;
    use ronoctua_shell_containers::containers;
    use ronoctua_shell_display::display;
    use ronoctua_shell_fast_access::fast_access;
    use ronoctua_shell_files::files;
    use ronoctua_shell_git::git_browser;
    use ronoctua_shell_history::history;
    use ronoctua_shell_icons::icons;
    use ronoctua_shell_incus::incus;
    use ronoctua_shell_locale::{geo, locale};
    use ronoctua_shell_manual::{manual, packets_window, refcard_window};
    use ronoctua_shell_network::network;
    use ronoctua_shell_notifications::notifications;
    use ronoctua_shell_player::{player, text_area};
    use ronoctua_shell_pointers::pointers;
    use ronoctua_shell_power::power;
    use ronoctua_shell_security::security;
    use ronoctua_shell_software::software;
    use ronoctua_shell_stick::stick;
    use ronoctua_shell_syncthing::syncthing;
    use ronoctua_shell_terminal::{terminal_keys, terminal_settings, terminals};
    use ronoctua_shell_update::update_window;

    let none: &[&[(&str, &str)]] = &[];
    window(
        "agents",
        &[&rows!(agents::OF_THE_VIEW), OF_THE_CONSENT],
        none,
    );
    window("audio", &[&rows!(audio::OF_THE_VIEW)], none);
    window(
        "belongings",
        &[&rows!(belongings::OF_THE_VIEW), OF_THE_CONSENT],
        none,
    );
    window("bluetooth", &[&rows!(bluetooth::OF_THE_VIEW)], none);
    window(
        "calc",
        &[
            calc::OF_THE_VIEW,
            calc::OF_THE_CONVERTER,
            calc::OF_THE_WINDOW,
            OF_THE_CONSENT,
        ],
        none,
    );
    window(
        "camera",
        &[&rows!(camera::OF_THE_VIEW)],
        &[camera::LEADER_ALT_KEYS],
    );
    window(
        "camera settings",
        &[&rows!(camera_settings::OF_THE_VIEW)],
        none,
    );
    window(
        "clipboard",
        &[clipboard_view::OF_THE_VIEW],
        &[clipboard_view::LEADER_ALT_KEYS],
    );
    window(
        "podman containers",
        &[OF_THE_WAY_BACK, &rows!(containers::OF_THE_VIEW)],
        none,
    );
    window("display", &[&rows!(display::OF_THE_VIEW)], none);
    window(
        "fast access",
        &[
            OF_THE_WAY_BACK,
            fast_access::OF_THE_VIEW,
            fast_access::OF_YOUTUBE,
            fast_access::OF_A_STRIP,
            OF_THE_CONSENT,
        ],
        none,
    );
    window(
        "files",
        &[&rows!(files::OF_THE_VIEW)],
        &[files::LEADER_ALT_KEYS],
    );
    window("git", &[OF_THE_WAY_BACK, git_browser::OF_THE_VIEW], none);
    window("history", &[history::OF_THE_VIEW], none);
    let tabs: Vec<(&str, &str)> = icons::TABS.iter().map(|t| (t.0, t.2)).collect();
    window(
        "icons",
        &[
            OF_THE_WAY_BACK,
            &tabs,
            icons::OF_THE_GRID,
            icons::OF_THE_PANEL,
            icons::OF_THE_PAGE,
        ],
        none,
    );
    window(
        "incus",
        &[OF_THE_WAY_BACK, &rows!(incus::OF_THE_VIEW), OF_THE_CONSENT],
        &[incus::LEADER_ALT_KEYS],
    );
    window("geo", &[geo::OF_THE_VIEW], none);
    window("locale", &[locale::OF_THE_VIEW], none);
    window("manual", &[manual::OF_THE_LIST, manual::OF_A_MANUAL], none);
    window(
        "protocols",
        &[
            OF_THE_WAY_BACK,
            packets_window::OF_THE_LIST,
            packets_window::OF_A_PROTOCOL,
        ],
        none,
    );
    window(
        "emacs-card",
        &[OF_THE_WAY_BACK, refcard_window::OF_THE_VIEW],
        none,
    );
    window("network", &[&rows!(network::OF_THE_VIEW)], none);
    window(
        "notifications",
        &[
            &rows!(notifications::OF_THE_VIEW),
            notifications::OF_THE_DRAFT,
            notifications::OF_THE_QUESTION,
            notifications::OF_THE_OPEN,
            notifications::OF_THE_SHEET,
        ],
        none,
    );
    window(
        "player",
        &[&rows!(player::OF_THE_VIEW)],
        &[player::LEADER_ALT_KEYS],
    );
    // A TEXT BEING EDITED, in the player and in the video metrics: its two
    // keys alone (the player's exceptions give its `C-s` another one key).
    window("a text being edited", &[text_area::OF_THE_TEXT_AREA], none);
    window("pointers", &[&rows!(pointers::OF_THE_VIEW)], none);
    window("power", &[power::OF_THE_VIEW], none);
    window("security", &[security::OF_THE_VIEW], none);
    window(
        "software",
        &[OF_THE_WAY_BACK, &rows!(software::OF_THE_VIEW)],
        none,
    );
    window("stick", &[stick::OF_THE_VIEW, OF_THE_CONSENT], none);
    window("syncthing", &[&rows!(syncthing::OF_THE_VIEW)], none);
    window(
        "terminal",
        &[&rows!(terminal_keys::GROUP)],
        &[terminal_keys::LEADER_ALT_KEYS],
    );
    window(
        "terminal settings",
        &[OF_THE_WAY_BACK, terminal_settings::OF_THE_VIEW],
        none,
    );
    window(
        "terminals",
        &[OF_THE_WAY_BACK, &rows!(terminals::OF_THE_VIEW)],
        none,
    );
    // THE UPDATE teaches one table a state of it — idle, queued, counting
    // down, running —, and its pages theirs.
    window(
        "update",
        &[
            update_window::OF_THE_VIEW,
            update_window::OF_THE_QUEUE,
            update_window::OF_THE_COUNTDOWN,
            update_window::OF_THE_RUN,
            update_window::OF_THE_PAGE,
            update_window::OF_THE_FOLLOW,
            update_window::OF_THE_PARTS,
        ],
        &[update_window::LEADER_ALT_KEYS],
    );
    window(
        "file picker",
        &[&rows!(ronoctua_shell_door::file_picker::OF_THE_VIEW)],
        none,
    );
}
