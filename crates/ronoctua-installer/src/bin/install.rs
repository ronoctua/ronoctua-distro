//! `nix run .#install` — section 7.1, milestone 8.
//!
//! ```text
//! ronoctua-install [--execute] [--from-release <rev> --distro <tree>] [--answers <file>]
//! ronoctua-install --show <network|access|token|passphrase|passphrase-new|hop|review>
//! ```
//!
//! # Two modes, and why there are two
//!
//! **With a terminal**, it brings up the TUI: it is the path of whoever installs, and the
//! milestone's criterion speaks of it.
//!
//! **Without a terminal**, it reads from the environment and prints. It is not a convenience mode
//! — it is what lets `nix flake check` exercise the whole plan with
//! no terminal at all. An installer that only runs with a TTY is not testable, and an
//! untested installer is the part of the system where that is least acceptable.
//!
//! # Nothing happens without `--execute`
//!
//! The default is a DRY RUN: it shows the `host.nix` it would generate and every command it
//! would run, and exits without touching a disk — or the network. A command whose first step
//! erases a whole disk cannot be the default behaviour.
//!
//! In the TUI, `--execute` enables the confirmation screen; without it the TUI collects
//! the answers, shows the plan and stops there. The TUI checks the network, the release,
//! the access and the passphrase as it asks; the terminal-less mode checks the same four
//! before its first step, and stops with exit code 3 when there is no way out (4 when the
//! repository refuses the access or the passphrase).
//!
//! # Following the release
//!
//! When the release is NEWER than this installer's revision, the release's installer is
//! started in its place (`nix run <release>#install -- --from-release <rev> --distro
//! <tree>`), with the answers given so far in a `0600` file (`--answers`) the new one reads
//! and deletes — from the TUI and from the environment alike. `--from-release` makes it stay.
//!
//! # `--show`
//!
//! Prints one of the screens that check the network or the repository, drawn as the TUI
//! draws it, from a fixture and with no network: what a person (or a reviewer) looks at
//! without a disk to install on.
//!
//! # The answers leave the environment first
//!
//! Without a terminal the answers — four of them passwords — arrive as `RONOCTUA_INST_*`
//! variables, and every program the installer starts would inherit them: `mkpasswd`,
//! `lsblk`, `btrfs`, `systemd-creds`, `nixos-install` for an hour, each with all four in its
//! `/proc/<pid>/environ`. A variable cannot be taken out of a running process without
//! `unsafe` (Rust 2024), so the installer starts ITSELF again, at once, with every one of
//! them removed — the same process id, a clean environment — and hands the answers to the
//! new image on a pipe (`--answers -`), read and gone before anything else runs.

use std::collections::HashMap;
use std::io::{IsTerminal as _, Read as _, Write as _};
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::process::{Command, ExitCode};

use anyhow::{Context as _, Result, bail};
use ronoctua_i18n::{Texts, key};
use ronoctua_installer::executor::{Event, run_command};
use ronoctua_installer::model::{ANSWER_PREFIX, Answers, Question, sentence, validate_it};
use ronoctua_installer::plan::{Origin, host_nix, steps};
use ronoctua_installer::reach::{Reach, Refused};
use ronoctua_installer::release::{
    ANSWERS, DISTRO, FLAKE_DIR, FROM_RELEASE, Follow, Hop, hop_arguments, is_development,
};
use ronoctua_installer::tui;

/// The usage, in English (invariant 18: a command line's help).
const USAGE: &str =
    "usage: ronoctua-install [--execute] [--from-release <rev> --distro <tree>] [--answers <file|->]
       ronoctua-install --show <network|access|token|passphrase|passphrase-new|hop|review>";

/// The `--answers` that names this process's standard input: the answers
/// the installer handed itself when it shed them from its environment.
const ON_STDIN: &str = "-";

/// The most the answers handed on a pipe may weigh: written whole before
/// the new image reads a byte, they must fit the pipe's buffer (64 KiB on
/// Linux), or the write would wait for a reader that never comes.
const ANSWERS_MOST_BYTES: usize = 32 * 1024;

