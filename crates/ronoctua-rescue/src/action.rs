//! How a row of the table becomes a process.
//!
//! # Everything by store path
//!
//! Not one program is spawned by name. This binary runs from a systemd unit on
//! a VT, where `PATH` is minimal, and a name that does not resolve fails far
//! from its cause — the trap `nix/rust.nix` documents for the footer, one level
//! more dangerous here: the failure would land on the screen someone opened
//! BECAUSE something was already broken.
//!
//! `rescue-runs-by-path` reads the names below out of this file and demands that
//! `nix/rust.nix` set every one of them, in the shape of
//! `shell-finds-its-buffers`.
//!
//! # And nothing here asks for a password
//!
//! The privileged rows go through `systemctl`, and it is polkit that decides —
//! `auth_self_keep`, asked once and remembered. On the VT the asking needs an
//! agent, which is why [`agent`] exists. This crate never reads a password,
//! never holds one and never checks one: a rescue console is the worst possible
//! place to get authentication wrong.

use std::process::{Command, Stdio};

use crate::{Own, Power, Run};

/// `systemctl`, by store path.
const SYSTEMCTL: Option<&str> = option_env!("RONOCTUA_SYSTEMCTL");
/// `fc-cache`, for the row that answers a screen full of boxes.
const FC_CACHE: Option<&str> = option_env!("RONOCTUA_FC_CACHE");
/// The shell the terminal row hands the screen to.
const SHELL: Option<&str> = option_env!("RONOCTUA_RESCUE_SHELL");
/// polkit's text-mode agent — how a VT with no desktop can ask anything.
const POLKIT_AGENT: Option<&str> = option_env!("RONOCTUA_PKTTYAGENT");

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It ran and said nothing worth repeating.
    Ran,
    /// It ran and failed; the text is the last thing it said.
    Failed(String),
    /// It could not even be started, and the reason is ours to fix.
    ///
    /// Separate from [`Outcome::Failed`] because it means a variable was not
    /// set at build time — a defect in `nix/rust.nix`, not in the machine.
    Missing(&'static str),
}

/// The program a row runs, or `None` when the app answers it itself.
///
/// Split out from [`run_it`] so the tests can look at the command line without
/// running anything — a test that shuts the machine down to prove it can shut
/// the machine down is not a test.
#[must_use]
pub fn command_of(run: Run) -> Option<Result<Command, &'static str>> {
    match run {
        Run::User { verb, units } => Some(systemctl(&["--user", verb], units)),
        Run::System { verb, units } => Some(systemctl(&[verb], units)),
        Run::Power(p) => {
            let verb = match p {
                Power::Reboot => "reboot",
                Power::Off => "poweroff",
            };
            Some(systemctl(&[verb], &[]))
        }
        Run::Own(Own::RefreshFonts) => Some(match FC_CACHE {
            // `-r` rebuilds from scratch and `-f` ignores the timestamps: a
            // stale cache whose mtime looks fresh is exactly the case that
            // brings someone here.
            Some(p) => {
                let mut c = Command::new(p);
                c.arg("-rf");
                Ok(c)
            }
            None => Err("RONOCTUA_FC_CACHE"),
        }),
        Run::Own(_) => None,
    }
}

/// Builds a `systemctl` invocation.
fn systemctl(flags: &[&str], units: &[&str]) -> Result<Command, &'static str> {
    let Some(path) = SYSTEMCTL else {
        return Err("RONOCTUA_SYSTEMCTL");
    };
    let mut c = Command::new(path);
    c.args(flags);
    c.args(units);
    Ok(c)
}

