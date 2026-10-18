//! `ronoctua-shell-terminal` — a terminal in an `xdg_toplevel` of its own.
//!
//! The Wayland half of `ronoctua-term`'s engine: one window and three
//! identities — the terminal (`ronoctua-shell terminal`), the monitor and the
//! rescue console — on the kit and the door only.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the module already uses
// (`crate::door::…`, `crate::floating::…`).
use ronoctua_shell_door::{door, floating};
use ronoctua_shell_kit::{daemon, paint};

pub mod terminal;

/// THE TERMINAL'S THREE IDENTITIES the binary runs by name: a terminal
/// (`ronoctua-shell terminal [program]`), the monitor's floating one
/// (`monitor <program>`) and the rescue console (`rescue <program>`).
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
];

/// A terminal in a window of its own. With no argument it starts `$SHELL`;
/// with one, it runs what is asked — and it is what the VM test uses so as
/// not to depend on some shell's prompt.
fn terminal_door(args: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let args = args.all();
    let (program, rest) = args
        .split_first()
        .map_or((None, &[][..]), |(p, r)| (Some(p.as_str()), r));
    terminal::host_it(program, rest).context("host the terminal")
}

/// THE SAME TERMINAL, under the monitor's `app_id`. It is a door and not a
/// flag on the one above because what changes is the window's IDENTITY,
/// which is what the compositor's rule reads to float it — see
/// `floating::APP_ID_MONITOR`. The program is required: a monitor with no
/// monitor to run is a floating window with a shell in it.
fn monitor_door(args: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let args = args.all();
    let (program, rest) = args
        .split_first()
        .context("usage: ronoctua-shell monitor <program> [args]")?;
    terminal::host_monitor(program, rest).context("host the monitor")
}

/// THE RESCUE CONSOLE, and a third identity for the same terminal.
///
/// `Ctrl+Alt+Del` reaches this, and so does the VT unit — the difference is
/// that over there nobody goes through the shell at all. That is the whole
/// point of the console being a terminal application: this door is a
/// convenience for the session that has a compositor, not the way in.
fn rescue_door(args: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let args = args.all();
    let (program, rest) = args
        .split_first()
        .context("usage: ronoctua-shell rescue <program> [args]")?;
    terminal::host_rescue(program, rest).context("host the rescue console")
}
