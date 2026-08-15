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
    ronoctua_rescue::tui::run()
}
