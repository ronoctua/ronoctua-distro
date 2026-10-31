//! The push — the update's last step, as the person, after an update that
//! built the machine a new system.
//!
//! # When
//!
//! Only when the machine names a cache ([`crate::SYSTEM_FILE`];
//! [`Why::NoCache`] otherwise), only when the machine's record says an
//! apply SET UP A SYSTEM since the last push — switched to it, or set it for
//! the next boot ([`Why::NothingNew`] otherwise: a failed update publishes
//! nothing, and an update that changed nothing pushes nothing again), only
//! with a token ([`Why::NoToken`]) and only with every host it reaches let
//! by the person ([`Why::NotLet`]). It never asks: the four are news the
//! update records, and the screen says them.
//!
//! # Which system
//!
//! The one the last apply set up. Switched to, it is the one that runs, and
//! its `/etc` says the cache. SET FOR THE NEXT BOOT (`staged`, the update's
//! way since the machine rolls on `nixos-unstable`), it does not run yet:
//! the record names it ([`staged_system`]), and the cache, its visibility
//! and its public roots are read from ITS file
//! (`<toplevel>/etc/ronoctua/binary-cache.json`), and a private cache gets
//! ITS closure — never the running system's, which the last push already
//! carried, nor the running `/etc`, which may name another cache.
//!
//! The LAST APPLY is the right record here, and not whether a system waits
//! for the restart (what the update's page and the software centre ask
//! `ronoctua-machine`): a push follows one apply, once, as the update's last
//! step right after it — nothing can fail its trial or roll it back between
//! the two —, and an apply that built nothing has nothing new to push, even
//! while an earlier system still waits (its own run pushed it).
//!
//! # What
//!
//! A PUBLIC cache: the distro's own outputs this machine has valid in its
//! store ([`crate::system::System::public_roots`]) and their closure, with
//! the downloads, the unfree programs and what refers to them taken out
//! ([`crate::closure::public`]). A PRIVATE cache: the system's closure
//! ([`crate::closure::private`]). And of either, only what
//! `cache.nixos.org` does not serve — asked of it, by `nix-store
//! --check-validity` against it.
//!
//! # How
//!
//! `cachix push <name>`, the paths on its standard input, the token in its
//! environment alone (`CACHIX_AUTH_TOKEN`) — never an argument, never this
//! process's environment. What it prints is relayed with the token taken
//! out, should it ever say it.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use ronoctua_consent::keys::{Dir, Secret};
use ronoctua_consent::{NotLet, Pass};
use ronoctua_person::Stamp;
use ronoctua_person::options::Visibility;
use ronoctua_person::state::{ApplyResult, MachineState};

use crate::closure::{self, Closure, PushSet};
use crate::record::{Outcome, Record};
use crate::system::System;
use crate::{Why, hosts_of};

/// The variable naming `cachix`'s program (`nix/modules/binary-cache.nix`).
pub const CACHIX_VARIABLE: &str = "RONOCTUA_CACHIX";
/// The variable naming the machine's Nix `bin/` folder (`nix`, `nix-store`).
pub const NIX_VARIABLE: &str = "RONOCTUA_CACHIX_NIX";
/// The variable naming the machine's record (`state.json` of
/// `ronoctua-machine`'s folder).
pub const STATE_VARIABLE: &str = "RONOCTUA_CACHIX_MACHINE_STATE";

/// The running system, whose closure a private cache receives.
pub const CURRENT_SYSTEM: &str = "/run/current-system";

/// The programs a push runs, by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    /// `nix`.
    pub nix: PathBuf,
    /// `nix-store`.
    pub nix_store: PathBuf,
    /// `cachix`.
    pub cachix: PathBuf,
}

impl Tools {
    /// From the environment the wrapper gave this program.
    ///
    /// # Errors
    ///
    /// [`Why::NoTools`] when a variable is missing or not absolute.
    pub fn from_env() -> Result<Self, Why> {
        let absolute = |name: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .ok_or(Why::NoTools)
        };
        let nix = absolute(NIX_VARIABLE)?;
        Ok(Self {
            nix: nix.join("nix"),
            nix_store: nix.join("nix-store"),
            cachix: absolute(CACHIX_VARIABLE)?,
        })
    }
}

