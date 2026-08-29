//! The brain's own process — and the doors that lead into it.
//!
//! # Three shapes, one binary
//!
//! - no argument: **the unit**. It listens on the socket, owns the mind, and
//!   runs the one thread that decides.
//! - `look`: prints the mind as JSON and leaves. It is what a person types when
//!   they want to know what the machine is planning, and what a check reads.
//! - `run <task>` / `rehearse <task>`: asks for something now. It adds and
//!   leaves; it does not wait for the rebuild.
//!
//! One binary because all three need the same model and the same socket, and
//! three would be three closures of the same size in the ISO.
//!
//! # Two threads, and only one of them may block
//!
//! The runner blocks — a rebuild is an hour — and it blocks its own thread. The
//! socket is answered on a thread per client, each reading the state through
//! the lock, so a screen asking what is happening is answered while the rebuild
//! is happening.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context as _, Result};
use ronoctua_brain::{
    Registry, Shared, client, mind, moment, prefs, pulse, runner, serve, tasks, wire,
};

fn main() -> Result<()> {
    // The unit's log goes to the journal; the short-lived doors are quiet
    // unless something is wrong, because whoever runs `look` wants the JSON on
    // stdout and nothing else on it.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_brain=info".into()),
        )
        .init();

    match std::env::args().nth(1).as_deref() {
        None => unit(),
        Some("look") => look(),
        Some("run") => ask_for(false),
        Some("rehearse") => ask_for(true),
        Some(other) => {
            anyhow::bail!(
                "I do not know how to do `{other}`; use `look`, `run <task>`, `rehearse <task>`, or nothing"
            )
        }
    }
}

/// The daemon: the socket, and the one thread that decides.
fn unit() -> Result<()> {
    let store = prefs::Where::of_env();
    let shared = Arc::new(Shared::of(store.prefs()));

    // THE MACHINE'S ZONE, read once. `ronoctua-files`'s reader says so itself
    // when it cannot read the file, and what it answers then is UTC — which is
    // a schedule that runs at the wrong hour rather than one that does not run.
    let zone: Arc<dyn moment::Offset> = Arc::new(ronoctua_files::when::Zone::read_from(
        std::path::Path::new("/etc/localtime"),
    ));
    let registry = Registry::of(tasks::tasks(Arc::clone(&zone)))
        // A malformed or repeated id is a PROGRAMMING error, and this is the
        // one moment somebody is looking. Starting anyway would put a bracketed
        // key on a screen instead.
        .context("the brain's tasks are badly declared")?;
    let desk = Arc::new(serve::Desk {
        shared: Arc::clone(&shared),
        tasks: Arc::new(registry),
        store: store.clone(),
    });

    // The runner gets a registry of its own: `Registry` holds boxed tasks, and
    // two owners of one set of trait objects would need a lock nothing else
    // needs. Building it twice costs a `vec![]`.
    let mut runner = runner::Runner::new(
        Arc::clone(&shared),
        Registry::of(tasks::tasks(zone)).context("the brain's tasks are badly declared")?,
        Box::new(pulse::Machine::here()),
        Box::new(moment::Wall),
        store,
    );
    // ONE PASS BEFORE THE SOCKET OPENS. The rows of the mind are built by the
    // runner, so a client that connected in the first milliseconds would get an
    // answer with no tasks in it — and "the brain knows nothing" and "the brain
    // has not looked yet" are the same picture on screen.
    runner.pump();

    std::thread::Builder::new()
        .name("decide".into())
        .spawn(move || {
            // Never stops: the daemon ends when the socket does, and the
            // process goes with it.
            runner.serve(&AtomicBool::new(false));
        })
        .context("start the thread that decides")?;

    serve::listen_on(&wire::socket_path()?, &desk)
}

/// Prints what the brain is thinking, as JSON, and leaves.
fn look() -> Result<()> {
    match client::ask(&wire::Ask::Look)? {
        wire::Say::Mind { mind } => {
            let text = serde_json::to_string_pretty(&*mind).context("codificar a mente")?;
            println!("{text}");
            Ok(())
        }
        wire::Say::Refused { why } => anyhow::bail!("{why}"),
    }
}

/// Asks for a task now, and does not wait for it.
fn ask_for(rehearsal: bool) -> Result<()> {
    let task = std::env::args()
        .nth(2)
        .context("uso: ronoctua-brain run <tarefa>")?;
    match client::ask(&wire::Ask::Run {
        task: task.clone(),
        rehearsal,
    })? {
        wire::Say::Mind { mind } => {
            report(&mind, &task);
            Ok(())
        }
        wire::Say::Refused { why } => anyhow::bail!("{why}"),
    }
}

/// One line saying where the task now stands.
///
/// A WORD and not a sentence built here: this binary has no bundle either, and
/// the person reading a terminal is the one case where the id itself is the
/// most useful thing to print.
fn report(m: &mind::Mind, task: &str) {
    let standing = m
        .task(task)
        .map_or("?", |t| mind::Standing::word(t.standing));
    println!("{task}: {standing}");
}
