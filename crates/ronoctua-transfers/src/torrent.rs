//! The torrent engine: one session, many rows.
//!
//! # Why this one is not a thread per job
//!
//! The other three engines are: a copy, an aria2 download and a yt-dlp process
//! each own their thread and their whole life happens inside it. A libtorrent
//! session cannot work that way — it is ONE object with its own threads, its
//! own disk cache and its own port, serving every torrent at once. Two sessions
//! would be two ports, two caches and two copies of the DHT.
//!
//! So there is one session thread, started the first time a torrent is asked
//! for, and the per-job thread that `serve` spawns merely hands the work over
//! and returns. What keeps the rows moving after that is the session thread's
//! poll, which reads every torrent at once and writes every row at once.
//!
//! # A finished torrent keeps seeding
//!
//! The row goes to `done` when the bytes are all there, and the session carries
//! on giving them back — which is what a torrent client is for, and what every
//! other one does. Giving up (`C-c`) is what stops it, and it takes the data
//! with it. Dropping the row from the list does not stop the seeding; that is
//! said here and in `docs/ARCHITECTURE.md` rather than left to be discovered.

use crate::job::{Stage, Work};
use crate::ledger::Shared;
use ronoctua_files::watch::{Control, Flow};
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};

/// How often the session is asked where everything is.
///
/// Half a second: a torrent's rate is averaged over seconds anyway, and one
/// poll reads EVERY row, so this is a fixed cost and not one per job.
const POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// The port range the session listens on.
///
/// Fixed, and declared in `nix/modules/transfers.nix` where a firewall rule can
/// name the same numbers. Seeding means accepting connections, and a program
/// that asked the router to open a port on its own would have changed the
/// machine's exposure without saying so — which is why UPnP is off in the shim.
const PORTS: (u16, u16) = (6881, 6889);

/// What the session thread is asked to do.
enum Order {
    /// Take this on, and report it under this job id.
    Add {
        id: u64,
        source: String,
        into: PathBuf,
        switch: Arc<Control>,
    },
}

/// The way in. `None` until the first torrent is asked for.
static SWARM: OnceLock<Option<Sender<Order>>> = OnceLock::new();

/// Where the session keeps what it needs to survive a reboot.
///
/// Under `XDG_DATA_HOME`, because it is state that should outlive a restart —
/// unlike the socket, which is deliberately in `XDG_RUNTIME_DIR` and dies with
/// the session.
fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        })?;
    Some(base.join("ronoctua").join("torrent"))
}

/// One row the session thread is watching.
struct Watched {
    /// The job it reports under.
    id: u64,
    /// What libtorrent knows it by. Empty until the add succeeds.
    hash: String,
    switch: Arc<Control>,
    /// Whether we have already told the session to hold it.
    held: bool,
    /// Whether the row has been reported finished, so `done` is written once.
    settled: bool,
}

/// Starts the session thread, once.
fn swarm(shared: &Arc<Shared>) -> Option<&'static Sender<Order>> {
    SWARM
        .get_or_init(|| {
            if !ronoctua_torrent::available() {
                tracing::error!("this build does not have the torrent engine");
                return None;
            }
            let dir = state_dir()?;
            let (orders, arriving) = channel::<Order>();
            let mine = Arc::clone(shared);
            let built = std::thread::Builder::new()
                .name("torrent".into())
                .spawn(move || session(&mine, dir, &arriving));
            match built {
                Ok(_) => Some(orders),
                Err(e) => {
                    tracing::error!(error = %e, "could not start the torrent session's thread");
                    None
                }
            }
        })
        .as_ref()
}

/// The session thread: it owns the session and never lets it out.
///
/// The same discipline `ronoctua-term` uses for its `!Send` engine: the object
/// is built INSIDE the thread and every request arrives as a message. It is
/// what lets a type that must not move be used without a lock.
fn session(shared: &Arc<Shared>, dir: PathBuf, orders: &std::sync::mpsc::Receiver<Order>) {
    let mut session = match ronoctua_torrent::Session::open(&dir, PORTS) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "the torrent session did not come up");
            return;
        }
    };
    // NO THROTTLE HERE, unlike the other engines: one poll writes every row
    // at once, twice a second, so the ledger is woken twice a second no matter
    // how many torrents there are. The throttle exists to stop a per-job loop
    // flooding it, and there is no per-job loop.
    let mut watching: Vec<Watched> = Vec::new();

    loop {
        // New work first, so a magnet pasted now shows a row on this very poll
        // rather than on the next one.
        while let Ok(order) = orders.try_recv() {
            let Order::Add {
                id,
                source,
                into,
                switch,
            } = order;
            let added = if source.starts_with("magnet:") {
                session.add_magnet(&source, &into)
            } else {
                session.add_file(std::path::Path::new(&source), &into)
            };
            match added {
                Ok(hash) => {
                    shared.touch(id, |j| j.stage = Stage::Running);
                    watching.push(Watched {
                        id,
                        hash,
                        switch,
                        held: false,
                        settled: false,
                    });
                }
                Err(e) => {
                    let text = format!("{e:#}");
                    shared.touch(id, |j| {
                        j.stage = Stage::Failed;
                        j.trouble = Some(text);
                    });
                }
            }
        }

        let rows = session.poll();
        watching.retain_mut(|w| keep(shared, &mut session, &rows, w));

        std::thread::sleep(POLL);
    }
}

