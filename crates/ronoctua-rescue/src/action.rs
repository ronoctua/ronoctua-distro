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
//! The privileged rows are calls THIS process makes over the system bus — the
//! system's units to systemd ([`crate::bus`]), a restart or a power-off to
//! logind (`ronoctua_power::leave_asking`) — and it is polkit that decides:
//! `auth_self_keep`, asked once and remembered. The asking needs an agent
//! registered for this very process, which is what [`agent`] starts. This
//! crate never reads a password, never holds one and never checks one: a
//! rescue console is the worst possible place to get authentication wrong.

use std::process::{Command, Stdio};
use std::time::Duration;

use crate::{Action, Own, Power, Run, Tier};

/// `systemctl`, by store path.
const SYSTEMCTL: Option<&str> = option_env!("RONOCTUA_SYSTEMCTL");
/// `fc-cache`, for the row that answers a screen full of boxes.
const FC_CACHE: Option<&str> = option_env!("RONOCTUA_FC_CACHE");
/// The shell the terminal row hands the screen to.
const SHELL: Option<&str> = option_env!("RONOCTUA_RESCUE_SHELL");
/// polkit's text-mode agent — how a VT with no desktop can ask anything.
const POLKIT_AGENT: Option<&str> = option_env!("RONOCTUA_PKTTYAGENT");

/// How long the agent has to register before the console goes on without
/// it: a polkit that does not answer must not hold the screen shut.
const REGISTERING: Duration = Duration::from_secs(10);

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It ran and said nothing worth repeating.
    Ran,
    /// It ran and failed; the text is EVERYTHING it said of why, every line.
    Failed(String),
    /// It could not even be started, and the reason is ours to fix.
    ///
    /// Separate from [`Outcome::Failed`] because it means a variable was not
    /// set at build time — a defect in `nix/rust.nix`, not in the machine.
    Missing(&'static str),
}

/// Whether a row is a call over the system bus — the system's units, or
/// leaving the machine — rather than a program.
#[must_use]
pub const fn by_the_bus(run: Run) -> bool {
    matches!(run, Run::System { .. } | Run::Power(_))
}

/// Whether a row ASKS: a call over the system bus of the password tier, which
/// polkit may answer only by asking the person — on the terminal the console
/// hands its agent first.
#[must_use]
pub fn asks(a: &Action) -> bool {
    a.tier == Tier::Password && by_the_bus(a.run)
}

