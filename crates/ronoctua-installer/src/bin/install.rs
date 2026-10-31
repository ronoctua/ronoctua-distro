//! `nix run .#install` — section 7.1, milestone 8.
//!
//! ```text
//! ronoctua-install [--execute] [--answers <file>]
//! ronoctua-install --show <network|network-refused|network-connected|access|access-offline|token|passphrase|passphrase-new|passphrase-offline|password-again|review|review-offline|left-out|compose|tpm|tpm-bios|bios>
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
//! the answers, shows the plan and stops there. The TUI reads the network, and checks the
//! access and the passphrase when there is one, as it asks; the terminal-less mode does the
//! same before its first step, and stops with exit code 4 when the repository refuses the
//! access or the passphrase. NO NETWORK STOPS NEITHER: the installation never uses it, the
//! machine is installed from this ISO's own distro and keeps it, and with no way out the
//! repository's answers are taken as given and checked on the machine's first boot.
//!
//! # `--answers`
//!
//! Answers handed on in a file — one `NAME=value` a line, the variables of the terminal-less
//! mode, and the question to resume at — read and deleted at once: with a terminal the TUI
//! picks up there; without one they answer as the environment would.
//!
//! # `--show`
//!
//! Prints one of the screens that read the network or check the repository (or the list of
//! programs a `full` installation leaves out, or the Compose question), drawn as the TUI
//! draws it, from a fixture and with no network: what a person (or a reviewer) looks at
//! without a disk to install on. To a terminal, at its size and in its colours, so a narrow
//! console is looked at as it is; to a file, its text at 100 × 34 cells.
//!
//! # The programs left out
//!
//! The terminal-less dry run prints, after the plan, what the installation would write of
//! the programs a `full` installation leaves out (`RONOCTUA_INST_LEFT_OUT`, the unmarked ids
//! joined by commas, or `none`), each under the path it goes to (`--- <path> ---`): the
//! machine's choice the copy of the plain part signs into a repository with something in it
//! (left out and kept, on this machine alone), or the declines an empty one stages and seeds.
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
use ronoctua_i18n::Texts;
use ronoctua_installer::executor::{Event, run_command, stamp_now, write_of};
use ronoctua_installer::key;
use ronoctua_installer::model::{ANSWER_PREFIX, Answers, Question, sentence, validate_it};
use ronoctua_installer::plan::{Origin, first_boot_record, host_nix, staged_declined, steps};
use ronoctua_installer::platform::{self, Platform};
use ronoctua_installer::reach::{Reach, Refused};
use ronoctua_installer::release::FLAKE_DIR;
use ronoctua_installer::tui;

/// The usage, in English (invariant 18: a command line's help).
const USAGE: &str = "usage: ronoctua-install [--execute] [--answers <file|->]
       ronoctua-install --show <network|network-refused|network-connected|access|access-offline|token|passphrase|passphrase-new|passphrase-offline|password-again|review|review-offline|left-out|compose|tpm|tpm-bios|bios>";

/// The flag naming the answers handed on: a `0600` file this installer
/// reads and deletes, or [`ON_STDIN`].
const ANSWERS: &str = "--answers";

