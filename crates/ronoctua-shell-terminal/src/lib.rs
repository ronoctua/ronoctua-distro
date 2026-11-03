//! `ronoctua-shell-terminal` — a terminal in an `xdg_toplevel` of its own.
//!
//! The window is a VIEWER: the terminal lives in a holder (`ronoctua-term`'s,
//! one process per terminal, in a scope of its own), and the window attaches
//! to it, mirrors it and draws the mirror as a grid of cells. One window and
//! three identities — the terminal (`ronoctua-shell terminal`), the monitor
//! and the rescue console — on the kit and the door only:
//!
//! - [`terminal`]: the window, its command line and the window rules;
//! - [`terminal_link`]: reaching the terminal (a holder of its own, or one in
//!   this process) and the threads that follow it;
//! - [`terminal_sheet`]: the page's colours and the frame made cells;
//! - [`terminal_keys`]: the keys the terminal keeps, and every other one as
//!   the frame the holder encodes;
//! - [`terminal_pointer`]: where the grid is, and the wheel's owners;
//! - [`terminal_ask`]: the line laid over the last row — a message, a
//!   question with its no first;
//! - [`terminal_group`]: the terminals one window holds, and the one in
//!   front; [`terminal_strip`], the only file that draws their lines;
//! - [`terminal_search`]: `C-S-f`, on a thread of its own;
//! - [`terminal_hyperlink`]: what `Ctrl` shows and opens;
//! - [`terminal_drop`]: files dropped, typed as their paths;
//! - [`terminal_photograph`]: its camera, over a recorded stream.
//!
//! And two views of their own, each with two doors — the footer's region and
//! a window: the terminals no window shows ([`terminals`],
//! [`terminals_region`], [`terminals_window`]; `LEADER RET b`), and the
//! terminal's settings ([`terminal_settings`], [`terminal_settings_region`],
//! [`terminal_settings_window`]; `LEADER RET s`).

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::door::…`, `crate::floating::…`).
use ronoctua_shell_door::{carry, cursor_shape, door, floating, pasting, region};
use ronoctua_shell_kit::{
    bars, bindings, chrome, clipboard_copy, daemon, echo, fields, partly_read, text_select, wheel,
};

pub mod key;
pub mod terminal;
pub mod terminal_ask;
pub mod terminal_drop;
pub mod terminal_group;
pub mod terminal_hyperlink;
pub mod terminal_keys;
pub mod terminal_link;
pub mod terminal_photograph;
pub mod terminal_pointer;
pub mod terminal_search;
pub mod terminal_settings;
pub mod terminal_settings_region;
pub mod terminal_settings_window;
pub mod terminal_sheet;
pub mod terminal_strip;
pub mod terminals;
pub mod terminals_region;
pub mod terminals_window;

/// THE TERMINAL'S THREE IDENTITIES the binary runs by name: a terminal
/// (`ronoctua-shell terminal [--cwd D] [--here] [--] [program]`, or
/// `terminal --show ID[,ID…] --front ID`), the monitor's floating one
/// (`monitor <program>`) and the rescue console (`rescue <program>`) — and
/// the windows of its two views.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[
    ronoctua_shell_kit::doors::Door {
        name: "terminal",
        run: terminal_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "monitor",
        run: monitor_door,
    },
    ronoctua_shell_kit::doors::Door {
        name: "rescue",
        run: rescue_door,
    },
    // THE TERMINALS NO WINDOW SHOWS, in a window of their own (`RET b w`).
    ronoctua_shell_kit::doors::Door {
        name: terminals_region::BACKGROUND_DOOR,
        run: terminals_door,
    },
    // THE TERMINAL'S SETTINGS, in a window of their own (`RET s w`).
    ronoctua_shell_kit::doors::Door {
        name: terminal_settings_region::SETTINGS_DOOR,
        run: settings_door,
    },
];

/// THE TERMINAL'S CAMERAS: a recorded stream drawn as the window draws a
/// terminal (`photograph --door terminal --corpus <file.vt>`), the terminals
/// no window shows over a planted census (`--door terminals --terminals
/// <file.json>`), and the settings over a planted file (`--door
/// terminal-settings --settings <terminal.json>`).
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[
    ronoctua_shell_door::photo::Camera {
        door: "terminal",
        take: terminal_photograph::taken,
    },
    ronoctua_shell_door::photo::Camera {
        door: terminals_region::BACKGROUND_DOOR,
        take: terminals_window::photograph::taken,
    },
    ronoctua_shell_door::photo::Camera {
        door: terminal_settings_region::SETTINGS_DOOR,
        take: terminal_settings_window::photograph::taken,
    },
];

/// A terminal in a window of its own. With no argument it starts `$SHELL`,
/// held in a scope of its own; with one, it runs what is asked — and it is
/// what the VM test uses so as not to depend on some shell's prompt.
fn terminal_door(args: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let plan = terminal::plan_of(&args.all())?;
    terminal::host(terminal::Identity::Terminal, plan).context("host the terminal")
}

/// THE SAME TERMINAL, under the monitor's `app_id`. It is a door and not a
/// flag on the one above because what changes is the window's IDENTITY,
/// which is what the compositor's rule reads to float it — see
/// `floating::APP_ID_MONITOR`. The program is required: a monitor with no
/// monitor to run is a floating window with a shell in it. In the window's
/// own process, as the rescue console: both must work where no scope can be
/// had.
fn monitor_door(args: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let plan =
        own_process(&args.all()).context("usage: ronoctua-shell monitor <program> [args]")?;
    terminal::host(terminal::Identity::Monitor, plan).context("host the monitor")
}