/// The command line, read.
#[derive(Debug, Default)]
struct Line {
    execute: bool,
    from_release: Option<String>,
    distro: Option<String>,
    answers: Option<String>,
    show: Option<String>,
}

impl Line {
    /// The command line, or `None` when it is not one this program takes.
    fn read(args: &[String]) -> Option<Self> {
        let mut line = Self::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let slot = match a.as_str() {
                "--execute" => {
                    line.execute = true;
                    continue;
                }
                FROM_RELEASE => &mut line.from_release,
                DISTRO => &mut line.distro,
                ANSWERS => &mut line.answers,
                "--show" => &mut line.show,
                _ => return None,
            };
            let value = it.next().filter(|v| !v.is_empty())?;
            if slot.replace(value.clone()).is_some() {
                return None;
            }
        }
        // The release's tree comes with its revision, or not at all.
        (line.from_release.is_some() == line.distro.is_some()).then_some(line)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(line) = Line::read(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    // THE ANSWERS LEAVE THIS PROCESS'S ENVIRONMENT before anything runs —
    // see the module's header. Returns only when there were none (or the
    // new image could not be started, which is said).
    let held = held_answers();
    if !held.is_empty() {
        let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        let err = shed(&args, &line, &held, interactive);
        eprintln!(
            "ronoctua-install: could not start again without the answers in the environment: {err:#}"
        );
        return ExitCode::FAILURE;
    }
    match install(&line) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

/// The installation, as the command line asks for it.
fn install(line: &Line) -> Result<ExitCode> {
    let texts = Texts::from_env();
    // THE THEME FROM THE ENVIRONMENT ONLY: the installer runs as root from the
    // ISO, where nobody has chosen, and a `theme.json` must not reach it.
    if let Some(theme) = ronoctua_tokens::theme::from_env() {
        ronoctua_tokens::theme::set(theme);
    }
    if let Some(screen) = &line.show {
        let Some(drawn) = tui::show(screen, &texts) else {
            bail!("no screen called `{screen}`\n{USAGE}");
        };
        print!("{drawn}");
        return Ok(ExitCode::SUCCESS);
    }
    let reach = Reach::from_env(line.distro.clone(), line.from_release.clone());
    // THE ANSWERS HANDED ON — by a hop, in a file, or by this installer to
    // itself, on its standard input: read, and the file gone at once — it
    // holds the passwords.
    let given = match line.answers.as_deref() {
        Some(ON_STDIN) => {
            let mut text = String::new();
            std::io::stdin()
                .take(u64::try_from(ANSWERS_MOST_BYTES).unwrap_or(u64::MAX))
                .read_to_string(&mut text)
                .context("read the answers on stdin")?;
            Some(text)
        }
        Some(file) => {
            let text = std::fs::read_to_string(file).with_context(|| format!("read {file}"))?;
            let _ = std::fs::remove_file(file);
            Some(text)
        }
        None => None,
    };

    // The TUI only makes sense with a terminal on BOTH sides: with no stdin there are no
    // keys, and with no stdout the drawing goes into a file as escape rubbish.
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();

    let (mut answers, stay) = if interactive {
        let resume = given.map(|text| {
            let (answers, at) = Answers::from_lines(&text);
            (answers, at.unwrap_or(Question::BelongingsAddress))
        });
        match tui::collect(&texts, &reach, resume)? {
            tui::Collected::GaveUp => return Ok(ExitCode::SUCCESS),
            tui::Collected::Hop(hop, answers) => return hop_to(&hop, line, &reach, &answers),
            tui::Collected::Answers(answers, stay) => (*answers, stay),
        }
    } else {
        let vars = given.as_deref().map(vars_of).unwrap_or_default();
        let answers =
            from_vars(&|name| vars.get(name).cloned().or_else(|| std::env::var(name).ok()))?;
        if !line.execute {
            // THE DRY RUN ASKS NOTHING OF THE NETWORK: it prints what would
            // run, as a development build would run it when that is what
            // this one is.
            let development = reach.own.as_ref().is_none_or(|o| is_development(&o.rev));
            print_plan(&answers, &origin_of(&reach, development), &texts);
            println!("\n{}", texts.t(key::INST_DRYRUN));
            println!("{}", texts.t(key::INST_TO_APPLY));
            return Ok(ExitCode::SUCCESS);
        }
        (answers, None)
    };

    if !line.execute {
        println!("\n{}", texts.t(key::INST_DRYRUN));
        println!("{}", texts.t(key::INST_TO_APPLY));
        return Ok(ExitCode::SUCCESS);
    }

    let stay = match stay {
        Some(stay) => stay,
        // THE TERMINAL-LESS MODE CHECKS NOW what the TUI checked screen by
        // screen — before anything is touched.
        None => match reach.preflight(&mut answers) {
            Ok(Follow::Hop(hop)) => return hop_to(&hop, line, &reach, &answers),
            Ok(Follow::Stay(stay)) => stay,
            Err(refused) => return Ok(refuse(&refused, &texts)),
        },
    };
    if let Some(said) = stay.key() {
        println!("{}", texts.t(said));
    }
    let origin = origin_of(&reach, stay.aims_at_own());
    if reach.distro.is_none() {
        bail!(
            "this installer does not know where its distro is: {FLAKE_DIR} is not set, and \
             no --distro was given"
        );
    }
    if !interactive {
        print_plan(&answers, &origin, &texts);
    }

    let mut current_step: Option<&'static str> = None;
    let r = run_command(&answers, &origin, &mut |e| match e {
        Event::Started {
            index,
            total,
            label,
        } => {
            current_step = Some(label);
            println!("\n[{}/{total}] {}", index + 1, texts.t(label));
        }
        Event::Line(l) => println!("  {l}"),
        Event::Skipped { .. } => println!("  {}", texts.t(key::INST_STEP_SKIPPED)),
        Event::Finished { .. } => {}
    });

    match r {
        Ok(()) => {
            println!("\n{}", texts.t(key::INST_DONE));
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            let where_it_was = current_step.map(|c| texts.t(c)).unwrap_or_default();
            bail!("{}: {where_it_was}\n\n{e:#}", texts.t(key::INST_FAILED))
        }
    }
}

/// Where the installation takes its distro from.
fn origin_of(reach: &Reach, aimed: bool) -> Origin {
    Origin {
        distro: reach
            .distro
            .clone()
            .unwrap_or_else(|| format!("${FLAKE_DIR}")),
        aimed,
        nix: reach.nix.to_string_lossy().into_owned(),
    }
}

/// Says why the terminal-less mode will not start, and ends with its code.
fn refuse(refused: &Refused, texts: &Texts) -> ExitCode {
    eprintln!("{}", sentence(texts, refused.key()));
    eprintln!(
        "ronoctua-install: refused before any step: {}",
        refused.id()
    );
    ExitCode::from(refused.exit_code())
}

/// Starts the release's installer in this one's place. The answers travel
/// in a `0600` file of the scratch folder, in both modes: the environment
/// no longer holds them (see the module's header).
fn hop_to(hop: &Hop, line: &Line, reach: &Reach, answers: &Answers) -> Result<ExitCode> {
    let file = reach.scratch.answers();
    ronoctua_installer::reach::keep_private(
        &reach.scratch.root,
        &file,
        answers.lines(Question::BelongingsAddress).as_bytes(),
    )
    .with_context(|| format!("write {file}"))?;
    let kept: Vec<String> = if line.execute {
        vec!["--execute".to_owned()]
    } else {
        Vec::new()
    };
    eprintln!(
        "ronoctua-install: the release is at {}; starting its installer",
        hop.short()
    );
    let err = ronoctua_installer::executor::command(&reach.nix)
        .args(hop_arguments(hop, &kept, Some(&file)))
        .exec();
    let _ = std::fs::remove_file(&file);
    bail!("the release's installer did not start: {err}")
}

/// The answers this process's environment holds, by name and value.
fn held_answers() -> Vec<(String, String)> {
    std::env::vars_os()
        .filter_map(|(name, value)| {
            let name = name.into_string().ok()?;
            name.starts_with(ANSWER_PREFIX)
                .then(|| (name, value.to_string_lossy().into_owned()))
        })
        .collect()
}

/// This installer started again WITHOUT the answers in its environment:
/// the command, and the answers it is handed on its standard input
/// (`--answers -`) — none when it has a terminal (the TUI asks; what the
/// environment said is dropped) or when a file of answers was named
/// already (that file is what it reads). Pure.
fn again(
    exe: &Path,
    args: &[String],
    line: &Line,
    held: &[(String, String)],
    interactive: bool,
) -> (Command, Option<String>) {
    let mut c = ronoctua_installer::executor::command(exe);
    c.args(args);
    for (name, _) in held {
        c.env_remove(name);
    }
    if interactive || line.answers.is_some() {
        return (c, None);
    }
    // ONE `NAME=value` A LINE, as the environment said them; a value with
    // a line break cannot cross, and is left out — its question then says
    // it was not answered, as an unset variable would.
    let mut text = String::new();
    for (name, value) in held {
        if !value.contains('\n') {
            text.push_str(name);
            text.push('=');
            text.push_str(value);
            text.push('\n');
        }
    }
    c.args(["--answers", ON_STDIN]);
    (c, Some(text))
}

/// Starts this installer again without the answers in its environment
/// ([`again`]); returns only if it could not.
fn shed(
    args: &[String],
    line: &Line,
    held: &[(String, String)],
    interactive: bool,
) -> anyhow::Error {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return anyhow::Error::new(e).context("find this program"),
    };
    let (mut command, text) = again(&exe, args, line, held, interactive);
    if let Some(text) = text {
        if text.len() > ANSWERS_MOST_BYTES {
            return anyhow::anyhow!("the answers weigh more than {ANSWERS_MOST_BYTES} bytes");
        }
        // WRITTEN WHOLE BEFORE THE EXEC, and the writing end closed: the
        // new image reads them to the end of the pipe, which is this.
        let (reader, mut writer) = match std::io::pipe() {
            Ok(pair) => pair,
            Err(e) => return anyhow::Error::new(e).context("a pipe for the answers"),
        };
        if let Err(e) = writer.write_all(text.as_bytes()) {
            return anyhow::Error::new(e).context("hand the answers on");
        }
        drop(writer);
        command.stdin(reader);
    }
    anyhow::Error::new(command.exec()).context(format!("start {}", exe.display()))
}

/// `NAME=value` lines, by name. Pure.
fn vars_of(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(n, v)| (n.to_owned(), v.to_owned()))
        .collect()
}

