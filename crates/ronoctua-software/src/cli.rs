//! The command line: what the queue and the update run call.
//!
//! # Who calls it, and how
//!
//! * **The queue** (`ronoctua-transfers`, the software engine) runs
//!   `ronoctua-software run --progress` inside a scope of its own and hands it
//!   ONE [`Order`] as JSON on stdin. What comes back is [`crate::progress`]'s
//!   lines on stdout, and the exit: 0 when the order was done, 1 when not.
//! * **The update run** calls `upgrade <source>` and `tidy <source>`, and reads
//!   plain sentences — the same orders, the same lanes, another reporter.
//! * **A web app's `.desktop`** runs `web-open <slug>`, which becomes Chrome.
//! * **A person** can run `list`, `search`, `details` and `help` in a terminal.
//!
//! # The one place that reads the machine
//!
//! Everything else in the crate takes a [`Context`] as a parameter; here is
//! where the real one is built ([`Context::of_session`]), once, and the uid
//! looked at. Parsing the arguments is a pure function ([`parse`]), and so is
//! everything that decides — which module runs an order ([`route`]), what the
//! journal says ([`name_of`], [`key_of`]), what an upgrade of everything
//! reports ([`summed`]) — so a test reads them without running anything.
//!
//! No program is named here: what runs comes out of [`crate::Tools`], through
//! the modules and [`crate::run`].

use std::io::{Read as _, Write as _};
use std::process::ExitCode;

use crate::details::Details;
use crate::found::{Batch, Status};
use crate::order::{Order, Scope};
use crate::progress::{Beat, Phase, Printed, Report};
use crate::run::{Call, tool};
use crate::state::{Snapshot, Supports};
use crate::{AppKey, Context, Done, Failure, Outcome, Refusal, Source};

/// What `help` prints, and what a wrong command line is answered with.
pub const USAGE: &str = "\
usage: ronoctua-software <command> [arguments]

The person's own programs: Nix profile, Flatpak (--user), AppImages, Android
apps and web apps. Every change is made for this user only.

commands:
  run [--progress]          run ONE order, read as JSON on standard input.
                            With --progress, print the queue's progress lines
                            (RONOCTUA<TAB>...) instead of sentences.
  upgrade <source>          bring one source's programs up to date:
                            nix, flatpak, appimage, android — or
                            android-system (the Android images; root only).
                            android asks F-Droid's signed index again first
                            and moves only the apps Android still lists;
                            while Android runs it replaces nothing and ends
                            `refused android-in-use`, and with f-droid.org
                            not let in the consent it ends `refused
                            fdroid-not-let`.
  tidy <source>             free what nothing uses any more: nix, flatpak,
                            appimage.
  web-open <slug>           open a web app (what its launcher entry runs).
  trial-exec <record>       become a trial's program (what its unit runs).
  list [--json]             what this person has installed.
  search <query> [--source <source>]... [--json]
                            search every source (or only the named ones).
  details <source> <reference> [--json]
                            what one program's sheet shows: nix <attribute>,
                            flatpak <app id>, appimage <key>,
                            android <package> or web <slug>.
  reconcile [<source>]      make this machine match the person's declaration:
                            Android's declared apps in (while Android runs)
                            and out, and the copies installed by hand that a
                            declared one replaces, once it is applied.
  pins refresh              look for newer downloads of the declared AppImages
                            and Android apps, and write them into the
                            person's declaration (nothing is staged); with
                            distro-apps-follow-releases on in their options,
                            the distro's AppImages that follow a repository
                            too, into software/distro-pins.json — and with it
                            off, that file goes. Nothing is followed on a
                            machine that trusts no signing key of theirs.
                            Ends `refused github-not-let` when GitHub was not
                            let and none of their own pins moved.
  person stage [--check]    lay the person's declaration where root's applier
                            reads it, and start the applier; --check only
                            reads it, as root would.
  help                      this text.

Every command but `upgrade android-system` refuses to run as root.
Exit status: 0 done, 1 failed or refused, 2 a wrong command line.
";

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verb {
    /// `run [--progress]`: one order, as JSON on standard input.
    Run {
        /// Print progress lines for the queue, not sentences.
        progress: bool,
    },
    /// `upgrade <target>`.
    Upgrade(Target),
    /// `tidy <source>`.
    Tidy(Source),
    /// `web-open <slug>`.
    WebOpen {
        /// The web app.
        slug: String,
    },
    /// `trial-exec <record>`: what a trial's unit runs — it becomes the
    /// program the record names (`crate::trial::exec`).
    TrialExec {
        /// The trial's `trial.json`.
        record: std::path::PathBuf,
    },
    /// `list [--json]`.
    List {
        /// Machine-readable.
        json: bool,
    },
    /// `search <query> [--source <s>]... [--json]`.
    Search {
        /// What to look for (the words, joined by one space).
        query: String,
        /// Where; empty means every source that can be searched.
        sources: Vec<Source>,
        /// One JSON object per source's answer.
        json: bool,
    },
    /// `details <source> <reference> [--json]`.
    Details {
        /// The program.
        key: AppKey,
        /// One JSON object.
        json: bool,
    },
    /// `reconcile [<source>]`.
    Reconcile(Option<Source>),
    /// `pins refresh`.
    PinsRefresh,
    /// `person stage [--check]`.
    PersonStage {
        /// Only read the declaration, as root would; stage nothing.
        check: bool,
    },
    /// `help`.
    Help,
}

/// What `upgrade` moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The person's programs of one source.
    Source(Source),
    /// Android's SYSTEM images (`waydroid upgrade`), which live in
    /// `/var/lib/waydroid` and are the one thing here that runs as root.
    AndroidSystem,
}

/// The sources an upgrade reaches, in the order an upgrade of everything takes
/// them. A web app has nothing to upgrade: it is a launcher entry and a site.
pub const UPGRADABLE: [Source; 4] = [
    Source::Nix,
    Source::Flatpak,
    Source::AppImage,
    Source::Android,
];

/// The sources a tidy reaches — the ones that leave something behind.
pub const TIDIED: [Source; 3] = [Source::Nix, Source::Flatpak, Source::AppImage];

/// The command line into a [`Verb`], or the sentence that says what is wrong.
///
/// # Errors
///
/// An English sentence naming the argument that is not understood.
pub fn parse(args: &[String]) -> Result<Verb, String> {
    let Some((command, rest)) = args.split_first() else {
        return Err("no command given".to_owned());
    };
    match command.as_str() {
        "help" | "--help" | "-h" => none_left(rest, Verb::Help),
        "run" => match rest {
            [] => Ok(Verb::Run { progress: false }),
            [flag] if flag == "--progress" => Ok(Verb::Run { progress: true }),
            _ => Err(format!(
                "`run` takes only --progress, not `{}`",
                rest.join(" ")
            )),
        },
        "upgrade" => match rest {
            [target] if target == "android-system" => Ok(Verb::Upgrade(Target::AndroidSystem)),
            [target] => {
                one_of(target, &UPGRADABLE, "upgrade").map(|s| Verb::Upgrade(Target::Source(s)))
            }
            _ => Err(
                "`upgrade` takes one source: nix, flatpak, appimage, android or android-system"
                    .to_owned(),
            ),
        },
        "tidy" => match rest {
            [source] => one_of(source, &TIDIED, "tidy").map(Verb::Tidy),
            _ => Err("`tidy` takes one source: nix, flatpak or appimage".to_owned()),
        },
        "web-open" => match rest {
            [slug] if !slug.starts_with('-') && !slug.is_empty() => {
                Ok(Verb::WebOpen { slug: slug.clone() })
            }
            _ => Err("`web-open` takes one web app's slug".to_owned()),
        },
        "trial-exec" => match rest {
            [record] if !record.starts_with('-') && !record.is_empty() => Ok(Verb::TrialExec {
                record: std::path::PathBuf::from(record),
            }),
            _ => Err("`trial-exec` takes one trial record".to_owned()),
        },
        "list" => match rest {
            [] => Ok(Verb::List { json: false }),
            [flag] if flag == "--json" => Ok(Verb::List { json: true }),
            _ => Err(format!(
                "`list` takes only --json, not `{}`",
                rest.join(" ")
            )),
        },
        "search" => search_of(rest),
        "details" => details_of(rest),
        "reconcile" => match rest {
            [] => Ok(Verb::Reconcile(None)),
            [source] => one_of(source, &Source::ALL, "reconcile").map(|s| Verb::Reconcile(Some(s))),
            _ => Err("`reconcile` takes one source at most".to_owned()),
        },
        "pins" => match rest {
            [word] if word == "refresh" => Ok(Verb::PinsRefresh),
            _ => Err("`pins` takes one word: refresh".to_owned()),
        },
        "person" => match rest {
            [word] if word == "stage" => Ok(Verb::PersonStage { check: false }),
            [word, flag] if word == "stage" && flag == "--check" => {
                Ok(Verb::PersonStage { check: true })
            }
            _ => Err("`person` takes `stage`, and --check after it".to_owned()),
        },
        other => Err(format!("`{other}` is not a command")),
    }
}

/// `help` takes nothing after it.
fn none_left(rest: &[String], verb: Verb) -> Result<Verb, String> {
    if rest.is_empty() {
        Ok(verb)
    } else {
        Err(format!("nothing goes after it, not `{}`", rest.join(" ")))
    }
}

/// A source's word, and only one of `allowed`.
fn one_of(word: &str, allowed: &[Source], command: &str) -> Result<Source, String> {
    Source::of_word(word)
        .filter(|s| allowed.contains(s))
        .ok_or_else(|| {
            let names: Vec<&str> = allowed.iter().map(|s| s.word()).collect();
            format!(
                "`{command}` does not know the source `{word}`; it knows {}",
                names.join(", ")
            )
        })
}

