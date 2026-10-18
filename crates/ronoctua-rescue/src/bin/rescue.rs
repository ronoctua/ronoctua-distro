//! The rescue console — `Ctrl+Alt+Del`.
//!
//! # Why the binary is four lines
//!
//! Because a rescue console that fails at startup has nowhere to say so. There
//! is no argument to parse, no configuration to read and no mode to pick: the
//! two doors — the floating window and the VT — hand it the same terminal, and
//! everything it needs to know it reads from that terminal.

use anyhow::Result;

fn main() -> Result<()> {
    // THE PALETTES FIRST, while the terminal is still a terminal: they are
    // data read at run time (`RONOCTUA_THEMES`), and a missing file said from
    // inside the raw screen would be a console that vanishes with nothing on
    // it. The refusal names the variable.
    ronoctua_tokens::palettes::load();
    // AND THE TEXTS, for the same reason: a console that cannot say its rows
    // says so in one English line, exit 78, while the terminal still shows it
    // (`RONOCTUA_TEXTS`, missing or written by another build).
    if let Err(missing) = ronoctua_i18n::texts(&ronoctua_i18n::Keys::of(ronoctua_rescue::key::sets))
    {
        ronoctua_i18n::refuse(&missing);
    }
    // The theme from the environment only: the console may be on VT 8 with no
    // session, and a file it cannot read must not decide anything for it.
    if let Some(theme) = ronoctua_tokens::theme::from_env() {
        ronoctua_tokens::theme::set(theme);
    }
    ronoctua_rescue::tui::run()
}
