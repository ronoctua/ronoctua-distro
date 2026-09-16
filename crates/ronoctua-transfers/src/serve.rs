//! Answering the socket: what each ask means, and what goes back.
//!
//! # Why this is in the library and not in the binary
//!
//! Because a protocol that is only exercised by a running daemon is a protocol
//! nobody tests. Here, a test binds a socket in a temporary directory, drives
//! it with the real client and reads the real answers — which is what caught
//! the one case a hand-written check would have skipped: a filename that is not
//! UTF-8 crossing the wire and coming back the same bytes.
//!
//! It is the arrangement `leader::listen_on` already uses, and for the same
//! stated reason: `XDG_RUNTIME_DIR` is process-global, and a test that changes
//! it races every other test in the binary.

use std::io::{BufReader, BufWriter, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use ronoctua_files::watch::Flow;

use crate::job::{Engine, Stage};
use crate::ledger::Shared;
use crate::local;
use crate::wire::{Ask, Say, read_line, write_line};

/// Takes the socket and answers on it until the listener dies.
///
/// A second daemon LEAVES rather than taking the socket by force. The footer
/// does take it — it owns the screen and only one may draw — but here the one
/// already running is the one holding the jobs, and evicting it would strand
/// every copy in flight.
///
/// # Errors
///
/// When the socket cannot be bound.
pub fn listen_on(path: &std::path::Path, shared: &Arc<Shared>) -> Result<()> {
    if UnixStream::connect(path).is_ok() {
        tracing::info!(path = %path.display(), "there is already a queue listening; leaving");
        return Ok(());
    }
    // An orphan socket of a dead process prevents the bind, and after the test
    // above what is left here belongs to a process that no longer answers.
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)
        .with_context(|| format!("open the socket at {}", path.display()))?;
    tracing::info!(path = %path.display(), "transfer queue opened");

    for connection in listener.incoming() {
        let Ok(stream) = connection else { continue };
        let mine = Arc::clone(shared);
        // A NAMED thread: an unnamed one says `ronoctua-transfers` in `top` and
        // nothing about what it is doing, which is the question one opens `top`
        // to answer.
        if let Err(e) = std::thread::Builder::new()
            .name("client".into())
            .spawn(move || serve(&mine, stream))
        {
            tracing::error!(error = %e, "could not serve the client");
        }
    }

    // Only reached if the listener itself died. Stopping the work is right:
    // with no socket, nothing can ever pause or cancel it again.
    shared.stop_everything();
    Ok(())
}

/// One client, for as long as it stays.
fn serve(shared: &Arc<Shared>, stream: UnixStream) {
    let Ok(twin) = stream.try_clone() else {
        return;
    };
    let mut input = BufReader::new(twin);
    let mut out = BufWriter::new(stream);
    loop {
        match read_line::<Ask>(&mut input) {
            Ok(Some(ask)) => {
                if answer(shared, &ask, &mut out).is_err() {
                    return;
                }
                // A watch never comes back: the connection IS the subscription.
                if matches!(ask, Ask::Watch) {
                    return;
                }
            }
            Ok(None) => return,
            // A line we could not read is the client's problem, not the
            // queue's. Saying so and dropping the connection beats taking the
            // daemon down over somebody's typo in `socat`.
            Err(e) => {
                tracing::warn!(error = %e, "a request I did not understand");
                let _ = write_line(
                    &mut out,
                    &Say::Refused {
                        why: format!("{e:#}"),
                    },
                );
                return;
            }
        }
    }
}