/// Where a push reads and writes.
#[derive(Debug, Clone)]
pub struct Places {
    /// What the machine was built with ([`crate::SYSTEM_FILE`]).
    pub system: PathBuf,
    /// The machine's record.
    pub machine_state: PathBuf,
    /// The running system.
    pub current_system: PathBuf,
    /// Where a store path is on the disk: `/` — a test's own folder in a
    /// test —, so a system set for the next boot is read where it is.
    pub root: PathBuf,
    /// The token's folder.
    pub keys: Option<Dir>,
    /// The record of pushes — `None` with no home: nothing is kept.
    pub record: Option<PathBuf>,
}

impl Places {
    /// What the system `toplevel` was built with: its own
    /// [`crate::SYSTEM_FILE`], under [`Places::root`].
    #[must_use]
    pub fn system_file_of(&self, toplevel: &str) -> PathBuf {
        self.root
            .join(toplevel.trim_start_matches('/'))
            .join(crate::SYSTEM_FILE.trim_start_matches('/'))
    }

    /// This machine's and this person's.
    ///
    /// # Errors
    ///
    /// [`Why::NoTools`] when the wrapper did not say where the machine's
    /// record is.
    pub fn here() -> Result<Self, Why> {
        let machine_state = std::env::var_os(STATE_VARIABLE)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(Why::NoTools)?;
        Ok(Self {
            system: PathBuf::from(crate::SYSTEM_FILE),
            machine_state,
            current_system: PathBuf::from(CURRENT_SYSTEM),
            root: PathBuf::from("/"),
            keys: crate::token::dir(),
            record: crate::record::path(),
        })
    }
}

/// The passes a push holds, one per host it reaches.
#[derive(Debug, Clone)]
pub struct Passes {
    /// `cachix.org`.
    pub api: Pass,
    /// `<name>.cachix.org`.
    pub cache: Pass,
    /// `cache.nixos.org`.
    pub upstream: Pass,
}

impl Passes {
    /// Every pass a push to `name` needs, minted by `mint` (the person's
    /// `consent.json`: `ronoctua_consent::pass`).
    ///
    /// # Errors
    ///
    /// [`Why::NotLet`] when one host is not let.
    pub fn of(name: &str, mint: &dyn Fn(&str) -> Result<Pass, NotLet>) -> Result<Self, Why> {
        let [api, cache, upstream] = hosts_of(name).map(|h| mint(&h));
        Ok(Self {
            api: api.map_err(|_| Why::NotLet)?,
            cache: cache.map_err(|_| Why::NotLet)?,
            upstream: upstream.map_err(|_| Why::NotLet)?,
        })
    }
}

/// The apply a push would follow: the machine's last, when it set up a
/// system — switched to it, or set it for the next boot.
#[must_use]
pub fn switched_at(state: &MachineState) -> Option<&Stamp> {
    state.last_apply.as_ref().and_then(|a| {
        matches!(
            a.result,
            ApplyResult::Switched | ApplyResult::SwitchedWithFailures | ApplyResult::Staged
        )
        .then_some(&a.at)
    })
}

/// The system the machine's last apply set for the next boot, if it set one
/// — what a push carries, and reads its cache from, instead of the one that
/// runs.
#[must_use]
pub fn staged_system(state: &MachineState) -> Option<&str> {
    state
        .last_apply
        .as_ref()
        .filter(|a| a.result == ApplyResult::Staged)
        .and_then(|a| a.toplevel.as_deref())
        .filter(|t| crate::system::is_store_path(t))
}

/// The machine's record, if it reads, its last apply whole
/// ([`MachineState::completed`]).
fn machine_state(places: &Places) -> Option<MachineState> {
    std::fs::read(&places.machine_state)
        .ok()
        .and_then(|b| ronoctua_person::parse::<MachineState>(&b).ok())
        .map(|s| s.completed(&places.machine_state))
}

