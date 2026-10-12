//! `verify`: every declared entry tried in a `nix` of its own, and what
//! failed written to `local/held.json` — the decisions are
//! [`ronoctua_person::verify`]'s; this runs the commands.

use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use ronoctua_person::declared::Declared;
use ronoctua_person::local::{Held, HeldWhy};
use ronoctua_person::verify::{self as decide, Fetch, Tried};
use ronoctua_person::{Stamp, names};
use rustix::fs::{AtFlags, Mode, OFlags};
use rustix::io::Errno;

use crate::apply;
use crate::disk::{self, Whose};

/// The `nix` this program runs, by store path (`RONOCTUA_NIX`): the
/// machine's own, which is the one that fills its store.
#[derive(Debug, Clone)]
pub struct Nix {
    /// Its path.
    pub program: PathBuf,
}

impl Nix {
    /// Runs `nix <args>`: its standard output when it succeeded. What it
    /// says on standard error goes to the journal as it is — said, never
    /// parsed.
    pub fn answer(&self, args: &[String]) -> Option<Vec<u8>> {
        let out = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .ok()?;
        out.status.success().then_some(out.stdout)
    }
}

/// The most a file of `local/` may weigh.
const LOCAL_MOST_BYTES: u64 = 1 << 20;

/// The `local/` file `name` of the machine directory, if it is there.
pub fn read_local(machine: &OwnedFd, name: &str) -> Result<Option<Vec<u8>>, String> {
    let me = Whose {
        owner: rustix::process::geteuid().as_raw(),
    };
    let (folder, file) = name.split_once('/').unwrap_or(("", name));
    let local = match rustix::fs::openat(
        machine,
        folder,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(None),
        Err(e) => return Err(format!("{folder}: {e}")),
    };
    let c_file = disk::c_name(file).map_err(|r| r.to_string())?;
    let was = match rustix::fs::statat(&local, c_file.as_c_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) => st,
        Err(Errno::NOENT) => return Ok(None),
        Err(e) => return Err(format!("{name}: {e}")),
    };
    disk::read_at(&local, &c_file, &was, me, LOCAL_MOST_BYTES, name)
        .map(Some)
        .map_err(|r| r.to_string())
}

/// Writes the `local/` file `name` of the machine directory, whole.
pub fn write_local(machine: &OwnedFd, name: &str, bytes: &[u8]) -> Result<(), String> {
    let (folder, file) = name.split_once('/').unwrap_or(("", name));
    let local = disk::dir_in(machine, folder, 0o755).map_err(|r| r.to_string())?;
    disk::write_whole(&local, file, bytes, 0o644).map_err(|r| r.to_string())
}

/// What verify says of one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// It evaluated and built.
    Ok,
    /// It failed now, for this reason.
    Held(HeldWhy),
    /// A rehearsal: it evaluated, its build was not tried, and `held.json`
    /// holds it at the fingerprint it still has — the rebuild will skip it.
    StillHeld(HeldWhy),
    /// A rehearsal: it evaluated, and its build was not tried.
    Untried,
}

/// What one round of verify found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Round {
    /// Every entry tried, and what was said of it.
    pub tried: Vec<(String, Said)>,
    /// The held entries after the round — in a rehearsal, what the rebuild
    /// would skip: what does not evaluate now, and what `held.json` still
    /// holds that only a build could let go.
    pub held: Held,
}

/// Tries one entry: its evaluation, then (unless rehearsing) its build, then
/// — when the build failed and it has a pin — the pin's fetch.
fn try_entry(nix: &Nix, flake: &str, id: &str, pin: Option<(&str, &str)>, rehearse: bool) -> Tried {
    if nix.answer(&decide::eval_args(flake, id)).is_none() {
        return Tried::EvalFailed;
    }
    if rehearse || nix.answer(&decide::build_args(flake, id)).is_some() {
        return Tried::Built;
    }
    let fetch = pin.and_then(|(url, hash)| {
        let args = decide::prefetch_args(url, hash)?;
        let got = nix.answer(&args).and_then(|out| decide::prefetched(&out));
        Some(Fetch::of(got.as_deref(), hash))
    });
    Tried::BuildFailed { fetch }
}

