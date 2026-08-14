//! `nix run .#install` — section 7.1, milestone 8.
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
//! # Nothing happens without `--run_command`
//!
//! The default is a DRY RUN: it shows the `host.nix` it would generate and every command it
//! would run, and exits without touching a disk. A command whose first step erases
//! a whole disk cannot be the default behaviour.
//!
//! In the TUI, `--run_command` enables the confirmation screen; without it the TUI collects
//! the answers, shows the plan and stops there.

use std::io::IsTerminal as _;

use anyhow::{Result, bail};
use ronoctua_i18n::{Texts, key};
use ronoctua_installer::executor::{Event, run_command};
use ronoctua_installer::model::{Answers, Question, validate_it};
use ronoctua_installer::plan::{host_nix, steps};
use ronoctua_installer::tui;

/// The target system's flake. The ISO carries it in `/etc/nixos`.
const FLAKE: &str = "/mnt/etc/nixos";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let executar_de_verdade = args.iter().any(|a| a == "--executar");

    let texts = Texts::from_env();

    // The TUI only makes sense with a terminal on BOTH sides: with no stdin there are no
    // keys, and with no stdout the drawing goes into a file as escape
    // rubbish. `nix run .#install > log` falls into the environment mode, and that is
    // right.
    let interativo = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();

    let answers = if interativo {
        match tui::collect(&texts)? {
            Some(r) => r,
            // They gave up. Nothing was touched, and there is nothing to say beyond that.
            None => return Ok(()),
        }
    } else {
        from_env(&texts)?
    };

    // The TUI has already shown the plan and already asked for confirmation; repeating it here would be
    // noise. In the environment mode, printing is the ONLY way for the person to see
    // what is going to happen.
    if !interativo {
        print_plan(&answers, &texts);
    }

    if !executar_de_verdade {
        println!("\n{}", texts.t(key::INST_DRYRUN));
        println!("{}", texts.t(key::INST_TO_APPLY));
        return Ok(());
    }

    let mut passo_atual: Option<&'static str> = None;
    let r = run_command(&answers, FLAKE, &mut |e| match e {
        Event::Started {
            index,
            total,
            label,
        } => {
            passo_atual = Some(label);
            println!("\n[{}/{total}] {}", index + 1, texts.t(label));
        }
        Event::Line(l) => println!("  {l}"),
        Event::Finished { .. } => {}
    });

    match r {
        Ok(()) => {
            println!("\n{}", texts.t(key::INST_DONE));
            Ok(())
        }
        Err(e) => {
            let onde = passo_atual.map(|c| texts.t(c)).unwrap_or_default();
            bail!("{}: {onde}\n\n{e:#}", texts.t(key::INST_FAILED))
        }
    }
}

/// Reads the answers from the environment variables.
///
/// One per question, with the name derived from the label — see
/// [`Question::variable`]. There is no second table to fall out of sync.
fn from_env(texts: &Texts) -> Result<Answers> {
    let languages: Vec<&str> = ronoctua_i18n::LANGUAGES.to_vec();
    let owned = ronoctua_installer::model::layout_names();
    let layouts: Vec<&str> = owned.iter().map(String::as_str).collect();
    let mut r = Answers::default();
    for q in Question::ALL_KEYS {
        if q.computed() {
            continue;
        }
        // THE SAME RULE AS THE TUI'S, and not a second one.
        //
        // Without this, the terminal-less mode would demand the Wi-Fi password from whoever
        // chose no network at all — and `nix flake check`, which runs through
        // here, would start demanding a password in order to install over a cable.
        //
        // `ALL_KEYS`'s order guarantees that `Wifi` has already been read when
        // `WifiPassword` is evaluated.
        if q.skipped_one(&r) {
            continue;
        }
        let var = q.variable();
        // THE DEFAULT ANSWERS HERE TOO, and from the same place the TUI reads.
        //
        // Without it the terminal-less mode would demand a locale and a timezone
        // from whoever only wants to test the plan — questions the screen answers
        // by opening already chosen.
        let v = std::env::var(&var)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| q.default_answer().map(ToOwned::to_owned))
            .unwrap_or_default();
        if let Err(e) = validate_it(q, &v, &languages, &layouts) {
            bail!(
                "{} ({}): {e}\n\
                 \n\
                 Defina {var}. A TUI faz isso perguntando; sem terminal,\n\
                 o ambiente e a forma de responder.",
                texts.t(q.label()),
                q.label()
            );
        }
        r.set(q, v);
    }
    Ok(r)
}

/// Shows the `host.nix` and the sequence, without running anything.
fn print_plan(r: &Answers, texts: &Texts) {
    println!("--- host.nix ---\n{}", host_nix(r));
    println!("--- {} ---", texts.t(key::INST_REVIEW));
    for (i, p) in steps(r, FLAKE).iter().enumerate() {
        let mark = if p.destructive { "!" } else { " " };
        println!(
            "{mark} {}. {} — {}",
            i + 1,
            texts.t(p.label),
            p.command.join(" ")
        );
    }
    println!("\n{}: {}", texts.t(key::INST_DESTRUCTIVE), r.disk);
}
