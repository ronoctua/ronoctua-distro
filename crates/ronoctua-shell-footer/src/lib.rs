//! `ronoctua-shell-footer` — the two-line footer, the system's only chrome.
//!
//! # The system's only chrome (sections 3 and 4.1)
//!
//! **ONE** `wlr-layer-shell` surface, anchored at the bottom, taking the whole
//! width, with an exclusive zone of two lines of text. There is no separate
//! bar, dock, bar clock, taskbar or floating popup — and there is not going
//! to be (invariant 14).
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │              content (buffers)           │
//! ├──────────────────────────────────────────┤
//! │ modeline                  [tray] [notif] │
//! │ minibuffer                               │
//! └──────────────────────────────────────────┘
//! ```
//!
//! What is here: the surface and its region (`surface`), the leader's state
//! machine and the base keymap (`leader`), the modeline and its adapters
//! (`modeline`, `adapt`, `kiosk`), the events said on the minibuffer, the
//! brain's line, the sudo bar, the on-screen keyboard, the hands, the glance,
//! the footer's own settings and its camera.
//!
//! # What this crate does NOT do
//!
//! - **It does not tile windows.** What does that is the compositor
//!   (invariant 4).
//! - **It does not talk to the compositor through anything other than IPC or
//!   layer-shell** (invariant 3); no crate of ours but `ronoctua-scroll`
//!   knows which compositor it is.
//! - **It names no feature.** A feature's view is a region of the slot host,
//!   known by its id and registered by the binary, which hands this crate its
//!   regions and their residents when it starts ([`Composition`]); what the
//!   footer says of a view's daemon for the whole session is that view's
//!   words leaf, under both (`footer-names-no-feature`).

// `deny`, not `forbid`: `surface/mod.rs` needs ONE unsafe block to pass
// Wayland's pointers to wgpu, and `forbid` accepts no exception even with the
// contract written out. `deny` requires an explicit, visible `#[allow]`.
#![deny(unsafe_code)]

// THE FOUNDATION, by the names these modules already use (`crate::chrome::…`,
// `crate::door::…`, `crate::jobs::…`): a `use` at the root is seen by every
// module below it and is no part of this crate's API.
use ronoctua_shell_door::{
    brainlink, capture_wire, carry, clipboard, clipboard_wire, column, cursor_shape, door, launch,
    pasting, photo, queue, region, right_column, room, sections, sudo, tags,
};
use ronoctua_shell_kit::{
    actions, aim, band, bars, bindings, chrome, clipboard_copy, daemon, desk, echo, fields, fold,
    foreign, glyph, hidpi, ipc, keyboard, language, line, mind_wire, minibuffer, seats,
    security_words, sizes, text_select, theme, toolkits, wake, watchdog, wheel, when,
};
use ronoctua_shell_report::report;
use ronoctua_shell_work::{jobs, programs_words};
// AND THE WORDS OF A VIEW THE FOOTER SAYS TOO — one leaf per view, under the
// view and the footer: a refusal of an order the footer's own line carried,
// the modeline's marks and what a click on one opens, the radio's question,
// the studio's refusals.
use ronoctua_shell_audio_words::audio_words;
use ronoctua_shell_bluetooth_words::bluez_words;
use ronoctua_shell_camera_words::studio_words;
use ronoctua_shell_display_words::screens_words;
use ronoctua_shell_history_words::history_words;
use ronoctua_shell_network_words::link_words;
use ronoctua_shell_player_words::{player_mark, player_state};
use ronoctua_shell_power_words::power_words;
use ronoctua_shell_syncthing_words::syncthing_words;
use ronoctua_shell_update_words::update_mark;

// THE MODELINE THAT ADAPTS TO A PROGRAM — one adapter per program, the
// Chrome kiosk's the first.
pub mod adapt;
mod apps;
// THE BRAIN'S LINE, under the minibuffer, and the thread that feeds it.
mod brain;
// WHICH BUILD IS RUNNING (the machine's own, or a distro checkout's reference),
// and the switch between them — `LEADER / b` and the modeline glyph.
mod build_switch;
pub mod events;
// THE CAMERA'S HANDS: what a pose means, and the command each runs.
pub mod gestures;
mod hidden;
pub mod key;
mod keys_link;
// THE CHROME KIOSK (`LEADER m b`): the pairs and the modeline's adapter, and
// the view its `…`, `LEADER w B` and the `,` group open.
mod kiosk;
mod kiosk_view;
pub mod leader;
mod modeline;
mod osk;
mod sound;
pub mod surface;
mod theme_chooser;
// THE COMPOSITOR'S SELECTION, as far as this footer can know it.
mod selection;
mod trails;
mod virtual_keys;
mod virtual_pointer;
mod window_menu;
mod workspaces;
mod world;

// THE KEYS PACKAGE (2026-09-30): the maximum alert bar, the classes of a
// message's life, and the footer's own settings in both doors.
mod alert;
mod footer_settings;
pub mod footer_settings_window;

// THE FOOTER IN A PNG (`photograph --door footer`).
pub mod footer_photograph;

// WHAT ENDS BEFORE THE SESSION DOES: the user units restarted, while the
// compositor is still up, before `session-quit` asks it to leave.
mod session_end;

// THE SUDO BAR (2026-10-11): where every root gesture of every screen is
// asked — the footer's strip above the modeline, and the gestures the views'
// doors make through it.
mod sudo_bar;

/// WHAT THE BINARY HANDS THE FOOTER — the one crate that names every
/// feature tells the one that names none what it hosts.
///
/// The regions are the views of the features, each a `region::Region` of
/// its own crate, ranked by their ids in the footer's precedence
/// (`surface::PRECEDENCE`); the residents are what a feature keeps in the
/// footer for the session beside its region; the plantings are what the
/// footer's camera plants of a feature in a photograph. The footer refuses
/// to start on a region registered without a rank, or a rank no region
/// answers (`surface::ranks_are_whole`).
#[derive(Clone, Copy)]
pub struct Composition {
    /// Every region the footer hosts.
    pub regions: &'static [region::Entry],
    /// What a feature keeps in the footer for the session, beside its
    /// region.
    pub residents: &'static [region::Residing],
    /// What the footer's camera plants of a feature.
    pub plantings: footer_photograph::Plantings,
}

impl std::fmt::Debug for Composition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composition")
            .field("regions", &self.regions)
            .field("residents", &self.residents)
            .finish_non_exhaustive()
    }
}

/// THE FOOTER'S OWN SETTINGS IN A WINDOW the binary runs by name
/// (`ronoctua-shell footer-settings`, `LEADER s r w`): the SAME
/// `footer_settings::View` the footer's region draws, and every change is a
/// command the footer runs.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "footer-settings",
    run: settings_door,
}];

/// `ronoctua-shell footer-settings` — the footer's own settings in a window.
fn settings_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    footer_settings_window::host_it().context("host the footer's settings")
}