/// `search`'s arguments: words, `--source <s>` (or `--source=<s>`) any number
/// of times, `--json`, and `--` before a query that starts with a dash.
fn search_of(rest: &[String]) -> Result<Verb, String> {
    let mut words: Vec<&str> = Vec::new();
    let mut sources = Vec::new();
    let mut json = false;
    let mut given = rest.iter();
    while let Some(arg) = given.next() {
        if let Some(word) = arg.strip_prefix("--source=") {
            sources.push(one_of(word, &Source::ALL, "search")?);
            continue;
        }
        match arg.as_str() {
            "--" => {
                words.extend(given.by_ref().map(String::as_str));
                break;
            }
            "--json" => json = true,
            "--source" => {
                let word = given.next().ok_or("`--source` needs a source after it")?;
                sources.push(one_of(word, &Source::ALL, "search")?);
            }
            flag if flag.starts_with('-') => {
                return Err(format!("`search` does not know `{flag}`"));
            }
            word => words.push(word),
        }
    }
    let query = words.join(" ");
    if query.trim().is_empty() {
        return Err("`search` needs something to look for".to_owned());
    }
    // In the screen's order, once each: a source named twice is asked once.
    sources.sort_unstable();
    sources.dedup();
    Ok(Verb::Search {
        query,
        sources,
        json,
    })
}

/// `details`'s arguments: a source, a reference, and `--json` anywhere; `--`
/// before a reference that starts with a dash.
fn details_of(rest: &[String]) -> Result<Verb, String> {
    let mut words: Vec<&str> = Vec::new();
    let mut json = false;
    let mut given = rest.iter();
    while let Some(arg) = given.next() {
        match arg.as_str() {
            "--" => {
                words.extend(given.by_ref().map(String::as_str));
                break;
            }
            "--json" => json = true,
            flag if flag.starts_with('-') => {
                return Err(format!("`details` does not know `{flag}`"));
            }
            word => words.push(word),
        }
    }
    let [source, reference] = words.as_slice() else {
        return Err("`details` takes a source and a reference: `details nix ripgrep`".to_owned());
    };
    let source = one_of(source, &Source::ALL, "details")?;
    if reference.trim().is_empty() {
        return Err("`details` needs a reference".to_owned());
    }
    Ok(Verb::Details {
        key: key_in(source, reference),
        json,
    })
}

/// The key a reference names in a source.
#[must_use]
pub fn key_in(source: Source, reference: &str) -> AppKey {
    let id = reference.to_owned();
    match source {
        Source::Nix => AppKey::Nix { attr: id },
        Source::Flatpak => AppKey::Flatpak { id },
        Source::AppImage => AppKey::AppImage { key: id },
        Source::Android => AppKey::Android { package: id },
        Source::Web => AppKey::Web { slug: id },
    }
}

/// Who may run a verb: `upgrade android-system` is root's and nobody else's;
/// every other one is the PERSON's and never root's — a `nix profile` or a
/// `flatpak --user` run as root would install into root's home, report
/// success, and leave the person with nothing.
///
/// # Errors
///
/// [`Refusal::WrongUser`].
pub const fn root_rule(verb: &Verb, root: bool) -> Result<(), Refusal> {
    let wants_root = matches!(verb, Verb::Upgrade(Target::AndroidSystem));
    match verb {
        Verb::Help => Ok(()),
        _ if wants_root == root => Ok(()),
        _ => Err(Refusal::WrongUser),
    }
}

/// Which module runs an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The module of one source.
    Source(Source),
    /// A trial (`crate::trial`), which is Nix's lane but not Nix's module.
    Trial,
    /// An upgrade of everything, one source's lane at a time.
    Everything,
    /// One of the person's declaration's orders (`crate::declare`), which
    /// takes the lane of its source itself, around what touches the source
    /// — never across a rebuild.
    Declaration(Option<Source>),
}

impl Route {
    /// The lane it takes: [`Order::lane`], by construction — a test holds the
    /// two to the same answer for every order.
    #[must_use]
    pub const fn lane(self) -> Option<Source> {
        match self {
            Self::Source(s) => Some(s),
            Self::Trial => Some(Source::Nix),
            Self::Everything => None,
            Self::Declaration(s) => s,
        }
    }
}

/// Which module runs an order, or why none does.
///
/// # Errors
///
/// [`Refusal::DeclaredInTheFlake`] for anything asked of a program the flake
/// installed; [`Refusal::UnknownItem`] for a file that is none of the three
/// kinds this crate installs.
pub fn route(order: &Order) -> Result<Route, Refusal> {
    match order {
        // THE DECLARATION'S ORDERS: what the flake installed is no source's,
        // and is declined or kept by the registry's id, never declared.
        // ── anchor: L18 ──
        Order::Declare { key, .. } | Order::Forget { key, .. } => key
            .source()
            .map(|s| Route::Declaration(Some(s)))
            .ok_or(Refusal::DeclaredInTheFlake),
        Order::Decline { .. }
        | Order::Undecline { .. }
        | Order::Reconcile { .. }
        | Order::RefreshPins => Ok(Route::Declaration(order.lane())),
        Order::Try { .. } | Order::StopTrial { .. } | Order::Keep { .. } => Ok(Route::Trial),
        Order::Upgrade {
            scope: Scope::Everything,
        } => Ok(Route::Everything),
        Order::Install { key, .. }
        | Order::Remove { key, .. }
        | Order::Upgrade {
            scope: Scope::One { key },
        } => key
            .source()
            .map(Route::Source)
            .ok_or(Refusal::DeclaredInTheFlake),
        Order::InstallFile { path, .. } => crate::order::file_source(path)
            .map(Route::Source)
            .ok_or(Refusal::UnknownItem),
        other => other.lane().map(Route::Source).ok_or(Refusal::UnknownItem),
    }
}

/// The program an order is about, for the journal.
#[must_use]
pub fn key_of(order: &Order) -> Option<AppKey> {
    match order {
        Order::Install { key, .. }
        | Order::Remove { key, .. }
        | Order::Declare { key, .. }
        | Order::Forget { key, .. }
        | Order::Upgrade {
            scope: Scope::One { key },
        } => Some(key.clone()),
        Order::Try { attr, .. } => Some(AppKey::Nix { attr: attr.clone() }),
        Order::WebEdit { slug, .. } => Some(AppKey::Web { slug: slug.clone() }),
        Order::Permission { id, .. } => Some(AppKey::Flatpak { id: id.clone() }),
        _ => None,
    }
}

/// The name to show for an order (data, never translated): the program's id,
/// a web app's name, a file's name, or the source a source-wide order is about.
#[must_use]
pub fn name_of(order: &Order) -> String {
    match order {
        Order::Install { key, .. }
        | Order::Remove { key, .. }
        | Order::Declare { key, .. }
        | Order::Forget { key, .. }
        | Order::Upgrade {
            scope: Scope::One { key },
        } => key.id().to_owned(),
        Order::Decline { id, .. } | Order::Undecline { id, .. } => id.clone(),
        Order::Reconcile {
            source: Some(source),
        } => source.word().to_owned(),
        Order::InstallFile { path, .. } => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        Order::Upgrade {
            scope: Scope::Source { source },
        }
        | Order::Tidy { source }
        | Order::Prepare { source, .. }
        | Order::Refresh { source } => source.word().to_owned(),
        Order::Try { attr, .. } => attr.clone(),
        Order::StopTrial { trial } | Order::Keep { trial } => trial.clone(),
        Order::Rollback { generation } => generation.to_string(),
        Order::WebCreate { spec } | Order::WebEdit { spec, .. } => spec.name.clone(),
        Order::Permission { id, .. } => id.clone(),
        Order::Upgrade {
            scope: Scope::Everything,
        }
        | Order::AndroidSession { .. }
        | Order::AndroidErase
        | Order::AndroidWindows { .. }
        | Order::Reconcile { source: None }
        | Order::RefreshPins => String::new(),
    }
}

/// Runs one order to its answer: the lane, the module, and the journal line.
/// The final beat is the CALLER's ([`answer`]), so an upgrade of everything
/// can run several of these under one.
///
/// Runs programs and may reach the network: call it on a thread (the CLI is
/// that thread).
pub fn perform(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    let done = route(order)
        .map_err(Failure::refused)
        .and_then(|r| run_routed(ctx, r, order, report));
    write_down(ctx, order, &done);
    done
}

/// The journal line of an order that ran.
fn write_down(ctx: &Context, order: &Order, done: &Done) {
    // A REHEARSAL LEAVES NO HISTORY: nothing was done, and a line saying
    // "installed" would be the history lying.
    if !ctx.rehearse {
        let entry = crate::journal::Entry {
            when: now(),
            order: order.word().to_owned(),
            key: done
                .as_ref()
                .ok()
                .and_then(|o| o.key.clone())
                .or_else(|| key_of(order)),
            name: name_of(order),
            ok: done.is_ok(),
            refusal: done.as_ref().err().and_then(|f| f.refusal),
        };
        if let Err(e) = crate::journal::add(&ctx.roots.ours_state(), &entry) {
            tracing::warn!(error = %e, "could not write the history");
        }
    }
}

/// One routed order, under its lane.
fn run_routed(ctx: &Context, route: Route, order: &Order, report: &mut dyn Report) -> Done {
    if let Route::Declaration(_) = route {
        return crate::declare::run(ctx, order, report);
    }
    // A DECLARED WEB APP CHANGED is declared again: an edit of the person's
    // entry alone would be taken away by the reconciler and never reach the
    // declaration. It takes the web lane itself, and then the declaration's.
    if let Order::WebEdit { slug, spec } = order
        && crate::declaration::read_declared(&ctx.roots)
            .is_ok_and(|d| d.entry(&format!("{}:{slug}", Source::Web.word())).is_some())
    {
        return crate::declare::edit_declared_web(ctx, slug, spec, report);
    }
    let Some(source) = route.lane() else {
        return everything(ctx, report);
    };
    let done = {
        let _lane = lane(ctx, source, &name_of(order), report)?;
        module(ctx, route, order, report)
    };
    // ANDROID STARTED: what the person declared for it goes in now — the
    // reconcile takes the lane this order just let go of.
    if matches!(order, Order::AndroidSession { on: true }) && done.is_ok() {
        let _ = crate::reconcile::run(ctx, Some(Source::Android), report)
            .map_err(|e| tracing::warn!(error = %e, "the declared Android apps wait"));
    }
    done
}

