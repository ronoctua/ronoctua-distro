//! The studio's own process: the unit, listening on its socket.
//!
//! Always running with the session, and costing nothing at rest: with no
//! window showing the preview, no recording, no stream and no virtual camera,
//! every source is closed and the loop sleeps on a condition variable. It is
//! not socket-activated because adopting the listening descriptor systemd
//! passes needs `from_raw_fd`, which is `unsafe`, for a saving of one idle
//! process.
//!
//! STARTING IT OPENS NO CAMERA. Reading the cameras opens every node, and a
//! UVC camera's light blinks when it is opened: the engine reads them only
//! while a camera is in use (`Engine::follow_cameras`), and the virtual
//! camera finds its node by its label (`outputs::Virtual::start`).

use std::{
    os::unix::net::UnixListener,
    sync::{Arc, Condvar, Mutex},
};

use anyhow::{Context as _, Result};
use ronoctua_studio::{engine, wire};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_studio=info".into()),
        )
        .init();

    let path = wire::socket_path()?;
    // A socket file left by a killed daemon answers nothing; one that
    // answers is another studio, and two would fight over the camera.
    if std::os::unix::net::UnixStream::connect(&path).is_ok() {
        anyhow::bail!("a studio is already listening at {}", path.display());
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("could not listen at {}", path.display()))?;

    let shared: engine::Shared = Arc::new((Mutex::new(engine::Engine::new()), Condvar::new()));

    // The virtual camera, when the person asked for it at every start.
    {
        let (lock, _) = &*shared;
        if let Ok(mut e) = lock.lock()
            && ronoctua_studio::prefs::store().read().virtual_at_start
        {
            let _ = e.ask(wire::Ask::Virtual { on: Some(true) }, &shared);
        }
    }

    let looping = shared.clone();
    std::thread::Builder::new()
        .name("studio-loop".into())
        .spawn(move || engine::run(&looping))?;

    tracing::info!(socket = %path.display(), "the studio is listening");
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("studio-client".into())
            .spawn(move || engine::serve_one(stream, &shared));
    }
    Ok(())
}
