//! Proves that EVERY action of the keymap exists in the compositor — §7.4 and §13.
//!
//! # Why this binary exists
//!
//! The window and workspace shortcuts stopped being niri binds and became
//! shell commands, dispatched by action name over IPC. The name travels as a
//! string: a `MoveColumnLeft` wrong by one letter compiles, runs, and niri
//! answers `{"Err": "error parsing request"}` — which becomes a `warn` in the
//! log and **nothing on screen**. It is exactly the defect this work came to
//! fix, now in other clothes.
//!
//! A `match` in Rust does not protect against that: the compiler checks the
//! command's *id*, not the name of the action on the other side. Whoever
//! checks is this, against a actual niri.
//!
//! Usage: `ronoctua-niri-actions <niri-socket>`

use anyhow::{Context as _, Result, bail};

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("uso: ronoctua-niri-actions <socket-do-niri>")?;

    let mut failures = Vec::new();
    let table = actions::all_of();
    for (id, action) in &table {
        // A new connection per action: the daemon's `Niri` is blocked reading the
        // event stream, and it is how `run_command` does it too.
        let mut niri = ronoctua_niri::Niri::connect_at(std::path::Path::new(&path))
            .with_context(|| format!("connect in order to test `{id}`"))?;
        match niri.action(action.clone()) {
            Ok(()) => println!("  ok   {id:24} -> {action}"),
            Err(e) => {
                println!("  ERRO {id:24} -> {action}");
                failures.push(format!("{id}: {e}"));
            }
        }
    }

    if !failures.is_empty() {
        eprintln!("\nERROR: actions niri did not accept:\n");
        for f in &failures {
            eprintln!("  {f}");
        }
        eprintln!(
            "\nA wrong action name does not break the build: it becomes a `warn`\n\
             in the log and NOTHING on screen. Fix it in `crates/ronoctua-shell/src/actions.rs`."
        );
        bail!("{} of {} actions refused", failures.len(), table.len());
    }

    println!(
        "\nOK: the keymap's {} actions exist in this niri",
        table.len()
    );
    Ok(())
}

/// The table, in a module of its own so the binary and the shell can share it.
///
/// `allow(dead_code)`: included through `#[path]`, the module goes into this
/// binary as a copy, and here only `all_of()` is used — the `kdl_of()` belongs to
/// the shell. Without this clippy refuses the function the OTHER consumer calls.
#[path = "../actions.rs"]
#[allow(dead_code)]
mod actions;