/// Carries out one ask.
fn answer(shared: &Arc<Shared>, ask: &Ask, out: &mut impl Write) -> Result<()> {
    match ask {
        Ask::List => {
            let (jobs, _) = shared.snapshot();
            write_line(out, &Say::Ledger { jobs })
        }
        Ask::Watch => stream_it(shared, out),
        Ask::Add { work } => {
            // THE ENGINE COMES OUT OF THE WORK, and the request carries none.
            // With an engine on the wire a caller could ask for a magnet to be
            // pasted, and this function would have to decide which half of the
            // request to believe.
            let engine = work.engine();
            let (id, switch) = shared.take(engine, work.clone());
            let mine = Arc::clone(shared);
            let work = work.clone();
            // ONE THREAD PER JOB, and named after the work so `top` answers
            // "what is my machine copying" without a debugger.
            if let Err(e) = std::thread::Builder::new()
                .name(format!("job-{}", work.word()))
                .spawn(move || match engine {
                    Engine::Local => local::run(&mine, id, &switch, work),
                    Engine::Fetch => crate::aria2::run(&mine, id, &switch, work),
                    Engine::Media => crate::media::run(&mine, id, &switch, work),
                    Engine::Torrent => crate::torrent::run(&mine, id, &switch, work),
                    Engine::Atlas => crate::atlas::run(&mine, id, &switch, work),
                })
            {
                tracing::error!(error = %e, "could not start the job's thread");
                shared.touch(id, |j| {
                    j.stage = Stage::Failed;
                    j.trouble = Some(e.to_string());
                });
            }
            write_line(out, &Say::Took { id })
        }
        Ask::Pause { id } => {
            flip(shared, *id, Flow::Pause, Stage::Paused);
            Ok(())
        }
        Ask::Resume { id } => {
            flip(shared, *id, Flow::Run, Stage::Running);
            Ok(())
        }
        // NO STAGE HERE, on purpose. What ends a job is the thread finding out
        // it was stopped and unwinding — which for a copy means removing what
        // it had written. Writing `Cancelled` from this side would show the row
        // as over while the file was still being taken back.
        Ask::Cancel { id } => {
            if let Some(switch) = shared.switch(*id) {
                switch.set(Flow::Stop);
            }
            Ok(())
        }
        Ask::Forget { id } => {
            shared.forget(*id);
            Ok(())
        }
        Ask::Sweep => {
            let gone = shared.sweep();
            tracing::debug!(gone, "swept the list");
            Ok(())
        }
    }
}

/// Sets the switch and the row together.
fn flip(shared: &Arc<Shared>, id: u64, flow: Flow, stage: Stage) {
    let Some(switch) = shared.switch(id) else {
        return;
    };
    switch.set(flow);
    // Only what is still afoot: pausing something that ended a minute ago would
    // put a held row on screen that nothing is holding.
    shared.touch(id, |j| {
        if j.afoot() {
            j.stage = stage;
        }
    });
}