/// The size `--show` draws at when it does not print to a terminal: the
/// review's plan fits it, and the tests read it.
const SHOWN: (u16, u16) = (100, 34);

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
                ANSWERS => &mut line.answers,
                "--show" => &mut line.show,
                _ => return None,
            };
            let value = it.next().filter(|v| !v.is_empty())?;
            if slot.replace(value.clone()).is_some() {
                return None;
            }
        }
        Some(line)
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
    // THE TEXTS FIRST, and checked: data read at run time (`RONOCTUA_TEXTS`,
    // which the wrapper sets), and texts that are missing or lack a key this
    // binary was compiled with are said in one English line, exit 78, before
    // the first question — never asked in `[inst-…]` brackets.
    let texts = ronoctua_i18n::texts(&ronoctua_i18n::Keys::of(ronoctua_installer::key::sets))
        .unwrap_or_else(|missing| ronoctua_i18n::refuse(&missing));
    // THE PALETTES FIRST: data read at run time (`RONOCTUA_THEMES`, which the
    // wrapper sets), and a missing file is said before a screen is drawn —
    // the plan itself names the default theme.
    ronoctua_tokens::palettes::load();
    // THE THEME FROM THE ENVIRONMENT ONLY: the installer runs as root from the
    // ISO, where nobody has chosen, and a `theme.json` must not reach it. AND
    // THE DEFAULT WITHOUT IT, the one a fresh machine shows: the process
    // starts on the first theme of the chooser's order, and the installer is
    // the first screen of the system — another palette there would be
    // another system.
    ronoctua_tokens::theme::set(
        ronoctua_tokens::theme::from_env().unwrap_or_else(ronoctua_tokens::theme::default_one),
    );
    if let Some(screen) = &line.show {
        // TO A TERMINAL, AT ITS SIZE AND IN ITS COLOURS: a console of sixty
        // columns is looked at as it is, and the tones are part of what a
        // screen says — on the kernel's console, in its slots, as the TUI
        // paints there. To a file, the text alone at the size the tests read.
        let (size, how) = if std::io::stdout().is_terminal() {
            (
                crossterm::terminal::size().unwrap_or(SHOWN),
                if ronoctua_vt::here() {
                    tui::Shown::Console
                } else {
                    tui::Shown::Painted
                },
            )
        } else {
            (SHOWN, tui::Shown::Text)
        };
        let Some(drawn) = tui::show(screen, &texts, size, how) else {
            bail!("no screen called `{screen}`\n{USAGE}");
        };
        print!("{drawn}");
        return Ok(ExitCode::SUCCESS);
    }
    let reach = Reach::from_env();
    // THE ANSWERS HANDED ON — in a file, or by this installer to itself, on
    // its standard input: read, and the file gone at once — it holds the
    // passwords.
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

    let mut answers = if interactive {
        let resume = given.map(|text| {
            let (answers, at) = Answers::from_lines(&text);
            (answers, at.unwrap_or(Question::BelongingsAddress))
        });
        match tui::collect(&texts, &reach, resume, !line.execute)? {
            tui::Collected::GaveUp => return Ok(ExitCode::SUCCESS),
            tui::Collected::Answers(answers) => *answers,
        }
    } else {
        let vars = given.as_deref().map(vars_of).unwrap_or_default();
        let mut answers =
            from_vars(&|name| vars.get(name).cloned().or_else(|| std::env::var(name).ok()))?;
        // THE MOMENT THE PROGRAMS ARE LEFT OUT, once: the plan prints the
        // declines with it, and the steps write the same bytes.
        answers.left_out_at = Some(stamp_now());
        // THE MACHINE, as the TUI's stops read it: BIOS goes on only chosen
        // by name — and the choice is refused where it chooses nothing — and
        // no TPM 2.0 installs nothing (below, for both modes).
        let platform = Platform::here();
        answers.bios = !platform.uefi;
        if line.execute {
            let chosen = vars
                .get(platform::FIRMWARE)
                .cloned()
                .or_else(|| std::env::var(platform::FIRMWARE).ok());
            if let Some(other) = chosen.as_deref().filter(|v| *v != platform::CHOOSES_BIOS) {
                bail!(
                    "{} takes `{}` alone, not `{other}`",
                    platform::FIRMWARE,
                    platform::CHOOSES_BIOS
                );
            }
            if chosen.is_some() && platform.uefi {
                bail!(
                    "{} chooses BIOS, and this machine started from UEFI: it installs for UEFI; \
                     unset the variable",
                    platform::FIRMWARE
                );
            }
            if answers.bios && chosen.is_none() {
                println!("{}", tui::stop_said(tui::Stop::Bios, &texts, true));
                eprintln!(
                    "ronoctua-install: this machine started from BIOS; to install on BIOS anyway, \
                     set {}={}",
                    platform::FIRMWARE,
                    platform::CHOOSES_BIOS
                );
                return Ok(ExitCode::from(platform::EXIT_BIOS));
            }
        }
        if !line.execute {
            // THE DRY RUN ASKS NOTHING OF THE NETWORK: it prints what would
            // run.
            print_plan(&answers, &origin_of(&reach), &texts);
            println!("\n{}", texts.t(key::INST_DRYRUN));
            println!("{}", texts.t(key::INST_TO_APPLY));
            return Ok(ExitCode::SUCCESS);
        }
        answers
    };

    if !line.execute {
        println!("\n{}", texts.t(key::INST_DRYRUN));
        println!("{}", texts.t(key::INST_TO_APPLY));
        return Ok(ExitCode::SUCCESS);
    }
    // NO TPM 2.0, NOTHING INSTALLED — in both modes: the TUI's stop lets
    // only a rehearsal past, and this is what holds whatever reached here.
    if !Platform::here().tpm2 {
        println!("{}", tui::stop_said(tui::Stop::Tpm, &texts, answers.bios));
        eprintln!("ronoctua-install: no TPM 2.0 on this machine: nothing is installed");
        return Ok(ExitCode::from(platform::EXIT_TPM));
    }

    // THE TERMINAL-LESS MODE CHECKS NOW what the TUI checked screen by
    // screen — before anything is touched. With no network it goes on, and
    // says what was not asked.
    if !interactive {
        // THE NETWORK SAID FIRST: with none, the installation goes on, and
        // what follows checks only what needs no network.
        let way = reach.network(&answers.wifi_ssid, &answers.wifi_password);
        if !way.open() {
            println!("{}", texts.t(key::INST_OFFLINE));
            eprintln!(
                "ronoctua-install: no network ({}): the repository is not asked; the \
                 machine's first boot checks it",
                way.id()
            );
        }
        if let Err(refused) = reach.checks(&mut answers, way) {
            return Ok(refuse(&refused, &texts));
        }
    }
    let origin = origin_of(&reach);
    if reach.distro.is_none() {
        bail!("this installer does not know where its distro is: {FLAKE_DIR} is not set");
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
        Event::Refused { key } => println!("  {}", texts.t(key)),
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

/// Where the installation takes its distro from: always this ISO's own
/// tree, the machine's flake aimed at it (`ronoctua_installer::release`).
fn origin_of(reach: &Reach) -> Origin {
    Origin {
        distro: reach
            .distro
            .clone()
            .unwrap_or_else(|| format!("${FLAKE_DIR}")),
        aimed: true,
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
    c.args([ANSWERS, ON_STDIN]);
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
/// itself (`--answers -`), a file handed on, or the environment.
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
        // THE DEFAULT ANSWERS HERE TOO, and from the same place the TUI reads
        // — given the answers so far, for the variant's is the layout's.
        // `ALL_KEYS`'s order guarantees the layout has already been read.
        //
        // AN EMPTY VARIABLE IS AN ANSWER where empty is one (`optional`):
        // the TUI's "no variant" and "no Compose key". Defaulted, plain `us`
        // could not be installed without a terminal — the empty variant
        // would come back `euro`.
        let v = match get(&var) {
            Some(given) if !given.is_empty() || q.optional() => given,
            _ => q
                .default_given(&r)
                .map(ToOwned::to_owned)
                .unwrap_or_default(),
        };
        if let Err(e) = validate_it(q, &v, &languages, &layouts) {
            // A PROGRAM THIS INSTALLER DOES NOT OFFER is named: answers
            // written for another installer may carry one it listed, and
            // nobody is here to choose again — so nothing is touched.
            let named = ronoctua_installer::model::not_offered(&v);
            let named = if q == Question::LeftOut && !named.is_empty() {
                format!(" (not offered here: {})", named.join(", "))
            } else {
                String::new()
            };
            bail!(
                "{} ({}): {e}{named}\n\
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

/// Shows the `host.nix` and the sequence, without running anything — and,
/// AFTER them, the declines and the first boot's record, each under the path
/// a step writes it to (whoever reads the `host.nix` off the top keeps
/// reading it so).
fn print_plan(r: &Answers, origin: &Origin, texts: &Texts) {
    println!("--- host.nix ---\n{}", host_nix(r));
    println!("--- {} ---", texts.t(key::INST_REVIEW));
    let plan = steps(r, origin);
    for (i, p) in plan.iter().enumerate() {
        let mark = if p.destructive { "!" } else { " " };
        println!("{mark} {}. {} — {}", i + 1, texts.t(p.label), p.shown());
    }
    println!("\n{}: {}", texts.t(key::INST_DESTRUCTIVE), r.disk);
    // WHAT IS WRITTEN WHERE: the very content the step writes.
    let shown = [staged_declined(), first_boot_record()];
    for p in &plan {
        if let Ok(Some(w)) = write_of(p, r)
            && shown.contains(&w.path)
        {
            print!("\n--- {} ---\n{}", w.path, w.content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(args: &[&str]) -> Option<Line> {
        let args: Vec<String> = args.iter().map(|&a| a.to_owned()).collect();
        Line::read(&args)
    }

    /// FAILS IF: a flag of the release's hop is still taken — nothing hops
    /// any more —, a flag is taken twice or without its value, or the ones
    /// this program takes are not read.
    #[test]
    fn the_command_line_is_read_whole_or_refused() {
        let l = read(&["--execute", "--answers", "/run/x/answers"]).unwrap();
        assert!(l.execute);
        assert_eq!(l.answers.as_deref(), Some("/run/x/answers"));
        assert_eq!(
            read(&["--show", "review"]).unwrap().show.as_deref(),
            Some("review")
        );
        for bad in [
            &["--from-release", "abc"][..],
            &["--distro", "/x"],
            &["--from-release", "abc", "--distro", "/nix/store/x"],
            &["--answers"],
            &["--answers", "a", "--answers", "b"],
            &["--force"],
            &["--show", ""],
        ] {
            assert!(read(bad).is_none(), "{bad:?}");
        }
        assert!(read(&[]).is_some_and(|l| !l.execute));
    }

    /// FAILS IF: without a terminal, an unset variant is not the default
    /// layout's own (`us(euro)`, the system's), the default layout's variant
    /// reaches another layout, or an EMPTY variant or Compose key — the
    /// TUI's "none", a choice — comes back as the default.
    #[test]
    fn an_unset_answer_is_the_default_and_an_empty_one_is_none() {
        use ronoctua_installer::model::{DEFAULT_COMPOSE, DEFAULT_LAYOUT, DEFAULT_VARIANT};
        let answered = |extra: &[(Question, &str)]| {
            let vars: HashMap<String, String> = [
                (Question::Disk, "/dev/disk/by-id/a-disk"),
                (Question::Hostname, "noctua-teste"),
                (Question::Password, "fixture-password"),
                (Question::Passphrase, "fixture-passphrase"),
                (
                    Question::BelongingsAddress,
                    "git@example.org:someone/belongings.git",
                ),
                (Question::BelongingsAccess, "/run/keys/deploy"),
                (
                    Question::BelongingsPassphrase,
                    "fixture-belongings-passphrase",
                ),
            ]
            .iter()
            .chain(extra)
            .map(|(q, v)| (q.variable(), (*v).to_owned()))
            .collect();
            from_vars(&|n| vars.get(n).cloned()).expect("the answers were refused")
        };
        let r = answered(&[]);
        assert_eq!(r.keyboard, DEFAULT_LAYOUT);
        assert_eq!(r.keyboard_variant, DEFAULT_VARIANT);
        assert_eq!(r.compose_key, DEFAULT_COMPOSE);
        // ANOTHER LAYOUT, no variant named: none, never `br(euro)`.
        let r = answered(&[(Question::Keyboard, "br")]);
        assert_eq!(r.keyboard, "br");
        assert!(r.keyboard_variant.is_empty(), "{}", r.keyboard_variant);
        // EMPTY, SAID: plain `us`, and no Compose key.
        let r = answered(&[(Question::KeyboardVariant, ""), (Question::ComposeKey, "")]);
        assert_eq!(r.keyboard, DEFAULT_LAYOUT);
        assert!(r.keyboard_variant.is_empty(), "{}", r.keyboard_variant);
        assert!(r.compose_key.is_empty(), "{}", r.compose_key);
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
}