/// Updates one watched row, and says whether to keep watching it.
fn keep(
    shared: &Arc<Shared>,
    session: &mut ronoctua_torrent::Session,
    rows: &[ronoctua_torrent::Torrent],
    w: &mut Watched,
) -> bool {
    match w.switch.flow() {
        Flow::Stop => {
            // The data goes with it, which is the promise a cancelled copy
            // makes — and the reason the key asks first.
            session.remove(&w.hash, true);
            shared.touch(w.id, |j| j.stage = Stage::Cancelled);
            return false;
        }
        Flow::Pause if !w.held => {
            session.pause(&w.hash);
            w.held = true;
        }
        Flow::Run if w.held => {
            session.resume(&w.hash);
            w.held = false;
        }
        Flow::Pause | Flow::Run => {}
    }

    let Some(row) = rows.iter().find(|r| r.hash == w.hash) else {
        // Not in the session's list any more, and nobody here removed it:
        // there is nothing left to report, and keeping the row afoot would
        // leave a bar that never moves.
        return true;
    };

    let stage = match row.state {
        ronoctua_torrent::State::Errored => Stage::Failed,
        _ if row.paused => Stage::Paused,
        // Metadata and checking are both "we do not know the total yet", which
        // is exactly what `weighing` means for a copy.
        ronoctua_torrent::State::Metadata | ronoctua_torrent::State::Checking => Stage::Weighing,
        ronoctua_torrent::State::Downloading => Stage::Running,
        ronoctua_torrent::State::Finished | ronoctua_torrent::State::Seeding => Stage::Done,
    };
    let trouble = (!row.trouble.is_empty()).then(|| row.trouble.clone());
    let (done, total, name) = (row.done, row.total, row.name.clone());
    let settled = stage == Stage::Done;
    shared.touch(w.id, |j| {
        j.stage = stage;
        j.done = done;
        j.total = total;
        j.now = name;
        if trouble.is_some() {
            j.trouble = trouble;
        }
    });
    if settled && !w.settled {
        w.settled = true;
        tracing::info!(id = w.id, hash = %w.hash, "torrent completo; continua semeando");
    }
    true
}

/// Hands one torrent to the session and returns.
///
/// It returns immediately on purpose: what keeps the row moving afterwards is
/// the session thread, and a thread per torrent sitting here doing nothing
/// would be one thread per row for the life of the seed.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Torrent { source, into } = work else {
        tracing::error!("the torrent engine got a job that is not a torrent");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    let Some(orders) = swarm(shared) else {
        shared.touch(id, |j| {
            j.stage = Stage::Failed;
            j.trouble = Some("the torrent engine is not available".to_owned());
        });
        return;
    };
    let sent = orders.send(Order::Add {
        id,
        source,
        into,
        switch: Arc::clone(switch),
    });
    if sent.is_err() {
        shared.touch(id, |j| {
            j.stage = Stage::Failed;
            j.trouble = Some("the torrent session is no longer up".to_owned());
        });
    }
}

/// Whether a source looks like something this engine can take.
///
/// Used by whoever builds the request — the file manager, the browser host,
/// the `.desktop` handler — so a magnet does not arrive at aria2, which would
/// refuse it with a message about a scheme.
#[must_use]
pub fn takes(source: &str) -> bool {
    source.starts_with("magnet:") || source.ends_with(".torrent")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_this_engine_takes_and_what_it_leaves() {
        assert!(takes("magnet:?xt=urn:btih:abc"));
        // BEP 52 magnets carry `btmh` and not `btih`, and a check that looked
        // for the older word would send every v2 magnet to the wrong engine.
        assert!(takes(
            "magnet:?xt=urn:btmh:1220caf1e1f4a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6"
        ));
        assert!(takes("/home/someone/debian.torrent"));
        assert!(!takes("https://example/file.iso"));
        assert!(!takes("https://exemplo/video"));
    }
}
