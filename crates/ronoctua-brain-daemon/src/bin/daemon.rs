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
//! - `modules`: prints the id of every module the build compiled from its
//!   folder, one per line — what the sealed check compares with the folders
//!   Nix sees. `modules --catalogue` prints what each one's manifest DECLARES
//!   (`ronoctua_brain::catalogue::Declared`), as JSON — what a check compares
//!   with the catalogue the footer reads at run time.
//! - `work <id> [--report <path>] [args…]`: the work of a long unit, done by
//!   the module itself (`Module::work`), in this binary.
//!
//! One binary because they all need the same model and the same socket, and
//! several would be several closures of the same size in the ISO.
//!
//! # Three threads, and only one of them may block
//!
//! The runner blocks — a rebuild is an hour — and it blocks its own thread.
//! The watch thread hears the journal, paths and children for the modules
//! that listen, and never waits for the runner. The socket is answered on a
//! thread per client, each reading the state through the lock, so a screen
//! asking what is happening is answered while the rebuild is happening. All
//! of them hold ONE core: one registry, one journal.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context as _, Result};
// THE DAEMON'S CRATE, by its own name: this binary is called `ronoctua-brain`
// (the unit, `rust.brain` and every module's `ExecStart` name it), so inside
// it `ronoctua_brain::` would be the LIBRARY — what crosses the socket — and
// the core is `ronoctua_brain_daemon`.
use ronoctua_brain_daemon::{
    Birth, Core, Registry, Shared, client, compiled, mind, moment, prefs, pulse, registry, runner,
    serve, watch, wire,
};

fn main() -> Result<()> {
    // The unit's log goes to the journal; the short-lived doors are quiet
    // unless something is wrong, because whoever runs `look` wants the JSON on
    // stdout and nothing else on it.
    //
    // AND EVERY LINE IT PRINTS IS ALSO KEPT (`tail::Copier`), under the same
    // filter — the ring the brain's page follows is a copy of this output and
    // nothing else.
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_new(ronoctua_brain_daemon::tail::directives(
                std::env::var("RUST_LOG").ok().as_deref(),
            ))
            .unwrap_or_else(|_| "ronoctua_brain=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(ronoctua_brain_daemon::tail::Copier)
        .init();

    match std::env::args().nth(1).as_deref() {
        None => unit(),
        Some("look") => look(),
        Some("run") => ask_for(false),
        Some("rehearse") => ask_for(true),
        Some("modules") => modules(),
        Some("work") => work(),
        Some(other) => {
            anyhow::bail!(
                "I do not know how to do `{other}`; use `look`, `run <task>`, `rehearse <task>`, \
                 `modules`, `work <id>`, or nothing"
            )
        }
    }
}

/// The daemon: the socket, the one thread that decides and the one that
/// listens — over ONE core.
fn unit() -> Result<()> {
    let store = prefs::Where::of_env();
    let shared = Arc::new(Shared::of(store.prefs()));

    // THE MACHINE'S ZONE, read once. `ronoctua-files`'s reader says so itself
    // when it cannot read the file, and what it answers then is UTC — which is
    // a schedule that runs at the wrong hour rather than one that does not run.
    let zone: Arc<dyn moment::Offset> = Arc::new(ronoctua_files::when::Zone::read_from(
        std::path::Path::new("/etc/localtime"),
    ));
    let birth = Birth {
        zone,
        roots: store.clone(),
    };
    // ONE REGISTRY, for the runner, the watch thread and the socket alike: it
    // was built twice, and a module with state would have had two of it.
    let modules = Registry::of(ronoctua_brain_daemon::every_module(&birth))
        // A malformed or repeated id is a PROGRAMMING error, and this is the
        // one moment somebody is looking. Starting anyway would put a
        // bracketed key on a screen instead.
        .context("the brain's modules are badly declared")?
        // AND THE ONES THE FLAKE TURNED OFF, which stay on the page refusing
        // with the reason. Absent (the nested loop, a test) is nothing off:
        // the unit always sets it, empty when nothing is.
        .switched_off(&registry::off_of(
            &std::env::var(registry::OFF_VARIABLE).unwrap_or_default(),
        ));
    let core = Arc::new(Core::new(shared, modules, store, Box::new(moment::Wall)));
    let mut runner = runner::Runner::new(Arc::clone(&core), Box::new(pulse::Machine::here()));
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

    // THE ONE THAT LISTENS, and only when some module does.
    let listening = watch::listeners(&core);
    let _watching = watch::Watch::new(Arc::clone(&core))
        .start()
        .context("start the thread that listens")?;
    if !listening.is_empty() {
        tracing::info!(modules = ?listening, "listening");
    }

    serve::listen_on(&wire::socket_path()?, &core)
}

/// Prints the id of every module the build compiled from its folder, one per
/// line — or, with `--catalogue`, what each declares, as JSON.
///
/// MADE and not only listed: each is built with a birth that reads and writes
/// nowhere, so a folder whose module answered with another's manifest prints
/// the other's id — and the check that compares this with Nix sees it.
///
/// # `--catalogue`
///
/// The footer draws a module from the brain's catalogue, data the build
/// writes out of the same folders (`nix/brain-catalogue.nix`) and the footer
/// reads at run time; this daemon draws nothing, and acts from what it
/// COMPILED. What the two must agree on — every module's marks, notices,
/// actions, events and reasons, in the catalogue's own words
/// (`catalogue::Declared`) — is printed here, of the modules it made, for
/// `brain-catalogue-is-what-the-daemon-compiled` to compare with the file.
fn modules() -> Result<()> {
    let made = compiled::all(&Birth::detached());
    match std::env::args().nth(2).as_deref() {
        None => {
            for m in &made {
                println!("{}", m.manifest().id);
            }
        }
        Some("--catalogue") => {
            let declared: Vec<ronoctua_brain::catalogue::Declared> = made
                .iter()
                .map(|m| ronoctua_brain::catalogue::Declared::of(m.manifest()))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&declared)
                    .context("write what the modules declare")?
            );
        }
        Some(other) => anyhow::bail!("usage: ronoctua-brain modules [--catalogue] (not `{other}`)"),
    }
    Ok(())
}