/// One push — or, `rehearse`, what one would carry, pushing nothing and
/// writing no record. `tools` is `None` when the environment did not say
/// where they are — which a machine with no cache never does, so it is asked
/// only once the push has something to do. `now` stamps the record; `say`
/// takes the progress lines (standard error, the update's journal). The
/// record is written (but for a rehearsal) and returned.
pub fn push(
    places: &Places,
    tools: Option<&Tools>,
    mint: &dyn Fn(&str) -> Result<Pass, NotLet>,
    rehearse: bool,
    now: Stamp,
    say: &mut dyn FnMut(String),
) -> Record {
    // THE SYSTEM THE LAST APPLY SET UP: one set for the next boot says its
    // cache in its own file; one that runs, in the running `/etc`.
    let state = machine_state(places);
    let staged = state.as_ref().and_then(staged_system).map(str::to_owned);
    let file = staged
        .as_deref()
        .map_or_else(|| places.system.clone(), |t| places.system_file_of(t));
    let system = System::read_at(&file).ok();
    let (cache, visibility) = system.as_ref().map_or((None, None), |s| {
        (Some(s.cache.name.clone()), Some(s.cache.visibility))
    });
    let ended = |outcome: Outcome, applied_at: Option<Stamp>| Record {
        at: now.clone(),
        cache: cache.clone(),
        visibility,
        applied_at,
        outcome,
    };
    let applied = state.as_ref().and_then(|s| switched_at(s).cloned());
    let done = system.ok_or(Why::NoCache).and_then(|s| {
        let of = Of {
            system: &s,
            applied: applied.as_ref(),
            staged: staged.as_deref(),
        };
        carried(places, tools, mint, rehearse, &of, say)
    });
    let record = match done {
        Ok(Carried::Rehearsed { paths, bytes }) => ended(Outcome::Rehearsed { paths, bytes }, None),
        Ok(Carried::Pushed {
            paths,
            bytes,
            applied,
        }) => ended(Outcome::Pushed { paths, bytes }, Some(applied)),
        Err(why) => ended(Outcome::Refused { why }, None),
    };
    if !rehearse && let Some(path) = places.record.as_ref() {
        let before = crate::record::read_at(path);
        if crate::record::write_at(path, &before.after(record.clone())).is_err() {
            say(format!(
                "the record of this push could not be written ({})",
                Why::Unwritable
            ));
        }
    }
    record
}

/// What a push carried.
enum Carried {
    Pushed {
        paths: u64,
        bytes: u64,
        applied: Stamp,
    },
    Rehearsed {
        paths: u64,
        bytes: u64,
    },
}

/// What a push is of: the system file it read, the apply it would follow
/// ([`switched_at`]) and the system that apply set for the next boot, if it
/// did ([`staged_system`]).
struct Of<'a> {
    system: &'a System,
    applied: Option<&'a Stamp>,
    staged: Option<&'a str>,
}

fn carried(
    places: &Places,
    tools: Option<&Tools>,
    mint: &dyn Fn(&str) -> Result<Pass, NotLet>,
    rehearse: bool,
    of: &Of<'_>,
    say: &mut dyn FnMut(String),
) -> Result<Carried, Why> {
    let system = of.system;
    let name = &system.cache.name;
    // AFTER AN UPDATE THAT SET UP A SYSTEM, once. A rehearsal follows no
    // apply.
    let applied = of.applied.cloned();
    if !rehearse {
        let Some(at) = applied.as_ref() else {
            return Err(Why::NothingNew);
        };
        let before = places
            .record
            .as_ref()
            .map(|p| crate::record::read_at(p))
            .unwrap_or_default();
        if before.followed().is_some_and(|f| f >= at) {
            return Err(Why::NothingNew);
        }
    }
    let token = places
        .keys
        .as_ref()
        .ok_or(Why::NoToken)
        .and_then(crate::token::read_from)?
        .ok_or(Why::NoToken)?;
    let passes = Passes::of(name, mint)?;
    let tools = tools.ok_or(Why::NoTools)?;
    let (closure, set) = match system.cache.visibility {
        Visibility::Public => {
            let roots = valid_roots(tools, &system.public_roots)?;
            if roots.is_empty() {
                say(format!(
                    "none of the distro's {} outputs is in this store: nothing goes to {name}",
                    system.public_roots.len()
                ));
                (Closure::new(), PushSet::default())
            } else {
                let closure = closure_of(tools, &roots)?;
                let set = closure::public(&closure, &system.unfree, system.android_unfree);
                (closure, set)
            }
        }
        Visibility::Private => {
            // THE SYSTEM SET FOR THE NEXT BOOT, when one waits; else THE
            // LINK'S TARGET, which is the running system's store path (a
            // link to a link is followed once more, never further).
            let current = match of.staged {
                Some(staged) => staged.to_owned(),
                None => std::fs::read_link(&places.current_system)
                    .map_err(|_| Why::StoreFailed)?
                    .to_string_lossy()
                    .into_owned(),
            };
            if !crate::system::is_store_path(&current) {
                return Err(Why::StoreFailed);
            }
            let closure = closure_of(tools, &[current])?;
            let set = closure::private(&closure);
            (closure, set)
        }
    };
    let going = not_upstream(tools, &passes.upstream, &set.pushable)?;
    let bytes = closure::bytes_of(&closure, &going);
    let paths = u64::try_from(going.len()).unwrap_or(u64::MAX);
    say(format!(
        "{paths} paths ({:.1} MiB) for {name}: {} left out as downloads, unfree programs or what refers to them, {} served by {}",
        mib(bytes),
        set.tainted.len(),
        set.pushable.len().saturating_sub(going.len()),
        crate::UPSTREAM_HOST
    ));
    if rehearse {
        return Ok(Carried::Rehearsed { paths, bytes });
    }
    let applied = applied.ok_or(Why::NothingNew)?;
    if !going.is_empty() {
        run_cachix(tools, &passes, name, &token, &going, say)?;
    }
    Ok(Carried::Pushed {
        paths,
        bytes,
        applied,
    })
}