/// Runs a row and says what happened.
///
/// It BLOCKS, and that is deliberate: this application has one screen and one
/// thing happening at a time, and a rescue that returned immediately would leave
/// the person guessing whether the button worked. The rows that could take long
/// enough to matter (`rebuild-switch`, the rollbacks) are `systemctl start` on a
/// oneshot, which returns as soon as systemd has taken the job.
#[must_use]
pub fn run_it(run: Run) -> Outcome {
    let Some(built) = command_of(run) else {
        return Outcome::Ran;
    };
    let mut command = match built {
        Ok(c) => c,
        Err(variable) => return Outcome::Missing(variable),
    };
    match command.output() {
        Ok(out) if out.status.success() => Outcome::Ran,
        Ok(out) => {
            let said = String::from_utf8_lossy(&out.stderr);
            let last = said.lines().rev().find(|l| !l.trim().is_empty());
            Outcome::Failed(last.unwrap_or("").to_owned())
        }
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

/// The shell the terminal row hands the screen to.
///
/// # Errors
///
/// When the build did not say which shell — the same class as everything else
/// here, and it says which variable rather than falling back to a name.
pub fn shell() -> Result<Command, &'static str> {
    let Some(path) = SHELL else {
        return Err("RONOCTUA_RESCUE_SHELL");
    };
    let mut c = Command::new(path);
    // A login shell: on the VT there is no desktop to have set anything up, and
    // the person's own environment is most of what makes a shell useful.
    c.arg("-l");
    Ok(c)
}

/// Starts polkit's text agent, so a privileged row can ask on this terminal.
///
/// Returns the child, which has to stay alive as long as the application does —
/// dropping it takes the agent with it, and the next question would fail with
/// "Interactive authentication required" on a screen with no way to answer.
///
/// A missing agent is NOT an error: with the desktop alive there is already a
/// graphical agent registered for the session, and starting a second one would
/// be the one that answers.
#[must_use]
pub fn agent() -> Option<std::process::Child> {
    let path = POLKIT_AGENT?;
    Command::new(path)
        .arg("--fallback")
        .stdout(Stdio::null())
        .spawn()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tier, all_of};

    /// The command line of a row, or nothing when the build did not name the
    /// program — `cargo test` outside Nix, which is a case this repository
    /// keeps working on purpose.
    fn line_of(run: Run) -> Option<Vec<String>> {
        let c = command_of(run)?.ok()?;
        Some(
            std::iter::once(c.get_program())
                .chain(c.get_args())
                .map(|s| s.to_string_lossy().into_owned())
                .collect(),
        )
    }

    #[test]
    fn the_session_rows_go_to_the_user_manager() {
        // `--user` is the whole difference between restarting the person's own
        // compositor and asking polkit for the system's.
        for a in all_of() {
            if let Run::User { .. } = a.run {
                let Some(line) = line_of(a.run) else { continue };
                assert!(
                    line.contains(&"--user".to_owned()),
                    "`{}` sem --user pediria privilegio para reiniciar a propria sessao: {line:?}",
                    a.id
                );
            }
        }
    }

    #[test]
    fn the_system_rows_do_not() {
        for a in all_of() {
            if let Run::System { .. } = a.run {
                let Some(line) = line_of(a.run) else { continue };
                assert!(
                    !line.contains(&"--user".to_owned()),
                    "`{}` com --user procuraria a unidade no lugar errado: {line:?}",
                    a.id
                );
            }
        }
    }

    #[test]
    fn nothing_is_spawned_by_name() {
        // THE TRAP THIS CLOSES: under a systemd unit `PATH` is minimal, and a
        // bare name fails far from the cause — on the screen someone opened
        // because something was already broken.
        //
        // Outside Nix there is no store path to find, and the test says so
        // instead of passing by not looking.
        let Some(_) = SYSTEMCTL else {
            eprintln!("sem RONOCTUA_SYSTEMCTL: o teste dos caminhos nao roda fora do Nix");
            return;
        };
        for a in all_of() {
            let Some(built) = command_of(a.run) else {
                continue;
            };
            let c = built.unwrap_or_else(|v| panic!("`{}` sem a variavel {v}", a.id));
            let program = c.get_program().to_string_lossy().into_owned();
            assert!(
                program.starts_with("/nix/store/"),
                "`{}` roda `{program}` por nome",
                a.id
            );
        }
    }

    #[test]
    fn every_row_either_runs_something_or_is_ours() {
        // A row that builds no command and is not an `Own` would be a link that
        // does nothing when clicked, silently.
        for a in all_of() {
            let has_command = command_of(a.run).is_some();
            let is_ours = matches!(a.run, Run::Own(_));
            assert!(
                has_command || is_ours,
                "`{}` nao roda nada e nao e nossa",
                a.id
            );
        }
    }

    #[test]
    fn the_two_that_end_the_session_ask_first() {
        for a in all_of() {
            if matches!(a.run, Run::Power(_)) {
                assert_eq!(
                    a.tier,
                    Tier::Password,
                    "`{}` desliga a maquina sem perguntar",
                    a.id
                );
            }
        }
    }
}
