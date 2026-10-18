//! `ronoctua-player` — the daemon.
//!
//! A user unit (`nix/modules/player.nix`) that is always up and costs a
//! socket at rest: no mpv exists until something is asked to play. The
//! nested loop starts it from beside the shell (`client::ensure`).

use std::{
    os::unix::net::UnixListener,
    sync::{Arc, Condvar, Mutex},
};

use anyhow::{Context as _, Result};
use ronoctua_player::wire;

mod engine;
mod mpris;
mod mpv;
mod remote;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                // THE BINARY'S OWN NAME is what its modules log under
                // (`ronoctua_player::engine`, `::mpris`, `::remote`) — the
                // package's name matched nothing, and the daemon said no
                // more than its warnings.
                .unwrap_or_else(|_| "ronoctua_player=info".into()),
        )
        .init();

    // THE PALETTES, read now and not at the first subtitle: the lanes it
    // burns into a film are tokens, and the colours are data read at run time
    // (`RONOCTUA_THEMES`, which the wrapper sets). A missing file stops the
    // daemon at its start, naming the variable, instead of in the middle of
    // a film.
    ronoctua_tokens::palettes::load();

    // OPEN THESE, when the daemon is started with paths — which is what the
    // `.desktop` entry does when there is no player yet.
    let items: Vec<String> = std::env::args()
        .skip(1)
        .map(|a| engine::absolute(&a))
        .collect();

    let path = wire::socket_path()?;
    // OPENED FROM OUTSIDE, what is named plays now, right after what played,
    // and the queue is kept (`Ask::Put`'s rule).
    let put = |items: Vec<String>| wire::Ask::Put {
        what: items.into_iter().map(wire::Thing::Location).collect(),
        after: None,
        into: None,
        play: true,
        replace: false,
        once: false,
    };
    // A socket that answers is another player: hand it the items and leave.
    if std::os::unix::net::UnixStream::connect(&path).is_ok() {
        if !items.is_empty() {
            ronoctua_player::client::tell(&put(items))?;
            return Ok(());
        }
        anyhow::bail!("a player is already listening at {}", path.display());
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("could not listen at {}", path.display()))?;

    engine::forget_old_decodes();
    let shared: engine::Shared = Arc::new((Mutex::new(engine::Engine::new()), Condvar::new()));
    if let Ok(mut e) = shared.0.lock() {
        e.set_shared(&shared);
    }
    if !items.is_empty() {
        let (lock, _) = &*shared;
        if let Ok(mut e) = lock.lock() {
            let _ = e.ask(put(items), &shared);
        }
    }

    // MPRIS, so the media keys and `playerctl` reach this player.
    let poke = mpris::start(shared.clone());
    if let Ok(mut e) = shared.0.lock() {
        e.set_mpris(poke);
    }

    // AND THE TAILNET: other machines join this one there, and only there.
    remote::listen(shared.clone());

    tracing::info!(socket = %path.display(), "the player is listening");
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("player-client".into())
            .spawn(move || engine::serve_one(stream, &shared));
    }
    Ok(())
}