#[allow(clippy::cast_precision_loss)]
fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

/// The roots this store has valid — `nix-store --check-validity
/// --print-invalid` names the others.
fn valid_roots(tools: &Tools, roots: &[String]) -> Result<Vec<String>, Why> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let out = Command::new(&tools.nix_store)
        .args(["--check-validity", "--print-invalid"])
        .args(roots)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| Why::NoTools)?;
    if !out.status.success() {
        return Err(Why::StoreFailed);
    }
    let invalid: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(roots
        .iter()
        .filter(|r| !invalid.contains(*r))
        .cloned()
        .collect())
}

/// The closure of `roots`, as `nix path-info` says it.
fn closure_of(tools: &Tools, roots: &[String]) -> Result<Closure, Why> {
    let out = Command::new(&tools.nix)
        .args([
            "--extra-experimental-features",
            "nix-command",
            "path-info",
            "--json",
            "--json-format",
            "1",
            "--recursive",
        ])
        .args(roots)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| Why::NoTools)?;
    if !out.status.success() {
        return Err(Why::StoreFailed);
    }
    closure::closure_of(&String::from_utf8_lossy(&out.stdout))
}

/// Which of `paths` `cache.nixos.org` does NOT serve, asked of it with the
/// pass for it.
fn not_upstream(tools: &Tools, pass: &Pass, paths: &[String]) -> Result<Vec<String>, Why> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let store = format!("https://{}", pass.host());
    let mut child = Command::new(&tools.nix_store)
        .args([
            "--store",
            &store,
            "--check-validity",
            "--print-invalid",
            "--stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Why::NoTools)?;
    let list = paths.join("\n") + "\n";
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(list.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| Why::StoreFailed)?;
    if !out.status.success() {
        return Err(Why::StoreFailed);
    }
    let asked: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| asked.contains(l))
        .map(str::to_owned)
        .collect())
}

/// `cachix push <name>`, the paths on its standard input and the token in
/// its environment alone; what it said relayed with the token taken out.
fn run_cachix(
    tools: &Tools,
    passes: &Passes,
    name: &str,
    token: &Secret,
    paths: &[String],
    say: &mut dyn FnMut(String),
) -> Result<(), Why> {
    // THE PASSES ARE THE CONSENT for the two hosts `cachix` reaches; it is a
    // program of somebody else's, so holding them is the proof, and the name
    // pushed to is the cache's own host's.
    if !passes.cache.covers(&format!("{name}.cachix.org")) || !passes.api.covers(crate::API_HOST) {
        return Err(Why::NotLet);
    }
    let mut child = Command::new(&tools.cachix)
        .arg("push")
        .arg(name)
        .env("CACHIX_AUTH_TOKEN", token.expose())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Why::NoTools)?;
    let list = paths.join("\n") + "\n";
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(list.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| Why::CachixFailed)?;
    for text in [&out.stdout, &out.stderr] {
        for line in String::from_utf8_lossy(text).lines() {
            say(scrubbed(line, token));
        }
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(Why::CachixFailed)
    }
}