/// The whole list now, and again on every change, until the client goes.
fn stream_it(shared: &Arc<Shared>, out: &mut impl Write) -> Result<()> {
    // THE FIRST ONE IS NOT A CHANGE. A client that has drawn nothing has to be
    // correct from its first frame; without this it would show an empty panel
    // until somebody happened to start a job.
    let (jobs, mut seen) = shared.snapshot();
    write_line(out, &Say::Ledger { jobs })?;
    loop {
        let (jobs, now) = shared.wait_past(seen);
        write_line(out, &Say::Ledger { jobs })?;
        seen = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Work;
    use crate::wire::Paths;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-serve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    /// A daemon on a socket of its own, and the path to reach it.
    fn a_daemon(root: &Path) -> (Arc<Shared>, PathBuf) {
        let shared = Arc::new(Shared::fresh_one());
        let socket = root.join("queue.sock");
        let mine = Arc::clone(&shared);
        let its = socket.clone();
        std::thread::spawn(move || {
            let _ = listen_on(&its, &mine);
        });
        // Wait for the bind rather than sleeping a guessed amount: a fixed
        // sleep is either slow or flaky, and on a loaded machine it is both.
        let until = Instant::now() + Duration::from_secs(5);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < until, "the queue did not come up");
            std::thread::sleep(Duration::from_millis(5));
        }
        (shared, socket)
    }

    fn jobs_now(socket: &Path) -> Vec<crate::job::Job> {
        match crate::client::ask_at(socket, &Ask::List).unwrap() {
            Say::Ledger { jobs } => jobs,
            other => panic!("{other:?}"),
        }
    }

    /// Waits for a job to reach a stage, or gives up saying what it saw.
    ///
    /// THIRTY SECONDS, and the number is about the MACHINE and not the code:
    /// these tests run inside `nix build`, beside a VM and a dozen `rustc`s,
    /// and a daemon thread that gets no processor for ten seconds is not a
    /// daemon that ignored the ask. It failed exactly that way once, saying
    /// `expected Paused and saw Some(Running)`.
    fn until(socket: &Path, id: u64, stage: Stage) -> crate::job::Job {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let jobs = jobs_now(socket);
            let job = jobs.iter().find(|j| j.id == id).cloned();
            if let Some(j) = &job
                && j.stage == stage
            {
                return job.unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "expected {stage:?} and saw {:?}",
                job.map(|j| j.stage)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_name_that_is_not_utf8_crosses_the_wire_and_comes_back_the_same() {
        // THE DEFECT THIS CATCHES: a `PathBuf` serialised as a string. On Linux
        // a filename is a bag of bytes, and `serde_json` REFUSES one that is
        // not UTF-8 — so a file that came off a Windows disk would be
        // uncopyable, and it would be refused at the socket, three processes
        // away from the person who asked.
        use std::os::unix::ffi::OsStrExt as _;
        use std::os::unix::ffi::OsStringExt as _;

        let root = scratch("bytes");
        let (_shared, socket) = a_daemon(&root);

        // `0xFF` is not valid UTF-8 anywhere, in any encoding.
        let odd = root.join(std::ffi::OsString::from_vec(b"name-\xFF-crooked".to_vec()));
        std::fs::write(&odd, b"x").unwrap();

        let Say::Took { id } = crate::client::ask_at(
            &socket,
            &Ask::Add {
                work: Work::Delete {
                    paths: vec![odd.clone()],
                },
            },
        )
        .unwrap() else {
            panic!("the queue did not take the job");
        };

        let job = until(&socket, id, Stage::Done);
        // The bytes came back identical — not lossily replaced by `U+FFFD`,
        // which is what a `to_string_lossy` round trip would have produced.
        match job.work.subjects() {
            Paths::Some(p) => assert_eq!(p[0].as_os_str().as_bytes(), odd.as_os_str().as_bytes()),
            other => panic!("{other:?}"),
        }
        assert!(!odd.exists(), "the crooked-named file stayed");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pausing_holds_and_cancelling_takes_the_piece_back() {
        // The whole promise of the panel, end to end and over the real socket:
        // a copy can be held, let go, and given up — and giving up leaves
        // NOTHING in the destination. Half a file left behind turns "I pasted
        // 40 GiB into the wrong folder" into the same problem with a smaller
        // number.
        let root = scratch("hold");
        let (_shared, socket) = a_daemon(&root);

        // Big enough that the copy is still going when the asks arrive: three
        // bites, and the queue reports between each.
        let size = ronoctua_files::watch::CHUNK * 3;
        std::fs::write(root.join("large.bin"), vec![7u8; size]).unwrap();
        std::fs::create_dir(root.join("into")).unwrap();

        let Say::Took { id } = crate::client::ask_at(
            &socket,
            &Ask::Add {
                work: Work::Paste {
                    paths: vec![root.join("large.bin")],
                    into: root.join("into"),
                    moves: false,
                },
            },
        )
        .unwrap() else {
            panic!("the queue did not take the job");
        };

        crate::client::tell_at(&socket, &Ask::Pause { id }).unwrap();
        let held = until(&socket, id, Stage::Paused);
        // A pause that is really a stop would show `Cancelled` here, and a
        // pause that did nothing would have raced past `Paused` to `Done`.
        assert!(held.stage == Stage::Paused, "{held:?}");

        crate::client::tell_at(&socket, &Ask::Cancel { id }).unwrap();
        until(&socket, id, Stage::Cancelled);

        assert_eq!(
            std::fs::read_dir(root.join("into")).unwrap().count(),
            0,
            "a piece was left at the destination after the cancellation"
        );
        assert!(root.join("large.bin").exists(), "the original was touched");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn measuring_reports_the_size_as_an_answer_and_not_as_progress() {
        // A folder that turned out to be 40 GiB is not a job that is 0% of the
        // way through 40 GiB. The size is a RESULT; the progress is entries
        // walked.
        let root = scratch("measure");
        let (_shared, socket) = a_daemon(&root);
        std::fs::create_dir(root.join("folder")).unwrap();
        std::fs::write(root.join("folder/a"), vec![0u8; 1000]).unwrap();
        std::fs::write(root.join("folder/b"), vec![0u8; 24]).unwrap();

        let Say::Took { id } = crate::client::ask_at(
            &socket,
            &Ask::Add {
                work: Work::Measure {
                    path: root.join("folder"),
                },
            },
        )
        .unwrap() else {
            panic!("the queue did not take the job");
        };

        let job = until(&socket, id, Stage::Done);
        assert_eq!(job.answer, Some(1024), "the answer is not the size");
        assert_eq!(job.measure, crate::job::Measure::Items);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_watcher_is_correct_from_its_first_frame() {
        // A client that has drawn nothing has to be right immediately.
        // Without the first send, the panel would stay empty until somebody
        // happened to start a job — and "the panel is broken" and "nothing is
        // running" look identical on screen.
        let root = scratch("watch");
        let (shared, socket) = a_daemon(&root);
        shared.take(Engine::Local, Work::Measure { path: root.clone() });

        let stream = UnixStream::connect(&socket).unwrap();
        let mut out = BufWriter::new(stream.try_clone().unwrap());
        write_line(&mut out, &Ask::Watch).unwrap();
        let mut input = BufReader::new(stream);
        let first: Say = read_line(&mut input)
            .unwrap()
            .expect("nothing in the first frame");
        match first {
            Say::Ledger { jobs } => assert_eq!(jobs.len(), 1, "{jobs:?}"),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