/// `work <id> [--report <path>] [args…]`: the work of a long unit, done by the
/// module in this binary.
///
/// The report is where the runner watches — `<reports>/<id>.json` — unless
/// the unit says otherwise; a rehearsal unit passes `--report` with its own.
fn work() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(2).collect();
    if args.is_empty() {
        anyhow::bail!("usage: ronoctua-brain work <id> [--report <path>] [args…]");
    }
    let id = args.remove(0);
    let report = match args.iter().position(|a| a == "--report") {
        Some(at) => {
            let Some(path) = args.get(at + 1).cloned() else {
                anyhow::bail!("`--report` needs a path");
            };
            args.drain(at..=at + 1);
            std::path::PathBuf::from(path)
        }
        None => runner::report_in(&runner::reports_dir(), &id, false),
    };
    let store = prefs::Where::of_env();
    let birth = Birth {
        zone: Arc::new(ronoctua_files::when::Zone::read_from(std::path::Path::new(
            "/etc/localtime",
        ))),
        roots: store,
    };
    let modules = Registry::of(ronoctua_brain_daemon::every_module(&birth))
        .context("the brain's modules are badly declared")?;
    let module = modules.find(&id).with_context(|| {
        let known: Vec<&str> = modules.ids().collect();
        format!("there is no module `{id}`; there are: {}", known.join(", "))
    })?;
    module.work(&args, &report)
}

/// Prints what the brain is thinking, as JSON, and leaves.
fn look() -> Result<()> {
    match client::ask(&wire::Ask::Look)? {
        wire::Say::Mind { mind } => {
            let text = serde_json::to_string_pretty(&*mind).context("encode the mind")?;
            println!("{text}");
            Ok(())
        }
        wire::Say::Refused { why } => anyhow::bail!("{}", refusal(&why)),
        wire::Say::Lines { .. } => anyhow::bail!("the brain answered a look with its log"),
        wire::Say::Screen { .. } => anyhow::bail!("the brain answered a look with a screen"),
        wire::Say::Declined { key } => anyhow::bail!("the brain declined a look: {key}"),
    }
}

/// Asks for a task now, and does not wait for it.
fn ask_for(rehearsal: bool) -> Result<()> {
    let task = std::env::args()
        .nth(2)
        .context("usage: ronoctua-brain run <task>")?;
    match client::ask(&wire::Ask::Run {
        task: task.clone(),
        rehearsal,
    })? {
        wire::Say::Mind { mind } => {
            report(&mind, &task);
            Ok(())
        }
        wire::Say::Refused { why } => anyhow::bail!("{}", refusal(&why)),
        wire::Say::Lines { .. } => anyhow::bail!("the brain answered a run with its log"),
        wire::Say::Screen { .. } => anyhow::bail!("the brain answered a run with a screen"),
        wire::Say::Declined { key } => anyhow::bail!("the brain declined the run: {key}"),
    }
}

/// A refusal as the terminal says it.
///
/// The brain refuses in two shapes: its own sentence, when the ask made no
/// sense to it, and a TEXT KEY when a module declined or the system switched
/// the module off (`brain-why-system-off`) — which a door translates. This
/// binary has no bundle (see [`report`]), so a key is said as what it is,
/// inside a sentence, rather than printed bare as if it were one.
fn refusal(why: &str) -> String {
    if why.contains(char::is_whitespace) {
        why.to_owned()
    } else {
        format!("the brain refused it, for the reason its screens call `{why}`")
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

#[cfg(test)]
mod tests {
    /// A key is said inside a sentence; a sentence is said as it came.
    ///
    /// THE DEFECT THIS CATCHES: `ronoctua-brain rehearse <id>` on a module
    /// the system switched off printed `Error: brain-why-system-off` — a key,
    /// bare, where every other refusal on the socket is a sentence.
    #[test]
    fn a_refusal_key_is_said_inside_a_sentence() {
        let said = super::refusal("brain-why-system-off");
        assert!(said.contains("`brain-why-system-off`") && said.contains(' '));
        assert_eq!(
            super::refusal("`x` has no action called `y`"),
            "`x` has no action called `y`"
        );
    }
}