/// Verifies every entry of the machine flake at `machine`, and — unless
/// rehearsing — writes `local/held.json`. The caller holds root's lock
/// (`ronoctua_person::root::Lock`): the update, through the rebuild that
/// reads what this decides.
///
/// # Errors
///
/// A sentence for the journal when the machine directory cannot be opened,
/// or the flake does not evaluate at all (the baseline): then nothing is
/// held and `held.json` stays as it was.
pub fn verify(machine: &str, nix: &Nix, rehearse: bool, now: &Stamp) -> Result<Round, String> {
    let machine_fd = apply::open_machine(machine).map_err(|r| r.to_string())?;
    let flake = decide::flake(machine);

    // THE BASELINE. A flake that does not evaluate is nobody's entry's fault.
    let fingerprints = nix
        .answer(&decide::fingerprints_args(&flake))
        .ok_or_else(|| {
            format!(
                "the machine flake's {} does not evaluate; no entry is held for it",
                decide::FINGERPRINTS
            )
        })
        .and_then(|out| decide::fingerprints_of(&out))?;

    // THE PINS, from the machine's own copy of the declarations.
    let current = apply::current(&machine_fd).map_err(|r| r.to_string())?;
    let declared: Declared = match current.files.get(names::DECLARED) {
        None => Declared::default(),
        Some(bytes) => ronoctua_person::parse(bytes).unwrap_or_else(|r| {
            eprintln!("<4>the machine's declared.json was refused ({r}); no pin is fetched");
            Declared::default()
        }),
    };
    let before: Held = match read_local(&machine_fd, names::HELD)? {
        None => Held::default(),
        Some(bytes) => ronoctua_person::parse(&bytes).unwrap_or_else(|r| {
            eprintln!("<4>held.json was refused ({r}); every entry starts afresh");
            Held::default()
        }),
    };

    let mut tried = Vec::new();
    let mut failed = Vec::new();
    let mut evaluated = Vec::new();
    for (id, fingerprint) in &fingerprints {
        let pin = declared.entry(id).and_then(decide::pin_of);
        let said = match decide::reason(try_entry(nix, &flake, id, pin, rehearse)) {
            Some(why) => {
                failed.push((id.clone(), fingerprint.clone(), why));
                Said::Held(why)
            }
            None if rehearse => {
                evaluated.push(id.clone());
                Said::Untried
            }
            None => Said::Ok,
        };
        tried.push((id.clone(), said));
    }
    // A REHEARSAL builds nothing, so it cannot let go of what only a build
    // could: those stay held, as the rebuild will see them.
    if rehearse {
        for (id, fingerprint, why) in decide::still_held(&before, &fingerprints, &evaluated) {
            if let Some((_, said)) = tried.iter_mut().find(|(t, _)| *t == id) {
                *said = Said::StillHeld(why);
            }
            failed.push((id, fingerprint, why));
        }
    }
    let held = decide::held_after(&before, &failed, now);
    if !rehearse {
        write_local(
            &machine_fd,
            names::HELD,
            ronoctua_person::to_json(&held).as_bytes(),
        )?;
    }
    Ok(Round { tried, held })
}

/// The lines verify says on its standard output, one per entry: `ok <id>`
/// or `held <id> <reason>`; a rehearsal, which builds nothing, says
/// `untried <id>` of what evaluated, and `still-held <id> <reason>` of what
/// `held.json` goes on holding.
pub fn lines(round: &Round) -> Vec<String> {
    round
        .tried
        .iter()
        .map(|(id, said)| match said {
            Said::Ok => format!("ok {id}"),
            Said::Held(why) => format!("held {id} {}", why.id()),
            Said::StillHeld(why) => format!("still-held {id} {}", why.id()),
            Said::Untried => format!("untried {id}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::testing::Scratch;
    use ronoctua_person::local::HeldEntry;

    /// The fingerprints by id, as the fake `nix` prints them.
    fn fingerprints_json(ids: &[&str]) -> String {
        let map: BTreeMap<&str, String> = ids
            .iter()
            .map(|&id| (id, format!("fp-{}", id.len())))
            .collect();
        serde_json::to_string(&map).unwrap()
    }

    /// A `nix` that answers like the real one for a fixture flake: the
    /// fingerprints, `eval` and `build` by the entry's name, and the
    /// prefetch of two addresses — the answers' SHAPES are the real ones
    /// (`nix store prefetch-file --json` prints `{"hash", "storePath"}`).
    fn fake_nix(s: &Scratch, baseline_ok: bool) -> Nix {
        let fingerprints = fingerprints_json(&[
            "nix:good",
            "nix:eval-broken",
            "nix:build-broken",
            "appimage:gone",
            "appimage:changed",
            "appimage:same",
        ]);
        let baseline = if baseline_ok {
            format!("echo '{fingerprints}'")
        } else {
            "exit 1".to_owned()
        };
        let pin = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        let script = format!(
            r#"#!/bin/sh
echo "$*" >> "{log}"
case "$*" in
  *personFingerprints*) {baseline} ;;
  eval*eval-broken*) exit 1 ;;
  eval*) echo /nix/store/x.drv ;;
  build*good*) exit 0 ;;
  build*) exit 1 ;;
  *prefetch-file*gone*) exit 1 ;;
  *prefetch-file*changed*) echo '{{"hash":"sha256-mOpuTyFvL7S2n/+bOkSELDhobKaF8/VdxIxdP7EQe+Q=","storePath":"/nix/store/y"}}' ;;
  *prefetch-file*same*) echo '{{"hash":"{pin}","storePath":"/nix/store/z"}}' ;;
  *) exit 2 ;;
