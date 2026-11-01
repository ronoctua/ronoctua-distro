//! The system's units, asked by THIS process over the system bus.
//!
//! # Why not `systemctl`
//!
//! polkit answers a question for the process that asked it: the subject is
//! the caller of the D-Bus method, and the agent that may ask the person is
//! the one registered for that very process — or, failing that, for its
//! logind session. A `systemctl` this console spawned is another process with
//! no agent of its own (its standard input is not a terminal, so it neither
//! starts one nor allows the question at all), and a console on VT 8 that
//! systemd started has no session. Every row that asks failed, every time:
//! `Access denied as the requested operation requires interactive
//! authentication` (`tests/rescue-rollback-vm.nix` found it). So the console
//! calls systemd itself, and the agent registered for its process
//! ([`crate::action::agent`]) is the one polkit finds.
//!
//! # Asking, or not
//!
//! A row of the password tier ASKS: the call carries the flag that allows
//! polkit to ask, and the console has handed its terminal to the agent first
//! ([`crate::tui`]). A free row never asks: its refusal comes back at once,
//! with its name, instead of a question over the drawn screen.
//!
//! # As `systemctl start` waits
//!
//! A job is queued and the call returns; the row waits for the job's end
//! (`JobRemoved`), as `systemctl` did, so `done` means the oneshot ran and
//! ended well. A job that did not end well is told with what systemd and the
//! unit itself said of it ([`why`]): the job's result, the service's, and the
//! last lines the run wrote — the whole reason, not a line pointing at it.

use std::process::Command;

use zbus::blocking::{Connection, Proxy};
use zbus::proxy::MethodFlags;
use zbus::zvariant::OwnedObjectPath;

/// systemd, on the system bus.
const SERVICE: &str = "org.freedesktop.systemd1";
/// Its manager's object.
const PATH: &str = "/org/freedesktop/systemd1";
/// Its manager's interface.
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
/// A unit's interface, and a service's.
const UNIT: &str = "org.freedesktop.systemd1.Unit";
const SERVICE_UNIT: &str = "org.freedesktop.systemd1.Service";

/// `journalctl`, by store path: the run's own last lines.
const JOURNALCTL: Option<&str> = option_env!("RONOCTUA_JOURNALCTL");

/// How many of the run's last lines a refusal carries.
const LAST_LINES: usize = 12;

/// The manager's method for a row's verb.
fn method_of(verb: &str) -> Result<&'static str, String> {
    match verb {
        "start" => Ok("StartUnit"),
        "restart" => Ok("RestartUnit"),
        other => Err(format!("{other}?")),
    }
}

/// `verb` on each of `units`, in order, each waited for; the whole reason
/// when one did not end well. `asking`: polkit may ask the person.
///
/// # Errors
///
/// The bus, systemd's refusal (polkit's answer among them), or a job that
/// did not end `done`, said whole.
pub fn units(verb: &str, units: &[&str], asking: bool) -> Result<(), String> {
    let method = method_of(verb)?;
    let connection = Connection::system().map_err(|e| e.to_string())?;
    let manager = Proxy::new(&connection, SERVICE, PATH, MANAGER).map_err(|e| e.to_string())?;
    // THE SIGNALS FIRST: a job can end before its call's reply is read, and a
    // signal that came before the match was added is not delivered at all.
    manager
        .call::<_, _, ()>("Subscribe", &())
        .map_err(|e| e.to_string())?;
    let mut removed = manager
        .receive_signal("JobRemoved")
        .map_err(|e| e.to_string())?;
    for unit in units {
        let body = (*unit, "replace");
        let job: OwnedObjectPath = if asking {
            manager
                .call_with_flags(method, MethodFlags::AllowInteractiveAuth.into(), &body)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("{unit}: {method}"))?
        } else {
            manager.call(method, &body).map_err(|e| e.to_string())?
        };
        let result = loop {
            let Some(signal) = removed.next() else {
                return Err(format!("{unit}: {method}"));
            };
            let Ok((_, path, _, result)) = signal
                .body()
                .deserialize::<(u32, OwnedObjectPath, String, String)>()
            else {
                continue;
            };
            if path == job {
                break result;
            }
        };
        if result != "done" {
            return Err(why(&connection, &manager, unit, &result));
        }
    }
    Ok(())
}

/// What systemd and the unit say of a job that ended `result`: the job's
/// result, the service's own (`exit-code`, `signal`…) with its status, and
/// the last lines the run wrote to the journal.
fn why(connection: &Connection, manager: &Proxy<'_>, unit: &str, result: &str) -> String {
    let mut said = vec![format!("{unit}: {result}")];
    let Ok(path) = manager.call::<_, _, OwnedObjectPath>("GetUnit", &(unit,)) else {
        return said.join("\n");
    };
    let property =
        |interface: &'static str| Proxy::new(connection, SERVICE, path.as_ref(), interface).ok();
    if let Some(service) = property(SERVICE_UNIT) {
        let outcome = service.get_property::<String>("Result").ok();
        let status = service.get_property::<i32>("ExecMainStatus").ok();
        if let Some(outcome) = outcome {
            said.push(match status {
                Some(status) => format!("{outcome} ({status})"),
                None => outcome,
            });
        }
    }
    let invocation = property(UNIT).and_then(|u| u.get_property::<Vec<u8>>("InvocationID").ok());
    if let (Some(id), Some(journalctl)) = (invocation, JOURNALCTL)
        && !id.is_empty()
    {
        let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
        let lines = Command::new(journalctl)
            .args(["--no-pager", "--quiet", "-o", "cat"])
            .arg(format!("_SYSTEMD_INVOCATION_ID={hex}"))
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default();
        let lines: Vec<&str> = lines.lines().filter(|l| !l.trim().is_empty()).collect();
        said.extend(
            lines[lines.len().saturating_sub(LAST_LINES)..]
                .iter()
                .map(|l| (*l).to_owned()),
        );
    }
    said.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Run, all_of};

    #[test]
    fn every_system_row_has_a_method() {
        // A verb the manager has no method for is a row that fails in front
        // of the person, the first time anyone presses it.
        for a in all_of() {
            if let Run::System { verb, .. } = a.run {
                assert!(method_of(verb).is_ok(), "`{}` asks `{verb}`", a.id);
            }
        }
        assert!(method_of("stop").is_err());
    }
}