/// The module's own `run`.
pub(crate) fn module(ctx: &Context, route: Route, order: &Order, report: &mut dyn Report) -> Done {
    match route {
        Route::Trial => crate::trial::run(ctx, order, report),
        Route::Source(Source::Nix) => crate::nix::run(ctx, order, report),
        Route::Source(Source::Flatpak) => crate::flatpak::run(ctx, order, report),
        Route::Source(Source::AppImage) => crate::appimage::run(ctx, order, report),
        Route::Source(Source::Android) => crate::android::run(ctx, order, report),
        Route::Source(Source::Web) => crate::web::run(ctx, order, report),
        Route::Everything => everything(ctx, report),
        Route::Declaration(_) => crate::declare::run(ctx, order, report),
    }
}

/// Takes a source's lane, saying so when it has to wait for it.
fn lane(
    ctx: &Context,
    source: Source,
    name: &str,
    report: &mut dyn Report,
) -> Result<crate::lock::Lane, Failure> {
    crate::lock::take(&crate::lock::directory(&ctx.roots), source, || {
        report.step(Phase::Waiting, 0, 0, name)
    })
    .map_err(|e| Failure::trouble(format!("could not take the {} lane: {e}", source.word())))
}

/// An upgrade of every source this machine has, one lane at a time. A failure
/// in one does not stop the rest: the answer says which failed and why.
fn everything(ctx: &Context, report: &mut dyn Report) -> Done {
    let supports = Supports::of(&ctx.tools);
    let mut outcomes = Vec::new();
    let mut failed = Vec::new();
    for source in UPGRADABLE.into_iter().filter(|s| supports.has(*s)) {
        let order = Order::Upgrade {
            scope: Scope::Source { source },
        };
        let done = match lane(ctx, source, source.word(), report) {
            Ok(_lane) => module(ctx, Route::Source(source), &order, report),
            Err(f) => Err(f),
        };
        match done {
            Ok(o) => outcomes.push(o),
            Err(f) if nothing_to_upgrade(&f) => {}
            Err(f) => failed.push((source, f)),
        }
    }
    summed(outcomes, failed)
}

/// A source that was never PREPARED — Android with no image, Flatpak with no
/// Flathub remote — has nothing installed, so an upgrade of everything has
/// nothing to do there. Counting it as a failure would make every upgrade of a
/// machine that never used Android end in red.
#[must_use]
pub fn nothing_to_upgrade(failure: &Failure) -> bool {
    failure.refusal == Some(Refusal::SourceUnprepared)
}

/// The answer of an upgrade of everything, out of each source's.
///
/// Done when every source was; otherwise a failure whose trouble has one line
/// per source that failed (`flatpak: …`), and whose refusal is the one they
/// ALL share — a sentence on screen for "you are offline" when that is what
/// happened everywhere, and the lines alone when the reasons differ.
///
/// # Errors
///
/// When any source failed.
pub fn summed(outcomes: Vec<Outcome>, failed: Vec<(Source, Failure)>) -> Done {
    if failed.is_empty() {
        let mut all = Outcome::default();
        for o in outcomes {
            all.desktop_ids.extend(o.desktop_ids);
            all.amount = match (all.amount, o.amount) {
                (Some(a), Some(b)) => Some(a.saturating_add(b)),
                (a, b) => a.or(b),
            };
            all.rehearsed |= o.rehearsed;
        }
        return Ok(all);
    }
    let first = failed[0].1.refusal;
    let shared = if failed.iter().all(|(_, f)| f.refusal == first) {
        first
    } else {
        None
    };
    let lines: Vec<String> = failed
        .iter()
        .map(|(source, f)| match (shared, f.trouble.is_empty()) {
            // The sentence says the reason; the line says where.
            (Some(r), true) => format!("{}: {}", source.word(), r.word()),
            (Some(_), false) => format!("{}: {}", source.word(), f.trouble),
            (None, _) => format!("{}: {f}", source.word()),
        })
        .collect();
    Err(Failure {
        refusal: shared,
        trouble: lines.join("\n"),
    })
}

/// The final beat, and the exit status that goes with it.
pub fn answer(report: &mut dyn Report, done: Done) -> ExitCode {
    match done {
        Ok(outcome) => {
            report.beat(Beat::Done(outcome));
            ExitCode::SUCCESS
        }
        Err(failure) => {
            report.beat(Beat::Failed(failure));
            ExitCode::FAILURE
        }
    }
}

/// Seconds since the epoch, for the journal.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The order on standard input. An order is a few hundred bytes; the ceiling
/// is what keeps a wrong pipe from being read into memory whole.
///
/// # Errors
///
/// When it cannot be read or does not parse.
pub fn read_order(input: &mut dyn std::io::Read) -> Result<Order, Failure> {
    let mut bytes = Vec::new();
    input
        .take(ORDER_MOST)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            Failure::trouble(format!("could not read the order on standard input: {e}"))
        })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Failure::trouble(format!("the order on standard input does not parse: {e}")))
}

/// The most an order may weigh on standard input.
pub const ORDER_MOST: u64 = 1 << 20;

/// Beats as sentences, for a person's terminal and the update report.
///
/// A step is printed when its phase or its name CHANGES, never per byte: the
/// update report is a log, and a line per mebibyte would bury what matters.
#[derive(Debug, Default)]
pub struct Plain {
    last: Option<(Phase, String)>,
}

