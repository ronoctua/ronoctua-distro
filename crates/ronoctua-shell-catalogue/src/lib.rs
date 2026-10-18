//! `ronoctua-shell-catalogue` — channels by theme (`LEADER p n`): the local
//! catalogue as a page of the player.
//!
//! One source at a time, its themes and categories, the LISTING a row of them
//! opens and the adult lock (`player_catalog`), and the sources' page
//! (`player_sources`). The player's view holds it as one of its pages
//! (`player_seam::Page`, the `PAGE` the binary installs), asks it at the
//! places it asks its own pages, and names nothing of it but its words.
//!
//! It stands on the kit and the player: a line here recompiles this crate and
//! the binary, never the player's transport, its stage or its window.

#![deny(unsafe_code)]

// THE KIT AND THE PLAYER, by the names these modules already use
// (`crate::chrome::…`, `crate::player::…`).
use ronoctua_shell_kit::{chrome, wake};
use ronoctua_shell_player::{
    player, player_find, player_globe, player_posters, player_question, player_rows, player_seam,
    player_window,
};

pub mod key;
pub mod player_catalog;
pub mod player_sources;

/// THE ADULT CATALOGUE'S LOCK, asked as a door asks it, by name
/// (`ronoctua-shell unlock-check`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "unlock-check",
    run: unlock_door,
}];

/// `ronoctua-shell unlock-check` — the adult catalogue's lock asked as a door
/// asks it (`player_catalog::unlock`): this session's user, the
/// `ronoctua-unlock` service, the password read from STANDARD INPUT (never an
/// argument: an argv is every process's to read) into the `Secret` that wipes
/// it. It prints `open` or the refusal's name, and fails unless open — what
/// `player-vm` proves the declared service with, and what a person runs to
/// see why the lock answers as it does.
fn unlock_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("read the password from standard input")?;
    let kept = line.trim_end_matches(['\n', '\r']).len();
    line.truncate(kept);
    let secret = ronoctua_consent::keys::Secret::new(line);
    let answer = player_catalog::unlock(&secret);
    drop(secret);
    match answer {
        Ok(()) => {
            println!("open");
            Ok(())
        }
        Err(refusal) => {
            println!("{}", refusal.id());
            std::process::exit(1);
        }
    }
}
