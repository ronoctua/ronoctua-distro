//! The daemon: every keyboard unit grabbed, remapped and re-emitted by a twin.
//!
//! One thread and one `poll` over everything that can wake it — the listening
//! socket, each client, each keyboard, and one inotify descriptor that watches
//! `/dev/input` (a keyboard plugged or pulled) and the config directory (the
//! screen changed a rule). Nothing here sleeps on a timer beyond the poll's
//! own ceiling, which catches the one race inotify cannot: a node created
//! before `/proc/bus/input/devices` lists it.

use std::collections::BTreeMap;
use std::io::{BufRead as _, Write as _};
use std::os::fd::AsFd as _;
use std::os::unix::net::{UnixListener, UnixStream};

use anyhow::{Context as _, Result};
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::fs::inotify;

use crate::config;
use crate::device::{Grabbed, Twin};
use crate::engine::{Engine, Out};
use crate::wire::{self, FromDaemon, Held, ToDaemon};

/// How long a poll waits with nothing happening — the ceiling on noticing a
/// keyboard inotify told about before the kernel listed it.
const CEILING: Timespec = Timespec {
    tv_sec: 2,
    tv_nsec: 0,
};

/// One unit, held.
struct Unit {
    unit: ronoctua_keyboards::Unit,
    grabbed: Grabbed,
    twin: Twin,
    engine: Engine,
    diverted: bool,
}

/// One connected footer.
struct Client {
    stream: std::io::BufReader<UnixStream>,
}