impl Report for Plain {
    fn beat(&mut self, beat: Beat) {
        if let Beat::Step { phase, now, .. } = &beat {
            let here = Some((*phase, now.clone()));
            if self.last == here {
                return;
            }
            self.last = here;
        }
        let line = plain_line(&beat);
        if matches!(beat, Beat::Failed(_)) {
            let _ = writeln!(std::io::stderr(), "{line}");
        } else {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
    }
}

/// One beat as a sentence (English, invariant 18).
#[must_use]
pub fn plain_line(beat: &Beat) -> String {
    match beat {
        Beat::Step {
            phase,
            done,
            total,
            now,
        } => {
            let mut line = phase.word().to_owned();
            if !now.is_empty() {
                line.push_str(": ");
                line.push_str(now);
            }
            if *total > 0 {
                line.push_str(&format!(" ({done} of {total})"));
            }
            line
        }
        Beat::Done(o) => {
            let mut line = if o.rehearsed {
                "rehearsed, nothing changed".to_owned()
            } else {
                "done".to_owned()
            };
            if let Some(amount) = o.amount {
                line.push_str(&format!(" ({amount})"));
            }
            line
        }
        Beat::Failed(f) => format!("failed: {f}"),
    }
}

/// The argument vector of the Android images' upgrade, program aside.
#[must_use]
pub fn images_argv() -> Vec<String> {
    vec!["upgrade".to_owned()]
}

/// Android's system images, as root: `waydroid upgrade`, and only when there
/// is an image to upgrade and no session using it.
///
/// Not an [`Order`]: the queue never runs it (the queue is the person's, and
/// this is root's), and it takes no lane — lanes live in the person's runtime
/// directory, which root's run does not share.
fn android_images(ctx: &Context, report: &mut dyn Report) -> Done {
    let waydroid = tool(ctx.tools.waydroid.as_ref())?;
    let android = crate::android::status(ctx);
    if !android.prepared {
        println!("Android has not been prepared: there are no images to upgrade");
        return Ok(Outcome::default());
    }
    // A RUNNING SESSION KEEPS ITS IMAGES: they are mounted, and the next run
    // of the update takes them — a wait, not a failure.
    if android.session {
        println!("an Android session is running: the images wait for the next run");
        return Ok(Outcome::default());
    }
    report.step(Phase::Downloading, 0, 0, "android");
    let ran = Call::new(waydroid, images_argv())
        .changing()
        .run(ctx, &mut |line| println!("{line}"))?
        .ok()?;
    Ok(Outcome {
        rehearsed: ran.rehearsed,
        ..Outcome::default()
    })
}

/// What the person has, as sentences: one line per program.
#[must_use]
pub fn plain_list(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    for i in &snapshot.installed {
        let source = i.key.source().map_or("system", Source::word);
        out.push_str(&format!("{source}  {}  {}", i.key.id(), i.name));
        if !i.version.is_empty() {
            out.push_str(&format!("  {}", i.version));
        }
        if let Some(update) = &i.update {
            out.push_str(&format!("  (update: {update})"));
        }
        out.push('\n');
    }
    for t in &snapshot.trials {
        let state = if t.running { "running" } else { "ended" };
        out.push_str(&format!("trial  {}  {}  {state}\n", t.attr, t.name));
    }
    out
}

/// One source's answer to a search, as sentences.
#[must_use]
pub fn plain_batch(batch: &Batch) -> String {
    let status = match &batch.status {
        Status::Answered { from: Some(from) } => format!("answered, from {from}"),
        Status::Answered { from: None } => "answered".to_owned(),
        Status::Asking => "asking".to_owned(),
        Status::Refused { refusal } => format!("refused: {}", refusal.word()),
        Status::Off => "off".to_owned(),
    };
    let mut out = format!(
        "{}: {status}, {} found\n",
        batch.source.word(),
        batch.found.len()
    );
    for f in &batch.found {
        out.push_str(&format!("  {}  {}", f.key.id(), f.name));
        if !f.version.is_empty() {
            out.push_str(&format!("  {}", f.version));
        }
        if !f.summary.is_empty() {
            out.push_str(&format!("  — {}", f.summary));
        }
        if !f.badges.is_empty() {
            let words: Vec<&str> = f.badges.iter().map(|b| b.word()).collect();
            out.push_str(&format!("  [{}]", words.join(", ")));
        }
        out.push('\n');
    }
    out
}

/// The CLI's entry.
#[must_use]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verb = match parse(&args) {
        Ok(verb) => verb,
        Err(why) => {
            let _ = writeln!(std::io::stderr(), "ronoctua-software: {why}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if verb == Verb::Help {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    // THE QUEUE READS A REFUSAL AS A LINE, and everybody else as a sentence:
    // `run --progress` answers in beats even before it has an order.
    let mut report: Box<dyn Report> = if verb == (Verb::Run { progress: true }) {
        Box::new(Printed)
    } else {
        Box::new(Plain::default())
    };
    if let Err(refusal) = root_rule(&verb, rustix::process::getuid().is_root()) {
        return answer(report.as_mut(), Err(Failure::refused(refusal)));
    }
    let ctx = match Context::of_session() {
        Ok(ctx) => ctx,
        Err(e) => return answer(report.as_mut(), Err(e.into())),
    };
    match verb {
        Verb::Run { .. } => {
            let done = read_order(&mut std::io::stdin().lock())
                .and_then(|order| perform(&ctx, &order, report.as_mut()));
            answer(report.as_mut(), done)
        }
        Verb::Upgrade(Target::Source(source)) => {
            let order = Order::Upgrade {
                scope: Scope::Source { source },
            };
            if source == Source::Android {
                return android_upgrade(&ctx, &order, report.as_mut());
            }
            let done = perform(&ctx, &order, report.as_mut());
            answer(report.as_mut(), done)
        }
        Verb::Upgrade(Target::AndroidSystem) => {
            let done = android_images(&ctx, report.as_mut());
            answer(report.as_mut(), done)
        }
        Verb::Tidy(source) => {
            let done = perform(&ctx, &Order::Tidy { source }, report.as_mut());
            answer(report.as_mut(), done)
        }
        Verb::WebOpen { slug } => web_open(&ctx, &slug),
        // NO LANE AND NO PROGRESS LINES: this is the trial's own process, inside
        // its unit, becoming the program. It comes back only when it could not.
        Verb::TrialExec { record } => {
            let failure = crate::trial::exec(&record);
            let _ = writeln!(std::io::stderr(), "ronoctua-software: {failure}");
            ExitCode::FAILURE
        }
        Verb::List { json } => list(&ctx, json),
        Verb::Search {
            query,
            sources,
            json,
        } => search(&ctx, &query, &sources, json),
        Verb::Details { key, json } => details(&ctx, &key, json),
        Verb::Reconcile(source) => {
            let done = perform(&ctx, &Order::Reconcile { source }, report.as_mut());
            answer(report.as_mut(), done)
        }
        Verb::PinsRefresh => pins_refresh(&ctx, report.as_mut()),
        Verb::PersonStage { check } => person_stage(&ctx, check),
        Verb::Help => ExitCode::SUCCESS,
    }
}

/// `pins refresh`: newer pins into the declaration, and nothing staged —
/// the update's `person-stage` comes after it. And the distro's AppImages
/// the person follows past its pins (`crate::follow`): followed while their
/// options say so, forgotten when they say not, left alone when the options
/// do not read here. A host following asks that is not let ends the run
/// `refused github-not-let` — news, which the update's step says — unless
/// the person's own pins moved: the run then says what moved, and a line
/// says the distro's were not looked at.
fn pins_refresh(ctx: &Context, report: &mut dyn Report) -> ExitCode {
    let host = crate::declaration::host_of(&crate::declaration::this_host());
    let mut not_let = false;
    let mut own_moved = 0;
    let done = host.and_then(|host| {
        // THE DOWNLOADS RUN WITH NO LANE HELD, and only the write takes it.
        let newer = crate::pins::newer(ctx, &crate::github::Net, &host, report)?;
        let following = crate::follow::following(&ctx.roots);
        let looked = if following == Some(true) {
            crate::follow::look(ctx, &crate::github::Net, report)
        } else {
            crate::follow::Looked::default()
        };
        not_let = looked.not_let;
        if looked.no_signer {
            println!(
                "this machine trusts no signing key of yours: the distro's apps that follow their releases were not looked at"
            );
        }
        let moved = if ctx.rehearse {
            (newer.len() + looked.changes.len()) as u64
        } else {
            let _held = crate::lock::take_declaration(&crate::lock::directory(&ctx.roots), || {
                report.step(Phase::Waiting, 0, 0, crate::lock::DECLARATION);
            })
            .map_err(|e| Failure::trouble(format!("could not take the declaration's lane: {e}")))?;
            let mut moved = crate::pins::take(&ctx.roots, &newer)?;
            own_moved = moved;
            match following {
                Some(true) => moved += crate::follow::apply(&ctx.roots, &looked.changes)?,
                Some(false) => moved += u64::from(crate::follow::forget(&ctx.roots)?),
                None => {}
            }
            moved
        };
        Ok(Outcome {
            amount: Some(moved),
            rehearsed: ctx.rehearse,
            ..Outcome::default()
        })
    });
    let went_through = done.is_ok();
    let code = answer(report, done);
    if went_through && not_let {
        // THE PERSON'S OWN PINS THAT MOVED are what the run did: said, and
        // the GitHub refusal a line beside them rather than the verdict.
        if own_moved > 0 {
            println!(
                "GitHub is not allowed in your consent: the distro's apps that follow their releases were not looked at"
            );
            return code;
        }
        println!("refused {}", crate::follow::NOT_LET);
        return ExitCode::FAILURE;
    }
    code
}

/// `person stage [--check]`: the declaration laid where root's applier reads
/// it AND applied by it — the applier's unit started, as the software
/// centre starts it — or, with `--check`, only read. Laid and never
/// applied, the update's root half would verify and build what the machine
/// had.
fn person_stage(ctx: &Context, check: bool) -> ExitCode {
    let said = person_stage_said(ctx, check);
    if let Some(line) = &said.out {
        println!("{line}");
    }
    if let Some(line) = &said.err {
        let _ = writeln!(std::io::stderr(), "ronoctua-software: {line}");
    }
    if said.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// What a verb the update's runner reads says (`person stage`, `upgrade
/// android`): its last line on standard output, its line on standard error,
/// and whether it did what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Said {
    out: Option<String>,
    err: Option<String>,
    ok: bool,
}

/// [`person_stage`]'s answer. The applier's own answer is the last line:
/// `applied` or `unchanged`; a refusal ends `refused <word>`, which the
/// update's runner reads — `busy` when another of root's programs holds the
/// machine (the software centre's rebuild: the mirror keeps the
/// declaration, this update builds without it, and the step declares the
/// word, with its sentence), else the applier's reason (`TreeWhy`). Any
/// other failure says only why, on standard error.
fn person_stage_said(ctx: &Context, check: bool) -> Said {
    use crate::stage::Applied;
    use ronoctua_person::tree::TreeWhy;
    let refused = |word: Option<TreeWhy>, why: String| Said {
        out: word.map(|w| format!("refused {}", w.id())),
        err: Some(why),
        ok: false,
    };
    let said = |line: String| Said {
        out: Some(line),
        err: None,
        ok: true,
    };
    if check || ctx.rehearse {
        return match crate::stage::check(&ctx.roots) {
            Ok((declared, declined)) => {
                said(format!("reads: {declared} declared, {declined} declined"))
            }
            Err(f) => refused(None, f.to_string()),
        };
    }
    let handed = crate::lock::take_declaration(&crate::lock::directory(&ctx.roots), || {})
        .map_err(|e| Failure::trouble(format!("could not take the declaration's lane: {e}")))
        .and_then(|_held| crate::stage::hand_over(ctx, &mut Vec::new()));
    match handed {
        Ok(Applied::Applied) => said("applied".to_owned()),
        Ok(Applied::Unchanged) => said("unchanged".to_owned()),
        Ok(Applied::Busy) => refused(
            Some(TreeWhy::Busy),
            "root's applier is busy: another of root's programs holds the machine, and this \
             run builds the declarations it had"
                .to_owned(),
        ),
        Err(f) => refused(crate::stage::refused_why(&f), f.to_string()),
    }
}

/// `upgrade android`, as the update runs it: the Android apps moved, unless
/// the person is using Android. Its words go out the way `person stage`'s
/// do, after the upgrade's own final beat.
fn android_upgrade(ctx: &Context, order: &Order, report: &mut dyn Report) -> ExitCode {
    let said = android_upgrade_said(ctx, order, report);
    if let Some(line) = &said.err {
        let _ = writeln!(std::io::stderr(), "ronoctua-software: {line}");
    }
    if let Some(line) = &said.out {
        println!("{line}");
    }
    if said.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// [`android_upgrade`]'s answer.
///
/// THE UPDATE NEVER REPLACES AN APP UNDER THE PERSON: Android stops an app it
/// replaces, and a run nobody watches would close what somebody is using. So
/// while a session runs nothing moves, and the last line is `refused
/// android-in-use` — news the update's step declares, and the apps wait for
/// a run when Android is not running. Asked WHILE ANDROID'S LANE IS HELD: a
/// software view's order holding the lane may start Android and leave it
/// running, and a question asked before the wait would be stale after it.
/// The software view's own update comes through `run`, where the person
/// asked for it, and does not wait.
///
/// The upgrade itself is `perform`'s, minus the lane this already holds:
/// the module, and the journal line. A consent that does not let
/// f-droid.org ends `refused fdroid-not-let`, news too; offline and a broken
/// signature stay failures.
fn android_upgrade_said(ctx: &Context, order: &Order, report: &mut dyn Report) -> Said {
    android_upgrade_said_with(ctx, order, report, &|ctx, order, report| {
        module(ctx, Route::Source(Source::Android), order, report)
    })
}

/// [`android_upgrade_said`] over any `upgrade` — the module on the machine,
/// and in a test the module over the test's network and places, so all the
/// rest (the lane, the question, the journal line, the final beat, the
/// word) is the one the update runs.
fn android_upgrade_said_with(
    ctx: &Context,
    order: &Order,
    report: &mut dyn Report,
    upgrade: &dyn Fn(&Context, &Order, &mut dyn Report) -> Done,
) -> Said {
    let done = match lane(ctx, Source::Android, &name_of(order), report) {
        Ok(_held) => {
            if crate::android::running(ctx) {
                return Said {
                    out: Some(format!("refused {}", crate::android::IN_USE)),
                    err: Some(
                        "Android is running: its apps are not replaced while it is, and wait \
                         for a run when it is not"
                            .to_owned(),
                    ),
                    ok: false,
                };
            }
            upgrade(ctx, order, report)
        }
        Err(f) => Err(f),
    };
    write_down(ctx, order, &done);
    let news = android_news(&done);
    let ok = done.is_ok();
    answer(report, done);
    Said {
        out: news.map(|word| format!("refused {word}")),
        err: None,
        ok,
    }
}

/// The word an Android upgrade's answer ends with when it is news and not a
/// failure: f-droid.org not let in the person's consent, which is their
/// choice. Offline, a broken signature and the rest stay failures.
fn android_news(done: &Done) -> Option<&'static str> {
    matches!(done, Err(f) if f.refusal == Some(Refusal::NotLet)).then_some(crate::fdroid::NOT_LET)
}

/// Becomes Chrome, with the web app's window: `exec`, so the launcher's scope
/// holds the browser and not a parent waiting on it.
fn web_open(ctx: &Context, slug: &str) -> ExitCode {
    let argv = match crate::web::open_argv(ctx, slug) {
        Ok(argv) => argv,
        Err(refusal) => {
            let _ = writeln!(
                std::io::stderr(),
                "ronoctua-software: cannot open the web app `{slug}`: {}",
                refusal.word()
            );
            return ExitCode::FAILURE;
        }
    };
    let Some((program, args)) = argv.split_first() else {
        let _ = writeln!(
            std::io::stderr(),
            "ronoctua-software: cannot open the web app `{slug}`: no browser"
        );
        return ExitCode::FAILURE;
    };
    let e =
        std::os::unix::process::CommandExt::exec(std::process::Command::new(program).args(args));
    let _ = writeln!(
        std::io::stderr(),
        "ronoctua-software: could not start {program}: {e}"
    );
    ExitCode::FAILURE
}

/// `list`.
fn list(ctx: &Context, json: bool) -> ExitCode {
    let snapshot = crate::state::read(ctx);
    let text = if json {
        match serde_json::to_string_pretty(&snapshot) {
            Ok(text) => text + "\n",
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "ronoctua-software: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        plain_list(&snapshot)
    };
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    ExitCode::SUCCESS
}

/// `search`: every source answers on a thread of its own, and each answer is
/// printed WHOLE, under the lock, so two sources never interleave.
fn search(ctx: &Context, query: &str, sources: &[Source], json: bool) -> ExitCode {
    let everywhere: Vec<Source> = Source::ALL
        .into_iter()
        .filter(|s| *s != Source::Web)
        .collect();
    let sources = if sources.is_empty() {
        everywhere.as_slice()
    } else {
        sources
    };
    let sink = |batch: Batch| {
        let text = if json {
            serde_json::to_string(&batch)
                .map(|t| t + "\n")
                .unwrap_or_default()
        } else {
            plain_batch(&batch)
        };
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    };
    let language = language_of(|name| std::env::var(name).ok());
    crate::search::run(ctx, query, sources, &language, 1, &sink);
    ExitCode::SUCCESS
}

/// `details`: the sheet of one program, as sentences or one JSON object.
fn details(ctx: &Context, key: &AppKey, json: bool) -> ExitCode {
    let language = language_of(|name| std::env::var(name).ok());
    let d = match crate::details::of(ctx, key, &language) {
        Ok(d) => d,
        Err(refusal) => {
            let _ = writeln!(
                std::io::stderr(),
                "ronoctua-software: {} {}: {}",
                key.source().map_or("system", Source::word),
                key.id(),
                refusal.word()
            );
            return ExitCode::FAILURE;
        }
    };
    let text = if json {
        match serde_json::to_string_pretty(&d) {
            Ok(text) => text + "\n",
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "ronoctua-software: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        plain_details(&d)
    };
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    ExitCode::SUCCESS
}

/// A program's sheet as sentences: what it is, one fact a line, then its
/// description.
#[must_use]
pub fn plain_details(d: &Details) -> String {
    let mut out = format!("{}  ({})\n", d.name, d.reference);
    if !d.summary.is_empty() {
        out.push_str(&format!("{}\n", d.summary));
    }
    let mut fact = |label: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            out.push_str(&format!("{label}: {v}\n"));
        }
    };
    fact("version", Some(d.version.clone()));
    fact("licence", d.license.clone());
    fact("site", d.homepage.clone());
    fact("publisher", d.publisher.clone());
    fact("command", d.program.clone());
    fact("needs", d.needs.clone());
    fact("download", d.download_bytes.map(megabytes));
    fact("installed", d.installed_bytes.map(megabytes));
    fact(
        "badges",
        Some(
            d.badges
                .iter()
                .map(|b| b.word())
                .collect::<Vec<_>>()
                .join(", "),
        ),
    );
    fact("anti-features", Some(d.anti_features.join(", ")));
    if !d.permissions.is_empty() {
        out.push_str("permissions:\n");
        for p in &d.permissions {
            out.push_str(&format!("  {p}\n"));
        }
    }
    for paragraph in &d.description {
        out.push_str(&format!("\n{paragraph}\n"));
    }
    out
}

/// Bytes as megabytes with one decimal (`73.4 MB`), the unit a person reads a
/// download in.
fn megabytes(bytes: u64) -> String {
    let tenths = (bytes + 50_000) / 100_000;
    format!("{}.{} MB", tenths / 10, tenths % 10)
}

/// The language a terminal speaks, as a tag (`pt_BR.UTF-8` → `pt-BR`), from
/// the variables POSIX reads in that order; `C` and `POSIX` are English. The
/// screen passes its own language instead: this is only for the command line.
fn language_of(var: impl Fn(&str) -> Option<String>) -> String {
    let named = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(&var)
        .find(|value| !value.is_empty())
        .unwrap_or_default();
    let bare = named.split(['.', '@']).next().unwrap_or_default();
    if bare.is_empty() || bare == "C" || bare == "POSIX" {
        return "en".to_owned();
    }
    bare.replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{AndroidImage, Consent, IconChoice, Traces, WebSpec};
    use crate::{Roots, Tools};

    #[test]
    fn the_terminal_language_is_a_tag() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(language_of(env(&[("LANG", "pt_BR.UTF-8")])), "pt-BR");
        assert_eq!(
            language_of(env(&[("LANG", "pt_BR.UTF-8"), ("LC_ALL", "en_GB.UTF-8")])),
            "en-GB"
        );
        assert_eq!(
            language_of(env(&[("LC_MESSAGES", ""), ("LANG", "de_DE@euro")])),
            "de-DE"
        );
        assert_eq!(language_of(env(&[("LANG", "C.UTF-8")])), "en");
        assert_eq!(language_of(env(&[])), "en");
    }

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    fn spec() -> WebSpec {
        WebSpec {
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon: IconChoice::Site {
                url: "https://mail.example.org/icon.png".into(),
                page: None,
            },
            prepared: None,
            category: None,
        }
    }

    fn every_order() -> Vec<Order> {
        let nix = AppKey::Nix {
            attr: "hello".into(),
        };
        vec![
            Order::Install {
                key: nix.clone(),
                consent: Consent::default(),
            },
            Order::Install {
                key: AppKey::Flatpak {
                    id: "org.x.Y".into(),
                },
                consent: Consent::default(),
            },
            Order::InstallFile {
                path: "/d/Obsidian.AppImage".into(),
                consent: Consent::default(),
            },
            Order::InstallFile {
                path: "/d/fdroid.apk".into(),
                consent: Consent::default(),
            },
            Order::InstallFile {
                path: "/d/x.flatpakref".into(),
                consent: Consent::default(),
            },
            Order::Remove {
                key: nix,
                keep_data: true,
            },
            Order::Remove {
                key: AppKey::Web {
                    slug: "mail".into(),
                },
                keep_data: true,
            },
            Order::Upgrade {
                scope: Scope::One {
                    key: AppKey::Android {
                        package: "org.fdroid.fdroid".into(),
                    },
                },
            },
            Order::Upgrade {
                scope: Scope::Source {
                    source: Source::AppImage,
                },
            },
            Order::Upgrade {
                scope: Scope::Everything,
            },
            Order::Tidy {
                source: Source::Flatpak,
            },
            Order::Try {
                attr: "hello".into(),
                traces: Traces::Sandbox,
                network: false,
                consent: Consent::default(),
            },
            Order::StopTrial {
                trial: "hello".into(),
            },
            Order::Keep {
                trial: "hello".into(),
            },
            Order::Prepare {
                source: Source::Android,
                image: Some(AndroidImage::Gapps),
            },
            Order::Refresh {
                source: Source::Flatpak,
            },
            Order::Rollback { generation: 3 },
            Order::WebCreate { spec: spec() },
            Order::WebEdit {
                slug: "mail".into(),
                spec: spec(),
            },
            Order::AndroidSession { on: true },
            Order::AndroidErase,
            Order::Declare {
                key: AppKey::Flatpak {
                    id: "org.x.Y".into(),
                },
                consent: Consent::default(),
                machines: None,
            },
            Order::Forget {
                key: AppKey::Android {
                    package: "org.x".into(),
                },
                keep_data: true,
            },
            Order::Decline {
                id: "gimp".into(),
                machines: None,
            },
            Order::Undecline {
                id: "gimp".into(),
                everywhere: false,
            },
            Order::Reconcile {
                source: Some(Source::Android),
            },
            Order::Reconcile { source: None },
            Order::RefreshPins,
        ]
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-cli-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// FAILS IF: `person stage` does not end with the applier's own answer —
    /// `applied`, or `refused <word>` the update's runner reads: `busy`
    /// (another of root's programs holds the machine) as a failure of this
    /// run, and the applier's reason for a staging it refused — or calls
    /// anything done that was not.
    #[test]
    fn the_stage_says_what_the_applier_answered() {
        let verdict = |nonce: &str, outcome: &str, why: &str| {
            let refused = if why.is_empty() {
                "null".to_owned()
            } else {
                format!(
                    "{{\"why\":\"{why}\",\"at\":\"person/software/declined.json\",\"detail\":\"x\"}}"
                )
            };
            format!(
                "{{\"format\":1,\"at\":\"2099-01-01T00:00:00Z\",\"nonce\":{nonce},\"outcome\":\"{outcome}\",\"refused\":{refused},\"ignored\":[],\"left-out\":[]}}"
            )
        };
        for (tag, written, line, ok) in [
            (
                "applied",
                verdict("\"$(cat STAGED/nonce)\"", "applied", ""),
                Some("applied"),
                true,
            ),
            // BUSY is said before the staging is read: no nonce.
            (
                "busy",
                verdict("null", "refused", "busy"),
                Some("refused busy"),
                false,
            ),
            (
                "refused",
                verdict("\"$(cat STAGED/nonce)\"", "refused", "not-signed"),
                Some("refused not-signed"),
                false,
            ),
        ] {
            let root = scratch(&format!("stage-{tag}"));
            let roots = Roots::under(&root);
            let bin = root.join("bin");
            // SYSTEMCTL, as the applier's unit answers: its verdict, written
            // beside the staging.
            let written = written.replace("STAGED", &roots.system.staged().display().to_string());
            crate::write_executable(
                &bin.join("systemctl"),
                &format!(
                    "#!/bin/sh\nprintf '%s' \"{}\" > {}\n",
                    written.replace('"', "\\\""),
                    roots.system.verdict().display()
                ),
            );
            let ctx = Context {
                roots,
                ..context(
                    &root,
                    Tools {
                        systemctl: Some(bin.join("systemctl")),
                        ..Tools::default()
                    },
                )
            };
            let said = person_stage_said(&ctx, false);
            assert_eq!(said.out.as_deref(), line, "{tag}: {said:?}");
            assert_eq!(said.ok, ok, "{tag}: {said:?}");
            assert_eq!(said.err.is_some(), !ok, "{tag}: {said:?}");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// A prepared machine whose Android session is `state` (RUNNING or
    /// STOPPED), every call to its waydroid written down, and a record
    /// holding one app that came from `source` — with the places a test hands
    /// in for the machine's (`/var/lib/waydroid`).
    fn android_at(
        tag: &str,
        state: &str,
        source: crate::android::Came,
    ) -> (std::path::PathBuf, Context, crate::android::Places) {
        let root = scratch(tag);
        let waydroid = root.join("bin/waydroid");
        crate::write_executable(
            &waydroid,
            &format!(
                "#!/bin/sh\necho \"waydroid $*\" >> '{}'\ncase \"$1\" in\n\
                 status) printf 'Session:\\t{state}\\n';;\nesac\n",
                root.join("calls.log").display()
            ),
        );
        let ctx = context(
            &root,
            Tools {
                waydroid: Some(waydroid),
                ..Tools::default()
            },
        );
        let record = [(
            "org.example.notes".to_owned(),
            crate::android::Recorded {
                name: "Notes".into(),
                version_code: 3,
                version_name: "1.3".into(),
                sha256: String::new(),
                source,
            },
        )]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        std::fs::create_dir_all(ctx.roots.ours_state()).unwrap();
        std::fs::write(
            ctx.roots.ours_state().join(crate::android::RECORD),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        let places = crate::android::Places {
            waydroid: root.join("var-lib-waydroid"),
            android_id: root.join("android-id"),
            patience: std::time::Duration::from_millis(300),
            every: std::time::Duration::from_millis(20),
        };
        std::fs::create_dir_all(&places.waydroid).unwrap();
        std::fs::write(places.waydroid.join("waydroid.cfg"), "[waydroid]\n").unwrap();
        (root, ctx, places)
    }

    fn android_calls(root: &std::path::Path) -> String {
        std::fs::read_to_string(root.join("calls.log")).unwrap_or_default()
    }

    fn upgrade_android() -> Order {
        Order::Upgrade {
            scope: Scope::Source {
                source: Source::Android,
            },
        }
    }

    /// The update's `upgrade android` with the module over the test's
    /// network and places, never the machine's.
    fn android_upgrade_here(
        ctx: &Context,
        places: &crate::android::Places,
        net: &dyn crate::net::Fetch,
        beats: &mut Vec<Beat>,
    ) -> Said {
        android_upgrade_said_with(
            ctx,
            &upgrade_android(),
            beats,
            &|ctx: &Context, order: &Order, report: &mut dyn Report| {
                crate::android::run_in(ctx, order, report, net, places)
            },
        )
    }

    /// A network on which the person's consent lets no host: what
    /// `net::request` answers before a byte is sent (`net::pass_for`).
    #[derive(Default)]
    struct NotLetting {
        asked: std::sync::Mutex<Vec<String>>,
    }

    impl NotLetting {
        fn refuse(&self, url: &str) -> Failure {
            self.asked.lock().unwrap().push(url.to_owned());
            Failure {
                refusal: Some(Refusal::NotLet),
                trouble: format!("{url}: its host was not let in consent.json"),
            }
        }
    }

    impl crate::net::Fetch for NotLetting {
        fn get(&self, url: &str, _: &[(&str, &str)], _: u64) -> Result<crate::net::Got, Failure> {
            Err(self.refuse(url))
        }

        fn post_json(&self, url: &str, _: &str, _: u64) -> Result<crate::net::Got, Failure> {
            Err(self.refuse(url))
        }

        fn download(
            &self,
            url: &str,
            _: u64,
            _: &std::path::Path,
            _: &mut dyn FnMut(u64, Option<u64>) -> bool,
        ) -> Result<(), Failure> {
            Err(self.refuse(url))
        }
    }

    /// FAILS IF: the update's `upgrade android` goes ahead while the
    /// person's Android runs, does not end `refused android-in-use` (the
    /// word its step declares), calls that done, or writes a history of an
    /// upgrade that did not happen.
    #[test]
    fn the_update_replaces_no_android_app_while_android_runs() {
        let (root, ctx, places) =
            android_at("android-in-use", "RUNNING", crate::android::Came::Fdroid);
        let net = crate::net::canned::Canned::default();
        let mut beats: Vec<Beat> = Vec::new();
        let said = android_upgrade_here(&ctx, &places, &net, &mut beats);
        assert_eq!(said.out.as_deref(), Some("refused android-in-use"));
        assert!(!said.ok, "{said:?}");
        assert!(said.err.is_some(), "the person is told why: {said:?}");
        assert!(
            !beats
                .iter()
                .any(|b| matches!(b, Beat::Done(_) | Beat::Failed(_))),
            "no verdict of an upgrade that did not run: {beats:?}"
        );
        assert!(net.asked.borrow().is_empty(), "{:?}", net.asked.borrow());
        let calls = android_calls(&root);
        assert!(calls.contains("waydroid status"), "{calls}");
        assert!(!calls.contains("session"), "{calls}");
        assert!(crate::journal::read(&ctx.roots.ours_state()).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: with Android stopped, the update's `upgrade android` does
    /// not run the upgrade and write it down, or ends with a refusal word.
    #[test]
    fn a_stopped_android_is_upgraded_and_written_down() {
        let (root, ctx, places) =
            android_at("android-stopped", "STOPPED", crate::android::Came::File);
        let net = crate::net::canned::Canned::default();
        let mut beats: Vec<Beat> = Vec::new();
        let said = android_upgrade_here(&ctx, &places, &net, &mut beats);
        assert_eq!(
            said,
            Said {
                out: None,
                err: None,
                ok: true
            }
        );
        assert!(
            matches!(beats.last(), Some(Beat::Done(o)) if o.amount == Some(0)),
            "{beats:?}"
        );
        assert!(net.asked.borrow().is_empty(), "nothing from F-Droid to ask");
        let history = crate::journal::read(&ctx.roots.ours_state());
        assert_eq!(history.len(), 1);
        assert_eq!(
            (history[0].order.as_str(), history[0].ok),
            ("upgrade", true)
        );
        assert!(android_calls(&root).contains("waydroid status"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the update's `upgrade android`, with f-droid.org not let in
    /// the person's consent, does not end `refused fdroid-not-let` (the word
    /// its step declares as news) after the upgrade's final beat, calls it
    /// done, starts Android, or writes the history without the reason — or
    /// if an offline network is said as that news instead of a failure.
    #[test]
    fn a_consent_without_fdroid_ends_with_the_word_its_step_declares() {
        let (root, ctx, places) =
            android_at("android-not-let", "STOPPED", crate::android::Came::Fdroid);
        let net = NotLetting::default();
        let mut beats: Vec<Beat> = Vec::new();
        let said = android_upgrade_here(&ctx, &places, &net, &mut beats);
        assert_eq!(
            said,
            Said {
                out: Some("refused fdroid-not-let".to_owned()),
                err: None,
                ok: false
            }
        );
        assert!(
            matches!(beats.last(), Some(Beat::Failed(f)) if f.refusal == Some(Refusal::NotLet)),
            "the final beat is the upgrade's own: {beats:?}"
        );
        assert_eq!(*net.asked.lock().unwrap(), [crate::fdroid::ENTRY]);
        let calls = android_calls(&root);
        assert!(!calls.contains("session"), "{calls}");
        let history = crate::journal::read(&ctx.roots.ours_state());
        assert_eq!(history.len(), 1);
        assert_eq!(
            (history[0].ok, history[0].refusal),
            (false, Some(Refusal::NotLet))
        );
        // OFFLINE STAYS A FAILURE: no word, so the step is red.
        let offline = crate::net::canned::Canned::default();
        let mut beats: Vec<Beat> = Vec::new();
        let said = android_upgrade_here(&ctx, &places, &offline, &mut beats);
        assert_eq!(
            said,
            Said {
                out: None,
                err: None,
                ok: false
            }
        );
        assert!(
            matches!(beats.last(), Some(Beat::Failed(f)) if f.refusal == Some(Refusal::Offline)),
            "{beats:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a consent that does not let f-droid.org stops being news
    /// (`fdroid-not-let`), or a failure that is one — offline, a broken
    /// signature — is said as news.
    #[test]
    fn only_a_consent_taken_back_is_news_for_android() {
        let failed = |r: Refusal| -> Done { Err(Failure::refused(r)) };
        assert_eq!(
            android_news(&failed(Refusal::NotLet)),
            Some("fdroid-not-let")
        );
        assert_eq!(android_news(&failed(Refusal::Offline)), None);
        assert_eq!(android_news(&failed(Refusal::Integrity)), None);
        assert_eq!(android_news(&Ok(Outcome::default())), None);
    }

    fn context(root: &std::path::Path, tools: Tools) -> Context {
        Context {
            tools,
            roots: Roots::under(root),
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        }
    }

    #[test]
    fn every_command_is_read_and_a_wrong_one_is_named() {
        assert_eq!(
            parse(&args("run --progress")),
            Ok(Verb::Run { progress: true })
        );
        assert_eq!(parse(&args("run")), Ok(Verb::Run { progress: false }));
        assert_eq!(
            parse(&args("upgrade nix")),
            Ok(Verb::Upgrade(Target::Source(Source::Nix)))
        );
        assert_eq!(
            parse(&args("upgrade android")),
            Ok(Verb::Upgrade(Target::Source(Source::Android)))
        );
        assert_eq!(
            parse(&args("upgrade android-system")),
            Ok(Verb::Upgrade(Target::AndroidSystem))
        );
        assert_eq!(
            parse(&args("tidy appimage")),
            Ok(Verb::Tidy(Source::AppImage))
        );
        assert_eq!(
            parse(&args("web-open whatsapp")),
            Ok(Verb::WebOpen {
                slug: "whatsapp".into()
            })
        );
        assert_eq!(parse(&args("list --json")), Ok(Verb::List { json: true }));
        assert_eq!(parse(&args("help")), Ok(Verb::Help));
        assert_eq!(parse(&args("--help")), Ok(Verb::Help));

        // What is refused, and the refusal NAMES what it did not understand.
        assert!(parse(&[]).is_err());
        assert!(parse(&args("upgrade web")).unwrap_err().contains("`web`"));
        assert!(
            parse(&args("tidy android"))
                .unwrap_err()
                .contains("`android`")
        );
        assert!(parse(&args("upgrade")).is_err());
        assert!(parse(&args("upgrade nix flatpak")).is_err());
        assert!(parse(&args("run --quiet")).unwrap_err().contains("--quiet"));
        assert!(parse(&args("web-open")).is_err());
        assert!(
            parse(&args("web-open --kiosk")).is_err(),
            "a flag is not a slug"
        );
        assert!(
            parse(&args("install gimp"))
                .unwrap_err()
                .contains("`install`")
        );
        assert!(parse(&args("help me")).is_err());
        assert_eq!(parse(&args("reconcile")), Ok(Verb::Reconcile(None)));
        assert_eq!(
            parse(&args("reconcile android")),
            Ok(Verb::Reconcile(Some(Source::Android)))
        );
        assert!(
            parse(&args("reconcile pypi"))
                .unwrap_err()
                .contains("`pypi`")
        );
        assert_eq!(parse(&args("pins refresh")), Ok(Verb::PinsRefresh));
        assert!(parse(&args("pins")).is_err());
        assert_eq!(
            parse(&args("person stage")),
            Ok(Verb::PersonStage { check: false })
        );
        assert_eq!(
            parse(&args("person stage --check")),
            Ok(Verb::PersonStage { check: true })
        );
        assert!(parse(&args("person apply")).is_err());
    }

    #[test]
    fn a_search_takes_its_words_and_its_sources_anywhere() {
        assert_eq!(
            parse(&args(
                "search image editor --source nix --source=flatpak --json"
            )),
            Ok(Verb::Search {
                query: "image editor".into(),
                sources: vec![Source::Nix, Source::Flatpak],
                json: true
            })
        );
        assert_eq!(
            parse(&args("search -- --weird")),
            Ok(Verb::Search {
                query: "--weird".into(),
                sources: vec![],
                json: false
            })
        );
        assert!(parse(&args("search")).is_err(), "nothing to look for");
        assert!(parse(&args("search x --source")).is_err());
        assert!(
            parse(&args("search x --source pypi"))
                .unwrap_err()
                .contains("`pypi`")
        );
        assert!(
            parse(&args("search x --fast"))
                .unwrap_err()
                .contains("--fast")
        );
    }

    #[test]
    fn details_take_a_source_and_a_reference_and_name_what_is_wrong() {
        assert_eq!(
            parse(&args("details nix ripgrep")),
            Ok(Verb::Details {
                key: AppKey::Nix {
                    attr: "ripgrep".into()
                },
                json: false
            })
        );
        assert_eq!(
            parse(&args("details --json flatpak org.gnome.Calculator")),
            Ok(Verb::Details {
                key: AppKey::Flatpak {
                    id: "org.gnome.Calculator".into()
                },
                json: true
            })
        );
        assert_eq!(
            parse(&args("details android org.fdroid.fdroid --json")),
            Ok(Verb::Details {
                key: AppKey::Android {
                    package: "org.fdroid.fdroid".into()
                },
                json: true
            })
        );
        assert_eq!(
            parse(&args("details appimage obsidian")),
            Ok(Verb::Details {
                key: AppKey::AppImage {
                    key: "obsidian".into()
                },
                json: false
            })
        );
        assert_eq!(
            parse(&args("details web -- -mail")),
            Ok(Verb::Details {
                key: AppKey::Web {
                    slug: "-mail".into()
                },
                json: false
            }),
            "after `--` a dash is a reference"
        );
        for source in Source::ALL {
            let parsed = parse(&args(&format!("details {} x", source.word()))).unwrap();
            assert!(
                matches!(&parsed, Verb::Details { key, .. } if key.source() == Some(source) && key.id() == "x"),
                "{parsed:?}"
            );
        }
        assert!(parse(&args("details")).is_err());
        assert!(parse(&args("details nix")).is_err(), "no reference");
        assert!(parse(&args("details nix a b")).is_err(), "two references");
        assert!(
            parse(&args("details pypi requests"))
                .unwrap_err()
                .contains("`pypi`")
        );
        assert!(
            parse(&args("details nix hello --fast"))
                .unwrap_err()
                .contains("--fast")
        );
    }

    #[test]
    fn a_sheet_reads_as_lines_and_says_only_what_it_knows() {
        use crate::found::Badge;
        let d = Details {
            description: vec!["First.".into(), "Second.".into()],
            download_bytes: Some(73_415_160),
            permissions: vec!["share=network".into()],
            reference: "org.x.Y".into(),
            name: "Y".into(),
            summary: "Does y".into(),
            version: "1.0".into(),
            badges: vec![Badge::Sandboxed, Badge::Verified],
            ..Details::default()
        };
        assert_eq!(
            plain_details(&d),
            "Y  (org.x.Y)\nDoes y\nversion: 1.0\ndownload: 73.4 MB\n\
             badges: sandboxed, verified\n\
             permissions:\n  share=network\n\nFirst.\n\nSecond.\n"
        );
        assert_eq!(megabytes(0), "0.0 MB");
        assert_eq!(megabytes(49_999), "0.0 MB");
        assert_eq!(megabytes(50_000), "0.1 MB");
        assert_eq!(megabytes(1_125_792_768), "1125.8 MB");
    }

    #[test]
    fn only_the_android_images_run_as_root_and_nothing_else_does() {
        let person = [
            Verb::Run { progress: true },
            Verb::Upgrade(Target::Source(Source::Nix)),
            Verb::Tidy(Source::Flatpak),
            Verb::WebOpen { slug: "x".into() },
            Verb::List { json: true },
            Verb::Search {
                query: "x".into(),
                sources: vec![],
                json: false,
            },
            Verb::Details {
                key: AppKey::Nix { attr: "x".into() },
                json: false,
            },
        ];
        for verb in &person {
            assert_eq!(
                root_rule(verb, true),
                Err(Refusal::WrongUser),
                "{verb:?} ran as root"
            );
            assert_eq!(root_rule(verb, false), Ok(()), "{verb:?}");
        }
        for verb in [
            Verb::Reconcile(None),
            Verb::PinsRefresh,
            Verb::PersonStage { check: true },
        ] {
            assert_eq!(root_rule(&verb, true), Err(Refusal::WrongUser), "{verb:?}");
            assert_eq!(root_rule(&verb, false), Ok(()), "{verb:?}");
        }
        let images = Verb::Upgrade(Target::AndroidSystem);
        assert_eq!(root_rule(&images, true), Ok(()));
        assert_eq!(root_rule(&images, false), Err(Refusal::WrongUser));
        assert_eq!(root_rule(&Verb::Help, true), Ok(()));
    }

    #[test]
    fn every_order_goes_to_its_module_under_the_lane_it_declares() {
        for order in every_order() {
            let route =
                route(&order).unwrap_or_else(|r| panic!("{} was refused: {r:?}", order.word()));
            assert_eq!(
                route.lane(),
                order.lane(),
                "{} takes another lane than it declares",
                order.word()
            );
        }
        let trial = Order::Keep {
            trial: "hello".into(),
        };
        assert_eq!(route(&trial), Ok(Route::Trial));
        let file = Order::InstallFile {
            path: "/d/x.flatpakref".into(),
            consent: Consent::default(),
        };
        assert_eq!(route(&file), Ok(Route::Source(Source::Flatpak)));
        assert_eq!(
            route(&Order::Upgrade {
                scope: Scope::Everything
            }),
            Ok(Route::Everything)
        );
    }

    #[test]
    fn what_the_flake_installed_is_refused_and_so_is_a_file_of_no_kind() {
        let system = AppKey::System {
            desktop_id: "firefox.desktop".into(),
        };
        for order in [
            Order::Remove {
                key: system.clone(),
                keep_data: true,
            },
            Order::Install {
                key: system.clone(),
                consent: Consent::default(),
            },
            Order::Upgrade {
                scope: Scope::One { key: system },
            },
        ] {
            assert_eq!(
                route(&order),
                Err(Refusal::DeclaredInTheFlake),
                "{}",
                order.word()
            );
        }
        let notes = Order::InstallFile {
            path: "/d/notes.txt".into(),
            consent: Consent::default(),
        };
        assert_eq!(route(&notes), Err(Refusal::UnknownItem));
        // NOR IS IT DECLARED OR FORGOTTEN: the registry declines it by id.
        let flake = AppKey::System {
            desktop_id: "firefox.desktop".into(),
        };
        for order in [
            Order::Declare {
                key: flake.clone(),
                consent: Consent::default(),
                machines: None,
            },
            Order::Forget {
                key: flake,
                keep_data: true,
            },
        ] {
            assert_eq!(route(&order), Err(Refusal::DeclaredInTheFlake));
        }
    }

    #[test]
    fn the_journal_names_the_program_and_never_invents_one() {
        let install = Order::Install {
            key: AppKey::Flatpak {
                id: "org.x.Y".into(),
            },
            consent: Consent::default(),
        };
        assert_eq!(name_of(&install), "org.x.Y");
        assert_eq!(
            key_of(&install),
            Some(AppKey::Flatpak {
                id: "org.x.Y".into()
            })
        );
        let file = Order::InstallFile {
            path: "/d/Obsidian-1.6.AppImage".into(),
            consent: Consent::default(),
        };
        assert_eq!(name_of(&file), "Obsidian-1.6.AppImage");
        assert_eq!(
            key_of(&file),
            None,
            "the key is the outcome's, once the file is read"
        );
        assert_eq!(
            name_of(&Order::Tidy {
                source: Source::Nix
            }),
            "nix"
        );
        assert_eq!(name_of(&Order::WebCreate { spec: spec() }), "Mail");
        assert_eq!(name_of(&Order::Rollback { generation: 12 }), "12");
        assert_eq!(name_of(&Order::AndroidErase), "");
    }

    #[test]
    fn an_upgrade_of_everything_says_which_sources_failed_and_why() {
        let ok = Outcome {
            amount: Some(2),
            desktop_ids: vec!["a.desktop".into()],
            ..Outcome::default()
        };
        let also = Outcome {
            amount: Some(3),
            ..Outcome::default()
        };
        assert_eq!(
            summed(vec![ok.clone(), also], vec![]),
            Ok(Outcome {
                amount: Some(5),
                desktop_ids: vec!["a.desktop".into()],
                ..Outcome::default()
            })
        );
        // One reason everywhere: the sentence is that reason, the lines say where.
        let offline = |t: &str| Failure {
            refusal: Some(Refusal::Offline),
            trouble: t.into(),
        };
        let failure = summed(
            vec![ok],
            vec![
                (Source::Nix, offline("")),
                (Source::Flatpak, offline("no route")),
            ],
        )
        .unwrap_err();
        assert_eq!(failure.refusal, Some(Refusal::Offline));
        assert_eq!(failure.trouble, "nix: offline\nflatpak: no route");
        // Different reasons: no sentence would be true for all, so there is none.
        let mixed = summed(
            vec![],
            vec![
                (Source::Flatpak, offline("x")),
                (Source::Android, Failure::refused(Refusal::SessionDown)),
            ],
        )
        .unwrap_err();
        assert_eq!(mixed.refusal, None);
        assert_eq!(mixed.trouble, "flatpak: offline: x\nandroid: session-down");
    }

    #[test]
    fn an_order_is_read_from_its_json_and_garbage_is_a_failure_with_words() {
        let order = Order::Tidy {
            source: Source::AppImage,
        };
        let text = serde_json::to_vec(&order).unwrap();
        assert_eq!(read_order(&mut text.as_slice()), Ok(order));
        let garbage = read_order(&mut &b"{ not an order"[..]).unwrap_err();
        assert_eq!(garbage.refusal, None);
        assert!(
            garbage.trouble.contains("does not parse"),
            "{}",
            garbage.trouble
        );
        let huge = vec![b' '; (ORDER_MOST + 10) as usize];
        assert!(
            read_order(&mut huge.as_slice()).is_err(),
            "the ceiling let a whole pipe in"
        );
    }

    #[test]
    fn a_refusal_is_answered_and_written_down() {
        let root = scratch("refused");
        let ctx = context(&root, Tools::default());
        let order = Order::Remove {
            key: AppKey::System {
                desktop_id: "firefox.desktop".into(),
            },
            keep_data: true,
        };
        let mut beats: Vec<Beat> = Vec::new();
        let done = perform(&ctx, &order, &mut beats);
        assert_eq!(
            done.clone().unwrap_err().refusal,
            Some(Refusal::DeclaredInTheFlake)
        );
        assert_eq!(answer(&mut beats, done), ExitCode::FAILURE);
        assert!(
            matches!(beats.last(), Some(Beat::Failed(f)) if f.refusal == Some(Refusal::DeclaredInTheFlake))
        );
        let history = crate::journal::read(&ctx.roots.ours_state());
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].order, "remove");
        assert_eq!(history[0].name, "firefox.desktop");
        assert!(!history[0].ok);
        assert_eq!(history[0].refusal, Some(Refusal::DeclaredInTheFlake));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_rehearsal_leaves_no_history() {
        let root = scratch("rehearsal");
        let mut ctx = context(&root, Tools::default());
        ctx.rehearse = true;
        let order = Order::Remove {
            key: AppKey::System {
                desktop_id: "x.desktop".into(),
            },
            keep_data: true,
        };
        let _ = perform(&ctx, &order, &mut Vec::new());
        assert!(crate::journal::read(&ctx.roots.ours_state()).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_upgrade_of_everything_goes_through_every_source_this_machine_has() {
        // Programs that do not exist: whatever a module tries, it cannot
        // succeed, and nothing on this machine is touched.
        let root = scratch("everything");
        let nowhere = root.join("no-such-program");
        let tools = Tools {
            nix: Some(nowhere.clone()),
            flatpak: Some(nowhere),
            ..Tools::default()
        };
        let ctx = context(&root, tools);
        let failure = perform(
            &ctx,
            &Order::Upgrade {
                scope: Scope::Everything,
            },
            &mut Vec::new(),
        )
        .unwrap_err();
        let lines: Vec<&str> = failure.trouble.lines().collect();
        assert!(lines.iter().any(|l| l.starts_with("nix: ")), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.starts_with("flatpak: ")),
            "one failure stopped the rest: {lines:?}"
        );
        // What the machine does not have is not asked at all.
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("appimage: ") || l.starts_with("android: ")),
            "{lines:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_step_is_a_sentence_and_a_person_sees_it_once_per_phase() {
        let step = |done, total, now: &str| Beat::Step {
            phase: Phase::Downloading,
            done,
            total,
            now: now.into(),
        };
        assert_eq!(
            plain_line(&step(1, 4, "gimp")),
            "downloading: gimp (1 of 4)"
        );
        assert_eq!(plain_line(&step(0, 0, "")), "downloading");
        assert_eq!(
            plain_line(&Beat::Done(Outcome {
                amount: Some(7),
                ..Outcome::default()
            })),
            "done (7)"
        );
        assert_eq!(
            plain_line(&Beat::Done(Outcome {
                rehearsed: true,
                ..Outcome::default()
            })),
            "rehearsed, nothing changed"
        );
        assert_eq!(
            plain_line(&Beat::Failed(Failure::refused(Refusal::Offline))),
            "failed: offline"
        );
    }

    #[test]
    fn a_source_never_prepared_is_not_a_failure_of_everything() {
        assert!(nothing_to_upgrade(&Failure::refused(
            Refusal::SourceUnprepared
        )));
        for r in Refusal::ALL
            .into_iter()
            .filter(|r| *r != Refusal::SourceUnprepared)
        {
            assert!(
                !nothing_to_upgrade(&Failure::refused(r)),
                "{r:?} was swallowed"
            );
        }
    }

    #[test]
    fn the_images_are_upgraded_by_the_one_verb_that_does() {
        assert_eq!(images_argv(), ["upgrade"]);
    }

    #[test]
    fn a_list_and_a_search_read_as_lines() {
        use crate::found::{Badge, Found};
        use crate::state::Installed;
        let snapshot = Snapshot {
            installed: vec![Installed {
                key: AppKey::Flatpak {
                    id: "org.gnome.Calculator".into(),
                },
                name: "Calculator".into(),
                version: "48.1".into(),
                desktop_ids: vec![],
                size_bytes: None,
                update: Some("48.2".into()),
                unfree: false,
                origin: crate::state::Origin::Mine,
                icon: None,
                trouble: None,
            }],
            ..Snapshot::default()
        };
        assert_eq!(
            plain_list(&snapshot),
            "flatpak  org.gnome.Calculator  Calculator  48.1  (update: 48.2)\n"
        );
        let batch = Batch {
            source: Source::Nix,
            generation: 1,
            found: vec![Found {
                key: AppKey::Nix {
                    attr: "ripgrep".into(),
                },
                name: "ripgrep".into(),
                summary: "Searches directories".into(),
                version: "14.1.1".into(),
                badges: vec![Badge::CommandLine],
                download_bytes: None,
                icon: None,
                homepage: None,
                license: None,
                publisher: None,
                program: Some("rg".into()),
                by_description: false,
            }],
            status: Status::Answered {
                from: Some("abc123".into()),
            },
        };
        assert_eq!(
            plain_batch(&batch),
            "nix: answered, from abc123, 1 found\n  ripgrep  ripgrep  14.1.1  — Searches directories  [command-line]\n"
        );
    }
}