/// Reads the answers from their variables — the ones this installer handed
/// itself (`--answers -`), a hop's file, or the environment.
///
/// One per question, with the name derived from the label — see
/// [`Question::variable`]. There is no second table to fall out of sync.
fn from_vars(get: &dyn Fn(&str) -> Option<String>) -> Result<Answers> {
    let languages: Vec<&str> = ronoctua_i18n::LANGUAGES.to_vec();
    let owned = ronoctua_installer::model::layout_names();
    let layouts: Vec<&str> = owned.iter().map(String::as_str).collect();
    let mut r = Answers::default();
    for q in Question::ALL_KEYS {
        if q.computed() {
            continue;
        }
        // THE SAME RULE AS THE TUI'S, and not a second one: no Wi-Fi
        // password is demanded of whoever chose no network. `ALL_KEYS`'s
        // order guarantees that `Wifi` has already been read.
        if q.skipped_one(&r) {
            continue;
        }
        let var = q.variable();
        // THE DEFAULT ANSWERS HERE TOO, and from the same place the TUI reads.
        let v = get(&var)
            .filter(|s| !s.is_empty())
            .or_else(|| q.default_answer().map(ToOwned::to_owned))
            .unwrap_or_default();
        if let Err(e) = validate_it(q, &v, &languages, &layouts) {
            bail!(
                "{} ({}): {e}\n\
                 \n\
                 Set {var}. The TUI does that by asking; without a terminal,\n\
                 the environment is the way to answer.",
                q.label(),
                q.label()
            );
        }
        r.set(q, v);
    }
    Ok(r)
}