/// The program a row runs, or `None` when the app answers it itself or calls
/// the bus ([`by_the_bus`]).
///
/// Split out from [`run_it`] so the tests can look at the command line without
/// running anything — a test that shuts the machine down to prove it can shut
/// the machine down is not a test.
#[must_use]
pub fn command_of(run: Run) -> Option<Result<Command, &'static str>> {
    match run {
        Run::User { verb, units } => Some(systemctl(&["--user", verb], units)),
        Run::System { .. } | Run::Power(_) => None,
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

/// Runs a row and says what happened. `asking`: polkit may ask the person
/// (a row that [`asks`], whose terminal the agent has); leaving the machine
/// always may, being of the password tier alone.
///
/// It BLOCKS, and that is deliberate: this application has one screen and one
/// thing happening at a time, and a rescue that returned immediately would leave
/// the person guessing whether the button worked. The rows that could take long
/// enough to matter (`rebuild-switch`, the rollbacks) start a oneshot, and the
/// row waits for its job to end, as `systemctl start` waits.
#[must_use]
pub fn run_it(run: Run, asking: bool) -> Outcome {
    match run {
        Run::System { verb, units } => {
            return crate::bus::units(verb, units, asking)
                .map_or_else(Outcome::Failed, |()| Outcome::Ran);
        }
        Run::Power(p) => {
            let action = match p {
                Power::Reboot => ronoctua_power::Action::Reboot,
                Power::Off => ronoctua_power::Action::PowerOff,
            };
            return ronoctua_power::leave_asking(action)
                .map_or_else(|e| Outcome::Failed(format!("{e:#}")), |()| Outcome::Ran);
        }
        _ => {}
    }
    let Some(built) = command_of(run) else {
        return Outcome::Ran;
    };
    let mut command = match built {
        Ok(c) => c,
        Err(variable) => return Outcome::Missing(variable),
    };
    match command.output() {
        Ok(out) if out.status.success() => Outcome::Ran,
        Ok(out) => Outcome::Failed(String::from_utf8_lossy(&out.stderr).trim().to_owned()),
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

/// polkit's text agent for this process, while a row that asks runs: killed
/// when dropped.
#[derive(Debug)]
pub struct Agent(std::process::Child);

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts polkit's text agent FOR THIS PROCESS, so the row about to run can
/// ask on this terminal — and waits until polkit has it.
///
/// polkit finds the agent of the process that asked, else of its logind
/// session; this console has no session on VT 8, and the session the
/// floating window lives in has no agent. So the agent is this process's own
/// (`--process`), never a `--fallback` one (polkit uses a fallback only for a
/// subject with a session), and the calls are this process's own
/// ([`crate::bus`]). It closes the descriptor it is given (`--notify-fd`, its
/// standard output here) once registered; a polkit that does not answer is
/// waited for [`REGISTERING`], no longer.
///
/// ONE PER ROW, and not one for the console's life: a switch the row ran
/// (the rollback's, the rebuild's) restarts polkit when its rules changed,
/// and polkit forgets every agent it had — the next row would be refused.
/// Holding the [`Agent`] for the call is what keeps it registered.
#[must_use]
pub fn agent() -> Option<Agent> {
    let path = POLKIT_AGENT?;
    let mut child = Command::new(path)
        .arg("--process")
        .arg(std::process::id().to_string())
        .args(["--notify-fd", "1"])
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    if let Some(mut registered) = child.stdout.take() {
        let (done, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut registered, &mut std::io::sink());
            let _ = done.send(());
        });
        let _ = wait.recv_timeout(REGISTERING);
    }
    Some(Agent(child))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::all_of;

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
                    "`{}` without --user would ask for privilege to restart its own session: {line:?}",
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
            eprintln!("no RONOCTUA_SYSTEMCTL: the path test does not run outside Nix");
            return;
        };
        for a in all_of() {
            let Some(built) = command_of(a.run) else {
                continue;
            };
            let c = built.unwrap_or_else(|v| panic!("`{}` without the variable {v}", a.id));
            let program = c.get_program().to_string_lossy().into_owned();
            assert!(
                program.starts_with("/nix/store/"),
                "`{}` runs `{program}` by name",
                a.id
            );
        }
    }

    #[test]
    fn every_row_either_runs_something_or_is_ours() {
        // A row that builds no command, calls no bus and is not an `Own` would
        // be a link that does nothing when clicked, silently.
        for a in all_of() {
            let has_command = command_of(a.run).is_some();
            let is_ours = matches!(a.run, Run::Own(_));
            assert!(
                has_command || by_the_bus(a.run) || is_ours,
                "`{}` runs nothing and is not ours",
                a.id
            );
        }
    }

    #[test]
    fn what_asks_is_the_password_tier_on_the_bus() {
        // THE DEFECT THIS CATCHES: a row of the password tier run as a
        // program — a `systemctl` of its own is a process no agent answers
        // for, and the row is refused every time, in front of the person.
        for a in all_of() {
            if matches!(a.run, Run::System { .. } | Run::Power(_)) {
                assert!(
                    command_of(a.run).is_none(),
                    "`{}` runs a program for the system",
                    a.id
                );
                assert_eq!(asks(a), a.tier == Tier::Password, "`{}`", a.id);
            } else {
                assert!(!asks(a), "`{}` asks and is not on the bus", a.id);
            }
        }
        // And the two that leave the machine ask, through logind.
        assert!(
            all_of()
                .iter()
                .filter(|a| matches!(a.run, Run::Power(_)))
                .all(asks)
        );
    }

    #[test]
    fn the_two_that_end_the_session_ask_first() {
        for a in all_of() {
            if matches!(a.run, Run::Power(_)) {
                assert_eq!(
                    a.tier,
                    Tier::Password,
                    "`{}` shuts the machine down without asking",
                    a.id
                );
            }
        }
    }
}
