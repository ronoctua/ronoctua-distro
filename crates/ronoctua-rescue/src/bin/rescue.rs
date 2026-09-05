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
    // The theme from the environment only: the console may be on VT 8 with no
    // session, and a file it cannot read must not decide anything for it.
    if let Some(theme) = ronoctua_tokens::theme::from_env() {
        ronoctua_tokens::theme::set(theme);
    }
    ronoctua_rescue::tui::run()
}