esac
"#,
            log = s.0.join("nix.log").display(),
        );
        Nix {
            program: s.program("nix", &script),
        }
    }

    fn declared_with_pins() -> String {
        let entry = |key: &str| {
            format!(
                r#"{{"source": "appimage", "id": "appimage:{key}", "machines": null, "edited-at": "2026-10-05T12:00:00Z", "edited-on": "noctua", "key": "{key}", "name": "X", "pin": {{"url": "https://example.org/{key}.AppImage", "hash": "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=", "version": "1", "pinned-at": "2026-10-05T12:00:00Z"}}, "follows": null, "icon": null}}"#
            )
        };
        format!(
            r#"{{"format": 1, "apps": [{}, {}, {}]}}"#,
            entry("gone"),
            entry("changed"),
            entry("same")
        )
    }

    fn now() -> Stamp {
        Stamp::parse("2026-10-05T12:00:00Z").unwrap()
    }

    /// FAILS IF: a good entry is held, a broken one is not, or the four
    /// reasons are confused — eval, build, a download that is gone and one
    /// that changed upstream.
    #[test]
    fn only_the_broken_entries_are_held_and_each_for_its_reason() {
        let s = Scratch::new("verify-round");
        s.file(
            "machine/person/software/declared.json",
            declared_with_pins().as_bytes(),
        );
        let nix = fake_nix(&s, true);
        let machine = s.0.join("machine");
        let round = verify(machine.to_str().unwrap(), &nix, false, &now()).unwrap();
        let by_id: BTreeMap<_, _> = round.tried.iter().cloned().collect();
        assert_eq!(by_id["nix:good"], Said::Ok);
        assert_eq!(by_id["nix:eval-broken"], Said::Held(HeldWhy::EvalFailed));
        assert_eq!(by_id["nix:build-broken"], Said::Held(HeldWhy::BuildFailed));
        assert_eq!(by_id["appimage:gone"], Said::Held(HeldWhy::FetchFailed));
        assert_eq!(
            by_id["appimage:changed"],
            Said::Held(HeldWhy::UpstreamChanged)
        );
        assert_eq!(by_id["appimage:same"], Said::Held(HeldWhy::BuildFailed));
        let written: Held =
            ronoctua_person::parse(&std::fs::read(machine.join("local/held.json")).unwrap())
                .unwrap();
        assert_eq!(written, round.held);
        assert_eq!(written.held.len(), 5);
        assert!(!written.held.iter().any(|h| h.id == "nix:good"));
        let gone = written
            .held
            .iter()
            .find(|h| h.id == "appimage:gone")
            .unwrap();
        assert_eq!(gone.fingerprint, "fp-13", "the fingerprint nix gave");
        // One process per question: the eval of a broken entry never ran
        // in the same `nix` as another's.
        let log = std::fs::read_to_string(s.0.join("nix.log")).unwrap();
        assert_eq!(
            log.lines().filter(|l| l.starts_with("eval --raw")).count(),
            6
        );
        assert!(
            log.lines()
                .all(|l| !l.contains("prefetch-file") || l.contains("--hash-type sha256"))
        );
    }

    /// FAILS IF: a machine flake that does not evaluate at all gets every
    /// entry held, or rewrites `held.json`.
    #[test]
    fn a_flake_that_does_not_evaluate_holds_nothing() {
        let s = Scratch::new("verify-baseline");
        s.file("machine/local/held.json", b"kept as it was");
        let nix = fake_nix(&s, false);
        let machine = s.0.join("machine");
        assert!(verify(machine.to_str().unwrap(), &nix, false, &now()).is_err());
        assert_eq!(
            std::fs::read(machine.join("local/held.json")).unwrap(),
            b"kept as it was"
        );
    }

    /// FAILS IF: a rehearsal builds or writes anything, says `ok` of an
    /// entry it never built, or lets go of one `held.json` still holds —
    /// which the rebuild will go on skipping.
    #[test]
    fn a_rehearsal_builds_nothing_writes_nothing_and_says_what_stays_held() {
        let s = Scratch::new("verify-rehearse");
        let gone = HeldEntry {
            id: "appimage:gone".to_owned(),
            fingerprint: "fp-13".to_owned(),
            reason: HeldWhy::FetchFailed,
            since: Stamp::parse("2026-10-01T00:00:00Z").unwrap(),
        };
        let held_before = Held {
            format: ronoctua_person::FORMAT,
            held: vec![gone.clone()],
        };
        let before = ronoctua_person::to_json(&held_before);
        s.file("machine/local/held.json", before.as_bytes());
        let nix = fake_nix(&s, true);
        let machine = s.0.join("machine");
        let round = verify(machine.to_str().unwrap(), &nix, true, &now()).unwrap();
        assert_eq!(
            std::fs::read_to_string(machine.join("local/held.json")).unwrap(),
            before,
            "nothing written"
        );
        let log = std::fs::read_to_string(s.0.join("nix.log")).unwrap();
        assert!(!log.lines().any(|l| l.starts_with("build")), "{log}");
        let said = lines(&round);
        assert!(said.contains(&"held nix:eval-broken eval-failed".to_owned()));
        assert!(said.contains(&"still-held appimage:gone fetch-failed".to_owned()));
        assert!(said.contains(&"untried nix:good".to_owned()));
        assert!(!said.iter().any(|l| l.starts_with("ok ")), "{said:?}");
        let ids: Vec<_> = round.held.held.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["appimage:gone", "nix:eval-broken"]);
        assert_eq!(round.held.held[0].since, gone.since);
    }
}