/// Runs until killed.
///
/// # Errors
///
/// The socket or the inotify descriptor could not be made.
pub fn run(directory: &str, user: Option<&str>) -> Result<()> {
    let _ = std::fs::remove_file(wire::SOCKET);
    let listener = UnixListener::bind(wire::SOCKET)
        .with_context(|| format!("listening on {}", wire::SOCKET))?;
    listener.set_nonblocking(true)?;
    // THE SESSION'S USER, AND ONLY THEM: diverted keys cross this socket.
    rustix::fs::chmod(wire::SOCKET, rustix::fs::Mode::from_raw_mode(0o600))?;
    if let Some(uid) = user.and_then(uid_of) {
        rustix::fs::chown(wire::SOCKET, Some(rustix::fs::Uid::from_raw(uid)), None)?;
    }

    let watch = inotify::init(inotify::CreateFlags::NONBLOCK | inotify::CreateFlags::CLOEXEC)?;
    inotify::add_watch(
        &watch,
        "/dev/input",
        inotify::WatchFlags::CREATE | inotify::WatchFlags::DELETE | inotify::WatchFlags::ATTRIB,
    )?;
    if let Err(e) = inotify::add_watch(
        &watch,
        directory,
        inotify::WatchFlags::CLOSE_WRITE | inotify::WatchFlags::MOVED_TO,
    ) {
        tracing::warn!(error = %e, directory, "the config directory is not watched; no remaps will reload");
    }

    let mut units: BTreeMap<String, Unit> = BTreeMap::new();
    let mut clients: Vec<Client> = Vec::new();
    let mut rules = config::read(directory);
    rescan(&mut units, &rules);

    loop {
        let identities: Vec<String> = units.keys().cloned().collect();
        let quiet;
        let (ready_listener, ready_watch, ready_clients, ready_units) = {
            let mut fds = vec![
                PollFd::new(&listener, PollFlags::IN),
                PollFd::new(&watch, PollFlags::IN),
            ];
            for c in &clients {
                fds.push(PollFd::from_borrowed_fd(
                    c.stream.get_ref().as_fd(),
                    PollFlags::IN,
                ));
            }
            for id in &identities {
                fds.push(PollFd::from_borrowed_fd(
                    units[id].grabbed.fd(),
                    PollFlags::IN,
                ));
            }
            if let Err(e) = rustix::event::poll(&mut fds, Some(&CEILING))
                && e != rustix::io::Errno::INTR
            {
                return Err(e).context("polling");
            }
            let ready = |f: &PollFd<'_>| !f.revents().is_empty();
            quiet = !fds.iter().any(ready);
            let c = clients.len();
            (
                ready(&fds[0]),
                ready(&fds[1]),
                fds[2..2 + c].iter().map(ready).collect::<Vec<_>>(),
                fds[2 + c..].iter().map(ready).collect::<Vec<_>>(),
            )
        };

        // KEYS FIRST: a keyboard waiting behind a rescan is typing that lags.
        let mut gone = Vec::new();
        for (id, ready) in identities.iter().zip(ready_units) {
            if !ready {
                continue;
            }
            let Some(u) = units.get_mut(id) else {
                continue;
            };
            match u.grabbed.keys() {
                Ok(keys) => {
                    for (code, value) in keys {
                        for out in u.engine.feed(code, value) {
                            match out {
                                Out::Leader => {
                                    broadcast(
                                        &mut clients,
                                        &FromDaemon::Leader { unit: id.clone() },
                                    );
                                }
                                Out::Key { code, value } if u.diverted => broadcast(
                                    &mut clients,
                                    &FromDaemon::Key {
                                        unit: id.clone(),
                                        code,
                                        value,
                                    },
                                ),
                                Out::Key { code, value } => {
                                    if let Err(e) = u.twin.key(code, value) {
                                        tracing::warn!(error = %e, "a twin refused a key");
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => gone.push(id.clone()),
            }
        }
        let mut changed = !gone.is_empty();
        for id in gone {
            units.remove(&id);
        }

        if ready_listener {
            while let Ok((stream, _)) = listener.accept() {
                let _ = stream.set_nonblocking(true);
                let mut client = Client {
                    stream: std::io::BufReader::new(stream),
                };
                let said = units_message(&units);
                if send(&mut client, &said) {
                    clients.push(client);
                }
            }
        }

        let mut dropped = Vec::new();
        for (i, ready) in ready_clients.into_iter().enumerate() {
            if !ready {
                continue;
            }
            let mut line = String::new();
            loop {
                line.clear();
                match clients[i].stream.read_line(&mut line) {
                    Ok(0) => {
                        dropped.push(i);
                        break;
                    }
                    Ok(_) => obey(&mut units, line.trim()),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        dropped.push(i);
                        break;
                    }
                }
            }
        }
        for i in dropped.into_iter().rev() {
            clients.remove(i);
        }
        // A FOOTER THAT WENT AWAY HOLDS NOTHING: with nobody to send them to,
        // a diverted keyboard would type into the void until the next boot.
        if clients.is_empty() {
            for u in units.values_mut() {
                u.diverted = false;
            }
        }

        if ready_watch {
            let mut buf = [0u8; 4096];
            while rustix::io::read(&watch, &mut buf).is_ok_and(|n| n > 0) {}
            let fresh = config::read(directory);
            if fresh != rules {
                rules = fresh;
                for u in units.values_mut() {
                    release_all(u);
                    let r = rules.rules_of(&u.unit);
                    u.engine = Engine::new(
                        &r.leader,
                        r.keys.iter().map(|(k, v)| (k.as_str(), v.as_str())),
                    );
                }
            }
            changed |= rescan(&mut units, &rules);
        } else if quiet {
            // THE CEILING: nothing woke us, so look once for a node inotify
            // announced before the kernel's list had it.
            changed |= rescan(&mut units, &rules);
        }

        if changed {
            broadcast(&mut clients, &units_message(&units));
        }
    }
}

/// Grabs every unit not held yet, drops the ones gone. `true` when anything
/// changed.
fn rescan(units: &mut BTreeMap<String, Unit>, rules: &config::Config) -> bool {
    let Ok(seen) = ronoctua_keyboards::units() else {
        return false;
    };
    let mut changed = false;
    let present: Vec<String> = seen
        .iter()
        .map(ronoctua_keyboards::Unit::identity)
        .collect();
    units.retain(|id, _| {
        let keep = present.contains(id);
        changed |= !keep;
        keep
    });
    for unit in seen {
        let id = unit.identity();
        if units.contains_key(&id) {
            continue;
        }
        // THE TWIN FIRST, THEN THE GRAB: a grab with no twin is a keyboard
        // that types nowhere.
        let twin = match Twin::make(&wire::twin_name(&id)) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(error = %e, name = unit.name, "no twin; the keyboard is left alone");
                continue;
            }
        };
        let grabbed = match Grabbed::take(&unit.event) {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, name = unit.name, "not grabbed; the keyboard is left alone");
                continue;
            }
        };
        let r = rules.rules_of(&unit);
        let engine = Engine::new(
            &r.leader,
            r.keys.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        );
        tracing::info!(name = unit.name, identity = id, "holding a keyboard");
        units.insert(
            id,
            Unit {
                unit,
                grabbed,
                twin,
                engine,
                diverted: false,
            },
        );
        changed = true;
    }
    changed
}

/// Releases every key down on a unit's twin — before its rules change, and
/// before it is diverted, so nothing stays pressed in the compositor.
fn release_all(u: &mut Unit) {
    for out in u.engine.let_go() {
        if let Out::Key { code, value } = out {
            let _ = u.twin.key(code, value);
        }
    }
}

/// One line from a footer.
fn obey(units: &mut BTreeMap<String, Unit>, line: &str) {
    let Ok(ask) = serde_json::from_str::<ToDaemon>(line) else {
        return;
    };
    match ask {
        ToDaemon::Divert { unit } => {
            if let Some(u) = units.get_mut(&unit) {
                release_all(u);
                u.diverted = true;
            }
        }
        ToDaemon::Release { unit } => {
            if let Some(u) = units.get_mut(&unit) {
                u.engine.let_go();
                u.diverted = false;
            }
        }
    }
}

fn units_message(units: &BTreeMap<String, Unit>) -> FromDaemon {
    FromDaemon::Units {
        units: units
            .iter()
            .map(|(id, u)| Held {
                identity: id.clone(),
                name: u.unit.name.clone(),
                model: u.unit.id.clone(),
            })
            .collect(),
    }
}

/// `false` when the client is gone.
fn send(client: &mut Client, said: &FromDaemon) -> bool {
    let Ok(mut line) = serde_json::to_string(said) else {
        return true;
    };
    line.push('\n');
    client.stream.get_mut().write_all(line.as_bytes()).is_ok()
}

fn broadcast(clients: &mut Vec<Client>, said: &FromDaemon) {
    clients.retain_mut(|c| send(c, said));
}

/// A user's id, from `/etc/passwd` — the one lookup a root daemon makes.
fn uid_of(user: &str) -> Option<u32> {
    std::fs::read_to_string("/etc/passwd")
        .ok()?
        .lines()
        .find_map(|l| {
            let mut f = l.split(':');
            (f.next()? == user).then(|| f.nth(1)?.parse().ok())?
        })
}