/// THE RESCUE CONSOLE, and a third identity for the same terminal.
///
/// `Ctrl+Alt+Del` reaches this, and so does the VT unit — the difference is
/// that over there nobody goes through the shell at all. That is the whole
/// point of the console being a terminal application: this door is a
/// convenience for the session that has a compositor, not the way in.
fn rescue_door(args: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let plan = own_process(&args.all()).context("usage: ronoctua-shell rescue <program> [args]")?;
    terminal::host(terminal::Identity::Rescue, plan).context("host the rescue console")
}

/// `ronoctua-shell terminals` — the terminals no window shows, in a window
/// of their own.
fn terminals_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    terminals_window::host_it().context("host the background terminals")
}

/// `ronoctua-shell terminal-settings` — the terminal's settings, in a window
/// of their own.
fn settings_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    terminal_settings_window::host_it().context("host the terminal's settings")
}

/// A program, run in the window's own process.
fn own_process(args: &[String]) -> Option<terminal_link::Plan> {
    let (program, rest) = args.split_first()?;
    Some(terminal_link::Plan::New {
        program: program.clone(),
        args: rest.to_vec(),
        cwd: std::env::current_dir().ok(),
        here: true,
    })
}

/// What this crate adds to the leader's keymap ([`bindings::Contribution`]) —
/// one for the crate, as every crate's is, and what each part of it says:
///
/// - THE GROUP'S FOUR COMMANDS, which only `LEADER :` reaches by name: the
///   keys are a terminal window's own (`C-T`, `C-TAB`, `C-S-TAB`, `C-S-RET`),
///   and the footer hands a command to the window in focus that publishes it
///   (`Content::shortcuts`);
/// - THE TERMINALS WITH NO WINDOW — `RET b b` in the footer and `RET b w` in
///   a window, the doubled letter and the `w` every view of this shape uses;
///   `b` for BACKGROUND, under the terminal's own group, whose `RET RET` is
///   the terminal and `RET /` the external one ([`terminals`]);
/// - THE TERMINAL'S SETTINGS — `RET s s` and `RET s w`, `s` for SETTINGS as
///   it is the system's at the root ([`terminal_settings`]);
/// - and the two views' own gestures, reached through their `OF_THE_VIEW`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "terminal",
    commands: &[
        ronoctua_cmd::Command {
            id: terminal_keys::NEW,
            label: key::CMD_TERMINAL_NEW,
        },
        ronoctua_cmd::Command {
            id: terminal_keys::NEXT,
            label: key::CMD_TERMINAL_NEXT,
        },
        ronoctua_cmd::Command {
            id: terminal_keys::PREVIOUS,
            label: key::CMD_TERMINAL_PREVIOUS,
        },
        ronoctua_cmd::Command {
            id: terminal_keys::DETACH,
            label: key::CMD_TERMINAL_DETACH,
        },
        ronoctua_cmd::Command {
            id: terminals::OPEN,
            label: key::CMD_TERMINALS_OPEN,
        },
        ronoctua_cmd::Command {
            id: terminals::WINDOW,
            label: key::CMD_TERMINALS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "terminals-up",
            label: key::CMD_TERMINALS_UP,
        },
        ronoctua_cmd::Command {
            id: "terminals-down",
            label: key::CMD_TERMINALS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "terminals-enter",
            label: key::CMD_TERMINALS_ENTER,
        },
        ronoctua_cmd::Command {
            id: "terminals-end",
            label: key::CMD_TERMINALS_END,
        },
        ronoctua_cmd::Command {
            id: terminal_settings::OPEN,
            label: key::CMD_TERMINAL_SETTINGS_OPEN,
        },
        ronoctua_cmd::Command {
            id: terminal_settings::WINDOW,
            label: key::CMD_TERMINAL_SETTINGS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "terminal-settings-up",
            label: key::CMD_TERMINAL_SETTINGS_UP,
        },
        ronoctua_cmd::Command {
            id: "terminal-settings-down",
            label: key::CMD_TERMINAL_SETTINGS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "terminal-settings-less",
            label: key::CMD_TERMINAL_SETTINGS_LESS,
        },
        ronoctua_cmd::Command {
            id: "terminal-settings-more",
            label: key::CMD_TERMINAL_SETTINGS_MORE,
        },
        ronoctua_cmd::Command {
            id: "terminal-settings-enter",
            label: key::CMD_TERMINAL_SETTINGS_ENTER,
        },
    ],
    // THE IDS SPELT OUT, as every view's binding is — what a reader of the
    // keymap (`footer-mouse-reaches-nothing-new`) finds the mark's twin by.
    bindings: &[
        ("RET b b", "terminals-open", None),
        ("RET b w", "terminals-window", None),
        ("RET s s", "terminal-settings-open", None),
        ("RET s w", "terminal-settings-window", None),
    ],
    groups: &[
        ("RET b", key::GROUP_TERMINALS),
        ("RET s", key::GROUP_TERMINAL_SETTINGS),
    ],
    by_name: &[
        // A TERMINAL WINDOW'S OWN KEYS: each is a key of the window in focus
        // (`terminal_keys::GROUP`), and a sequence of the leader's would run
        // it where no terminal is. By name, the footer hands it to the
        // window that published it.
        terminal_keys::NEW,
        terminal_keys::NEXT,
        terminal_keys::PREVIOUS,
        terminal_keys::DETACH,
    ],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the two views' own keys and rows run, for the keymap's
/// reachability (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    terminals::keys_reached()
        .into_iter()
        .chain(terminal_settings::keys_reached())
        .collect()
}