/// `line` with every occurrence of the token taken out. Pure.
#[must_use]
pub fn scrubbed(line: &str, token: &Secret) -> String {
    if token.is_empty() {
        line.to_owned()
    } else {
        line.replace(token.expose(), "<token>")
    }
}

/// The line the program ends with, which the update reads (`refused
/// <word>` is how a refusal ends; three words of it are news to the step).
#[must_use]
pub fn last_line(record: &Record) -> String {
    match record.outcome {
        Outcome::Pushed { paths, .. } => format!("pushed {paths}"),
        Outcome::Rehearsed { paths, .. } => format!("would push {paths}"),
        Outcome::Refused { why } => format!("refused {}", why.id()),
    }
}

/// The folder a test's stand-ins live in, and the places beside them.
#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use std::path::Path;

    use ronoctua_consent::keys::Dir;

    use super::*;
    use crate::record::Pushes;

    const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJqdGkiOiIxIn0.c2lnbmF0dXJl_-";
    const KEY: &str = "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=";

    fn s(name: &str) -> String {
        format!(
            "/nix/store/{}-{name}",
            name.chars().next().unwrap().to_string().repeat(32)
        )
    }

    struct Bench {
        root: PathBuf,
        places: Places,
        tools: Tools,
        consent: ronoctua_store::Store<ronoctua_consent::Consents>,
    }

    fn script(path: &Path, body: &str) {
        std::fs::write(path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A machine with a cache of `visibility`, its record saying an apply
    /// switched at `applied` (seconds), and stand-ins for `nix`, `nix-store`
    /// and `cachix` that write down how they were called (`calls`) and
    /// what `cachix` was given (`pushed`, and its environment's token).
    fn bench(name: &str, visibility: &str, applied: Option<i64>) -> Bench {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-cachix-push-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("state")).unwrap();
        let shell = s("ronoctua-shell");
        let workspace = s("ronoctua-0.1.0");
        let glibc = s("glibc-2.40-66");
        let model = s("model.onnx");
        let chrome = s("google-chrome-141.0");
        let system = s("nixos-system-noctua-26.05");
        let etc = s("etc");
        std::fs::write(
            root.join("binary-cache.json"),
            format!(
                r#"{{"format": 1, "cache": {{"name": "someone", "public-key": "{KEY}", "visibility": "{visibility}"}},
                   "public-roots": ["{shell}", "{workspace}", "{gone}"], "unfree": ["google-chrome"], "android-unfree": false}}"#,
                gone = s("absent-1.0")
            ),
        )
        .unwrap();
        if let Some(at) = applied {
            std::fs::write(
                root.join("state.json"),
                format!(
                    r#"{{"format": 1, "last-good": null, "trial": null, "hold": null,
                       "last-apply": {{"at": "{}", "result": "switched"}}}}"#,
                    Stamp::from_unix_s(at)
                ),
            )
            .unwrap();
        }
        std::os::unix::fs::symlink(&system, root.join("current-system")).unwrap();
        let calls = root.join("calls");
        // `nix path-info --recursive`: the closure of what it was asked.
        let public = format!(
            r#"{{"{shell}": {{"ca": null, "references": ["{workspace}", "{model}"], "narSize": 10}},
                "{workspace}": {{"ca": null, "references": ["{glibc}"], "narSize": 100}},
                "{glibc}": {{"ca": null, "references": [], "narSize": 5}},
                "{model}": {{"ca": "fixed:r:sha256:00", "references": [], "narSize": 7}}}}"#
        );
        let private = format!(
            r#"{{"{system}": {{"ca": null, "references": ["{etc}", "{shell}", "{chrome}"], "narSize": 1}},
                "{etc}": {{"ca": null, "references": [], "narSize": 1}},
                "{chrome}": {{"ca": null, "references": ["{glibc}"], "narSize": 50}},
                "{shell}": {{"ca": null, "references": ["{glibc}"], "narSize": 10}},
                "{glibc}": {{"ca": null, "references": [], "narSize": 5}}}}"#
        );
        std::fs::write(root.join("public.json"), public).unwrap();
        std::fs::write(root.join("private.json"), private).unwrap();
        // A SYSTEM SET FOR THE NEXT BOOT: its own closure — the shell
        // again, and a newer system and `/etc`.
        let staged = s("nixos-system-noctua-26.11");
        let staged_etc = s("etc-26.11");
        std::fs::write(
            root.join("staged.json"),
            format!(
                r#"{{"{staged}": {{"ca": null, "references": ["{staged_etc}", "{shell}"], "narSize": 2}},
                    "{staged_etc}": {{"ca": null, "references": [], "narSize": 3}},
                    "{shell}": {{"ca": null, "references": ["{glibc}"], "narSize": 10}},
                    "{glibc}": {{"ca": null, "references": [], "narSize": 5}}}}"#
            ),
        )
        .unwrap();
        script(
            &root.join("bin/nix"),
            &format!(
                "echo \"nix $*\" >>{calls}\ncase \"$*\" in *{system}*) cat {r}/private.json ;; *{staged}*) cat {r}/staged.json ;; *) cat {r}/public.json ;; esac\n",
                calls = calls.display(),
                r = root.display()
            ),
        );
        // `nix-store`: locally, one root is not valid; upstream serves glibc.
        script(
            &root.join("bin/nix-store"),
            &format!(
                "echo \"nix-store $*\" >>{calls}\ncase \"$*\" in\n  *--store*) while read -r p; do [ \"$p\" = {glibc} ] || echo \"$p\"; done ;;\n  *) echo {gone} ;;\nesac\n",
                calls = calls.display(),
                gone = s("absent-1.0")
            ),
        );
        script(
            &root.join("bin/cachix"),
            &format!(
                "echo \"cachix $*\" >>{calls}\necho \"token=$CACHIX_AUTH_TOKEN\" >>{r}/env\ncat >{r}/pushed\necho \"pushing with $CACHIX_AUTH_TOKEN\"\nexit ${{CACHIX_EXIT:-0}}\n",
                calls = calls.display(),
                r = root.display()
            ),
        );
        let keys = Dir::at(root.join("keys"));
        let consent = ronoctua_consent::store_at(Some(root.join("config")));
        let places = Places {
            system: root.join("binary-cache.json"),
            machine_state: root.join("state.json"),
            current_system: root.join("current-system"),
            root: root.clone(),
            keys: Some(keys),
            record: Some(root.join("state").join(crate::record::FILE)),
        };
        let tools = Tools {
            nix: root.join("bin/nix"),
            nix_store: root.join("bin/nix-store"),
            cachix: root.join("bin/cachix"),
        };
        Bench {
            root,
            places,
            tools,
            consent,
        }
    }

    impl Bench {
        fn let_all(&self) {
            let hosts = hosts_of("someone");
            let names: Vec<&str> = hosts.iter().map(String::as_str).collect();
            self.consent
                .write(&ronoctua_consent::Consents::default().with(&names, true))
                .unwrap();
        }

        fn token(&self) {
            crate::token::keep(self.places.keys.as_ref().unwrap(), &Secret::new(TOKEN)).unwrap();
        }

        fn run(&self, rehearse: bool, now: i64) -> (Record, Vec<String>) {
            let mut said = Vec::new();
            let mint = |h: &str| ronoctua_consent::pass_in(&self.consent, h);
            let r = push(
                &self.places,
                Some(&self.tools),
                &mint,
                rehearse,
                Stamp::from_unix_s(now),
                &mut |l| said.push(l),
            );
            (r, said)
        }

        fn calls(&self) -> String {
            std::fs::read_to_string(self.root.join("calls")).unwrap_or_default()
        }

        fn pushed(&self) -> Vec<String> {
            std::fs::read_to_string(self.root.join("pushed"))
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        }
    }

    /// FAILS IF: a push goes without a cache, before an update switched the
    /// machine, without a token or without every host let — or one of
    /// those refusals reaches `cachix` at all.
    #[test]
    fn a_push_waits_for_a_switch_a_token_and_the_persons_consent() {
        let b = bench("gates", "public", None);
        assert_eq!(
            b.run(false, 100).0.outcome,
            Outcome::Refused {
                why: Why::NothingNew
            }
        );
        let b = bench("gates2", "public", Some(50));
        assert_eq!(
            b.run(false, 100).0.outcome,
            Outcome::Refused { why: Why::NoToken }
        );
        b.token();
        assert_eq!(
            b.run(false, 100).0.outcome,
            Outcome::Refused { why: Why::NotLet }
        );
        assert!(!b.calls().contains("cachix"), "{}", b.calls());
        std::fs::remove_file(&b.places.system).unwrap();
        let (r, _) = b.run(false, 100);
        assert_eq!(r.outcome, Outcome::Refused { why: Why::NoCache });
        assert_eq!(last_line(&r), "refused no-cache");
        // A MACHINE WITH NO CACHE IS GIVEN NO `cachix`: it says no cache,
        // never that a tool is missing.
        let mint = |h: &str| ronoctua_consent::pass_in(&b.consent, h);
        let bare = push(
            &b.places,
            None,
            &mint,
            false,
            Stamp::from_unix_s(100),
            &mut |_| {},
        );
        assert_eq!(bare.outcome, Outcome::Refused { why: Why::NoCache });
        let _ = std::fs::remove_dir_all(&b.root);
    }

    /// FAILS IF: a public push carries anything but the distro's valid
    /// roots' free closure — a download, what refers to one, a root this
    /// store does not have, what `cache.nixos.org` serves — or pushes the
    /// same apply twice, or the token reaches `cachix` other than by its
    /// environment, or what `cachix` says keeps the token.
    #[test]
    fn a_public_push_carries_only_the_distros_free_outputs() {
        let b = bench("public", "public", Some(50));
        b.token();
        b.let_all();
        let (r, said) = b.run(false, 100);
        assert_eq!(
            r.outcome,
            Outcome::Pushed {
                paths: 1,
                bytes: 100
            }
        );
        assert_eq!(b.pushed(), [s("ronoctua-0.1.0")]);
        let calls = b.calls();
        assert!(calls.contains("cachix push someone\n"), "{calls}");
        assert!(
            !calls.contains(TOKEN),
            "the token reached an argument: {calls}"
        );
        assert!(calls.contains("--store https://cache.nixos.org"), "{calls}");
        assert!(!calls.contains(&s("absent-1.0").replace("/nix/store/", "path-info /nix/store/")));
        let env = std::fs::read_to_string(b.root.join("env")).unwrap();
        assert_eq!(env, format!("token={TOKEN}\n"));
        assert!(said.iter().all(|l| !l.contains(TOKEN)), "{said:?}");
        assert!(said.iter().any(|l| l == "pushing with <token>"), "{said:?}");
        // THE SAME APPLY, ONCE: the next run has nothing new to push.
        let (again, _) = b.run(false, 200);
        assert_eq!(
            again.outcome,
            Outcome::Refused {
                why: Why::NothingNew
            }
        );
        assert_eq!(
            crate::record::read_at(b.places.record.as_ref().unwrap()).followed(),
            Some(&Stamp::from_unix_s(50))
        );
        let _ = std::fs::remove_dir_all(&b.root);
    }

    /// FAILS IF: a private push leaves out what the public one may not
    /// carry, or pushes what `cache.nixos.org` serves — or a rehearsal
    /// pushes, or writes the record.
    #[test]
    fn a_private_push_takes_the_system_and_a_rehearsal_takes_nothing() {
        let b = bench("private", "private", Some(50));
        b.token();
        b.let_all();
        let (r, _) = b.run(true, 100);
        assert_eq!(
            r.outcome,
            Outcome::Rehearsed {
                paths: 4,
                bytes: 62
            }
        );
        assert!(!b.calls().contains("cachix"));
        assert_eq!(
            crate::record::read_at(b.places.record.as_ref().unwrap()),
            Pushes::default()
        );
        let (r, _) = b.run(false, 100);
        assert_eq!(
            r.outcome,
            Outcome::Pushed {
                paths: 4,
                bytes: 62
            }
        );
        let pushed: BTreeSet<String> = b.pushed().into_iter().collect();
        assert!(pushed.contains(&s("google-chrome-141.0")));
        assert!(pushed.contains(&s("nixos-system-noctua-26.05")));
        assert!(
            !pushed.contains(&s("glibc-2.40-66")),
            "cache.nixos.org serves it"
        );
        let _ = std::fs::remove_dir_all(&b.root);
    }

    /// FAILS IF: after an apply that set a system for the next boot — the
    /// update's, with no boot trial or with one —, the push says nothing is
    /// new, carries the RUNNING system's closure, or takes the cache, its
    /// visibility or its roots from the running `/etc` rather than from the
    /// system that waits; or does not follow that apply once.
    #[test]
    fn a_system_set_for_the_next_boot_is_the_one_pushed() {
        // THE RUNNING `/etc` says a public cache; the system set for the next
        // boot, a private one.
        let b = bench("staged", "public", None);
        b.token();
        b.let_all();
        let staged = s("nixos-system-noctua-26.11");
        let own = b.places.system_file_of(&staged);
        std::fs::create_dir_all(own.parent().unwrap()).unwrap();
        std::fs::write(
            &own,
            format!(
                r#"{{"format": 1, "cache": {{"name": "someone", "public-key": "{KEY}", "visibility": "private"}},
                   "public-roots": [], "unfree": [], "android-unfree": false}}"#
            ),
        )
        .unwrap();
        // AS THE MACHINE WRITES IT: `state.json` told for an older reader
        // (`switched`, no system), the apply whole beside it.
        let record: MachineState = ronoctua_person::parse(
            format!(
                r#"{{"format": 1, "last-good": null, "trial": null, "hold": null,
                   "last-apply": {{"at": "{}", "result": "staged", "toplevel": "{staged}"}}}}"#,
                Stamp::from_unix_s(50)
            )
            .as_bytes(),
        )
        .unwrap();
        let (told, whole) = record.split();
        std::fs::write(&b.places.machine_state, ronoctua_person::to_json(&told)).unwrap();
        std::fs::write(
            b.places
                .machine_state
                .with_file_name(ronoctua_person::names::LAST_APPLY),
            ronoctua_person::to_json(&whole.unwrap()),
        )
        .unwrap();
        let (r, _) = b.run(false, 100);
        assert_eq!(
            r.outcome,
            Outcome::Pushed {
                paths: 3,
                bytes: 15
            },
            "{}",
            b.calls()
        );
        assert_eq!(r.visibility, Some(Visibility::Private));
        let pushed: BTreeSet<String> = b.pushed().into_iter().collect();
        assert!(pushed.contains(&staged), "{pushed:?}");
        assert!(pushed.contains(&s("etc-26.11")), "{pushed:?}");
        assert!(
            !pushed.contains(&s("nixos-system-noctua-26.05")),
            "the running system went: {pushed:?}"
        );
        assert_eq!(
            crate::record::read_at(b.places.record.as_ref().unwrap()).followed(),
            Some(&Stamp::from_unix_s(50))
        );
        let (again, _) = b.run(false, 200);
        assert_eq!(
            again.outcome,
            Outcome::Refused {
                why: Why::NothingNew
            }
        );
        // A SYSTEM THAT NAMES NO CACHE OF ITS OWN is no cache, whatever the
        // running `/etc` says.
        std::fs::remove_file(&own).unwrap();
        assert_eq!(
            b.run(true, 300).0.outcome,
            Outcome::Refused { why: Why::NoCache }
        );
        let _ = std::fs::remove_dir_all(&b.root);
    }

    /// FAILS IF: a `cachix` that failed is a push that happened (the next
    /// run would skip the apply it never pushed).
    #[test]
    fn a_failed_cachix_is_a_refusal_and_the_apply_waits() {
        let b = bench("failed", "public", Some(50));
        b.token();
        b.let_all();
        script(
            &b.tools.cachix,
            "cat >/dev/null\necho 'unauthorised' >&2\nexit 1\n",
        );
        let (r, said) = b.run(false, 100);
        assert_eq!(
            r.outcome,
            Outcome::Refused {
                why: Why::CachixFailed
            }
        );
        assert!(said.iter().any(|l| l == "unauthorised"));
        assert_eq!(
            crate::record::read_at(b.places.record.as_ref().unwrap()).followed(),
            None
        );
        let _ = std::fs::remove_dir_all(&b.root);
    }
}