/// Shows the `host.nix` and the sequence, without running anything.
fn print_plan(r: &Answers, origin: &Origin, texts: &Texts) {
    println!("--- host.nix ---\n{}", host_nix(r));
    println!("--- {} ---", texts.t(key::INST_REVIEW));
    for (i, p) in steps(r, origin).iter().enumerate() {
        let mark = if p.destructive { "!" } else { " " };
        println!("{mark} {}. {} — {}", i + 1, texts.t(p.label), p.shown());
    }
    println!("\n{}: {}", texts.t(key::INST_DESTRUCTIVE), r.disk);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_installer::release::Stay;

    fn read(args: &[&str]) -> Option<Line> {
        let args: Vec<String> = args.iter().map(|&a| a.to_owned()).collect();
        Line::read(&args)
    }

    #[test]
    fn the_command_line_is_read_whole_or_refused() {
        let l = read(&[
            "--execute",
            "--from-release",
            "abc",
            "--distro",
            "/nix/store/x",
        ])
        .unwrap();
        assert!(l.execute);
        assert_eq!(l.from_release.as_deref(), Some("abc"));
        assert_eq!(l.distro.as_deref(), Some("/nix/store/x"));
        for bad in [
            &["--from-release", "abc"][..],
            &["--distro", "/x"],
            &["--answers"],
            &["--answers", "a", "--answers", "b"],
            &["--force"],
            &["--show", ""],
        ] {
            assert!(read(bad).is_none(), "{bad:?}");
        }
        assert!(read(&[]).is_some_and(|l| !l.execute));
    }

    /// FAILS IF: the installer started again keeps an answer in its
    /// environment, loses one on the way (the terminal-less mode), hands
    /// them on when it has a terminal or a file already, or changes the
    /// command line it was given.
    #[test]
    fn the_answers_leave_the_environment_and_cross_on_a_pipe() {
        let held = vec![
            (
                "RONOCTUA_INST_PASSWORD".to_owned(),
                "Zq7 with = and spaces".to_owned(),
            ),
            (
                "RONOCTUA_INST_HOSTNAME".to_owned(),
                "noctua-teste".to_owned(),
            ),
            (
                "RONOCTUA_INST_WIFI_PASSWORD".to_owned(),
                "two\nlines".to_owned(),
            ),
        ];
        let args = vec!["--execute".to_owned()];
        let line = Line::read(&args).unwrap();
        let (c, text) = again(Path::new("/x/ronoctua-install"), &args, &line, &held, false);
        let removed: Vec<String> = c
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        for (name, _) in &held {
            assert!(removed.contains(name), "{name} stays in the environment");
        }
        let given: Vec<String> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(given, ["--execute", "--answers", ON_STDIN]);
        let text = text.expect("the terminal-less mode lost its answers");
        let vars = vars_of(&text);
        assert_eq!(
            vars.get("RONOCTUA_INST_PASSWORD").map(String::as_str),
            Some("Zq7 with = and spaces")
        );
        assert_eq!(
            vars.get("RONOCTUA_INST_HOSTNAME").map(String::as_str),
            Some("noctua-teste")
        );
        assert!(!vars.contains_key("RONOCTUA_INST_WIFI_PASSWORD"));
        // READ BACK as the environment would have been read.
        let r = from_vars(&|n| vars.get(n).cloned())
            .err()
            .map(|e| e.to_string());
        assert!(
            r.as_deref().is_none_or(|e| !e.contains("HOSTNAME")),
            "the hostname did not cross: {r:?}"
        );
        // WITH A TERMINAL, or a file named: stripped, nothing handed on.
        let (c, text) = again(Path::new("/x/ronoctua-install"), &args, &line, &held, true);
        assert!(text.is_none());
        assert_eq!(c.get_args().count(), 1);
        let filed = vec!["--answers".to_owned(), "/run/x/answers".to_owned()];
        let line = Line::read(&filed).unwrap();
        let (c, text) = again(
            Path::new("/x/ronoctua-install"),
            &filed,
            &line,
            &held,
            false,
        );
        assert!(text.is_none());
        assert_eq!(c.get_args().count(), 2);
    }

    #[test]
    fn a_stay_the_person_should_hear_is_said() {
        assert!(Stay::Unreachable.key().is_some());
        assert!(Stay::Current.key().is_none());
    }
}
